use std::{
    error::Error,
    fmt, fs,
    path::{Component, Path, PathBuf},
};

use crate::{
    db::{Database, DatabaseError, now_ms},
    domain::{LocalPlaylistDetail, LocalPlaylistEntry},
    playlist_artwork::{
        DownloadedPlaylistArtwork, PLAYLIST_DIRECTORY, configured_custom_artwork_directory,
        download_playlist_artwork, import_custom_playlist_artwork,
        is_owned_custom_playlist_artwork, remove_custom_playlist_artwork,
    },
};

pub(crate) const MANAGED_M3U_MARKER: &str = "#REFRAIN-MANAGED-PLAYLIST";

pub fn sync_local_playlist(
    database: &Database,
    playlist_id: i64,
) -> Result<LocalPlaylistDetail, LocalPlaylistSyncError> {
    let playlist = database.get_local_playlist(playlist_id)?;
    let prepared = prepare_playlist_for_sync(database, &playlist)?;
    let result = sync_local_playlist_file(&prepared);
    match result {
        Ok(missing) => {
            if prepared.m3u_managed && prepared.m3u_path != playlist.m3u_path {
                let path = prepared.m3u_path.as_deref().ok_or_else(|| {
                    LocalPlaylistSyncError::InvalidTarget(
                        "Managed playlist target could not be resolved.".to_owned(),
                    )
                })?;
                database.set_local_playlist_managed_m3u_path(playlist_id, path)?;
            }
            let mut synced = database.record_local_playlist_sync_success(playlist_id, now_ms())?;
            if !missing.is_empty() {
                let sample = missing
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                let message = format!(
                    "{} unavailable {} omitted from the regenerated M3U: {}{}",
                    missing.len(),
                    if missing.len() == 1 {
                        "track was"
                    } else {
                        "tracks were"
                    },
                    sample,
                    if missing.len() > 3 { ", …" } else { "" }
                );
                database.record_local_playlist_sync_error(playlist_id, &message)?;
                synced = database.get_local_playlist(playlist_id)?;
            }
            if let Err(error) = sync_playlist_cover(database, &synced) {
                tracing::warn!(
                    playlist_id,
                    %error,
                    "could not synchronize Spotify playlist cover sidecar"
                );
            }
            if let Err(error) = migrate_custom_artwork(database, &synced) {
                tracing::warn!(playlist_id, %error, "could not move saved playlist artwork into library folder");
            }
            remove_previous_managed_file(&playlist, &prepared);
            database.get_local_playlist(playlist_id).map_err(Into::into)
        }
        Err(error) => {
            let message = error.to_string();
            if let Err(record_error) =
                database.record_local_playlist_sync_error(playlist_id, &message)
            {
                tracing::warn!(
                    playlist_id,
                    %record_error,
                    "failed to persist local playlist sync error"
                );
            }
            Err(error)
        }
    }
}

pub fn sync_local_playlist_after_change(
    database: &Database,
    playlist_id: i64,
) -> Result<LocalPlaylistDetail, DatabaseError> {
    match sync_local_playlist(database, playlist_id) {
        Ok(detail) => Ok(detail),
        Err(error) => {
            tracing::warn!(playlist_id, %error, "automatic local playlist sync did not complete");
            database.get_local_playlist(playlist_id)
        }
    }
}

pub fn sync_managed_local_playlists(database: &Database) -> Result<(), DatabaseError> {
    let playlist_ids = database
        .list_local_playlists()?
        .into_iter()
        .filter(|playlist| playlist.m3u_managed || playlist.m3u_path.is_none())
        .map(|playlist| playlist.id)
        .collect::<Vec<_>>();

    for playlist_id in playlist_ids {
        if let Err(error) = sync_local_playlist(database, playlist_id) {
            tracing::warn!(playlist_id, %error, "managed local playlist sync did not complete");
        }
    }
    // External M3Us stay in their original location, but their custom artwork
    // belongs to the same Refrain-managed artwork folder as other playlists.
    for playlist in database.list_local_playlists()? {
        if !playlist.m3u_managed && playlist.m3u_path.is_some() {
            let detail = database.get_local_playlist(playlist.id)?;
            if let Err(error) = migrate_custom_artwork(database, &detail) {
                tracing::warn!(playlist_id = playlist.id, %error, "could not migrate imported playlist artwork");
            }
        }
    }
    Ok(())
}

fn migrate_custom_artwork(
    database: &Database,
    playlist: &LocalPlaylistDetail,
) -> Result<(), String> {
    let Some(previous) = playlist.custom_artwork_path.as_deref() else {
        return Ok(());
    };
    let previous_path = Path::new(previous);
    let app_data_dir = database
        .path()
        .parent()
        .ok_or("Application data folder is unavailable.")?;
    if !is_owned_custom_playlist_artwork(app_data_dir, playlist.id, previous_path) {
        return Ok(());
    }
    let destination_folder = configured_custom_artwork_directory(database)?;
    if previous_path.parent() == Some(destination_folder.as_path()) || !previous_path.exists() {
        return Ok(());
    }
    let moved = import_custom_playlist_artwork(&destination_folder, playlist.id, previous_path)?;
    if let Err(error) =
        database.set_local_playlist_artwork_path(playlist.id, Some(&moved.to_string_lossy()))
    {
        remove_custom_playlist_artwork(app_data_dir, playlist.id, &moved.to_string_lossy());
        return Err(error.to_string());
    }
    remove_custom_playlist_artwork(app_data_dir, playlist.id, previous);
    Ok(())
}

fn prepare_playlist_for_sync(
    database: &Database,
    playlist: &LocalPlaylistDetail,
) -> Result<LocalPlaylistDetail, LocalPlaylistSyncError> {
    if !playlist.m3u_managed && playlist.m3u_path.is_some() {
        return Ok(playlist.clone());
    }

    let target = managed_playlist_target(database, playlist)?;
    let mut prepared = playlist.clone();
    prepared.m3u_path = Some(target.to_string_lossy().into_owned());
    prepared.m3u_managed = true;
    Ok(prepared)
}

fn managed_playlist_target(
    database: &Database,
    playlist: &LocalPlaylistDetail,
) -> Result<PathBuf, LocalPlaylistSyncError> {
    let settings = database.get_settings()?;
    let library_root = settings
        .library_root
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            LocalPlaylistSyncError::InvalidTarget(
                "Set a library folder before creating managed playlist files.".to_owned(),
            )
        })?;
    let library_root = PathBuf::from(library_root);
    if !library_root.is_absolute() {
        return Err(LocalPlaylistSyncError::InvalidTarget(
            "Library folder must be an absolute path before creating managed playlist files."
                .to_owned(),
        ));
    }

    let directory = library_root.join(PLAYLIST_DIRECTORY);
    let base = sanitize_playlist_filename(&playlist.name);
    let current = playlist.m3u_path.as_deref().map(PathBuf::from);
    let used = database
        .list_local_playlists()?
        .into_iter()
        .filter(|item| item.id != playlist.id)
        .filter_map(|item| item.m3u_path.map(PathBuf::from))
        .collect::<Vec<_>>();

    for attempt in 0_u16..=999 {
        let filename = match attempt {
            0 => format!("{base}.m3u"),
            1 => format!("{base} ({}).m3u", playlist.id),
            value => format!("{base} ({}-{value}).m3u", playlist.id),
        };
        let candidate = directory.join(filename);
        let belongs_to_current = current.as_ref().is_some_and(|path| path == &candidate);
        let reserved_by_playlist = used.iter().any(|path| path == &candidate);
        let occupied_on_disk = candidate.exists() && !belongs_to_current;
        if !reserved_by_playlist && !occupied_on_disk {
            return Ok(candidate);
        }
    }

    Err(LocalPlaylistSyncError::InvalidTarget(format!(
        "Could not choose a unique managed playlist filename for {}.",
        playlist.name
    )))
}

fn sanitize_playlist_filename(value: &str) -> String {
    let value = value
        .chars()
        .map(|character| match character {
            '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\0'..='\u{1f}' => '_',
            _ => character,
        })
        .collect::<String>();
    let value = value.trim().trim_end_matches(['.', ' ']).trim();
    let mut value = if value.is_empty() || matches!(value, "." | "..") {
        "Playlist".to_owned()
    } else {
        value.to_owned()
    };
    let upper = value.to_ascii_uppercase();
    let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.as_bytes()[3].is_ascii_digit()
            && upper.as_bytes()[3] != b'0');
    if reserved {
        value.push('_');
    }
    value
}

fn remove_previous_managed_file(previous: &LocalPlaylistDetail, current: &LocalPlaylistDetail) {
    if !previous.m3u_managed || previous.m3u_path == current.m3u_path {
        return;
    }
    let Some(previous_path) = previous.m3u_path.as_deref() else {
        return;
    };
    let previous_path = Path::new(previous_path);
    if !previous_path.is_file() {
        return;
    }
    if let Err(error) = fs::remove_file(previous_path) {
        tracing::warn!(
            path = %previous_path.display(),
            %error,
            "could not remove superseded managed playlist file"
        );
    }
    // Only remove the legacy directory when it is empty. Imported M3Us and
    // other user files left there must survive the migration.
    if let (Some(old_parent), Some(new_parent)) = (
        previous_path.parent(),
        current
            .m3u_path
            .as_deref()
            .and_then(|path| Path::new(path).parent()),
    ) && old_parent.file_name().and_then(|name| name.to_str()) == Some("Playlists")
        && old_parent.parent() == new_parent.parent()
        && new_parent.file_name().and_then(|name| name.to_str()) == Some(PLAYLIST_DIRECTORY)
    {
        let _ = fs::remove_dir(old_parent);
    }
}

fn sync_playlist_cover(database: &Database, playlist: &LocalPlaylistDetail) -> Result<(), String> {
    sync_playlist_cover_with_fetch(database, playlist, download_playlist_artwork)
}

fn sync_playlist_cover_with_fetch(
    database: &Database,
    playlist: &LocalPlaylistDetail,
    fetch: impl FnOnce(&str) -> Result<DownloadedPlaylistArtwork, String>,
) -> Result<(), String> {
    if !playlist.m3u_managed || playlist.source_collection_id.is_none() {
        return Ok(());
    }

    let (previous_source_url, previous_path) = database
        .local_playlist_cover_state(playlist.id)
        .map_err(|error| error.to_string())?;
    let previous_path = previous_path.map(PathBuf::from);
    let Some(image_url) = playlist.image_url.as_deref() else {
        if let Some(path) = previous_path.as_deref()
            && path.is_file()
        {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
        database
            .set_local_playlist_cover_state(playlist.id, None, None)
            .map_err(|error| error.to_string())?;
        return Ok(());
    };

    let m3u_path = playlist
        .m3u_path
        .as_deref()
        .map(Path::new)
        .ok_or_else(|| "Managed playlist cover has no playlist-file target.".to_owned())?;

    if previous_source_url.as_deref() == Some(image_url)
        && let Some(previous_path) = previous_path.as_deref()
        && previous_path.is_file()
        && let Some(extension) = previous_path.extension().and_then(|value| value.to_str())
    {
        let target = managed_cover_target(m3u_path, extension, Some(previous_path));
        if target == previous_path {
            return Ok(());
        }
        let bytes = fs::read(previous_path).map_err(|error| error.to_string())?;
        replace_file(&target, &bytes).map_err(|error| error.to_string())?;
        fs::remove_file(previous_path).map_err(|error| error.to_string())?;
        database
            .set_local_playlist_cover_state(
                playlist.id,
                Some(image_url),
                Some(&target.to_string_lossy()),
            )
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    let artwork = fetch(image_url)?;
    let target = managed_cover_target(m3u_path, artwork.extension, previous_path.as_deref());
    replace_file(&target, &artwork.bytes).map_err(|error| error.to_string())?;
    if let Some(previous_path) = previous_path.as_deref()
        && previous_path != target
        && previous_path.is_file()
    {
        fs::remove_file(previous_path).map_err(|error| error.to_string())?;
    }
    database
        .set_local_playlist_cover_state(
            playlist.id,
            Some(image_url),
            Some(&target.to_string_lossy()),
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn managed_cover_target(m3u_path: &Path, extension: &str, current: Option<&Path>) -> PathBuf {
    let primary = m3u_path.with_extension(extension);
    if current == Some(primary.as_path()) || !primary.exists() {
        return primary;
    }

    let parent = m3u_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = m3u_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("playlist");
    for attempt in 0_u16..=999 {
        let filename = match attempt {
            0 => format!("{stem}.cover.{extension}"),
            value => format!("{stem}.cover-{value}.{extension}"),
        };
        let candidate = parent.join(filename);
        if current == Some(candidate.as_path()) || !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem}.cover-{}.{}", now_ms(), extension))
}

fn sync_local_playlist_file(
    playlist: &LocalPlaylistDetail,
) -> Result<Vec<String>, LocalPlaylistSyncError> {
    let path = playlist.m3u_path.as_deref().ok_or_else(|| {
        LocalPlaylistSyncError::InvalidTarget(
            "Choose an M3U or M3U8 file before syncing this playlist.".to_owned(),
        )
    })?;
    let target = Path::new(path);
    if !target.is_absolute() {
        return Err(LocalPlaylistSyncError::InvalidTarget(
            "Playlist sync target must be an absolute path.".to_owned(),
        ));
    }
    let valid_extension = target
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "m3u" | "m3u8"));
    if !valid_extension {
        return Err(LocalPlaylistSyncError::InvalidTarget(
            "Playlist sync target must end in .m3u or .m3u8.".to_owned(),
        ));
    }
    let parent = target.parent().ok_or_else(|| {
        LocalPlaylistSyncError::InvalidTarget(
            "Playlist sync target must have a containing folder.".to_owned(),
        )
    })?;
    if playlist.m3u_managed {
        fs::create_dir_all(parent)?;
    } else if !parent.is_dir() {
        return Err(LocalPlaylistSyncError::InvalidTarget(format!(
            "Playlist folder does not exist: {}",
            parent.display()
        )));
    }

    let mut missing = Vec::new();
    for entry in &playlist.entries {
        match entry.file_path.as_deref() {
            Some(file_path) if Path::new(file_path).is_file() => {}
            _ => missing.push(entry.title.clone()),
        }
    }
    let mut output = String::from("#EXTM3U\n");
    if playlist.m3u_managed {
        output.push_str(MANAGED_M3U_MARKER);
        output.push('\n');
    }
    for entry in &playlist.entries {
        if entry
            .file_path
            .as_deref()
            .is_some_and(|path| Path::new(path).is_file())
        {
            write_entry(&mut output, parent, entry)?;
        } else {
            output.push_str("#REFRAIN-UNAVAILABLE: ");
            output.push_str(&sanitize_m3u_metadata(&entry.title));
            output.push('\n');
        }
    }
    replace_file(target, output.as_bytes())?;
    Ok(missing)
}

fn write_entry(
    output: &mut String,
    playlist_directory: &Path,
    entry: &LocalPlaylistEntry,
) -> Result<(), LocalPlaylistSyncError> {
    let file_path = entry
        .file_path
        .as_deref()
        .ok_or_else(|| LocalPlaylistSyncError::UnavailableTracks(vec![entry.title.clone()]))?;
    let file_path = Path::new(file_path);
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

    let display_path = relative_path(playlist_directory, file_path)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| file_path.to_path_buf());
    let display_path = display_path.to_string_lossy();
    if display_path.contains(['\r', '\n']) {
        return Err(LocalPlaylistSyncError::InvalidTarget(format!(
            "A playlist entry has a path that cannot be represented in M3U: {}",
            file_path.display()
        )));
    }
    output.push_str(&display_path);
    output.push('\n');
    Ok(())
}

fn sanitize_m3u_metadata(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn relative_path(from_directory: &Path, target: &Path) -> Option<PathBuf> {
    if from_directory.is_absolute() != target.is_absolute() {
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

fn replace_file(target: &Path, bytes: &[u8]) -> Result<(), LocalPlaylistSyncError> {
    let parent = target.parent().ok_or_else(|| {
        LocalPlaylistSyncError::InvalidTarget(
            "Playlist sync target must have a containing folder.".to_owned(),
        )
    })?;
    let filename = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("playlist.m3u");
    let temporary = parent.join(format!(
        ".{filename}.refrain-{}-{}.tmp",
        std::process::id(),
        now_ms()
    ));
    fs::write(&temporary, bytes)?;
    if let Err(rename_error) = fs::rename(&temporary, target) {
        if target.exists() {
            fs::remove_file(target)?;
            fs::rename(&temporary, target)?;
        } else {
            let _ = fs::remove_file(&temporary);
            return Err(LocalPlaylistSyncError::Io(rename_error));
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum LocalPlaylistSyncError {
    Database(DatabaseError),
    Io(std::io::Error),
    InvalidTarget(String),
    UnavailableTracks(Vec<String>),
}

impl fmt::Display for LocalPlaylistSyncError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "Could not write playlist file: {error}"),
            Self::InvalidTarget(message) => formatter.write_str(message),
            Self::UnavailableTracks(titles) => {
                let count = titles.len();
                write!(
                    formatter,
                    "Cannot sync because {count} playlist {} no usable local file: {}",
                    if count == 1 {
                        "entry has"
                    } else {
                        "entries have"
                    },
                    titles.join(", ")
                )
            }
        }
    }
}

impl Error for LocalPlaylistSyncError {}

impl From<DatabaseError> for LocalPlaylistSyncError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<std::io::Error> for LocalPlaylistSyncError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use rusqlite::params;

    use crate::{
        db::{Database, now_ms},
        domain::{SourceAccount, SourceCollection},
        playlist_artwork::DownloadedPlaylistArtwork,
    };

    use super::{
        MANAGED_M3U_MARKER, sync_local_playlist, sync_local_playlist_after_change,
        sync_managed_local_playlists, sync_playlist_cover_with_fetch,
    };

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    struct TestWorkspace {
        root: PathBuf,
        database: Database,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join(format!("refrain-playlist-sync-{}-{id}", std::process::id()));
            fs::create_dir_all(&root).expect("test directory should exist");
            let database =
                Database::open(root.join("refrain.sqlite3")).expect("test database should open");
            Self { root, database }
        }

        fn add_track(&self, title: &str, filename: &str, create_file: bool) -> i64 {
            let path = self.root.join(filename);
            if create_file {
                fs::write(&path, b"audio").expect("test audio file should write");
            }
            let connection = rusqlite::Connection::open(self.database.path())
                .expect("database should reopen for test setup");
            let now = now_ms();
            connection
                .execute(
                    "INSERT INTO library_tracks (
                        title, normalized_title, artists_json, normalized_artists,
                        duration_ms, created_at, updated_at
                     ) VALUES (?1, ?2, '[\"Artist\"]', 'artist', 180000, ?3, ?3)",
                    params![title, title.to_ascii_lowercase(), now],
                )
                .expect("library track should insert");
            let track_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO local_files (
                        library_track_id, path, ownership, is_preferred, state,
                        file_size, modified_at, created_at, updated_at
                     ) VALUES (?1, ?2, 'external', 1, 'present', 100, 1, ?3, ?3)",
                    params![track_id, path.to_string_lossy(), now],
                )
                .expect("local file should insert");
            track_id
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn sync_writes_ordered_extended_m3u_with_duplicates() {
        let workspace = TestWorkspace::new();
        let first = workspace.add_track("First", "first.flac", true);
        let second = workspace.add_track("Second", "second.flac", true);
        let playlist = workspace
            .database
            .create_local_playlist("Road Trip")
            .expect("playlist should create");
        let playlist = workspace
            .database
            .add_tracks_to_local_playlist(playlist.id, &[first, second, first])
            .expect("tracks should add");
        let target = workspace.root.join("road-trip.m3u8");
        workspace
            .database
            .set_local_playlist_m3u_path(playlist.id, Some(&target.to_string_lossy()))
            .expect("target should set");

        let synced =
            sync_local_playlist(&workspace.database, playlist.id).expect("playlist should sync");
        let output = fs::read_to_string(&target).expect("playlist file should exist");

        assert!(synced.last_synced_at.is_some());
        assert_eq!(synced.sync_error, None);
        assert_eq!(
            output.lines().collect::<Vec<_>>(),
            vec![
                "#EXTM3U",
                "#EXTINF:180,Artist - First",
                "first.flac",
                "#EXTINF:180,Artist - Second",
                "second.flac",
                "#EXTINF:180,Artist - First",
                "first.flac",
            ]
        );
    }

    #[test]
    fn sync_creates_a_managed_m3u_under_the_library_root() {
        let workspace = TestWorkspace::new();
        let mut settings = workspace
            .database
            .get_settings()
            .expect("settings should load");
        settings.library_root = Some(workspace.root.to_string_lossy().into_owned());
        workspace
            .database
            .update_settings(&settings)
            .expect("library root should persist");

        let playlist = workspace
            .database
            .create_local_playlist("Late Nights")
            .expect("playlist should create");

        let synced = sync_local_playlist(&workspace.database, playlist.id)
            .expect("playlist should create and sync its managed M3U");
        let expected = workspace.root.join("00 Playlists").join("Late Nights.m3u");

        assert_eq!(synced.m3u_path.as_deref(), expected.to_str());
        assert!(expected.is_file());
        assert_eq!(
            fs::read_to_string(expected).expect("managed playlist should be readable"),
            format!("#EXTM3U\n{MANAGED_M3U_MARKER}\n")
        );
    }

    #[test]
    fn spotify_mirror_downloads_cover_next_to_managed_m3u() {
        let workspace = TestWorkspace::new();
        let account_id = workspace
            .database
            .upsert_source_account(&SourceAccount {
                provider: "spotify".into(),
                provider_account_id: "listener".into(),
                display_name: Some("Listener".into()),
                image_url: None,
                client_id: "client".into(),
            })
            .expect("source account should persist");
        let collection_id = workspace
            .database
            .upsert_source_collection(&SourceCollection {
                source_account_id: account_id,
                provider_collection_id: "late-nights".into(),
                kind: "playlist".into(),
                name: "Late Nights".into(),
                snapshot_id: None,
                owner_provider_id: Some("listener".into()),
                is_accessible: true,
                access_issue: None,
                image_url: Some("https://i.scdn.co/image/late-nights".into()),
                external_url: None,
                album_metadata: None,
            })
            .expect("source playlist should persist");
        workspace
            .database
            .set_source_collection_tracking(collection_id, true)
            .expect("playlist tracking should persist");
        workspace
            .database
            .sync_tracked_spotify_playlist_mirrors()
            .expect("playlist mirror should materialize");
        let mirror = workspace
            .database
            .list_local_playlists()
            .expect("local playlists should list")
            .into_iter()
            .find(|playlist| playlist.source_collection_id == Some(collection_id))
            .expect("mirror should exist");
        let playlist_path = workspace.root.join("00 Playlists/Late Nights.m3u");
        fs::create_dir_all(playlist_path.parent().unwrap()).unwrap();
        workspace
            .database
            .set_local_playlist_managed_m3u_path(mirror.id, &playlist_path.to_string_lossy())
            .expect("managed playlist path should persist");
        let detail = workspace
            .database
            .get_local_playlist(mirror.id)
            .expect("mirror should load");

        sync_playlist_cover_with_fetch(&workspace.database, &detail, |_| {
            Ok(DownloadedPlaylistArtwork {
                bytes: vec![0xff, 0xd8, 0xff, 0x00],
                extension: "jpg",
            })
        })
        .expect("cover should synchronize");

        let cover_path = workspace.root.join("00 Playlists/Late Nights.jpg");
        assert_eq!(fs::read(&cover_path).unwrap(), [0xff, 0xd8, 0xff, 0x00]);
        let (source_url, stored_path) = workspace
            .database
            .local_playlist_cover_state(mirror.id)
            .expect("cover state should load");
        assert_eq!(source_url.as_deref(), detail.image_url.as_deref());
        assert_eq!(stored_path.as_deref(), cover_path.to_str());
    }

    #[test]
    fn managed_m3u_follows_playlist_rename_without_leaving_the_old_file() {
        let workspace = TestWorkspace::new();
        let mut settings = workspace
            .database
            .get_settings()
            .expect("settings should load");
        settings.library_root = Some(workspace.root.to_string_lossy().into_owned());
        workspace
            .database
            .update_settings(&settings)
            .expect("library root should persist");
        let playlist = workspace
            .database
            .create_local_playlist("Late Nights")
            .expect("playlist should create");
        let first = sync_local_playlist(&workspace.database, playlist.id)
            .expect("initial managed playlist should sync");
        let first_path = PathBuf::from(first.m3u_path.expect("managed path should persist"));

        workspace
            .database
            .rename_local_playlist(playlist.id, "After Hours")
            .expect("playlist should rename");
        let renamed = sync_local_playlist(&workspace.database, playlist.id)
            .expect("renamed managed playlist should sync");
        let renamed_path = workspace.root.join("00 Playlists/After Hours.m3u");

        assert_eq!(renamed.m3u_path.as_deref(), renamed_path.to_str());
        assert!(renamed_path.is_file());
        assert!(!first_path.exists());
    }

    #[test]
    fn external_m3u_target_is_preserved_when_playlist_is_renamed() {
        let workspace = TestWorkspace::new();
        let target = workspace.root.join("user-owned.m3u8");
        fs::write(&target, "#EXTM3U\n").expect("external playlist should exist");
        let playlist = workspace
            .database
            .import_local_playlist_from_m3u("Imported", &target.to_string_lossy(), &[])
            .expect("external playlist should import");

        workspace
            .database
            .rename_local_playlist(playlist.id, "Renamed")
            .expect("imported playlist should rename");
        let synced = sync_local_playlist(&workspace.database, playlist.id)
            .expect("external target should still sync");

        assert_eq!(synced.m3u_path.as_deref(), target.to_str());
        assert!(!synced.m3u_managed);
        assert!(target.is_file());
    }

    #[test]
    fn automatic_sync_after_membership_change_rewrites_managed_m3u() {
        let workspace = TestWorkspace::new();
        let mut settings = workspace
            .database
            .get_settings()
            .expect("settings should load");
        settings.library_root = Some(workspace.root.to_string_lossy().into_owned());
        workspace
            .database
            .update_settings(&settings)
            .expect("library root should persist");
        let track = workspace.add_track("New Track", "new-track.flac", true);
        let playlist = workspace
            .database
            .create_local_playlist("Automatic")
            .expect("playlist should create");
        sync_local_playlist(&workspace.database, playlist.id)
            .expect("empty managed playlist should sync");

        workspace
            .database
            .add_tracks_to_local_playlist(playlist.id, &[track])
            .expect("track should add");
        let synced = sync_local_playlist_after_change(&workspace.database, playlist.id)
            .expect("automatic sync should return the updated playlist");
        let output = fs::read_to_string(
            synced
                .m3u_path
                .as_deref()
                .expect("managed playlist path should remain set"),
        )
        .expect("managed playlist should be readable");

        assert!(output.contains("New Track"));
        assert!(output.contains("new-track.flac"));
    }

    #[test]
    fn sync_replaces_stale_m3u_with_playable_entries_when_tracks_are_missing() {
        let workspace = TestWorkspace::new();
        let present = workspace.add_track("Playable", "playable.flac", true);
        let missing = workspace.add_track("Missing", "missing.flac", false);
        let playlist = workspace
            .database
            .create_local_playlist("Broken")
            .expect("playlist should create");
        let playlist = workspace
            .database
            .add_tracks_to_local_playlist(playlist.id, &[present, missing])
            .expect("track should add");
        let target = workspace.root.join("broken.m3u");
        fs::write(&target, "existing\n").expect("existing playlist should write");
        workspace
            .database
            .set_local_playlist_m3u_path(playlist.id, Some(&target.to_string_lossy()))
            .expect("target should set");

        let synced = sync_local_playlist(&workspace.database, playlist.id)
            .expect("missing files should not block an updated playable M3U");

        let updated = fs::read_to_string(&target).expect("updated playlist should exist");
        assert!(updated.contains("playable.flac"));
        assert!(updated.contains("#REFRAIN-UNAVAILABLE: Missing"));
        assert!(!updated.contains("existing"));
        assert!(!updated.contains("missing.flac"));
        assert!(synced.last_synced_at.is_some());
        assert!(
            synced
                .sync_error
                .as_deref()
                .is_some_and(|value| value.contains("Missing"))
        );

        fs::write(workspace.root.join("missing.flac"), b"audio").unwrap();
        let recovered = sync_local_playlist(&workspace.database, playlist.id).unwrap();
        assert_eq!(recovered.sync_error, None);
        assert!(
            fs::read_to_string(&target)
                .unwrap()
                .contains("missing.flac")
        );
        assert!(
            !fs::read_to_string(&target)
                .unwrap()
                .contains("#REFRAIN-UNAVAILABLE")
        );
    }

    #[test]
    fn startup_sync_migrates_previous_managed_m3u8_to_m3u() {
        let workspace = TestWorkspace::new();
        let mut settings = workspace.database.get_settings().unwrap();
        settings.library_root = Some(workspace.root.to_string_lossy().into_owned());
        workspace.database.update_settings(&settings).unwrap();
        let playlist = workspace.database.create_local_playlist("Legacy").unwrap();
        let legacy = workspace.root.join("Playlists/Legacy.m3u8");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, format!("#EXTM3U\n{MANAGED_M3U_MARKER}\nold\n")).unwrap();
        workspace
            .database
            .set_local_playlist_managed_m3u_path(playlist.id, &legacy.to_string_lossy())
            .unwrap();

        sync_managed_local_playlists(&workspace.database).unwrap();

        let expected = workspace.root.join("00 Playlists/Legacy.m3u");
        assert_eq!(
            workspace
                .database
                .get_local_playlist(playlist.id)
                .unwrap()
                .m3u_path
                .as_deref(),
            expected.to_str()
        );
        assert_eq!(
            fs::read_to_string(&expected).unwrap(),
            format!("#EXTM3U\n{MANAGED_M3U_MARKER}\n")
        );
        assert!(!legacy.exists());
        assert!(!workspace.root.join("Playlists").exists());
    }

    #[test]
    fn startup_migrates_existing_custom_artwork_without_changing_source_image() {
        let workspace = TestWorkspace::new();
        let mut settings = workspace.database.get_settings().unwrap();
        settings.library_root = Some(workspace.root.to_string_lossy().into_owned());
        workspace.database.update_settings(&settings).unwrap();
        let playlist = workspace.database.create_local_playlist("Feels").unwrap();
        let old_file = workspace
            .root
            .join("playlist-artwork")
            .join(format!("{}-123.png", playlist.id));
        fs::create_dir_all(old_file.parent().unwrap()).unwrap();
        let bytes = b"\x89PNG\r\n\x1a\noriginal";
        fs::write(&old_file, bytes).unwrap();
        workspace
            .database
            .set_local_playlist_artwork_path(playlist.id, Some(&old_file.to_string_lossy()))
            .unwrap();
        sync_managed_local_playlists(&workspace.database).unwrap();

        let detail = workspace.database.get_local_playlist(playlist.id).unwrap();
        let artwork = PathBuf::from(detail.custom_artwork_path.unwrap());
        assert_eq!(
            artwork.parent(),
            Some(workspace.root.join("00 Playlists").as_path())
        );
        assert_eq!(fs::read(&artwork).unwrap(), bytes);
        assert!(!old_file.exists());
        assert!(workspace.root.join("00 Playlists/Feels.m3u").exists());
    }

    #[test]
    fn imported_playlist_keeps_m3u_but_moves_legacy_artwork_to_managed_folder() {
        let workspace = TestWorkspace::new();
        let mut settings = workspace.database.get_settings().unwrap();
        settings.library_root = Some(workspace.root.to_string_lossy().into_owned());
        workspace.database.update_settings(&settings).unwrap();
        let m3u = workspace.root.join("Imported.m3u");
        fs::write(&m3u, "#EXTM3U\n").unwrap();
        let playlist = workspace
            .database
            .import_local_playlist_from_m3u("Imported", &m3u.to_string_lossy(), &[])
            .unwrap();
        let old_artwork = workspace
            .root
            .join("playlist-artwork")
            .join(format!("{}-999.png", playlist.id));
        fs::create_dir_all(old_artwork.parent().unwrap()).unwrap();
        fs::write(&old_artwork, b"\x89PNG\r\n\x1a\nart").unwrap();
        workspace
            .database
            .set_local_playlist_artwork_path(playlist.id, Some(&old_artwork.to_string_lossy()))
            .unwrap();

        sync_managed_local_playlists(&workspace.database).unwrap();
        let detail = workspace.database.get_local_playlist(playlist.id).unwrap();
        assert_eq!(detail.m3u_path.as_deref(), m3u.to_str());
        assert_eq!(fs::read_to_string(&m3u).unwrap(), "#EXTM3U\n");
        assert_eq!(
            Path::new(detail.custom_artwork_path.as_deref().unwrap()).parent(),
            Some(workspace.root.join("00 Playlists").as_path())
        );
        assert!(!old_artwork.exists());
    }

    #[test]
    fn changing_library_root_relocates_custom_artwork_and_managed_m3u() {
        let workspace = TestWorkspace::new();
        let mut settings = workspace.database.get_settings().unwrap();
        settings.library_root = Some(workspace.root.to_string_lossy().into_owned());
        workspace.database.update_settings(&settings).unwrap();
        let playlist = workspace.database.create_local_playlist("Drive").unwrap();
        sync_local_playlist(&workspace.database, playlist.id).unwrap();
        let original = workspace
            .root
            .join("00 Playlists")
            .join(format!("refrain-artwork-{}-123.png", playlist.id));
        fs::write(&original, b"\x89PNG\r\n\x1a\ncover").unwrap();
        workspace
            .database
            .set_local_playlist_artwork_path(playlist.id, Some(&original.to_string_lossy()))
            .unwrap();

        let next_root = workspace.root.join("new-library");
        fs::create_dir_all(&next_root).unwrap();
        settings.library_root = Some(next_root.to_string_lossy().into_owned());
        workspace.database.update_settings(&settings).unwrap();
        sync_managed_local_playlists(&workspace.database).unwrap();

        let updated = workspace.database.get_local_playlist(playlist.id).unwrap();
        let artwork = Path::new(updated.custom_artwork_path.as_deref().unwrap());
        assert_eq!(
            artwork.parent(),
            Some(next_root.join("00 Playlists").as_path())
        );
        assert_eq!(fs::read(artwork).unwrap(), b"\x89PNG\r\n\x1a\ncover");
        assert!(!original.exists());
        assert_eq!(
            updated.m3u_path.as_deref(),
            next_root.join("00 Playlists/Drive.m3u").to_str()
        );
        assert!(!workspace.root.join("00 Playlists/Drive.m3u").exists());
    }

    #[test]
    fn legacy_playlist_directory_is_retained_if_it_contains_unmanaged_files() {
        let workspace = TestWorkspace::new();
        let mut settings = workspace.database.get_settings().unwrap();
        settings.library_root = Some(workspace.root.to_string_lossy().into_owned());
        workspace.database.update_settings(&settings).unwrap();
        let playlist = workspace.database.create_local_playlist("Managed").unwrap();
        let legacy = workspace.root.join("Playlists/Managed.m3u");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, format!("#EXTM3U\n{MANAGED_M3U_MARKER}\n")).unwrap();
        fs::write(workspace.root.join("Playlists/Personal.m3u"), "#EXTM3U\n").unwrap();
        workspace
            .database
            .set_local_playlist_managed_m3u_path(playlist.id, &legacy.to_string_lossy())
            .unwrap();

        sync_managed_local_playlists(&workspace.database).unwrap();
        assert!(workspace.root.join("00 Playlists/Managed.m3u").exists());
        assert!(workspace.root.join("Playlists/Personal.m3u").exists());
        assert!(!legacy.exists());
    }

    #[test]
    fn sync_requires_an_absolute_m3u_target() {
        let workspace = TestWorkspace::new();
        let playlist = workspace
            .database
            .create_local_playlist("Target")
            .expect("playlist should create");
        workspace
            .database
            .set_local_playlist_m3u_path(
                playlist.id,
                Some(Path::new("relative.m3u8").to_str().unwrap()),
            )
            .expect("target should set");

        let error = sync_local_playlist(&workspace.database, playlist.id)
            .expect_err("relative target should fail");
        assert!(error.to_string().contains("absolute"));
    }
}
