use std::{
    collections::HashSet,
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::Duration,
};

use lofty::{
    config::WriteOptions,
    file::TaggedFileExt,
    picture::{Picture, PictureType},
    tag::{Accessor, ItemKey, ItemValue, Tag, TagExt, TagItem},
};
use reqwest::{blocking::Client, header::ACCEPT};
use serde::Serialize;

use crate::{
    db::{AcquisitionImportMetadata, Database, LocalFileWrite},
    domain::{
        AcquisitionCandidate, AcquisitionJob, MatchOutcome, MatchTrackDescriptor, TrackQuery,
    },
    local_library::{inspect_audio_file, system_time_ms},
    matching::MatcherIndex,
    normalization::{move_acquired_file_to_library, move_file},
};

const ARTWORK_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_ARTWORK_BYTES: u64 = 12 * 1024 * 1024;

#[derive(Debug)]
struct VerificationImportError {
    code: &'static str,
    message: String,
    terminal: bool,
}

impl VerificationImportError {
    fn import(message: impl Into<String>) -> Self {
        Self {
            code: "importFailed",
            message: message.into(),
            terminal: false,
        }
    }

    fn verification(message: impl Into<String>) -> Self {
        Self {
            code: "verificationFailed",
            message: message.into(),
            terminal: true,
        }
    }

    fn staging_missing(message: impl Into<String>) -> Self {
        Self {
            code: "stagingMissing",
            message: message.into(),
            terminal: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContinueStagingItemResult {
    pub job_id: i64,
    pub library_track_id: Option<i64>,
    pub imported: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContinueStagingSummary {
    pub imported: usize,
    pub failed: usize,
    pub results: Vec<ContinueStagingItemResult>,
}

fn verify_staged_metadata(
    query: &TrackQuery,
    candidate: &AcquisitionCandidate,
    scanned: &LocalFileWrite,
    manually_selected: bool,
) -> Result<(), String> {
    if scanned.state != "present" {
        return Err(scanned
            .scan_error
            .clone()
            .unwrap_or_else(|| "The staged audio file could not be read.".into()));
    }

    if let (Some(expected), Some(actual)) = (
        normalized_isrc(query.isrc.as_deref()),
        normalized_isrc(candidate.isrc.as_deref()),
    ) && expected != actual
        && !manually_selected
    {
        return Err(format!(
            "The selected provider candidate has ISRC {actual}, but {} expects {expected}.",
            query.title
        ));
    }

    let title = scanned
        .tag_title
        .clone()
        .or_else(|| candidate.title.clone())
        .unwrap_or_default();
    let mut artists = scanned.tag_artists.clone();
    if candidate.artists.len() > 1 && artists.len() == 1 && artists[0].contains(',') {
        artists = artists[0]
            .split(',')
            .map(str::trim)
            .filter(|artist| !artist.is_empty())
            .map(str::to_owned)
            .collect();
    }
    if artists.is_empty() {
        artists = candidate.artists.clone();
    }

    let staged = MatchTrackDescriptor {
        id: -1,
        title,
        artists,
        album: scanned
            .tag_album
            .clone()
            .or_else(|| candidate.album.clone()),
        isrc: scanned.tag_isrc.clone().or_else(|| candidate.isrc.clone()),
        duration_ms: scanned.duration_ms.or(candidate.duration_ms),
        disc_number: None,
        track_number: None,
        explicit: None,
        version_kind: None,
        version_detail: None,
    };
    let requested = MatchTrackDescriptor {
        id: query.library_track_id,
        title: query.title.clone(),
        artists: query.artists.clone(),
        album: query.album.clone(),
        isrc: query.isrc.clone(),
        duration_ms: query.duration_ms,
        disc_number: None,
        track_number: None,
        explicit: None,
        version_kind: query.version_kind.clone(),
        version_detail: query.version_detail.clone(),
    };

    let result = MatcherIndex::new(vec![requested]).match_track(&staged, None, &HashSet::new());
    if result.outcome == MatchOutcome::Automatic
        && result.selected_library_track_id == Some(query.library_track_id)
    {
        return Ok(());
    }

    // Provider streams can be valid audio while carrying no embedded tags. In that case the
    // staged descriptor falls back to the selected provider candidate. Do not require optional
    // album/track-disc evidence to reach the general matcher auto threshold when the recording's
    // core identity is exact and the actual decoded duration agrees.
    if result.outcome == MatchOutcome::Review
        && result.candidates.iter().any(|evidence| {
            evidence.library_track_id == query.library_track_id
                && evidence.incompatibilities.is_empty()
                && evidence.warnings.is_empty()
                && evidence.score.title == 4_000
                && evidence.score.artists == 2_500
                && evidence.score.duration == 2_000
        })
    {
        return Ok(());
    }

    // Provider search results can omit ISRCs, while the downloaded file can carry release-level
    // album/ISRC metadata from a different release of the same recording. Accept that metadata
    // drift when the provider did not contradict the requested ISRC, decoded title/artist/duration
    // are exact, and there is no evidence of a different recording version.
    let candidate_isrc = normalized_isrc(candidate.isrc.as_deref());
    let provider_isrc_does_not_conflict = candidate_isrc.is_none()
        || same_isrc(query.isrc.as_deref(), candidate.isrc.as_deref())
        || manually_selected;
    if provider_isrc_does_not_conflict
        && result.outcome == MatchOutcome::Review
        && result.candidates.iter().any(|evidence| {
            let only_embedded_isrc_conflicts =
                evidence.warnings.len() == 1 && evidence.warnings[0] == "conflictingIsrc";
            evidence.library_track_id == query.library_track_id
                && evidence.incompatibilities.is_empty()
                && only_embedded_isrc_conflicts
                && evidence.score.title == 4_000
                && evidence.score.artists == 2_500
                && evidence.score.duration == 2_000
        })
    {
        return Ok(());
    }

    Err(format!(
        "The downloaded recording does not match {} closely enough to import safely.",
        query.title
    ))
}

fn same_isrc(left: Option<&str>, right: Option<&str>) -> bool {
    match (normalized_isrc(left), normalized_isrc(right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn normalized_isrc(value: Option<&str>) -> Option<String> {
    value
        .map(|value| {
            value
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .flat_map(char::to_uppercase)
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
}

fn fetch_cover_artwork(image_url: &str) -> Result<Picture, String> {
    let url = url::Url::parse(image_url)
        .map_err(|error| format!("The source artwork URL is invalid: {error}"))?;
    if url.scheme() != "https" {
        return Err("The source artwork URL must use HTTPS.".into());
    }

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(ARTWORK_DOWNLOAD_TIMEOUT)
        .user_agent(concat!("Refrain/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("Could not initialize artwork download: {error}"))?;
    let response = client
        .get(url)
        .header(ACCEPT, "image/*")
        .send()
        .map_err(|error| format!("Could not download source artwork: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Source artwork download returned HTTP {}.",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_ARTWORK_BYTES)
    {
        return Err("Source artwork is larger than the 12 MiB import limit.".into());
    }

    let mut bytes = Vec::new();
    response
        .take(MAX_ARTWORK_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read source artwork: {error}"))?;
    if bytes.len() as u64 > MAX_ARTWORK_BYTES {
        return Err("Source artwork is larger than the 12 MiB import limit.".into());
    }

    let mut picture = Picture::from_reader(&mut Cursor::new(bytes))
        .map_err(|error| format!("Source artwork is not a supported image: {error}"))?;
    picture.set_pic_type(PictureType::CoverFront);
    Ok(picture)
}

fn apply_import_metadata_to_tag(
    tag: &mut Tag,
    metadata: &AcquisitionImportMetadata,
    cover: Option<Picture>,
) {
    tag.set_title(metadata.title.clone());

    tag.remove_key(ItemKey::TrackArtist);
    tag.remove_key(ItemKey::TrackArtists);
    if !metadata.artists.is_empty() {
        tag.set_artist(metadata.artists.join(", "));
        for artist in &metadata.artists {
            let _ = tag.push(TagItem::new(
                ItemKey::TrackArtists,
                ItemValue::Text(artist.clone()),
            ));
        }
    }

    if let Some(album) = metadata.album.as_deref().filter(|value| !value.is_empty()) {
        tag.set_album(album.to_owned());
    }
    if let Some(isrc) = metadata.isrc.as_deref().filter(|value| !value.is_empty()) {
        let _ = tag.insert_text(ItemKey::Isrc, isrc.to_owned());
    }
    if let Some(track_number) = metadata
        .track_number
        .and_then(|value| u32::try_from(value).ok())
    {
        tag.set_track(track_number);
    }
    if let Some(disc_number) = metadata
        .disc_number
        .and_then(|value| u32::try_from(value).ok())
    {
        tag.set_disk(disc_number);
    }
    if let Some(release_year) = metadata.release_year {
        let _ = tag.insert_text(ItemKey::RecordingDate, release_year.to_string());
    }
    if let Some(cover) = cover {
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(cover);
    }
}

fn write_import_metadata(
    source: &Path,
    metadata: &AcquisitionImportMetadata,
    cover: Option<Picture>,
) -> Result<(), String> {
    let tagged_file = lofty::read_from_path(source)
        .map_err(|error| format!("Could not read downloaded audio before tagging: {error}"))?;
    let mut tag = tagged_file
        .primary_tag()
        .cloned()
        .unwrap_or_else(|| Tag::new(tagged_file.primary_tag_type()));
    apply_import_metadata_to_tag(&mut tag, metadata, cover);
    tag.save_to_path(source, WriteOptions::new())
        .map_err(|error| format!("Could not write downloaded audio metadata: {error}"))
}

pub(crate) fn import_verified_file(
    database: &Database,
    library_root: &Path,
    library_track_id: i64,
    source: &Path,
    mut metadata: LocalFileWrite,
) -> Result<PathBuf, String> {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "The staged audio file has no usable extension.".to_owned())?;
    let target =
        move_acquired_file_to_library(database, library_root, library_track_id, source, extension)
            .map_err(|error| error.to_string())?;

    metadata.path = target.to_string_lossy().into_owned();
    if let Ok(file_metadata) = fs::metadata(&target) {
        metadata.file_size = i64::try_from(file_metadata.len()).unwrap_or(i64::MAX);
        metadata.modified_at = file_metadata
            .modified()
            .map(system_time_ms)
            .unwrap_or(metadata.modified_at);
    }

    if let Err(error) = database.insert_managed_local_file_for_track(library_track_id, &metadata) {
        let rollback = move_file(&target, source);
        return Err(match rollback {
            Ok(()) => format!("Could not record the imported file: {error}"),
            Err(rollback_error) => format!(
                "Could not record the imported file: {error}. The file also could not be restored to staging: {rollback_error}"
            ),
        });
    }

    Ok(target)
}

fn is_supported_audio_file(path: &Path) -> bool {
    path.is_file()
        && matches!(
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("flac" | "mp3" | "m4a" | "aac" | "ogg" | "opus" | "wav" | "alac")
        )
}

fn staged_audio_file(staging_path: &Path) -> Result<PathBuf, VerificationImportError> {
    if !staging_path.is_dir() {
        return Err(VerificationImportError::staging_missing(format!(
            "The downloaded staging directory {} is no longer available.",
            staging_path.display()
        )));
    }
    let entries = fs::read_dir(staging_path).map_err(|error| {
        VerificationImportError::staging_missing(format!(
            "Could not inspect the staging directory {}: {error}",
            staging_path.display()
        ))
    })?;
    let mut audio_files = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_supported_audio_file(path))
        .collect::<Vec<_>>();
    audio_files.sort();
    match audio_files.len() {
        1 => Ok(audio_files.remove(0)),
        0 => Err(VerificationImportError::staging_missing(
            "The downloaded staging audio is no longer available.",
        )),
        count => Err(VerificationImportError::import(format!(
            "Found {count} completed audio files in staging; Refrain cannot safely choose one automatically."
        ))),
    }
}

pub(crate) fn downloaded_staging_artifact_exists(job: &AcquisitionJob) -> bool {
    let Some(staging_path) = job.staging_path.as_deref() else {
        return false;
    };
    let Ok(entries) = fs::read_dir(staging_path) else {
        return false;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .any(|path| is_supported_audio_file(&path))
}

fn verify_and_import_job(
    database: &Database,
    library_root: &Path,
    job_id: i64,
) -> Result<(i64, PathBuf), VerificationImportError> {
    let job = database
        .acquisition_job(job_id)
        .map_err(|error| VerificationImportError::import(error.to_string()))?
        .ok_or_else(|| {
            VerificationImportError::import(format!("Staging job {job_id} was not found."))
        })?;
    if job.status != "staged" || job.stage.as_deref() != Some("downloaded") {
        return Err(VerificationImportError::import(
            "Only downloaded staging items can continue to verification and import.",
        ));
    }
    let candidate = job.candidate.as_ref().ok_or_else(|| {
        VerificationImportError::import("The downloaded staging item has no selected candidate.")
    })?;
    let manually_selected = job.candidates.iter().any(|offered| {
        offered.provider_token == candidate.provider_token && offered.provider == candidate.provider
    });
    let query = database
        .acquisition_track_query(job.library_track_id)
        .map_err(|error| VerificationImportError::import(error.to_string()))?
        .ok_or_else(|| {
            VerificationImportError::import("The requested library track no longer exists.")
        })?;
    let staging_path = job
        .staging_path
        .as_deref()
        .map(Path::new)
        .ok_or_else(|| VerificationImportError::import("The staging directory is missing."))?;
    let source = staged_audio_file(staging_path)?;
    let file_metadata = fs::metadata(&source).map_err(|error| {
        VerificationImportError::import(format!(
            "Could not inspect staged audio {}: {error}",
            source.display()
        ))
    })?;
    let artwork_cache_dir = database
        .path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("artwork");
    fs::create_dir_all(&artwork_cache_dir).map_err(|error| {
        VerificationImportError::import(format!("Could not prepare artwork cache: {error}"))
    })?;
    let scanned = inspect_audio_file(
        &source,
        i64::try_from(file_metadata.len()).unwrap_or(i64::MAX),
        file_metadata
            .modified()
            .map(system_time_ms)
            .unwrap_or_default(),
        &artwork_cache_dir,
    );
    verify_staged_metadata(&query, candidate, &scanned, manually_selected)
        .map_err(VerificationImportError::verification)?;

    let import_metadata = database
        .acquisition_import_metadata(job.library_track_id)
        .map_err(|error| VerificationImportError::import(error.to_string()))?
        .ok_or_else(|| {
            VerificationImportError::import("The requested library track metadata is missing.")
        })?;
    let cover = if scanned.artwork_path.is_some() {
        None
    } else {
        import_metadata
            .image_url
            .as_deref()
            .map(fetch_cover_artwork)
            .transpose()
            .map_err(VerificationImportError::import)?
    };
    write_import_metadata(&source, &import_metadata, cover)
        .map_err(VerificationImportError::import)?;

    let tagged_file_metadata = fs::metadata(&source).map_err(|error| {
        VerificationImportError::import(format!(
            "Could not inspect tagged audio {}: {error}",
            source.display()
        ))
    })?;
    let scanned = inspect_audio_file(
        &source,
        i64::try_from(tagged_file_metadata.len()).unwrap_or(i64::MAX),
        tagged_file_metadata
            .modified()
            .map(system_time_ms)
            .unwrap_or_default(),
        &artwork_cache_dir,
    );
    if scanned.state != "present" {
        return Err(VerificationImportError::import(
            scanned
                .scan_error
                .unwrap_or_else(|| "The tagged audio file could not be read back.".into()),
        ));
    }
    let target = import_verified_file(
        database,
        library_root,
        job.library_track_id,
        &source,
        scanned,
    )
    .map_err(VerificationImportError::import)?;
    let _ = fs::remove_dir_all(staging_path);
    Ok((job.library_track_id, target))
}

pub(crate) fn continue_downloaded_jobs(
    database: &Database,
    library_root: &Path,
    job_ids: &[i64],
) -> ContinueStagingSummary {
    let mut summary = ContinueStagingSummary::default();
    for &job_id in job_ids {
        let library_track_id = database
            .acquisition_job(job_id)
            .ok()
            .flatten()
            .map(|job| job.library_track_id);
        match verify_and_import_job(database, library_root, job_id) {
            Ok((library_track_id, _target)) => {
                let _ = database.set_acquisition_recovery_error(job_id, None, None);
                summary.imported += 1;
                summary.results.push(ContinueStagingItemResult {
                    job_id,
                    library_track_id: Some(library_track_id),
                    imported: true,
                    error_code: None,
                    error_message: None,
                });
            }
            Err(error) => {
                if error.terminal {
                    let _ = database.finish_acquisition_job(
                        job_id,
                        "failed",
                        Some(error.code),
                        Some(&error.message),
                    );
                } else {
                    let _ = database.set_acquisition_recovery_error(
                        job_id,
                        Some(error.code),
                        Some(&error.message),
                    );
                }
                summary.failed += 1;
                summary.results.push(ContinueStagingItemResult {
                    job_id,
                    library_track_id,
                    imported: false,
                    error_code: Some(error.code.to_owned()),
                    error_message: Some(error.message),
                });
            }
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::Database,
        db::LocalFileWrite,
        domain::{
            AcquisitionCandidate, CollectionEntry, SourceAccount, SourceCollection, SourceTrack,
            TrackQuery,
        },
    };
    use lofty::picture::MimeType;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "refrain-verification-{name}-{}-{id}",
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

    fn query() -> TrackQuery {
        TrackQuery {
            library_track_id: 42,
            title: "Lagi Na Lang".into(),
            artists: vec!["Sugarcane".into()],
            album: Some("Memory".into()),
            duration_ms: Some(249_725),
            isrc: None,
            version_kind: None,
            version_detail: None,
        }
    }

    fn candidate() -> AcquisitionCandidate {
        AcquisitionCandidate {
            provider: None,
            provider_token: "candidate".into(),
            source: None,
            file_name: Some("Lagi Na Lang.flac".into()),
            title: Some("Lagi Na Lang".into()),
            artists: vec!["Sugarcane".into()],
            album: None,
            duration_ms: Some(249_725),
            format: Some("FLAC".into()),
            size_bytes: None,
            bitrate_kbps: None,
            sample_rate_hz: None,
            bit_depth: None,
            confidence: None,
            isrc: Some("PHW012400262".into()),
            recording_id: Some("212456142083723264".into()),
            release_id: Some("169623492306694144".into()),
        }
    }

    fn scanned(title: &str) -> LocalFileWrite {
        LocalFileWrite {
            path: "/tmp/Lagi Na Lang.flac".into(),
            state: "present".into(),
            format: Some("flac".into()),
            file_size: 31_149_348,
            modified_at: 1,
            duration_ms: Some(249_725),
            bitrate: Some(900),
            sample_rate: Some(44_100),
            channels: Some(2),
            content_hash: None,
            tag_title: Some(title.into()),
            tag_artists: vec!["Sugarcane".into()],
            tag_album: Some("Memory".into()),
            tag_year: Some(2024),
            tag_isrc: Some("PHW012400262".into()),
            artwork_path: None,
            artwork_mime: None,
            scan_error: None,
        }
    }

    fn scanned_without_tags(duration_ms: i64) -> LocalFileWrite {
        LocalFileWrite {
            path: "/tmp/untagged.flac".into(),
            state: "present".into(),
            format: Some("flac".into()),
            file_size: 31_149_348,
            modified_at: 1,
            duration_ms: Some(duration_ms),
            bitrate: Some(900),
            sample_rate: Some(44_100),
            channels: Some(2),
            content_hash: None,
            tag_title: None,
            tag_artists: Vec::new(),
            tag_album: None,
            tag_year: None,
            tag_isrc: None,
            artwork_path: None,
            artwork_mime: None,
            scan_error: None,
        }
    }

    fn seed_tracked_track_with_isrc(
        database: &Database,
        id: &str,
        title: &str,
        isrc: Option<&str>,
    ) -> i64 {
        let account_id = database
            .upsert_source_account(&SourceAccount {
                provider: "spotify".into(),
                provider_account_id: "listener".into(),
                display_name: Some("Listener".into()),
                image_url: None,
                client_id: "client".into(),
            })
            .unwrap();
        let collection_id = database
            .upsert_source_collection(&SourceCollection {
                source_account_id: account_id,
                provider_collection_id: format!("playlist-{id}"),
                kind: "playlist".into(),
                name: format!("Playlist {id}"),
                snapshot_id: None,
                owner_provider_id: None,
                is_accessible: true,
                access_issue: None,
                image_url: None,
                external_url: None,
                album_metadata: None,
            })
            .unwrap();
        let source_track_id = database
            .upsert_source_track(&SourceTrack {
                provider: "spotify".into(),
                provider_track_id: id.into(),
                uri: None,
                isrc: isrc.map(str::to_owned),
                title: title.into(),
                normalized_title: title.to_ascii_lowercase(),
                artists_json: "[\"Artist\"]".into(),
                normalized_artists: "artist".into(),
                album: Some("Album".into()),
                normalized_album: Some("album".into()),
                duration_ms: Some(1_000),
                disc_number: Some(1),
                track_number: Some(1),
                release_year: Some(2026),
                explicit: Some(false),
                version_kind: None,
                version_detail: None,
                image_url: None,
                external_url: None,
            })
            .unwrap();
        let library_track_id = database
            .create_library_track_from_source(source_track_id)
            .unwrap();
        database
            .persist_track_link(source_track_id, library_track_id, "existing", 10_000)
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
        database
            .set_source_collection_tracking(collection_id, true)
            .unwrap();
        library_track_id
    }

    fn seed_tracked_track(database: &Database, id: &str, title: &str) -> i64 {
        seed_tracked_track_with_isrc(database, id, title, None)
    }

    fn stage_downloaded_job(
        database: &Database,
        root: &Path,
        library_track_id: i64,
        candidate: AcquisitionCandidate,
    ) -> i64 {
        let job = database
            .queue_acquisition_job(library_track_id, "monochrome")
            .unwrap()
            .unwrap();
        let staging_dir = root.join(job.id.to_string());
        fs::create_dir_all(&staging_dir).unwrap();
        database
            .begin_acquisition_attempt(job.id, 1, &candidate, &staging_dir.to_string_lossy())
            .unwrap();
        database
            .finish_acquisition_job(job.id, "staged", None, None)
            .unwrap();
        write_silence_wav(&staging_dir.join("download.wav"));
        job.id
    }

    fn write_silence_wav(path: &Path) {
        let sample_rate = 8_000_u32;
        let samples = sample_rate;
        let data_size = samples * 2;
        let mut bytes = Vec::with_capacity((44 + data_size) as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes.resize((44 + data_size) as usize, 0);
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn verification_accepts_matching_metadata_and_rejects_wrong_recording() {
        assert!(
            verify_staged_metadata(&query(), &candidate(), &scanned("Lagi Na Lang"), false).is_ok()
        );

        let error = verify_staged_metadata(
            &query(),
            &candidate(),
            &scanned("Completely Different Song"),
            false,
        )
        .unwrap_err();
        assert!(error.contains("does not match"));
    }

    #[test]
    fn verification_accepts_untagged_audio_when_candidate_core_identity_matches_exactly() {
        assert!(
            verify_staged_metadata(
                &query(),
                &candidate(),
                &scanned_without_tags(249_725),
                false,
            )
            .is_ok()
        );
    }

    #[test]
    fn verification_accepts_embedded_isrc_conflict_when_selected_candidate_is_exact_isrc_match() {
        let mut query = query();
        query.isrc = Some("PHW012400262".into());
        let candidate = candidate();
        let mut scanned = scanned("Lagi Na Lang");
        scanned.tag_isrc = Some("QZY992300064".into());

        assert!(verify_staged_metadata(&query, &candidate, &scanned, false).is_ok());
    }

    #[test]
    fn verification_accepts_release_metadata_drift_when_candidate_has_no_isrc() {
        let mut query = query();
        query.isrc = Some("QZFYZ1908593".into());
        query.title = "Loverboy".into();
        query.artists = vec!["A-Wall".into()];
        query.album = Some("Loverboy".into());
        query.duration_ms = Some(224_520);

        let mut candidate = candidate();
        candidate.title = Some("Loverboy".into());
        candidate.artists = vec!["A-Wall".into()];
        candidate.album = None;
        candidate.duration_ms = Some(224_520);
        candidate.isrc = None;

        let mut scanned = scanned("Loverboy");
        scanned.tag_artists = vec!["A-Wall".into()];
        scanned.tag_album = Some("Helios".into());
        scanned.tag_isrc = Some("QZFZ41979172".into());
        scanned.duration_ms = Some(224_520);

        assert!(verify_staged_metadata(&query, &candidate, &scanned, false).is_ok());
    }

    #[test]
    fn verification_does_not_ignore_version_conflicts_when_candidate_has_no_isrc() {
        let mut query = query();
        query.isrc = Some("QZFYZ1908593".into());
        query.title = "Loverboy".into();
        query.artists = vec!["A-Wall".into()];
        query.album = Some("Loverboy".into());
        query.duration_ms = Some(224_520);

        let mut candidate = candidate();
        candidate.title = Some("Loverboy".into());
        candidate.artists = vec!["A-Wall".into()];
        candidate.album = None;
        candidate.duration_ms = Some(224_520);
        candidate.isrc = None;

        let mut scanned = scanned("Loverboy - Live");
        scanned.tag_artists = vec!["A-Wall".into()];
        scanned.tag_album = Some("Helios".into());
        scanned.tag_isrc = Some("QZFZ41979172".into());
        scanned.duration_ms = Some(224_520);

        assert!(verify_staged_metadata(&query, &candidate, &scanned, false).is_err());
    }

    #[test]
    fn verification_rejects_embedded_isrc_conflict_when_candidate_isrc_is_not_requested_isrc() {
        let mut query = query();
        query.isrc = Some("PHW012400262".into());
        let mut candidate = candidate();
        candidate.isrc = Some("USAT20611041".into());
        let mut scanned = scanned("Lagi Na Lang");
        scanned.tag_isrc = Some("USAT20611041".into());

        let error = verify_staged_metadata(&query, &candidate, &scanned, false).unwrap_err();

        assert!(error.contains("USAT20611041"));
        assert!(error.contains("PHW012400262"));
    }

    #[test]
    fn manual_resolution_accepts_alternate_release_isrc_when_core_identity_matches() {
        let temp = TestDir::new("manual-alternate-isrc");
        let database = Database::open(temp.0.join("refrain.sqlite3")).unwrap();
        let library_track_id =
            seed_tracked_track_with_isrc(&database, "loverboy", "Loverboy", Some("QZFYZ1908593"));
        let mut selected = candidate();
        selected.provider = Some("antra".into());
        selected.provider_token = "antra-loverboy".into();
        selected.title = Some("Loverboy".into());
        selected.artists = vec!["Artist".into()];
        selected.album = Some("Album".into());
        selected.duration_ms = Some(1_000);
        selected.confidence = Some(40);
        selected.isrc = Some("QZFZ41979172".into());

        let job = database
            .queue_acquisition_job(library_track_id, "antra")
            .unwrap()
            .unwrap();
        database
            .require_acquisition_resolution(job.id, &[selected.clone()])
            .unwrap();
        let staging_dir = temp.0.join("staging");
        fs::create_dir_all(&staging_dir).unwrap();
        database
            .begin_acquisition_attempt(job.id, 1, &selected, &staging_dir.to_string_lossy())
            .unwrap();
        database
            .finish_acquisition_job(job.id, "staged", None, None)
            .unwrap();
        write_silence_wav(&staging_dir.join("download.wav"));
        let library_root = temp.0.join("Library");
        fs::create_dir_all(&library_root).unwrap();

        let result = verify_and_import_job(&database, &library_root, job.id);

        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn import_metadata_populates_tags_and_embeds_front_cover() {
        let metadata = AcquisitionImportMetadata {
            title: "Lagi Na Lang".into(),
            artists: vec!["Sugarcane".into(), "The Ridleys".into()],
            album: Some("Memory".into()),
            isrc: Some("PHW012400262".into()),
            disc_number: Some(1),
            track_number: Some(2),
            release_year: Some(2024),
            image_url: Some("https://i.scdn.co/image/test".into()),
        };
        let cover = Picture::unchecked(vec![1, 2, 3, 4])
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Jpeg)
            .build();
        let mut tag = Tag::new(lofty::tag::TagType::VorbisComments);

        apply_import_metadata_to_tag(&mut tag, &metadata, Some(cover));

        assert_eq!(tag.title().as_deref(), Some("Lagi Na Lang"));
        assert_eq!(tag.album().as_deref(), Some("Memory"));
        assert_eq!(tag.track(), Some(2));
        assert_eq!(tag.disk(), Some(1));
        assert_eq!(tag.get_string(ItemKey::Isrc), Some("PHW012400262"));
        assert_eq!(
            tag.get_strings(ItemKey::TrackArtists).collect::<Vec<_>>(),
            vec!["Sugarcane", "The Ridleys"]
        );
        assert_eq!(tag.picture_count(), 1);
        assert!(tag.get_picture_type(PictureType::CoverFront).is_some());
    }

    #[test]
    fn import_metadata_uses_linked_spotify_artwork() {
        let temp = TestDir::new("import-metadata-artwork");
        let database = Database::open(temp.0.join("refrain.sqlite3")).unwrap();
        let source_track_id = database
            .upsert_source_track(&SourceTrack {
                provider: "spotify".into(),
                provider_track_id: "track-art".into(),
                uri: None,
                isrc: Some("PHW012400262".into()),
                title: "Lagi Na Lang".into(),
                normalized_title: "lagi na lang".into(),
                artists_json: "[\"Sugarcane\"]".into(),
                normalized_artists: "sugarcane".into(),
                album: Some("Memory".into()),
                normalized_album: Some("memory".into()),
                duration_ms: Some(249_725),
                disc_number: Some(1),
                track_number: Some(2),
                release_year: Some(2024),
                explicit: Some(false),
                version_kind: None,
                version_detail: None,
                image_url: Some("https://i.scdn.co/image/cover".into()),
                external_url: None,
            })
            .unwrap();
        let library_track_id = database
            .create_library_track_from_source(source_track_id)
            .unwrap();
        database
            .persist_track_link(source_track_id, library_track_id, "existing", 10_000)
            .unwrap();

        let metadata = database
            .acquisition_import_metadata(library_track_id)
            .unwrap()
            .unwrap();

        assert_eq!(metadata.title, "Lagi Na Lang");
        assert_eq!(metadata.artists, vec!["Sugarcane"]);
        assert_eq!(metadata.album.as_deref(), Some("Memory"));
        assert_eq!(metadata.track_number, Some(2));
        assert_eq!(metadata.disc_number, Some(1));
        assert_eq!(metadata.release_year, Some(2024));
        assert_eq!(
            metadata.image_url.as_deref(),
            Some("https://i.scdn.co/image/cover")
        );
    }

    #[test]
    fn verified_file_import_moves_and_links_only_that_library_track() {
        let temp = TestDir::new("import");
        let database = Database::open(temp.0.join("refrain.sqlite3")).unwrap();
        let source_track_id = database
            .upsert_source_track(&SourceTrack {
                provider: "spotify".into(),
                provider_track_id: "track-1".into(),
                uri: None,
                isrc: None,
                title: "Lagi Na Lang".into(),
                normalized_title: "lagi na lang".into(),
                artists_json: "[\"Sugarcane\"]".into(),
                normalized_artists: "sugarcane".into(),
                album: Some("Memory".into()),
                normalized_album: Some("memory".into()),
                duration_ms: Some(249_725),
                disc_number: Some(1),
                track_number: Some(2),
                release_year: Some(2024),
                explicit: Some(false),
                version_kind: None,
                version_detail: None,
                image_url: None,
                external_url: None,
            })
            .unwrap();
        let library_track_id = database
            .create_library_track_from_source(source_track_id)
            .unwrap();
        database
            .persist_track_link(source_track_id, library_track_id, "existing", 10_000)
            .unwrap();

        let library_root = temp.0.join("Library");
        let staging_dir = temp.0.join("staging");
        fs::create_dir_all(&library_root).unwrap();
        fs::create_dir_all(&staging_dir).unwrap();
        let source = staging_dir.join("download.flac");
        fs::write(&source, b"verified fixture bytes").unwrap();
        let mut metadata = scanned("Lagi Na Lang");
        metadata.path = source.to_string_lossy().into_owned();
        metadata.file_size = fs::metadata(&source).unwrap().len() as i64;

        let target = import_verified_file(
            &database,
            &library_root,
            library_track_id,
            &source,
            metadata,
        )
        .unwrap();

        assert!(target.exists());
        assert!(!source.exists());
        let files = database
            .local_files_for_library_track(library_track_id)
            .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].ownership, "managed");
        assert!(files[0].is_preferred);
        assert_eq!(files[0].path, target.to_string_lossy());
    }

    #[test]
    fn selected_job_processing_imports_valid_tracks_when_another_fails_verification() {
        let temp = TestDir::new("partial-success");
        let database = Database::open(temp.0.join("refrain.sqlite3")).unwrap();
        let library_root = temp.0.join("Library");
        let staging_root = temp.0.join("staging");
        fs::create_dir_all(&library_root).unwrap();
        fs::create_dir_all(&staging_root).unwrap();

        let valid_track_id = seed_tracked_track(&database, "valid", "Valid Song");
        let invalid_track_id = seed_tracked_track(&database, "invalid", "Expected Song");
        let valid_job_id = stage_downloaded_job(
            &database,
            &staging_root,
            valid_track_id,
            AcquisitionCandidate {
                provider: None,
                provider_token: "valid-candidate".into(),
                source: None,
                file_name: Some("Valid Song.wav".into()),
                title: Some("Valid Song".into()),
                artists: vec!["Artist".into()],
                album: Some("Album".into()),
                duration_ms: Some(1_000),
                format: Some("WAV".into()),
                size_bytes: None,
                bitrate_kbps: None,
                sample_rate_hz: None,
                bit_depth: None,
                confidence: None,
                isrc: None,
                recording_id: None,
                release_id: None,
            },
        );
        let invalid_job_id = stage_downloaded_job(
            &database,
            &staging_root,
            invalid_track_id,
            AcquisitionCandidate {
                provider: None,
                provider_token: "wrong-candidate".into(),
                source: None,
                file_name: Some("Wrong Song.wav".into()),
                title: Some("Wrong Song".into()),
                artists: vec!["Different Artist".into()],
                album: Some("Different Album".into()),
                duration_ms: Some(1_000),
                format: Some("WAV".into()),
                size_bytes: None,
                bitrate_kbps: None,
                sample_rate_hz: None,
                bit_depth: None,
                confidence: None,
                isrc: None,
                recording_id: None,
                release_id: None,
            },
        );

        let summary =
            continue_downloaded_jobs(&database, &library_root, &[valid_job_id, invalid_job_id]);

        assert_eq!(summary.imported, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.results.len(), 2);
        let valid_files = database
            .local_files_for_library_track(valid_track_id)
            .unwrap();
        assert_eq!(valid_files.len(), 1);
        assert_eq!(valid_files[0].ownership, "managed");
        assert!(valid_files[0].is_preferred);
        assert!(
            database
                .local_files_for_library_track(invalid_track_id)
                .unwrap()
                .is_empty()
        );
        let invalid_job = database.acquisition_job(invalid_job_id).unwrap().unwrap();
        assert_eq!(invalid_job.stage.as_deref(), Some("failed"));
        assert_eq!(
            invalid_job.error_code.as_deref(),
            Some("verificationFailed")
        );
        assert!(
            staging_root
                .join(invalid_job_id.to_string())
                .join("download.wav")
                .exists()
        );
    }

    #[test]
    fn missing_downloaded_artifact_is_marked_for_reacquisition() {
        let temp = TestDir::new("missing-staged-artifact");
        let database = Database::open(temp.0.join("refrain.sqlite3")).unwrap();
        let library_root = temp.0.join("Library");
        let staging_root = temp.0.join("staging");
        fs::create_dir_all(&library_root).unwrap();
        fs::create_dir_all(&staging_root).unwrap();

        let library_track_id = seed_tracked_track(&database, "missing", "Missing Song");
        let job_id = stage_downloaded_job(
            &database,
            &staging_root,
            library_track_id,
            AcquisitionCandidate {
                provider: None,
                provider_token: "missing-candidate".into(),
                source: None,
                file_name: Some("Missing Song.wav".into()),
                title: Some("Missing Song".into()),
                artists: vec!["Artist".into()],
                album: Some("Album".into()),
                duration_ms: Some(1_000),
                format: Some("WAV".into()),
                size_bytes: None,
                bitrate_kbps: None,
                sample_rate_hz: None,
                bit_depth: None,
                confidence: None,
                isrc: None,
                recording_id: None,
                release_id: None,
            },
        );
        let job = database.acquisition_job(job_id).unwrap().unwrap();
        fs::remove_dir_all(job.staging_path.as_deref().unwrap()).unwrap();

        assert_eq!(
            database
                .downloaded_acquisition_jobs_missing_local()
                .unwrap()
                .iter()
                .map(|job| job.id)
                .collect::<Vec<_>>(),
            vec![job_id]
        );
        assert!(!downloaded_staging_artifact_exists(&job));

        let summary = continue_downloaded_jobs(&database, &library_root, &[job_id]);

        assert_eq!(summary.imported, 0);
        assert_eq!(summary.failed, 1);
        assert_eq!(
            summary.results[0].error_code.as_deref(),
            Some("stagingMissing")
        );
        let failed_job = database.acquisition_job(job_id).unwrap().unwrap();
        assert_eq!(failed_job.status, "failed");
        assert_eq!(failed_job.stage.as_deref(), Some("failed"));
        assert_eq!(failed_job.error_code.as_deref(), Some("stagingMissing"));
    }
}
