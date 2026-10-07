use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    path::{Component, Path, PathBuf},
};

use serde::Serialize;

use crate::{
    db::{Database, DatabaseError, now_ms},
    domain::{
        PlaylistExport, PlaylistExportCandidateEntry, PlaylistExportSnapshotEntry,
        PlaylistExportSource,
    },
    playlist_artwork::{DownloadedPlaylistArtwork, download_playlist_artwork},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistExportCommandError {
    pub code: String,
    pub message: String,
    pub unresolved_count: Option<usize>,
}

impl From<PlaylistExportError> for PlaylistExportCommandError {
    fn from(error: PlaylistExportError) -> Self {
        let unresolved_count = match &error {
            PlaylistExportError::UnresolvedEntries(count) => Some(*count),
            _ => None,
        };
        let code = match &error {
            PlaylistExportError::UnresolvedEntries(_) => "UNRESOLVED_ENTRIES",
            PlaylistExportError::InvalidTarget(_) => "INVALID_EXPORT_TARGET",
            PlaylistExportError::InvalidMode(_) => "INVALID_EXPORT_MODE",
            PlaylistExportError::Artwork(_) => "EXPORT_ARTWORK_ERROR",
            PlaylistExportError::Database(_) => "EXPORT_DATABASE_ERROR",
            PlaylistExportError::Io(_) => "EXPORT_IO_ERROR",
            PlaylistExportError::ZipLimit(_) => "EXPORT_ZIP_LIMIT",
        };
        Self {
            code: code.to_owned(),
            message: error.to_string(),
            unresolved_count,
        }
    }
}

pub fn export_playlist(
    database: &Database,
    library_root: Option<&Path>,
    collection_id: i64,
    mode: &str,
    destination: &Path,
) -> Result<PlaylistExport, PlaylistExportError> {
    validate_destination(mode, destination)?;
    let source = database.playlist_export_source(collection_id)?;
    let resolved = resolve_entries(&source)?;
    let export_paths = match mode {
        "m3u8" => m3u8_export_paths(destination, &resolved)?,
        "bundle" => bundle_export_paths(library_root, &resolved)?,
        other => return Err(PlaylistExportError::InvalidMode(other.to_owned())),
    };
    let snapshot_entries = resolved
        .iter()
        .zip(export_paths.iter())
        .map(
            |(entry, exported_relative_path)| PlaylistExportSnapshotEntry {
                position: entry.position,
                source_track_id: entry.source_track_id,
                library_track_id: entry.library_track_id,
                local_file_id: entry.local_file_id,
                exported_relative_path: exported_relative_path.clone(),
            },
        )
        .collect::<Vec<_>>();
    let destination_text = destination.to_string_lossy().into_owned();
    let running = database.create_playlist_export_snapshot(
        collection_id,
        mode,
        &destination_text,
        &snapshot_entries,
    )?;

    let bundle_artwork = if mode == "bundle" {
        source
            .image_url
            .as_deref()
            .map(download_playlist_artwork)
            .transpose()
            .map_err(PlaylistExportError::Artwork)
    } else {
        Ok(None)
    };

    let write_result = match mode {
        "m3u8" => {
            let output = render_m3u8(&resolved, &export_paths);
            atomic_write(destination, |file| {
                file.write_all(output.as_bytes())?;
                Ok(())
            })
        }
        "bundle" => bundle_artwork.and_then(|artwork| {
            write_bundle(
                destination,
                &source.collection_name,
                &resolved,
                &export_paths,
                artwork.as_ref(),
            )
        }),
        _ => unreachable!("mode validated before snapshot creation"),
    };

    match write_result {
        Ok(()) => database
            .finish_playlist_export(running.id, "succeeded", None)
            .map_err(PlaylistExportError::from),
        Err(error) => {
            let message = error.to_string();
            if let Err(record_error) =
                database.finish_playlist_export(running.id, "failed", Some(&message))
            {
                tracing::warn!(
                    export_id = running.id,
                    %record_error,
                    "failed to persist playlist export failure"
                );
            }
            Err(error)
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedExportEntry {
    position: i64,
    source_track_id: i64,
    library_track_id: i64,
    local_file_id: i64,
    title: String,
    artists: Vec<String>,
    duration_ms: Option<i64>,
    file_path: PathBuf,
}

fn resolve_entries(
    source: &PlaylistExportSource,
) -> Result<Vec<ResolvedExportEntry>, PlaylistExportError> {
    let mut unresolved = 0_usize;
    let mut resolved = Vec::with_capacity(source.entries.len());
    for entry in &source.entries {
        match resolve_entry(entry) {
            Some(entry) if entry.file_path.is_file() => resolved.push(entry),
            _ => unresolved = unresolved.saturating_add(1),
        }
    }
    if unresolved > 0 {
        return Err(PlaylistExportError::UnresolvedEntries(unresolved));
    }
    Ok(resolved)
}

fn resolve_entry(entry: &PlaylistExportCandidateEntry) -> Option<ResolvedExportEntry> {
    Some(ResolvedExportEntry {
        position: entry.position,
        source_track_id: entry.source_track_id?,
        library_track_id: entry.library_track_id?,
        local_file_id: entry.local_file_id?,
        title: entry.title.clone(),
        artists: entry.artists.clone(),
        duration_ms: entry.duration_ms,
        file_path: PathBuf::from(entry.file_path.as_deref()?),
    })
}

fn validate_destination(mode: &str, destination: &Path) -> Result<(), PlaylistExportError> {
    let expected_extension = match mode {
        "m3u8" => "m3u8",
        "bundle" => "zip",
        other => return Err(PlaylistExportError::InvalidMode(other.to_owned())),
    };
    if !destination.is_absolute() {
        return Err(PlaylistExportError::InvalidTarget(
            "Playlist export destination must be an absolute path.".to_owned(),
        ));
    }
    let extension = destination
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case(expected_extension) {
        return Err(PlaylistExportError::InvalidTarget(format!(
            "{} export destination must end in .{expected_extension}.",
            if mode == "bundle" {
                "Bundle"
            } else {
                "Playlist"
            }
        )));
    }
    let parent = destination.parent().ok_or_else(|| {
        PlaylistExportError::InvalidTarget(
            "Playlist export destination must have a containing folder.".to_owned(),
        )
    })?;
    if !parent.is_dir() {
        return Err(PlaylistExportError::InvalidTarget(format!(
            "Playlist export folder does not exist: {}",
            parent.display()
        )));
    }
    Ok(())
}

fn m3u8_export_paths(
    destination: &Path,
    entries: &[ResolvedExportEntry],
) -> Result<Vec<String>, PlaylistExportError> {
    let directory = destination.parent().ok_or_else(|| {
        PlaylistExportError::InvalidTarget(
            "Playlist export destination must have a containing folder.".to_owned(),
        )
    })?;
    entries
        .iter()
        .map(|entry| {
            let path = relative_path(directory, &entry.file_path)
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| entry.file_path.clone());
            m3u_path_text(&path)
        })
        .collect()
}

fn bundle_export_paths(
    library_root: Option<&Path>,
    entries: &[ResolvedExportEntry],
) -> Result<Vec<String>, PlaylistExportError> {
    let mut local_paths = HashMap::<i64, String>::new();
    let mut used_paths = HashSet::<String>::new();
    let mut paths = Vec::with_capacity(entries.len());
    for entry in entries {
        if let Some(existing) = local_paths.get(&entry.local_file_id) {
            paths.push(existing.clone());
            continue;
        }
        let relative = library_root
            .and_then(|root| entry.file_path.strip_prefix(root).ok())
            .and_then(safe_relative_archive_path)
            .unwrap_or_else(|| external_archive_path(entry));
        let mut archive_path = format!("Music/{relative}");
        if used_paths.contains(&archive_path) {
            let file_name = entry
                .file_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("track");
            archive_path = format!(
                "Music/External/{}/{}",
                entry.local_file_id,
                sanitize_component(file_name)
            );
        }
        used_paths.insert(archive_path.clone());
        local_paths.insert(entry.local_file_id, archive_path.clone());
        paths.push(archive_path);
    }
    Ok(paths)
}

fn safe_relative_archive_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_str()?;
                parts.push(sanitize_component(value));
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn external_archive_path(entry: &ResolvedExportEntry) -> String {
    let file_name = entry
        .file_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("track");
    format!(
        "External/{}/{}",
        entry.library_track_id,
        sanitize_component(file_name)
    )
}

fn sanitize_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| match character {
            '/' | '\\' | '\0'..='\u{1f}' => '_',
            _ => character,
        })
        .collect::<String>();
    let sanitized = sanitized.trim();
    if sanitized.is_empty() {
        "track".to_owned()
    } else {
        sanitized.to_owned()
    }
}

fn render_m3u8(entries: &[ResolvedExportEntry], paths: &[String]) -> String {
    let mut output = String::from("#EXTM3U\n");
    for (entry, path) in entries.iter().zip(paths.iter()) {
        let duration = entry.duration_ms.map(|value| value / 1000).unwrap_or(-1);
        let artist = if entry.artists.is_empty() {
            "Unknown Artist".to_owned()
        } else {
            entry.artists.join(", ")
        };
        output.push_str("#EXTINF:");
        output.push_str(&duration.to_string());
        output.push(',');
        output.push_str(&sanitize_m3u_metadata(&artist));
        output.push_str(" - ");
        output.push_str(&sanitize_m3u_metadata(&entry.title));
        output.push('\n');
        output.push_str(path);
        output.push('\n');
    }
    output
}

fn sanitize_m3u_metadata(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn m3u_path_text(path: &Path) -> Result<String, PlaylistExportError> {
    let text = path.to_string_lossy();
    if text.contains(['\r', '\n']) {
        return Err(PlaylistExportError::InvalidTarget(format!(
            "A playlist entry has a path that cannot be represented in M3U8: {}",
            path.display()
        )));
    }
    Ok(text.into_owned())
}

fn relative_path(from_directory: &Path, target: &Path) -> Option<PathBuf> {
    if from_directory.is_absolute() != target.is_absolute() {
        return None;
    }
    if windows_volume(from_directory) != windows_volume(target)
        && windows_volume(from_directory).is_some()
        && windows_volume(target).is_some()
    {
        return None;
    }
    let from = from_directory.components().collect::<Vec<_>>();
    let to = target.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(to.iter())
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        return None;
    }
    let mut relative = PathBuf::new();
    for component in &from[common..] {
        match component {
            Component::Normal(_) | Component::ParentDir => relative.push(".."),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    for component in &to[common..] {
        match component {
            Component::Normal(value) => relative.push(value),
            Component::CurDir => {}
            Component::ParentDir => relative.push(".."),
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    Some(relative)
}

fn windows_volume(path: &Path) -> Option<char> {
    let text = path.as_os_str().to_string_lossy();
    let bytes = text.as_bytes();
    (bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic())
        .then(|| (bytes[0] as char).to_ascii_lowercase())
}

fn write_bundle(
    destination: &Path,
    collection_name: &str,
    entries: &[ResolvedExportEntry],
    archive_paths: &[String],
    artwork: Option<&DownloadedPlaylistArtwork>,
) -> Result<(), PlaylistExportError> {
    let playlist_name = format!("{}.m3u8", sanitize_component(collection_name));
    let playlist = render_m3u8(entries, archive_paths);
    atomic_write(destination, |file| {
        let mut writer = StoredZipWriter::new(file);
        writer.add_bytes(&playlist_name, playlist.as_bytes())?;
        if let Some(artwork) = artwork {
            writer.add_bytes(&format!("cover.{}", artwork.extension), &artwork.bytes)?;
        }
        let mut copied = HashSet::new();
        for (entry, archive_path) in entries.iter().zip(archive_paths.iter()) {
            if copied.insert(entry.local_file_id) {
                writer.add_file(archive_path, &entry.file_path)?;
            }
        }
        writer.finish()
    })
}

fn atomic_write(
    target: &Path,
    writer: impl FnOnce(&mut File) -> Result<(), PlaylistExportError>,
) -> Result<(), PlaylistExportError> {
    let parent = target.parent().ok_or_else(|| {
        PlaylistExportError::InvalidTarget(
            "Playlist export destination must have a containing folder.".to_owned(),
        )
    })?;
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("playlist-export");
    let temporary = (0_u8..10)
        .find_map(|attempt| {
            let path = parent.join(format!(
                ".{file_name}.refrain-{}-{}-{attempt}.tmp",
                std::process::id(),
                now_ms()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => Some(Ok((path, file))),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .transpose()?
        .ok_or_else(|| {
            PlaylistExportError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Could not allocate a temporary export file.",
            ))
        })?;
    let (temporary_path, mut file) = temporary;
    if let Err(error) = writer(&mut file) {
        drop(file);
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    file.sync_all()?;
    drop(file);
    if let Err(error) = atomic_replace(&temporary_path, target) {
        let _ = fs::remove_file(&temporary_path);
        return Err(PlaylistExportError::Io(error));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn atomic_replace(source: &Path, target: &Path) -> Result<(), std::io::Error> {
    fs::rename(source, target)
}

#[cfg(target_os = "windows")]
fn atomic_replace(source: &Path, target: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

struct StoredZipEntry {
    name: String,
    crc32: u32,
    size: u32,
    local_header_offset: u32,
}

struct StoredZipWriter<'a> {
    file: &'a mut File,
    entries: Vec<StoredZipEntry>,
}

impl<'a> StoredZipWriter<'a> {
    fn new(file: &'a mut File) -> Self {
        Self {
            file,
            entries: Vec::new(),
        }
    }

    fn add_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<(), PlaylistExportError> {
        let mut cursor = std::io::Cursor::new(bytes);
        self.add_reader(name, &mut cursor)
    }

    fn add_file(&mut self, name: &str, path: &Path) -> Result<(), PlaylistExportError> {
        let mut file = File::open(path)?;
        self.add_reader(name, &mut file)
    }

    fn add_reader(
        &mut self,
        name: &str,
        reader: &mut impl Read,
    ) -> Result<(), PlaylistExportError> {
        let offset = u32::try_from(self.file.stream_position()?).map_err(|_| {
            PlaylistExportError::ZipLimit("Bundle exceeds the classic ZIP size limit.".to_owned())
        })?;
        let name_bytes = name.as_bytes();
        let name_len = u16::try_from(name_bytes.len()).map_err(|_| {
            PlaylistExportError::ZipLimit("A bundle path is too long for ZIP.".to_owned())
        })?;
        write_u32(self.file, 0x0403_4b50)?;
        write_u16(self.file, 20)?;
        write_u16(self.file, 0x0808)?;
        write_u16(self.file, 0)?;
        write_u16(self.file, 0)?;
        write_u16(self.file, 33)?;
        write_u32(self.file, 0)?;
        write_u32(self.file, 0)?;
        write_u32(self.file, 0)?;
        write_u16(self.file, name_len)?;
        write_u16(self.file, 0)?;
        self.file.write_all(name_bytes)?;

        let mut crc = Crc32::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            self.file.write_all(&buffer[..read])?;
            crc.update(&buffer[..read]);
            size = size.saturating_add(read as u64);
        }
        let size = u32::try_from(size).map_err(|_| {
            PlaylistExportError::ZipLimit(
                "An audio file exceeds the classic ZIP per-file size limit.".to_owned(),
            )
        })?;
        let crc32 = crc.finish();
        write_u32(self.file, 0x0807_4b50)?;
        write_u32(self.file, crc32)?;
        write_u32(self.file, size)?;
        write_u32(self.file, size)?;
        self.entries.push(StoredZipEntry {
            name: name.to_owned(),
            crc32,
            size,
            local_header_offset: offset,
        });
        Ok(())
    }

    fn finish(&mut self) -> Result<(), PlaylistExportError> {
        let central_offset = u32::try_from(self.file.stream_position()?).map_err(|_| {
            PlaylistExportError::ZipLimit("Bundle exceeds the classic ZIP size limit.".to_owned())
        })?;
        for entry in &self.entries {
            let name = entry.name.as_bytes();
            let name_len = u16::try_from(name.len()).map_err(|_| {
                PlaylistExportError::ZipLimit("A bundle path is too long for ZIP.".to_owned())
            })?;
            write_u32(self.file, 0x0201_4b50)?;
            write_u16(self.file, 20)?;
            write_u16(self.file, 20)?;
            write_u16(self.file, 0x0808)?;
            write_u16(self.file, 0)?;
            write_u16(self.file, 0)?;
            write_u16(self.file, 33)?;
            write_u32(self.file, entry.crc32)?;
            write_u32(self.file, entry.size)?;
            write_u32(self.file, entry.size)?;
            write_u16(self.file, name_len)?;
            write_u16(self.file, 0)?;
            write_u16(self.file, 0)?;
            write_u16(self.file, 0)?;
            write_u16(self.file, 0)?;
            write_u32(self.file, 0)?;
            write_u32(self.file, entry.local_header_offset)?;
            self.file.write_all(name)?;
        }
        let end = u32::try_from(self.file.stream_position()?).map_err(|_| {
            PlaylistExportError::ZipLimit("Bundle exceeds the classic ZIP size limit.".to_owned())
        })?;
        let central_size = end.checked_sub(central_offset).ok_or_else(|| {
            PlaylistExportError::ZipLimit("Invalid ZIP directory size.".to_owned())
        })?;
        let entry_count = u16::try_from(self.entries.len()).map_err(|_| {
            PlaylistExportError::ZipLimit("Bundle contains too many ZIP entries.".to_owned())
        })?;
        write_u32(self.file, 0x0605_4b50)?;
        write_u16(self.file, 0)?;
        write_u16(self.file, 0)?;
        write_u16(self.file, entry_count)?;
        write_u16(self.file, entry_count)?;
        write_u32(self.file, central_size)?;
        write_u32(self.file, central_offset)?;
        write_u16(self.file, 0)?;
        Ok(())
    }
}

fn write_u16(writer: &mut impl Write, value: u16) -> Result<(), std::io::Error> {
    writer.write_all(&value.to_le_bytes())
}

fn write_u32(writer: &mut impl Write, value: u32) -> Result<(), std::io::Error> {
    writer.write_all(&value.to_le_bytes())
}

struct Crc32(u32);

impl Crc32 {
    fn new() -> Self {
        Self(0xffff_ffff)
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u32::from(*byte);
            for _ in 0..8 {
                self.0 = if self.0 & 1 == 1 {
                    (self.0 >> 1) ^ 0xedb8_8320
                } else {
                    self.0 >> 1
                };
            }
        }
    }

    fn finish(self) -> u32 {
        !self.0
    }
}

#[derive(Debug)]
pub enum PlaylistExportError {
    Database(DatabaseError),
    Io(std::io::Error),
    InvalidTarget(String),
    InvalidMode(String),
    Artwork(String),
    UnresolvedEntries(usize),
    ZipLimit(String),
}

impl fmt::Display for PlaylistExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "Could not write playlist export: {error}"),
            Self::InvalidTarget(message) | Self::ZipLimit(message) => formatter.write_str(message),
            Self::Artwork(message) => {
                write!(formatter, "Could not include playlist cover: {message}")
            }
            Self::InvalidMode(mode) => {
                write!(formatter, "Unsupported playlist export mode: {mode}")
            }
            Self::UnresolvedEntries(count) => write!(
                formatter,
                "Cannot export because {count} playlist {} unresolved.",
                if *count == 1 {
                    "entry is"
                } else {
                    "entries are"
                }
            ),
        }
    }
}

impl Error for PlaylistExportError {}

impl From<DatabaseError> for PlaylistExportError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<std::io::Error> for PlaylistExportError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::{
        db::{Database, LocalFileWrite},
        domain::{CollectionEntry, SourceAccount, SourceCollection, SourceTrack},
        reconciliation::reconcile_refreshed_spotify_source,
    };

    use crate::playlist_artwork::DownloadedPlaylistArtwork;

    use super::{
        Crc32, PlaylistExportError, atomic_write, bundle_export_paths, export_playlist,
        relative_path, resolve_entries, write_bundle,
    };

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    struct TestWorkspace {
        root: PathBuf,
        database: Database,
    }

    impl TestWorkspace {
        fn new(name: &str) -> Self {
            let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "refrain-playlist-export-{name}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            let database = Database::open(root.join("refrain.sqlite3")).unwrap();
            Self { root, database }
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn source_account() -> SourceAccount {
        SourceAccount {
            provider: "spotify".into(),
            provider_account_id: "account".into(),
            display_name: Some("Listener".into()),
            image_url: None,
            client_id: "client".into(),
        }
    }

    fn source_collection(account_id: i64) -> SourceCollection {
        SourceCollection {
            source_account_id: account_id,
            provider_collection_id: "playlist-export".into(),
            kind: "playlist".into(),
            name: "Road Trip".into(),
            snapshot_id: None,
            owner_provider_id: Some("account".into()),
            is_accessible: true,
            access_issue: None,
            image_url: None,
            external_url: None,
            album_metadata: None,
        }
    }

    fn source_track(provider_id: &str, title: &str, isrc: &str) -> SourceTrack {
        SourceTrack {
            provider: "spotify".into(),
            provider_track_id: provider_id.into(),
            uri: Some(format!("spotify:track:{provider_id}")),
            isrc: Some(isrc.into()),
            title: title.into(),
            normalized_title: title.to_lowercase(),
            artists_json: "[\"Artist\"]".into(),
            normalized_artists: "artist".into(),
            album: Some("Album".into()),
            normalized_album: Some("album".into()),
            duration_ms: Some(180_000),
            disc_number: Some(1),
            track_number: Some(1),
            release_year: Some(2026),
            explicit: Some(false),
            version_kind: None,
            version_detail: None,
            image_url: None,
            external_url: None,
        }
    }

    fn entry(position: i64, source_track_id: i64) -> CollectionEntry {
        CollectionEntry {
            position,
            source_track_id: Some(source_track_id),
            provider_item_uri: None,
            item_type: "track".into(),
            added_at: None,
            unavailable_reason: None,
        }
    }

    fn seed_playlist(workspace: &TestWorkspace, with_files: bool) -> (i64, PathBuf, PathBuf) {
        let account_id = workspace
            .database
            .upsert_source_account(&source_account())
            .unwrap();
        let collection_id = workspace
            .database
            .upsert_source_collection(&source_collection(account_id))
            .unwrap();
        let first = workspace
            .database
            .upsert_source_track(&source_track("track-a", "First Song", "USAAA1000001"))
            .unwrap();
        let second = workspace
            .database
            .upsert_source_track(&source_track("track-b", "Second Song", "USAAA1000002"))
            .unwrap();
        workspace
            .database
            .replace_collection_entries(
                collection_id,
                &[entry(0, first), entry(1, first), entry(2, second)],
            )
            .unwrap();
        workspace
            .database
            .set_source_collection_tracking(collection_id, true)
            .unwrap();
        reconcile_refreshed_spotify_source(&workspace.database).unwrap();

        let first_path = workspace
            .root
            .join("Music/Artist/Album/01 - First Song.flac");
        let second_path = workspace
            .root
            .join("Music/Artist/Album/02 - Second Song.flac");
        if with_files {
            fs::create_dir_all(first_path.parent().unwrap()).unwrap();
            fs::write(&first_path, b"first-audio").unwrap();
            fs::write(&second_path, b"second-audio").unwrap();
            for (source_track_id, title, path, isrc) in [
                (first, "First Song", &first_path, "USAAA1000001"),
                (second, "Second Song", &second_path, "USAAA1000002"),
            ] {
                let library_track_id = workspace
                    .database
                    .persisted_track_link(source_track_id)
                    .unwrap()
                    .unwrap()
                    .library_track_id;
                workspace
                    .database
                    .insert_managed_local_file_for_track(
                        library_track_id,
                        &LocalFileWrite {
                            path: path.to_string_lossy().into_owned(),
                            state: "present".into(),
                            format: Some("flac".into()),
                            file_size: fs::metadata(path).unwrap().len() as i64,
                            modified_at: 1,
                            duration_ms: Some(180_000),
                            bitrate: None,
                            sample_rate: None,
                            channels: None,
                            content_hash: None,
                            tag_title: Some(title.into()),
                            tag_artists: vec!["Artist".into()],
                            tag_album: Some("Album".into()),
                            tag_year: Some(2026),
                            tag_isrc: Some(isrc.into()),
                            artwork_path: None,
                            artwork_mime: None,
                            scan_error: None,
                        },
                    )
                    .unwrap();
            }
        }
        (collection_id, first_path, second_path)
    }

    #[test]
    fn m3u8_export_preserves_order_duplicates_and_relative_paths() {
        let workspace = TestWorkspace::new("m3u8");
        let (collection_id, first_path, second_path) = seed_playlist(&workspace, true);
        let export_dir = workspace.root.join("Exports");
        fs::create_dir_all(&export_dir).unwrap();
        let destination = export_dir.join("Road Trip.m3u8");

        let result = export_playlist(
            &workspace.database,
            Some(&workspace.root),
            collection_id,
            "m3u8",
            &destination,
        )
        .unwrap();

        assert_eq!(result.status, "succeeded");
        assert_eq!(result.entry_count, 3);
        let output = fs::read_to_string(&destination).unwrap();
        let first_relative = relative_path(&export_dir, &first_path)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let second_relative = relative_path(&export_dir, &second_path)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(output.matches(&first_relative).count(), 2);
        assert_eq!(output.matches(&second_relative).count(), 1);
        assert!(output.starts_with("#EXTM3U\n"));
        let history = workspace
            .database
            .playlist_exports_page(Some(collection_id), 0, 10)
            .unwrap();
        assert_eq!(history.total, 1);
        assert_eq!(history.items[0].entry_count, 3);
    }

    #[test]
    fn unresolved_entries_refuse_export_without_creating_history() {
        let workspace = TestWorkspace::new("unresolved");
        let (collection_id, _, _) = seed_playlist(&workspace, false);
        let destination = workspace.root.join("Road Trip.m3u8");

        let error = export_playlist(
            &workspace.database,
            Some(&workspace.root),
            collection_id,
            "m3u8",
            &destination,
        )
        .unwrap_err();

        assert!(matches!(error, PlaylistExportError::UnresolvedEntries(3)));
        assert!(!destination.exists());
        assert_eq!(
            workspace
                .database
                .playlist_exports_page(Some(collection_id), 0, 10)
                .unwrap()
                .total,
            0
        );
    }

    #[test]
    fn failed_temporary_write_preserves_existing_destination() {
        let workspace = TestWorkspace::new("atomic-failure");
        let target = workspace.root.join("playlist.m3u8");
        fs::write(&target, b"original").unwrap();

        let result = atomic_write(&target, |file| {
            file.write_all(b"partial")?;
            Err(PlaylistExportError::Io(std::io::Error::other(
                "simulated interruption",
            )))
        });

        assert!(result.is_err());
        assert_eq!(fs::read(&target).unwrap(), b"original");
    }

    #[test]
    fn windows_cross_volume_paths_fall_back_to_absolute() {
        assert!(
            relative_path(
                Path::new("C:\\Playlists"),
                Path::new("D:\\Music\\Song.flac")
            )
            .is_none()
        );
    }

    #[test]
    fn portable_bundle_copies_each_audio_file_once() {
        let workspace = TestWorkspace::new("bundle");
        let (collection_id, _, _) = seed_playlist(&workspace, true);
        let destination = workspace.root.join("Road Trip.zip");

        let result = export_playlist(
            &workspace.database,
            Some(&workspace.root),
            collection_id,
            "bundle",
            &destination,
        )
        .unwrap();

        assert_eq!(result.status, "succeeded");
        let names = central_directory_names(&fs::read(&destination).unwrap());
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"Road Trip.m3u8".to_owned()));
        assert!(
            names
                .iter()
                .filter(|name| name.starts_with("Music/"))
                .count()
                == 2
        );
    }

    #[test]
    fn portable_bundle_includes_playlist_cover() {
        let workspace = TestWorkspace::new("bundle-cover");
        let (collection_id, _, _) = seed_playlist(&workspace, true);
        let source = workspace
            .database
            .playlist_export_source(collection_id)
            .unwrap();
        let resolved = resolve_entries(&source).unwrap();
        let archive_paths = bundle_export_paths(Some(&workspace.root), &resolved).unwrap();
        let destination = workspace.root.join("Road Trip Cover.zip");
        let artwork = DownloadedPlaylistArtwork {
            bytes: vec![0xff, 0xd8, 0xff, 0x00],
            extension: "jpg",
        };

        write_bundle(
            &destination,
            &source.collection_name,
            &resolved,
            &archive_paths,
            Some(&artwork),
        )
        .unwrap();

        let names = central_directory_names(&fs::read(destination).unwrap());
        assert!(names.contains(&"Road Trip.m3u8".to_owned()));
        assert!(names.contains(&"cover.jpg".to_owned()));
    }

    #[test]
    fn crc32_matches_standard_test_vector() {
        let mut crc = Crc32::new();
        crc.update(b"123456789");
        assert_eq!(crc.finish(), 0xcbf4_3926);
    }

    fn central_directory_names(bytes: &[u8]) -> Vec<String> {
        let signature = 0x0201_4b50_u32.to_le_bytes();
        let mut names = Vec::new();
        let mut offset = 0_usize;
        while offset + 46 <= bytes.len() {
            if bytes[offset..].starts_with(&signature) {
                let name_len =
                    u16::from_le_bytes([bytes[offset + 28], bytes[offset + 29]]) as usize;
                let extra_len =
                    u16::from_le_bytes([bytes[offset + 30], bytes[offset + 31]]) as usize;
                let comment_len =
                    u16::from_le_bytes([bytes[offset + 32], bytes[offset + 33]]) as usize;
                let name_start = offset + 46;
                let name_end = name_start + name_len;
                if name_end > bytes.len() {
                    break;
                }
                names.push(String::from_utf8(bytes[name_start..name_end].to_vec()).unwrap());
                offset = name_end + extra_len + comment_len;
            } else {
                offset += 1;
            }
        }
        names
    }
}
