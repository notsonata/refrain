use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs,
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use lofty::{
    file::{AudioFile, TaggedFileExt},
    picture::PictureType,
    tag::{Accessor, ItemKey},
};
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

use crate::{
    db::{Database, DatabaseError, LocalFileWrite},
    domain::LocalFile,
};

pub const LOCAL_LIBRARY_SCAN_PROGRESS_EVENT: &str = "local-library-scan-progress";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLibraryScanProgress {
    pub phase: String,
    pub completed: usize,
    pub total: Option<usize>,
    pub message: String,
}

impl LocalLibraryScanProgress {
    fn new(
        phase: &str,
        completed: usize,
        total: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            phase: phase.into(),
            completed,
            total,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LocalLibraryScanSummary {
    pub discovered: usize,
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub moved: usize,
    pub missing: usize,
    pub invalid: usize,
}

#[derive(Debug)]
pub enum LocalLibraryError {
    InvalidRoot(String),
    Io(std::io::Error),
    Walk(walkdir::Error),
    Database(DatabaseError),
    FileNotFound(i64),
}

impl fmt::Display for LocalLibraryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRoot(message) => formatter.write_str(message),
            Self::Io(error) => write!(formatter, "library filesystem error: {error}"),
            Self::Walk(error) => write!(formatter, "library traversal error: {error}"),
            Self::Database(error) => write!(formatter, "library database error: {error}"),
            Self::FileNotFound(id) => write!(formatter, "local file {id} was not found"),
        }
    }
}

impl Error for LocalLibraryError {}

impl From<std::io::Error> for LocalLibraryError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<walkdir::Error> for LocalLibraryError {
    fn from(error: walkdir::Error) -> Self {
        Self::Walk(error)
    }
}

impl From<DatabaseError> for LocalLibraryError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

pub fn scan_library(
    database: &Database,
    root: &Path,
    progress: impl Fn(LocalLibraryScanProgress),
) -> Result<LocalLibraryScanSummary, LocalLibraryError> {
    if !root.exists() {
        return Err(LocalLibraryError::InvalidRoot(format!(
            "Library root does not exist: {}",
            root.display()
        )));
    }
    if !root.is_dir() {
        return Err(LocalLibraryError::InvalidRoot(format!(
            "Library root is not a directory: {}",
            root.display()
        )));
    }

    let root = fs::canonicalize(root)?;
    let artwork_cache_dir = database
        .path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("artwork");
    fs::create_dir_all(&artwork_cache_dir)?;
    progress(LocalLibraryScanProgress::new(
        "starting",
        0,
        None,
        "Preparing local library scan",
    ));

    let existing = database.local_files_snapshot()?;
    let existing_by_path = existing
        .iter()
        .map(|file| (PathBuf::from(&file.path), file.clone()))
        .collect::<HashMap<_, _>>();
    let root_existing = existing
        .iter()
        .filter(|file| Path::new(&file.path).starts_with(&root))
        .cloned()
        .collect::<Vec<_>>();

    let paths = discover_audio_files(&root)?;
    let canonical_paths = paths
        .iter()
        .map(fs::canonicalize)
        .collect::<Result<Vec<_>, _>>()?;
    let discovered_paths = canonical_paths.iter().cloned().collect::<HashSet<_>>();
    let total = canonical_paths.len();
    progress(LocalLibraryScanProgress::new(
        "scanning",
        0,
        Some(total),
        format!("Scanning {total} audio files"),
    ));

    let mut summary = LocalLibraryScanSummary {
        discovered: total,
        ..LocalLibraryScanSummary::default()
    };
    let mut seen_ids = HashSet::new();
    let mut unmatched_existing = root_existing
        .iter()
        .filter(|file| !discovered_paths.contains(Path::new(&file.path)))
        .map(|file| (file.id, file.clone()))
        .collect::<HashMap<_, _>>();

    for (index, canonical_path) in canonical_paths.iter().enumerate() {
        let metadata = fs::metadata(canonical_path)?;
        let file_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
        let modified_at = system_time_ms(metadata.modified()?);

        if let Some(existing_file) = existing_by_path.get(canonical_path) {
            seen_ids.insert(existing_file.id);
            unmatched_existing.remove(&existing_file.id);
            if existing_file.file_size == file_size
                && existing_file.modified_at == modified_at
                && existing_file.state != "missing"
                && existing_file.artwork_path.is_some()
            {
                summary.unchanged += 1;
                if existing_file.state == "invalid" {
                    summary.invalid += 1;
                }
            } else {
                let scanned =
                    inspect_audio_file(canonical_path, file_size, modified_at, &artwork_cache_dir);
                database.update_local_file_scan(existing_file.id, &scanned)?;
                summary.updated += 1;
                if scanned.state == "invalid" {
                    summary.invalid += 1;
                }
            }
        } else {
            let mut scanned =
                inspect_audio_file(canonical_path, file_size, modified_at, &artwork_cache_dir);
            let moved_match = find_moved_match(canonical_path, &mut scanned, &unmatched_existing)?;
            if let Some(existing_file) = moved_match {
                database.update_local_file_scan(existing_file.id, &scanned)?;
                seen_ids.insert(existing_file.id);
                unmatched_existing.remove(&existing_file.id);
                summary.moved += 1;
                if scanned.state == "invalid" {
                    summary.invalid += 1;
                }
            } else {
                let id = database.insert_local_file(&scanned)?;
                seen_ids.insert(id);
                summary.added += 1;
                if scanned.state == "invalid" {
                    summary.invalid += 1;
                }
            }
        }

        if index + 1 == total || (index + 1) % 50 == 0 {
            progress(LocalLibraryScanProgress::new(
                "scanning",
                index + 1,
                Some(total),
                format!("Scanned {} of {total} audio files", index + 1),
            ));
        }
    }

    let missing_ids = existing
        .iter()
        .filter(|file| !seen_ids.contains(&file.id))
        .map(|file| file.id)
        .collect::<Vec<_>>();
    summary.missing = database.mark_local_files_missing(&missing_ids)?;

    progress(LocalLibraryScanProgress::new(
        "complete",
        total,
        Some(total),
        format!(
            "Local library scan complete · {} indexed files",
            summary.discovered
        ),
    ));
    Ok(summary)
}

pub fn ensure_local_file_hash(
    database: &Database,
    local_file_id: i64,
) -> Result<String, LocalLibraryError> {
    let local_file = database
        .local_file(local_file_id)?
        .ok_or(LocalLibraryError::FileNotFound(local_file_id))?;
    if let Some(hash) = local_file.content_hash {
        return Ok(hash);
    }
    if local_file.state != "present" {
        return Err(LocalLibraryError::InvalidRoot(
            "Only present local files can be hashed.".into(),
        ));
    }
    let hash = hash_file(Path::new(&local_file.path))?;
    database.update_local_file_hash(local_file_id, &hash)?;
    Ok(hash)
}

fn discover_audio_files(root: &Path) -> Result<Vec<PathBuf>, LocalLibraryError> {
    let mut paths = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !should_skip_entry(entry, root))
    {
        let entry = entry?;
        if entry.file_type().is_file() && is_supported_audio_path(entry.path()) {
            paths.push(entry.path().to_path_buf());
        }
    }
    paths.sort();
    Ok(paths)
}

fn should_skip_entry(entry: &DirEntry, root: &Path) -> bool {
    if entry.path() == root {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.') || name.starts_with("refrain-temp-"))
}

fn is_supported_audio_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("flac" | "mp3" | "m4a" | "aac" | "ogg" | "opus" | "wav" | "alac")
    )
}

fn inspect_audio_file(
    path: &Path,
    file_size: i64,
    modified_at: i64,
    artwork_cache_dir: &Path,
) -> LocalFileWrite {
    let format = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    match lofty::read_from_path(path) {
        Ok(tagged_file) => {
            let properties = tagged_file.properties();
            let tag = tagged_file
                .primary_tag()
                .or_else(|| tagged_file.first_tag());
            let tag_title = tag
                .and_then(|tag| tag.title())
                .map(|value| value.into_owned());
            let tag_album = tag
                .and_then(|tag| tag.album())
                .map(|value| value.into_owned());
            let mut tag_artists = tag
                .map(|tag| {
                    tag.get_strings(ItemKey::TrackArtists)
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if tag_artists.is_empty() {
                tag_artists = tag
                    .map(|tag| {
                        tag.get_strings(ItemKey::TrackArtist)
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
            }
            if tag_artists.is_empty()
                && let Some(artist) = tag.and_then(|tag| tag.artist())
            {
                tag_artists.push(artist.into_owned());
            }
            let tag_isrc = tag
                .and_then(|tag| tag.get_string(ItemKey::Isrc))
                .map(str::to_owned);
            let (artwork_path, artwork_mime) = tag
                .and_then(|tag| {
                    tag.get_picture_type(PictureType::CoverFront)
                        .or_else(|| tag.pictures().first())
                })
                .and_then(|picture| cache_artwork(artwork_cache_dir, picture))
                .map_or((None, None), |(path, mime)| (Some(path), Some(mime)));
            LocalFileWrite {
                path: path.to_string_lossy().into_owned(),
                state: "present".into(),
                format,
                file_size,
                modified_at,
                duration_ms: Some(properties.duration().as_millis() as i64),
                bitrate: properties.audio_bitrate().map(i64::from),
                sample_rate: properties.sample_rate().map(i64::from),
                channels: properties.channels().map(i64::from),
                content_hash: None,
                tag_title,
                tag_artists,
                tag_album,
                tag_isrc,
                artwork_path,
                artwork_mime,
                scan_error: None,
            }
        }
        Err(error) => LocalFileWrite {
            path: path.to_string_lossy().into_owned(),
            state: "invalid".into(),
            format,
            file_size,
            modified_at,
            duration_ms: None,
            bitrate: None,
            sample_rate: None,
            channels: None,
            content_hash: None,
            tag_title: None,
            tag_artists: Vec::new(),
            tag_album: None,
            tag_isrc: None,
            artwork_path: None,
            artwork_mime: None,
            scan_error: Some(error.to_string()),
        },
    }
}

fn cache_artwork(cache_dir: &Path, picture: &lofty::picture::Picture) -> Option<(String, String)> {
    if picture.data().is_empty() {
        return None;
    }
    let mime = picture.mime_type()?.as_str();
    let extension = match mime {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        _ => return None,
    };
    let hash = blake3::hash(picture.data()).to_hex().to_string();
    let path = cache_dir.join(format!("{hash}.{extension}"));
    if !path.exists()
        && let Err(error) = fs::write(&path, picture.data())
    {
        tracing::warn!(%error, path = %path.display(), "failed to cache embedded artwork");
        return None;
    }
    Some((path.to_string_lossy().into_owned(), mime.to_owned()))
}

fn find_moved_match(
    path: &Path,
    scanned: &mut LocalFileWrite,
    candidates: &HashMap<i64, LocalFile>,
) -> Result<Option<LocalFile>, LocalLibraryError> {
    let plausible = candidates
        .values()
        .filter(|candidate| candidate.file_size == scanned.file_size)
        .collect::<Vec<_>>();

    let hashed_candidates = plausible
        .iter()
        .copied()
        .filter(|candidate| candidate.content_hash.is_some())
        .collect::<Vec<_>>();
    if !hashed_candidates.is_empty() {
        let hash = hash_file(path)?;
        let hash_matches = hashed_candidates
            .into_iter()
            .filter(|candidate| candidate.content_hash.as_deref() == Some(hash.as_str()))
            .collect::<Vec<_>>();
        match hash_matches.as_slice() {
            [candidate] => {
                scanned.content_hash = Some(hash);
                return Ok(Some((*candidate).clone()));
            }
            [] => {}
            _ => return Ok(None),
        }
    }

    let strong_matches = plausible
        .into_iter()
        .filter(|candidate| {
            candidate.content_hash.is_none() && strong_metadata_match(candidate, scanned)
        })
        .collect::<Vec<_>>();
    if strong_matches.len() == 1 {
        return Ok(Some(strong_matches[0].clone()));
    }
    Ok(None)
}

fn strong_metadata_match(existing: &LocalFile, scanned: &LocalFileWrite) -> bool {
    if existing.state == "invalid" || scanned.state == "invalid" {
        return false;
    }
    let same_duration =
        existing.duration_ms.is_some() && existing.duration_ms == scanned.duration_ms;
    let same_isrc = existing.tag_isrc.is_some() && existing.tag_isrc == scanned.tag_isrc;
    let same_named_recording = existing.tag_title.is_some()
        && existing.tag_title == scanned.tag_title
        && !existing.tag_artists.is_empty()
        && existing.tag_artists == scanned.tag_artists
        && existing.tag_album == scanned.tag_album;
    same_duration && (same_isrc || same_named_recording)
}

fn hash_file(path: &Path) -> Result<String, LocalLibraryError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn system_time_ms(time: std::time::SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, Ordering},
        },
    };

    use crate::db::Database;

    use super::*;

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    struct TestLibrary {
        root: PathBuf,
        database_path: PathBuf,
    }

    impl TestLibrary {
        fn new(name: &str) -> Self {
            let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
            let base = std::env::temp_dir().join(format!(
                "refrain-library-{name}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&base).expect("test library should be created");
            Self {
                root: base.join("music"),
                database_path: base.join("refrain.sqlite3"),
            }
        }

        fn open_database(&self) -> Database {
            fs::create_dir_all(&self.root).expect("music root should exist");
            Database::open(self.database_path.clone()).expect("database should open")
        }
    }

    impl Drop for TestLibrary {
        fn drop(&mut self) {
            if let Some(base) = self.root.parent() {
                let _ = fs::remove_dir_all(base);
            }
        }
    }

    fn write_test_wav(path: &Path, sample_count: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("audio parent should exist");
        }
        let sample_rate = 8_000u32;
        let channels = 1u16;
        let bits_per_sample = 8u16;
        let data = vec![128u8; sample_count];
        let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
        let block_align = channels * bits_per_sample / 8;
        let mut file = fs::File::create(path).expect("wav should be created");
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(36 + data.len() as u32).to_le_bytes())
            .unwrap();
        file.write_all(b"WAVEfmt ").unwrap();
        file.write_all(&16u32.to_le_bytes()).unwrap();
        file.write_all(&1u16.to_le_bytes()).unwrap();
        file.write_all(&channels.to_le_bytes()).unwrap();
        file.write_all(&sample_rate.to_le_bytes()).unwrap();
        file.write_all(&byte_rate.to_le_bytes()).unwrap();
        file.write_all(&block_align.to_le_bytes()).unwrap();
        file.write_all(&bits_per_sample.to_le_bytes()).unwrap();
        file.write_all(b"data").unwrap();
        file.write_all(&(data.len() as u32).to_le_bytes()).unwrap();
        file.write_all(&data).unwrap();
    }

    fn move_candidate(
        id: i64,
        path: &Path,
        file_size: i64,
        content_hash: Option<String>,
    ) -> LocalFile {
        LocalFile {
            id,
            library_track_id: None,
            path: path.to_string_lossy().into_owned(),
            ownership: "external".into(),
            is_preferred: false,
            state: "present".into(),
            format: Some("flac".into()),
            file_size,
            modified_at: 1,
            duration_ms: Some(180_000),
            bitrate: Some(900),
            sample_rate: Some(44_100),
            channels: Some(2),
            content_hash,
            tag_title: Some("Song".into()),
            tag_artists: vec!["Artist".into()],
            tag_album: Some("Album".into()),
            tag_isrc: Some("USABC1234567".into()),
            artwork_path: None,
            artwork_mime: None,
            scan_error: None,
        }
    }

    fn move_scan(path: &Path, file_size: i64) -> LocalFileWrite {
        LocalFileWrite {
            path: path.to_string_lossy().into_owned(),
            state: "present".into(),
            format: Some("flac".into()),
            file_size,
            modified_at: 2,
            duration_ms: Some(180_000),
            bitrate: Some(900),
            sample_rate: Some(44_100),
            channels: Some(2),
            content_hash: None,
            tag_title: Some("Song".into()),
            tag_artists: vec!["Artist".into()],
            tag_album: Some("Album".into()),
            tag_isrc: Some("USABC1234567".into()),
            artwork_path: None,
            artwork_mime: None,
            scan_error: None,
        }
    }

    #[test]
    fn scan_indexes_supported_audio_and_ignores_unsupported_files() {
        let test = TestLibrary::new("basic");
        let database = test.open_database();
        write_test_wav(&test.root.join("song.wav"), 800);
        fs::write(test.root.join("broken.mp3"), b"not an mp3").unwrap();
        fs::write(test.root.join("notes.txt"), b"ignore me").unwrap();
        fs::create_dir_all(test.root.join(".hidden")).unwrap();
        write_test_wav(&test.root.join(".hidden/hidden.wav"), 400);

        let progress = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&progress);
        let summary = scan_library(&database, &test.root, move |event| {
            captured.lock().unwrap().push(event.phase);
        })
        .expect("scan should succeed");

        assert_eq!(summary.discovered, 2);
        assert_eq!(summary.added, 2);
        assert_eq!(summary.invalid, 1);
        let page = database.local_files_page(0, 100).expect("page should load");
        assert_eq!(page.total, 2);
        let wav = page
            .items
            .iter()
            .find(|file| file.path.ends_with("song.wav"))
            .expect("wav should be indexed");
        assert_eq!(wav.state, "present");
        assert_eq!(wav.format.as_deref(), Some("wav"));
        assert!(wav.duration_ms.is_some_and(|duration| duration > 0));
        let broken = page
            .items
            .iter()
            .find(|file| file.path.ends_with("broken.mp3"))
            .expect("broken supported file should remain visible");
        assert_eq!(broken.state, "invalid");
        assert!(broken.scan_error.is_some());
        let phases = progress.lock().unwrap();
        assert_eq!(phases.first().map(String::as_str), Some("starting"));
        assert!(phases.iter().any(|phase| phase == "scanning"));
        assert_eq!(phases.last().map(String::as_str), Some("complete"));
    }

    #[test]
    fn unchanged_files_reuse_the_existing_index_row() {
        let test = TestLibrary::new("unchanged");
        let database = test.open_database();
        write_test_wav(&test.root.join("song.wav"), 800);

        let first = scan_library(&database, &test.root, |_| {}).expect("first scan should work");
        let first_page = database.local_files_page(0, 10).unwrap();
        let first_id = first_page.items[0].id;
        let second = scan_library(&database, &test.root, |_| {}).expect("second scan should work");
        let second_page = database.local_files_page(0, 10).unwrap();

        assert_eq!(first.added, 1);
        assert_eq!(second.unchanged, 1);
        assert_eq!(second.updated, 0);
        assert_eq!(second_page.items[0].id, first_id);
    }

    #[test]
    fn changed_files_refresh_metadata_and_invalidate_stale_hashes() {
        let test = TestLibrary::new("changed");
        let database = test.open_database();
        let path = test.root.join("song.wav");
        write_test_wav(&path, 800);
        scan_library(&database, &test.root, |_| {}).unwrap();
        let original = database.local_files_page(0, 10).unwrap().items.remove(0);
        ensure_local_file_hash(&database, original.id).unwrap();

        write_test_wav(&path, 1_600);
        let summary = scan_library(&database, &test.root, |_| {}).unwrap();
        let changed = database.local_files_page(0, 10).unwrap().items.remove(0);

        assert_eq!(summary.updated, 1);
        assert_eq!(changed.id, original.id);
        assert!(changed.file_size > original.file_size);
        assert_eq!(changed.content_hash, None);
        assert_eq!(changed.state, "present");
    }

    #[test]
    fn removed_files_are_marked_missing() {
        let test = TestLibrary::new("removed");
        let database = test.open_database();
        let path = test.root.join("song.wav");
        write_test_wav(&path, 800);
        scan_library(&database, &test.root, |_| {}).unwrap();
        fs::remove_file(&path).unwrap();

        let summary = scan_library(&database, &test.root, |_| {}).unwrap();
        let page = database.local_files_page(0, 10).unwrap();

        assert_eq!(summary.missing, 1);
        assert_eq!(page.items[0].state, "missing");
    }

    #[test]
    fn switching_library_roots_marks_previous_root_files_missing() {
        let test = TestLibrary::new("root-switch");
        let database = test.open_database();
        let first = test.root.join("first.wav");
        write_test_wav(&first, 800);
        scan_library(&database, &test.root, |_| {}).unwrap();

        let second_root = test.root.parent().unwrap().join("other-music");
        fs::create_dir_all(&second_root).unwrap();
        let second = second_root.join("second.wav");
        write_test_wav(&second, 1_600);
        scan_library(&database, &second_root, |_| {}).unwrap();

        let page = database.local_files_page(0, 10).unwrap();
        let old = page
            .items
            .iter()
            .find(|file| Path::new(&file.path) == fs::canonicalize(&first).unwrap())
            .unwrap();
        let current = page
            .items
            .iter()
            .find(|file| Path::new(&file.path) == fs::canonicalize(&second).unwrap())
            .unwrap();
        assert_eq!(old.state, "missing");
        assert_eq!(current.state, "present");
        let overview = database.local_library_overview().unwrap();
        assert_eq!(overview.present, 1);
        assert_eq!(overview.missing, 1);
    }

    #[test]
    fn hash_mismatch_does_not_fall_back_to_metadata_move_matching() {
        let test = TestLibrary::new("hash-mismatch");
        fs::create_dir_all(&test.root).unwrap();
        let path = test.root.join("new-file.bin");
        fs::write(&path, b"new file contents").unwrap();
        let size = fs::metadata(&path).unwrap().len() as i64;
        let mut scanned = move_scan(&path, size);
        let candidate = move_candidate(
            1,
            &test.root.join("old-file.flac"),
            size,
            Some("definitely-not-the-new-file-hash".into()),
        );
        let candidates = HashMap::from([(candidate.id, candidate)]);

        let matched = find_moved_match(&path, &mut scanned, &candidates).unwrap();

        assert!(matched.is_none());
        assert!(scanned.content_hash.is_none());
    }

    #[test]
    fn ambiguous_duplicate_hashes_do_not_reuse_an_arbitrary_file_identity() {
        let test = TestLibrary::new("hash-ambiguity");
        fs::create_dir_all(&test.root).unwrap();
        let path = test.root.join("new-file.bin");
        fs::write(&path, b"same contents").unwrap();
        let size = fs::metadata(&path).unwrap().len() as i64;
        let hash = hash_file(&path).unwrap();
        let mut scanned = move_scan(&path, size);
        let first = move_candidate(1, &test.root.join("old-a.flac"), size, Some(hash.clone()));
        let second = move_candidate(2, &test.root.join("old-b.flac"), size, Some(hash));
        let candidates = HashMap::from([(first.id, first), (second.id, second)]);

        let matched = find_moved_match(&path, &mut scanned, &candidates).unwrap();

        assert!(matched.is_none());
        assert!(scanned.content_hash.is_none());
    }

    #[test]
    fn hashed_files_are_recovered_after_a_move_without_creating_duplicates() {
        let test = TestLibrary::new("move");
        let database = test.open_database();
        let original = test.root.join("old/song.wav");
        let moved = test.root.join("new/song.wav");
        write_test_wav(&original, 800);
        scan_library(&database, &test.root, |_| {}).unwrap();
        let original_row = database.local_files_page(0, 10).unwrap().items.remove(0);
        let hash = ensure_local_file_hash(&database, original_row.id).expect("hash should compute");
        assert!(!hash.is_empty());
        fs::create_dir_all(moved.parent().unwrap()).unwrap();
        fs::rename(&original, &moved).unwrap();

        let summary = scan_library(&database, &test.root, |_| {}).unwrap();
        let page = database.local_files_page(0, 10).unwrap();

        assert_eq!(summary.moved, 1);
        assert_eq!(summary.missing, 0);
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, original_row.id);
        assert_eq!(
            Path::new(&page.items[0].path),
            fs::canonicalize(&moved).unwrap().as_path()
        );
        assert_eq!(page.items[0].content_hash.as_deref(), Some(hash.as_str()));
    }

    #[test]
    fn identical_files_are_kept_as_independent_physical_files() {
        let test = TestLibrary::new("duplicates");
        let database = test.open_database();
        write_test_wav(&test.root.join("a.wav"), 800);
        fs::copy(test.root.join("a.wav"), test.root.join("b.wav")).unwrap();

        scan_library(&database, &test.root, |_| {}).unwrap();
        let page = database.local_files_page(0, 10).unwrap();

        assert_eq!(page.total, 2);
        assert_ne!(page.items[0].id, page.items[1].id);
        assert!(page.items.iter().all(|file| !file.is_preferred));
    }

    #[test]
    fn hashed_existing_file_is_not_mistaken_for_a_move_when_a_copy_is_added() {
        let test = TestLibrary::new("copy-not-move");
        let database = test.open_database();
        let original = test.root.join("z-original.wav");
        let copy = test.root.join("a-copy.wav");
        write_test_wav(&original, 800);
        scan_library(&database, &test.root, |_| {}).unwrap();
        let original_row = database.local_files_page(0, 10).unwrap().items.remove(0);
        ensure_local_file_hash(&database, original_row.id).unwrap();
        fs::copy(&original, &copy).unwrap();

        let summary = scan_library(&database, &test.root, |_| {}).unwrap();
        let page = database.local_files_page(0, 10).unwrap();

        assert_eq!(summary.moved, 0);
        assert_eq!(summary.added, 1);
        assert_eq!(page.total, 2);
        assert!(
            page.items
                .iter()
                .any(|file| Path::new(&file.path) == fs::canonicalize(&original).unwrap())
        );
        assert!(
            page.items
                .iter()
                .any(|file| Path::new(&file.path) == fs::canonicalize(&copy).unwrap())
        );
    }

    #[test]
    fn pagination_and_large_incremental_rescans_are_stable() {
        let test = TestLibrary::new("pagination");
        let database = test.open_database();
        for index in 0..80 {
            write_test_wav(&test.root.join(format!("track-{index:03}.wav")), 80);
        }

        let first = scan_library(&database, &test.root, |_| {}).unwrap();
        let second = scan_library(&database, &test.root, |_| {}).unwrap();
        let page = database.local_files_page(10, 15).unwrap();

        assert_eq!(first.added, 80);
        assert_eq!(second.unchanged, 80);
        assert_eq!(page.total, 80);
        assert_eq!(page.offset, 10);
        assert_eq!(page.limit, 15);
        assert_eq!(page.items.len(), 15);
    }

    #[cfg(unix)]
    #[test]
    fn directory_symlinks_are_not_followed() {
        use std::os::unix::fs::symlink;

        let test = TestLibrary::new("symlink");
        let database = test.open_database();
        let outside = test.root.parent().unwrap().join("outside");
        fs::create_dir_all(&outside).unwrap();
        write_test_wav(&outside.join("outside.wav"), 800);
        symlink(&outside, test.root.join("linked")).unwrap();

        let summary = scan_library(&database, &test.root, |_| {}).unwrap();
        assert_eq!(summary.discovered, 0);
        assert_eq!(database.local_files_page(0, 10).unwrap().total, 0);
    }
}
