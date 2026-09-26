use std::{
    collections::HashMap,
    ffi::OsStr,
    fmt, fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use unicode_normalization::UnicodeNormalization;

use crate::db::{Database, DatabaseError, NormalizationCandidate};

const MAX_COMPONENT_BYTES: usize = 120;
static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct NormalizationSummary {
    pub moved: usize,
    pub unchanged: usize,
}

#[derive(Debug)]
pub(crate) enum NormalizationError {
    Cancelled,
    Database(DatabaseError),
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    InvalidState(String),
    UnsafePath(String),
}

impl fmt::Display for NormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("filesystem normalization was cancelled"),
            Self::Database(error) => write!(formatter, "normalization persistence failed: {error}"),
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} {}: {source}",
                path.display()
            ),
            Self::InvalidState(message) | Self::UnsafePath(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for NormalizationError {}

impl From<DatabaseError> for NormalizationError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

pub(crate) fn normalize_resolved_files(
    database: &Database,
    library_root: &Path,
    mut is_cancelled: impl FnMut() -> bool,
) -> Result<NormalizationSummary, NormalizationError> {
    let root = canonicalize(library_root, "resolve library root")?;
    let candidates = database.normalization_candidates()?;
    let mut occupied = database
        .local_files_snapshot()?
        .into_iter()
        .map(|file| (portable_path_key(Path::new(&file.path)), file.id))
        .collect::<HashMap<_, _>>();
    let mut summary = NormalizationSummary::default();

    for candidate in candidates {
        if is_cancelled() {
            return Err(NormalizationError::Cancelled);
        }

        let source = PathBuf::from(&candidate.path);
        let canonical_source = canonicalize(&source, "resolve source file")?;
        ensure_path_inside_root(&root, &canonical_source, "source file")?;
        if !canonical_source.is_file() {
            return Err(NormalizationError::InvalidState(format!(
                "normalization source is not a regular file: {}",
                source.display()
            )));
        }

        let extension = source
            .extension()
            .and_then(OsStr::to_str)
            .or(candidate.format.as_deref())
            .map(sanitize_extension)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                NormalizationError::InvalidState(format!(
                    "local file {} has no usable extension",
                    source.display()
                ))
            })?;
        let relative = canonical_relative_path(&candidate, &extension);
        validate_relative_path(&relative)?;
        let base_target = root.join(relative);
        validate_destination_parent(&root, &base_target)?;
        let target = resolve_collision_target(
            &base_target,
            &source,
            candidate.local_file_id,
            candidate.library_track_id,
            &occupied,
        )?;

        if source == target {
            summary.unchanged += 1;
            continue;
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| NormalizationError::Io {
                operation: "create normalization directory",
                path: parent.to_path_buf(),
                source,
            })?;
            validate_destination_parent(&root, &target)?;
        }

        move_file(&source, &target)?;
        let target_string = target.to_string_lossy().into_owned();
        if let Err(error) = database
            .update_local_file_path_after_normalization(candidate.local_file_id, &target_string)
        {
            if let Err(rollback_error) = move_file(&target, &source) {
                return Err(NormalizationError::InvalidState(format!(
                    "database update failed after moving {}; rollback also failed: {rollback_error}",
                    source.display()
                )));
            }
            return Err(NormalizationError::Database(error));
        }

        occupied.remove(&portable_path_key(&source));
        occupied.insert(portable_path_key(&target), candidate.local_file_id);
        summary.moved += 1;
    }

    Ok(summary)
}

fn canonical_relative_path(candidate: &NormalizationCandidate, extension: &str) -> PathBuf {
    let artist = candidate
        .artists
        .iter()
        .find(|artist| !artist.trim().is_empty())
        .map(String::as_str)
        .unwrap_or("Unknown Artist");
    let artist = sanitize_component(artist);

    let album = candidate
        .album
        .as_deref()
        .filter(|album| !album.trim().is_empty())
        .unwrap_or("Unknown Album");
    let album = if let Some(year) = candidate.release_year {
        format!("{} ({year})", sanitize_component(album))
    } else {
        sanitize_component(album)
    };

    let title = sanitize_component(&candidate.title);
    let track_number = candidate.track_number.unwrap_or(0).max(0);
    let file_stem = if candidate.is_multi_disc {
        let disc_number = candidate.disc_number.unwrap_or(1).max(1);
        format!("{disc_number}-{track_number:02} - {title}")
    } else {
        format!("{track_number:02} - {title}")
    };
    let file_name = format!("{}.{}", sanitize_component(&file_stem), extension);

    PathBuf::from(artist).join(album).join(file_name)
}

fn sanitize_component(value: &str) -> String {
    let normalized = value.nfkc().collect::<String>();
    let mut sanitized = String::with_capacity(normalized.len());
    for character in normalized.chars() {
        if character.is_control() {
            continue;
        }
        if matches!(
            character,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
        ) {
            sanitized.push('_');
        } else {
            sanitized.push(character);
        }
    }

    let mut sanitized = sanitized.trim().trim_end_matches([' ', '.']).to_owned();
    if sanitized.is_empty() || matches!(sanitized.as_str(), "." | "..") {
        sanitized = "_".into();
    }
    if is_windows_reserved_name(&sanitized) {
        sanitized.insert(0, '_');
    }
    shorten_component(&sanitized, MAX_COMPONENT_BYTES)
}

fn sanitize_extension(value: &str) -> String {
    value
        .nfkc()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_ascii_alphanumeric())
        .take(16)
        .collect()
}

fn shorten_component(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }

    let hash = blake3::hash(value.as_bytes()).to_hex().to_string();
    let suffix = format!(" [h-{}]", &hash[..8]);
    let budget = max_bytes.saturating_sub(suffix.len());
    let mut end = budget.min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    let prefix = value[..end].trim_end_matches([' ', '.']);
    let prefix = if prefix.is_empty() { "_" } else { prefix };
    format!("{prefix}{suffix}")
}

fn is_windows_reserved_name(value: &str) -> bool {
    let stem = value
        .split('.')
        .next()
        .unwrap_or(value)
        .trim()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .is_some_and(|number| (1..=9).contains(&number))
}

fn resolve_collision_target(
    base_target: &Path,
    source: &Path,
    local_file_id: i64,
    library_track_id: i64,
    occupied: &HashMap<String, i64>,
) -> Result<PathBuf, NormalizationError> {
    if !target_conflicts(base_target, source, local_file_id, occupied)? {
        return Ok(base_target.to_path_buf());
    }

    let suffixed = append_library_track_suffix(base_target, library_track_id)?;
    if target_conflicts(&suffixed, source, local_file_id, occupied)? {
        return Err(NormalizationError::InvalidState(format!(
            "normalization target remains occupied after stable collision suffixing: {}",
            suffixed.display()
        )));
    }
    Ok(suffixed)
}

fn target_conflicts(
    target: &Path,
    source: &Path,
    local_file_id: i64,
    occupied: &HashMap<String, i64>,
) -> Result<bool, NormalizationError> {
    let target_key = portable_path_key(target);
    let source_key = portable_path_key(source);
    if let Some(existing_id) = occupied.get(&target_key)
        && (*existing_id != local_file_id || target_key != source_key)
    {
        return Ok(true);
    }

    let Some(parent) = target.parent() else {
        return Ok(false);
    };
    if !parent.exists() {
        return Ok(false);
    }
    let target_name = target
        .file_name()
        .map(portable_name_key)
        .unwrap_or_default();
    for entry in fs::read_dir(parent).map_err(|source| NormalizationError::Io {
        operation: "inspect normalization target directory",
        path: parent.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| NormalizationError::Io {
            operation: "inspect normalization target entry",
            path: parent.to_path_buf(),
            source,
        })?;
        if portable_name_key(&entry.file_name()) == target_name {
            let entry_key = portable_path_key(&entry.path());
            if entry_key != source_key {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn append_library_track_suffix(
    path: &Path,
    library_track_id: i64,
) -> Result<PathBuf, NormalizationError> {
    let stem = path.file_stem().and_then(OsStr::to_str).ok_or_else(|| {
        NormalizationError::InvalidState("target filename is not valid UTF-8".into())
    })?;
    let extension = path.extension().and_then(OsStr::to_str);
    let suffix = format!(" [lt-{library_track_id}]");
    let stem = shorten_component(stem, MAX_COMPONENT_BYTES.saturating_sub(suffix.len()));
    let mut file_name = format!("{stem}{suffix}");
    if let Some(extension) = extension {
        file_name.push('.');
        file_name.push_str(extension);
    }
    Ok(path.with_file_name(file_name))
}

fn validate_relative_path(path: &Path) -> Result<(), NormalizationError> {
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(NormalizationError::UnsafePath(format!(
            "normalization path escapes the library root: {}",
            path.display()
        )));
    }
    Ok(())
}

fn ensure_path_inside_root(
    root: &Path,
    path: &Path,
    label: &str,
) -> Result<(), NormalizationError> {
    if path.strip_prefix(root).is_err() {
        return Err(NormalizationError::UnsafePath(format!(
            "{label} is outside the configured library root: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_destination_parent(root: &Path, target: &Path) -> Result<(), NormalizationError> {
    ensure_path_inside_root(root, target, "normalization target")?;
    let mut ancestor = target.parent();
    while let Some(path) = ancestor {
        if path.exists() {
            let canonical = canonicalize(path, "resolve normalization target parent")?;
            ensure_path_inside_root(root, &canonical, "normalization target parent")?;
            return Ok(());
        }
        ancestor = path.parent();
    }
    Err(NormalizationError::UnsafePath(format!(
        "normalization target has no existing ancestor inside the library root: {}",
        target.display()
    )))
}

fn canonicalize(path: &Path, operation: &'static str) -> Result<PathBuf, NormalizationError> {
    fs::canonicalize(path).map_err(|source| NormalizationError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })
}

fn portable_path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn portable_name_key(value: &OsStr) -> String {
    value.to_string_lossy().to_lowercase()
}

trait FileOperations {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn len(&self, path: &Path) -> io::Result<u64>;
    fn hash(&self, path: &Path) -> io::Result<blake3::Hash>;
}

struct StdFileOperations;

impl FileOperations for StdFileOperations {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64> {
        fs::copy(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn len(&self, path: &Path) -> io::Result<u64> {
        Ok(fs::metadata(path)?.len())
    }

    fn hash(&self, path: &Path) -> io::Result<blake3::Hash> {
        let mut file = fs::File::open(path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hasher.finalize())
    }
}

fn move_file(source: &Path, target: &Path) -> Result<(), NormalizationError> {
    move_file_with(&StdFileOperations, source, target)
}

fn move_file_with(
    operations: &impl FileOperations,
    source: &Path,
    target: &Path,
) -> Result<(), NormalizationError> {
    if portable_path_key(source) == portable_path_key(target) && source != target {
        return case_only_rename(operations, source, target);
    }

    match operations.rename(source, target) {
        Ok(()) => Ok(()),
        Err(error) if is_cross_device_error(&error) => {
            copy_move_with_verification(operations, source, target)
        }
        Err(source_error) => Err(NormalizationError::Io {
            operation: "move normalized file",
            path: source.to_path_buf(),
            source: source_error,
        }),
    }
}

fn case_only_rename(
    operations: &impl FileOperations,
    source: &Path,
    target: &Path,
) -> Result<(), NormalizationError> {
    let temporary = temporary_destination(source)?;
    operations
        .rename(source, &temporary)
        .map_err(|source_error| NormalizationError::Io {
            operation: "stage case-only rename",
            path: source.to_path_buf(),
            source: source_error,
        })?;
    if let Err(source_error) = operations.rename(&temporary, target) {
        let _ = operations.rename(&temporary, source);
        return Err(NormalizationError::Io {
            operation: "finish case-only rename",
            path: target.to_path_buf(),
            source: source_error,
        });
    }
    Ok(())
}

fn copy_move_with_verification(
    operations: &impl FileOperations,
    source: &Path,
    target: &Path,
) -> Result<(), NormalizationError> {
    let temporary = temporary_destination(target)?;
    if let Err(source_error) = operations.copy(source, &temporary) {
        let _ = operations.remove_file(&temporary);
        return Err(NormalizationError::Io {
            operation: "copy normalized file across filesystems",
            path: source.to_path_buf(),
            source: source_error,
        });
    }

    let verification = (|| -> io::Result<bool> {
        Ok(operations.len(source)? == operations.len(&temporary)?
            && operations.hash(source)? == operations.hash(&temporary)?)
    })();
    match verification {
        Ok(true) => {}
        Ok(false) => {
            let _ = operations.remove_file(&temporary);
            return Err(NormalizationError::InvalidState(format!(
                "copied normalization file failed verification: {}",
                source.display()
            )));
        }
        Err(source_error) => {
            let _ = operations.remove_file(&temporary);
            return Err(NormalizationError::Io {
                operation: "verify copied normalization file",
                path: temporary,
                source: source_error,
            });
        }
    }

    if let Err(source_error) = operations.rename(&temporary, target) {
        let _ = operations.remove_file(&temporary);
        return Err(NormalizationError::Io {
            operation: "commit copied normalization file",
            path: target.to_path_buf(),
            source: source_error,
        });
    }
    if let Err(source_error) = operations.remove_file(source) {
        let _ = operations.remove_file(target);
        return Err(NormalizationError::Io {
            operation: "remove copied normalization source",
            path: source.to_path_buf(),
            source: source_error,
        });
    }
    Ok(())
}

fn temporary_destination(path: &Path) -> Result<PathBuf, NormalizationError> {
    let parent = path.parent().ok_or_else(|| {
        NormalizationError::InvalidState(format!(
            "normalization path has no parent: {}",
            path.display()
        ))
    })?;
    let id = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(".refrain-tmp-{}-{id}", std::process::id())))
}

fn is_cross_device_error(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::CrossesDevices
}

#[allow(dead_code)]
pub(crate) fn trash_managed_local_file(
    database: &Database,
    library_root: &Path,
    local_file_id: i64,
) -> Result<(), NormalizationError> {
    let file = database.local_file(local_file_id)?.ok_or_else(|| {
        NormalizationError::InvalidState(format!("local file {local_file_id} was not found"))
    })?;
    if file.ownership != "managed" {
        return Err(NormalizationError::InvalidState(
            "external files cannot be moved to Trash automatically".into(),
        ));
    }
    let root = canonicalize(library_root, "resolve library root")?;
    let path = canonicalize(Path::new(&file.path), "resolve managed file")?;
    ensure_path_inside_root(&root, &path, "managed file")?;
    platform_trash(&path)?;
    database.mark_local_files_missing(&[local_file_id])?;
    Ok(())
}

fn platform_trash(path: &Path) -> Result<(), NormalizationError> {
    #[cfg(target_os = "macos")]
    let status = Command::new("osascript")
        .arg("-e")
        .arg(
            "on run argv\n tell application \"Finder\" to delete POSIX file (item 1 of argv)\nend run",
        )
        .arg(path)
        .status();

    #[cfg(target_os = "windows")]
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($args[0], 'OnlyErrorDialogs', 'SendToRecycleBin')",
        ])
        .arg(path)
        .status();

    #[cfg(target_os = "linux")]
    let status = Command::new("gio")
        .arg("trash")
        .arg("--")
        .arg(path)
        .status();

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    let status: io::Result<std::process::ExitStatus> = Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "platform Trash integration is unavailable",
    ));

    let status = status.map_err(|source| NormalizationError::Io {
        operation: "move managed file to Trash",
        path: path.to_path_buf(),
        source,
    })?;
    if !status.success() {
        return Err(NormalizationError::InvalidState(format!(
            "platform Trash operation failed for {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        fs, io,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::{
        db::{Database, LocalFileWrite},
        domain::{CollectionEntry, SourceAccount, SourceCollection, SourceTrack},
    };

    use super::*;

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "refrain-normalization-{name}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn candidate() -> NormalizationCandidate {
        NormalizationCandidate {
            library_track_id: 1234,
            title: "Weird Fishes / Arpeggi".into(),
            artists: vec!["Radiohead".into()],
            album: Some("In Rainbows".into()),
            release_year: Some(2007),
            disc_number: Some(1),
            track_number: Some(4),
            is_multi_disc: false,
            local_file_id: 42,
            path: "/music/source.flac".into(),
            format: Some("flac".into()),
        }
    }

    #[test]
    fn sanitizer_handles_windows_invalid_and_reserved_names() {
        assert_eq!(
            sanitize_component("bad<>:\"/\\|?*name. "),
            "bad_________name"
        );
        assert_eq!(sanitize_component("CON"), "_CON");
        assert_eq!(sanitize_component("lpt9.txt"), "_lpt9.txt");
        assert_eq!(sanitize_component(".."), "_");
    }

    #[test]
    fn sanitizer_normalizes_unicode_and_shortens_long_components_stably() {
        assert_eq!(sanitize_component("Ａrtist"), "Artist");
        let long = "é".repeat(100);
        let first = sanitize_component(&long);
        let second = sanitize_component(&long);
        assert_eq!(first, second);
        assert!(first.len() <= MAX_COMPONENT_BYTES);
        assert!(first.contains("[h-"));
    }

    #[test]
    fn canonical_paths_cover_single_multi_disc_and_unknown_album_year() {
        let single = canonical_relative_path(&candidate(), "flac");
        assert_eq!(
            single,
            PathBuf::from("Radiohead/In Rainbows (2007)/04 - Weird Fishes _ Arpeggi.flac")
        );

        let mut multi = candidate();
        multi.is_multi_disc = true;
        multi.disc_number = Some(2);
        multi.track_number = Some(3);
        assert_eq!(
            canonical_relative_path(&multi, "flac"),
            PathBuf::from("Radiohead/In Rainbows (2007)/2-03 - Weird Fishes _ Arpeggi.flac")
        );

        let mut unknown = candidate();
        unknown.album = None;
        unknown.release_year = None;
        assert_eq!(
            canonical_relative_path(&unknown, "flac"),
            PathBuf::from("Radiohead/Unknown Album/04 - Weird Fishes _ Arpeggi.flac")
        );
    }

    #[test]
    fn collision_resolution_is_case_insensitive_and_stable() {
        let directory = TestDir::new("collision");
        let source = directory.0.join("source.flac");
        fs::write(&source, b"source").unwrap();
        let target = directory.0.join("Song.flac");
        fs::write(directory.0.join("song.FLAC"), b"other").unwrap();
        let occupied = HashMap::new();
        let resolved = resolve_collision_target(&target, &source, 1, 77, &occupied).unwrap();
        assert_eq!(resolved, directory.0.join("Song [lt-77].flac"));
    }

    struct RecordingOperations {
        rename_calls: Cell<usize>,
    }

    impl FileOperations for RecordingOperations {
        fn rename(&self, _from: &Path, _to: &Path) -> io::Result<()> {
            self.rename_calls.set(self.rename_calls.get() + 1);
            Ok(())
        }

        fn copy(&self, _from: &Path, _to: &Path) -> io::Result<u64> {
            unreachable!("case-only renames should not copy")
        }

        fn remove_file(&self, _path: &Path) -> io::Result<()> {
            unreachable!("case-only renames should not remove files")
        }

        fn len(&self, _path: &Path) -> io::Result<u64> {
            unreachable!("case-only renames should not verify copies")
        }

        fn hash(&self, _path: &Path) -> io::Result<blake3::Hash> {
            unreachable!("case-only renames should not hash files")
        }
    }

    #[test]
    fn case_only_rename_uses_a_temporary_path() {
        let operations = RecordingOperations {
            rename_calls: Cell::new(0),
        };
        move_file_with(
            &operations,
            Path::new("/music/Song.flac"),
            Path::new("/music/song.flac"),
        )
        .unwrap();
        assert_eq!(operations.rename_calls.get(), 2);
    }

    #[test]
    fn path_validation_rejects_traversal() {
        assert!(validate_relative_path(Path::new("Artist/../escape.flac")).is_err());
        assert!(validate_relative_path(Path::new("Artist/Album/track.flac")).is_ok());
    }

    struct FailingOperations {
        rename_error: io::ErrorKind,
        copy_error: Option<io::ErrorKind>,
        copy_calls: Cell<usize>,
        remove_calls: Cell<usize>,
    }

    impl FileOperations for FailingOperations {
        fn rename(&self, _from: &Path, _to: &Path) -> io::Result<()> {
            if self.rename_error == io::ErrorKind::CrossesDevices {
                Err(io::Error::new(
                    io::ErrorKind::CrossesDevices,
                    "cross-device rename",
                ))
            } else {
                Err(io::Error::new(self.rename_error, "rename failed"))
            }
        }

        fn copy(&self, _from: &Path, _to: &Path) -> io::Result<u64> {
            self.copy_calls.set(self.copy_calls.get() + 1);
            if let Some(kind) = self.copy_error {
                Err(io::Error::new(kind, "copy failed"))
            } else {
                Ok(1)
            }
        }

        fn remove_file(&self, _path: &Path) -> io::Result<()> {
            self.remove_calls.set(self.remove_calls.get() + 1);
            Ok(())
        }

        fn len(&self, _path: &Path) -> io::Result<u64> {
            Ok(1)
        }

        fn hash(&self, _path: &Path) -> io::Result<blake3::Hash> {
            Ok(blake3::hash(b"same"))
        }
    }

    #[test]
    fn atomic_move_failure_does_not_fall_back_to_copy() {
        let operations = FailingOperations {
            rename_error: io::ErrorKind::PermissionDenied,
            copy_error: None,
            copy_calls: Cell::new(0),
            remove_calls: Cell::new(0),
        };
        assert!(move_file_with(&operations, Path::new("a"), Path::new("b")).is_err());
        assert_eq!(operations.copy_calls.get(), 0);
    }

    #[test]
    fn cross_filesystem_copy_failure_preserves_source() {
        let operations = FailingOperations {
            rename_error: io::ErrorKind::CrossesDevices,
            copy_error: Some(io::ErrorKind::PermissionDenied),
            copy_calls: Cell::new(0),
            remove_calls: Cell::new(0),
        };
        assert!(move_file_with(&operations, Path::new("a"), Path::new("b")).is_err());
        assert_eq!(operations.copy_calls.get(), 1);
        assert_eq!(operations.remove_calls.get(), 1);
    }

    #[test]
    fn normalization_uses_local_library_metadata_without_a_spotify_link() {
        let directory = TestDir::new("local-only");
        let database_path = directory.0.join("refrain.sqlite3");
        let library_root = directory.0.join("Music");
        fs::create_dir_all(&library_root).unwrap();
        let source_path = library_root.join("loose.flac");
        fs::write(&source_path, b"audio").unwrap();
        let database = Database::open(database_path).unwrap();

        let local_file_id = database
            .insert_local_file(&LocalFileWrite {
                path: source_path.to_string_lossy().into_owned(),
                state: "present".into(),
                format: Some("flac".into()),
                file_size: 5,
                modified_at: 1,
                duration_ms: Some(180_000),
                bitrate: None,
                sample_rate: None,
                channels: None,
                content_hash: None,
                tag_title: Some("Local Track".into()),
                tag_artists: vec!["Local Artist".into()],
                tag_album: Some("Local Album".into()),
                tag_isrc: Some("LOCAL1234567".into()),
                artwork_path: None,
                artwork_mime: None,
                scan_error: None,
            })
            .unwrap();
        database
            .ensure_library_tracks_for_present_local_files()
            .unwrap();

        let summary = normalize_resolved_files(&database, &library_root, || false).unwrap();
        assert_eq!(summary.moved, 1);
        let file = database.local_file(local_file_id).unwrap().unwrap();
        let canonical_root = fs::canonicalize(&library_root).unwrap();
        assert_eq!(
            PathBuf::from(file.path),
            canonical_root.join("Local Artist/Local Album/00 - Local Track.flac")
        );
    }

    #[test]
    fn normalization_preserves_external_ownership_and_preferred_file() {
        let directory = TestDir::new("ownership");
        let database_path = directory.0.join("refrain.sqlite3");
        let library_root = directory.0.join("Music");
        fs::create_dir_all(&library_root).unwrap();
        let source_path = library_root.join("loose.flac");
        fs::write(&source_path, b"audio").unwrap();
        let database = Database::open(database_path).unwrap();

        let account_id = database
            .upsert_source_account(&SourceAccount {
                provider: "spotify".into(),
                provider_account_id: "account".into(),
                display_name: None,
                client_id: "client".into(),
            })
            .unwrap();
        let collection_id = database
            .upsert_source_collection(&SourceCollection {
                source_account_id: account_id,
                provider_collection_id: "playlist".into(),
                kind: "playlist".into(),
                name: "Playlist".into(),
                snapshot_id: None,
                owner_provider_id: None,
                is_accessible: true,
                access_issue: None,
            })
            .unwrap();
        let source_track_id = database
            .upsert_source_track(&SourceTrack {
                provider: "spotify".into(),
                provider_track_id: "track".into(),
                uri: None,
                isrc: Some("USABC1234567".into()),
                title: "Track".into(),
                normalized_title: "track".into(),
                artists_json: "[\"Artist\"]".into(),
                normalized_artists: "artist".into(),
                album: Some("Album".into()),
                normalized_album: Some("album".into()),
                duration_ms: Some(180_000),
                disc_number: Some(1),
                track_number: Some(1),
                release_year: Some(2024),
                explicit: Some(false),
                version_kind: None,
                version_detail: None,
                image_url: None,
                external_url: None,
            })
            .unwrap();
        database
            .replace_collection_entries(
                collection_id,
                &[CollectionEntry {
                    position: 0,
                    source_track_id: Some(source_track_id),
                    provider_item_uri: None,
                    item_type: "track".into(),
                    added_at: None,
                    unavailable_reason: None,
                }],
            )
            .unwrap();
        let local_file_id = database
            .insert_local_file(&LocalFileWrite {
                path: source_path.to_string_lossy().into_owned(),
                state: "present".into(),
                format: Some("flac".into()),
                file_size: 5,
                modified_at: 1,
                duration_ms: Some(180_000),
                bitrate: None,
                sample_rate: None,
                channels: None,
                content_hash: None,
                tag_title: Some("Track".into()),
                tag_artists: vec!["Artist".into()],
                tag_album: Some("Album".into()),
                tag_isrc: Some("USABC1234567".into()),
                artwork_path: None,
                artwork_mime: None,
                scan_error: None,
            })
            .unwrap();
        database
            .ensure_library_tracks_for_present_local_files()
            .unwrap();
        let library_track_id = database
            .local_file(local_file_id)
            .unwrap()
            .unwrap()
            .library_track_id
            .unwrap();
        database
            .confirm_match(source_track_id, library_track_id)
            .unwrap();

        let summary = normalize_resolved_files(&database, &library_root, || false).unwrap();
        assert_eq!(summary.moved, 1);
        let file = database.local_file(local_file_id).unwrap().unwrap();
        assert_eq!(file.ownership, "external");
        assert!(file.is_preferred);
        let canonical_root = fs::canonicalize(&library_root).unwrap();
        assert_eq!(
            PathBuf::from(file.path),
            canonical_root.join("Artist/Album (2024)/01 - Track.flac")
        );
    }
}
