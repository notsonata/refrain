use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose};
use reqwest::{
    StatusCode,
    blocking::Client,
    header::{ACCEPT, CONTENT_RANGE, CONTENT_TYPE, RANGE},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    acquisition::{AcquisitionProvider, ProviderError},
    domain::{
        AcquisitionCandidate, AcquisitionRequest, ProviderHealth, ProviderJob, ProviderJobStatus,
        TrackQuery,
    },
};

const MONOCHROME_PROVIDER_ID: &str = "monochrome";
const INSTANCE_OVERRIDE_ENV: &str = "REFRAIN_MONOCHROME_API_URLS";
const TRACKS_BASE_URL: &str = "https://tracks.monochrome.st";
const API_TIMEOUT: Duration = Duration::from_secs(12);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const QUALITY_PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const QUALITY_PROBE_CONCURRENCY: usize = 4;
const MAX_DOWNLOAD_STREAM_ATTEMPTS: usize = 6;
const MAX_SEARCH_RESULTS: usize = 12;
const PLAYBACK_OUTAGE_COOLDOWN: Duration = Duration::from_secs(60);
const DEFAULT_API_URLS: &[&str] = &[
    "https://api.monochrome.tf",
    "https://monochrome-api.samidy.com",
    "https://hifi.geeked.wtf",
    "https://wolf.qqdl.site",
    "https://maus.qqdl.site",
    "https://vogel.qqdl.site",
    "https://katze.qqdl.site",
    "https://hund.qqdl.site",
    "https://tidal.kinoplus.online",
];

#[derive(Clone)]
pub struct MonochromeProvider {
    inner: Arc<MonochromeInner>,
}

struct MonochromeInner {
    http: Client,
    tracks_base_url: String,
    base_urls: Vec<String>,
    jobs: Mutex<HashMap<String, Arc<MonochromeJob>>>,
    playback_degraded_until: Mutex<Option<Instant>>,
    next_job_id: AtomicU64,
}

struct MonochromeJob {
    status: Mutex<ProviderJobStatus>,
    cancelled: AtomicBool,
    bytes_transferred: AtomicI64,
    total_bytes: AtomicI64,
}

impl MonochromeJob {
    fn new() -> Self {
        Self {
            status: Mutex::new(ProviderJobStatus::Pending),
            cancelled: AtomicBool::new(false),
            bytes_transferred: AtomicI64::new(0),
            total_bytes: AtomicI64::new(0),
        }
    }

    fn set_status(&self, status: ProviderJobStatus) {
        if let Ok(mut current) = self.status.lock() {
            *current = status;
        }
    }

    fn status(&self) -> Result<ProviderJobStatus, ProviderError> {
        self.status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| {
                ProviderError::new(
                    "providerStateUnavailable",
                    "Monochrome download state is unavailable.",
                    true,
                )
            })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchData {
    #[serde(default)]
    items: Vec<MonochromeTrack>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TracksSearchData {
    #[serde(default)]
    tracks: Vec<TracksSearchTrack>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TracksSearchTrack {
    id: Option<Value>,
    track_id: Option<Value>,
    recording_id: Option<Value>,
    release_id: Option<Value>,
    title: String,
    duration: Option<i64>,
    playable: Option<bool>,
    isrc: Option<String>,
    #[serde(default)]
    artist_names: Vec<String>,
    #[serde(default)]
    artists: Vec<TracksSearchArtist>,
    album_title: Option<String>,
    release_title: Option<String>,
    #[serde(
        default,
        alias = "sampleRate",
        alias = "samplingRate",
        alias = "samplingRateHz"
    )]
    sample_rate_hz: Option<Value>,
    #[serde(default, alias = "bitsPerSample")]
    bit_depth: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TracksSearchArtist {
    name: Option<String>,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MonochromeTrack {
    id: Value,
    title: String,
    duration: Option<i64>,
    stream_ready: Option<bool>,
    audio_quality: Option<String>,
    #[serde(
        default,
        alias = "sampleRate",
        alias = "samplingRate",
        alias = "samplingRateHz"
    )]
    sample_rate_hz: Option<Value>,
    #[serde(default, alias = "bitsPerSample")]
    bit_depth: Option<Value>,
    isrc: Option<String>,
    artist: Option<MonochromeArtist>,
    #[serde(default)]
    artists: Vec<MonochromeArtist>,
    album: Option<MonochromeAlbum>,
}

#[derive(Debug, Deserialize, Clone)]
struct MonochromeArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct MonochromeAlbum {
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackData {
    manifest: String,
    manifest_mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackManifest {
    #[serde(default)]
    urls: Vec<String>,
    mime_type: Option<String>,
    codecs: Option<String>,
    encryption_type: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum CandidateSource {
    Tracks,
    #[default]
    LegacyApi,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateToken {
    track_id: String,
    #[serde(default)]
    source: CandidateSource,
}

impl MonochromeProvider {
    pub fn new() -> Result<Self, ProviderError> {
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(4))
            .user_agent(concat!("Refrain/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| {
                ProviderError::new(
                    "providerInitializationFailed",
                    format!("Could not initialize the Monochrome client: {error}"),
                    false,
                )
            })?;
        Ok(Self::with_client_and_urls(
            http,
            TRACKS_BASE_URL.to_owned(),
            configured_base_urls(),
        ))
    }

    fn with_client_and_urls(http: Client, tracks_base_url: String, base_urls: Vec<String>) -> Self {
        Self {
            inner: Arc::new(MonochromeInner {
                http,
                tracks_base_url,
                base_urls,
                jobs: Mutex::new(HashMap::new()),
                playback_degraded_until: Mutex::new(None),
                next_job_id: AtomicU64::new(1),
            }),
        }
    }

    fn playback_cooldown_active(&self) -> bool {
        let Ok(mut degraded_until) = self.inner.playback_degraded_until.lock() else {
            return false;
        };
        match *degraded_until {
            Some(until) if until > Instant::now() => true,
            Some(_) => {
                *degraded_until = None;
                false
            }
            None => false,
        }
    }

    fn mark_playback_degraded(&self, error: &ProviderError) {
        if !is_playback_outage_error(error) {
            return;
        }
        if let Ok(mut degraded_until) = self.inner.playback_degraded_until.lock() {
            *degraded_until = Some(Instant::now() + PLAYBACK_OUTAGE_COOLDOWN);
        }
    }

    fn clear_playback_degraded(&self) {
        if let Ok(mut degraded_until) = self.inner.playback_degraded_until.lock() {
            *degraded_until = None;
        }
    }

    pub(crate) fn reset_session_state(&self) {
        self.clear_playback_degraded();
    }

    pub fn progress_for_job(&self, provider_job_id: &str) -> Option<(i64, i64)> {
        let jobs = self.inner.jobs.lock().ok()?;
        let job = jobs.get(provider_job_id)?;
        let total = job.total_bytes.load(Ordering::Acquire);
        if total <= 0 {
            return None;
        }
        Some((job.bytes_transferred.load(Ordering::Acquire), total))
    }

    pub fn cancel_active_job(&self, provider_job_id: &str) -> Result<(), ProviderError> {
        self.cancel(provider_job_id)
    }

    fn api_get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, ProviderError> {
        let mut last_error = None;
        for base_url in &self.inner.base_urls {
            let raw_url = format!("{}{}", base_url.trim_end_matches('/'), path);
            let mut url = match url::Url::parse(&raw_url) {
                Ok(url) => url,
                Err(error) => {
                    last_error = Some(format!("{base_url}: invalid URL ({error})"));
                    continue;
                }
            };
            {
                let mut pairs = url.query_pairs_mut();
                for (key, value) in query {
                    pairs.append_pair(key, value);
                }
            }
            let response = match self
                .inner
                .http
                .get(url)
                .header(ACCEPT, "application/json")
                .timeout(API_TIMEOUT)
                .send()
            {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(format!("{base_url}: {error}"));
                    continue;
                }
            };
            if !response.status().is_success() {
                last_error = Some(format!("{base_url}: HTTP {}", response.status()));
                continue;
            }
            let value: Value = match response.json() {
                Ok(value) => value,
                Err(error) => {
                    last_error = Some(format!("{base_url}: invalid JSON ({error})"));
                    continue;
                }
            };
            let payload = value.get("data").cloned().unwrap_or(value);
            match serde_json::from_value(payload) {
                Ok(parsed) => return Ok(parsed),
                Err(error) => {
                    last_error = Some(format!("{base_url}: unexpected response ({error})"));
                }
            }
        }

        Err(ProviderError::new(
            "providerOffline",
            last_error.map_or_else(
                || "No Monochrome API instances are configured.".to_owned(),
                |error| {
                    format!("No Monochrome API instance is currently usable. Last error: {error}")
                },
            ),
            true,
        ))
    }

    fn tracks_get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, ProviderError> {
        let raw_url = format!(
            "{}{}",
            self.inner.tracks_base_url.trim_end_matches('/'),
            path
        );
        let mut url = url::Url::parse(&raw_url).map_err(|error| {
            ProviderError::new(
                "providerConfigurationInvalid",
                format!("Monochrome track service URL is invalid: {error}"),
                false,
            )
        })?;
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                pairs.append_pair(key, value);
            }
        }
        let response = self
            .inner
            .http
            .get(url)
            .header(ACCEPT, "application/json")
            .timeout(API_TIMEOUT)
            .send()
            .map_err(|error| {
                ProviderError::new(
                    "providerOffline",
                    format!("Monochrome track service is unavailable: {error}"),
                    true,
                )
            })?;
        if !response.status().is_success() {
            return Err(ProviderError::new(
                "providerOffline",
                format!(
                    "Monochrome track service returned HTTP {}.",
                    response.status()
                ),
                response.status().is_server_error() || response.status().as_u16() == 429,
            ));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.bytes().map_err(|error| {
            ProviderError::new(
                "providerProtocol",
                format!("Could not read the Monochrome track service response: {error}"),
                true,
            )
        })?;
        serde_json::from_slice(&body).map_err(|error| {
            ProviderError::new(
                "providerProtocol",
                invalid_json_message(content_type.as_deref(), &body, &error),
                true,
            )
        })
    }

    fn search_tracks_streamer(
        &self,
        query: &TrackQuery,
    ) -> Result<Vec<TracksSearchTrack>, ProviderError> {
        let text = if query.artists.is_empty() {
            query.title.clone()
        } else {
            format!("{} {}", query.artists.join(" "), query.title)
        };
        self.tracks_get::<TracksSearchData>(
            "/search/tracks",
            &[("q", text), ("limit", MAX_SEARCH_RESULTS.to_string())],
        )
        .map(|data| data.tracks)
    }

    fn search_legacy_tracks(
        &self,
        query: &TrackQuery,
    ) -> Result<Vec<MonochromeTrack>, ProviderError> {
        if let Some(isrc) = query
            .isrc
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            && let Ok(data) =
                self.api_get::<SearchData>("/search/", &[("i", isrc.trim().to_owned())])
            && !data.items.is_empty()
        {
            return Ok(data.items);
        }

        let text = if query.artists.is_empty() {
            query.title.clone()
        } else {
            format!("{} {}", query.artists.join(" "), query.title)
        };
        self.api_get::<SearchData>("/search/", &[("s", text)])
            .map(|data| data.items)
    }

    fn start_download(
        &self,
        token: CandidateToken,
        query: TrackQuery,
        output_path: PathBuf,
        job: Arc<MonochromeJob>,
    ) {
        let provider = self.clone();
        thread::spawn(move || {
            job.set_status(ProviderJobStatus::Running);
            let result = match token.source {
                CandidateSource::Tracks => provider.download_tracks_with_legacy_fallback(
                    &token.track_id,
                    &query,
                    &output_path,
                    &job,
                ),
                CandidateSource::LegacyApi => {
                    provider.download_lossless(&token.track_id, &output_path, &job)
                }
            };
            if job.cancelled.load(Ordering::Acquire) {
                let _ = fs::remove_file(output_path.with_extension("flac.part"));
                job.set_status(ProviderJobStatus::Cancelled);
                return;
            }
            match result {
                Ok(()) => {
                    provider.clear_playback_degraded();
                    job.set_status(ProviderJobStatus::Succeeded {
                        output_paths: vec![output_path.to_string_lossy().into_owned()],
                    });
                }
                Err(error) => {
                    provider.mark_playback_degraded(&error);
                    job.set_status(ProviderJobStatus::Failed {
                        code: error.code,
                        message: error.message,
                        retryable: error.retryable,
                    });
                }
            }
        });
    }

    fn download_tracks_with_legacy_fallback(
        &self,
        track_id: &str,
        query: &TrackQuery,
        output_path: &Path,
        job: &MonochromeJob,
    ) -> Result<(), ProviderError> {
        let direct_error = match self.download_tracks_lossless(track_id, output_path, job) {
            Ok(()) => return Ok(()),
            Err(error) if should_try_legacy_download_fallback(&error) => error,
            Err(error) => return Err(error),
        };

        if job.cancelled.load(Ordering::Acquire) {
            return Err(cancelled_error());
        }

        let legacy_candidates = match self.search_legacy_tracks(query) {
            Ok(tracks) => legacy_tracks_to_candidates(query, tracks)?,
            Err(legacy_error) => {
                return Err(combine_download_fallback_errors(direct_error, legacy_error));
            }
        };
        let Some(candidate) = legacy_candidates
            .into_iter()
            .find(|candidate| legacy_fallback_candidate_matches(query, candidate))
        else {
            return Err(ProviderError::new(
                direct_error.code,
                format!(
                    "{} Legacy fallback did not return a compatible lossless match.",
                    direct_error.message
                ),
                direct_error.retryable,
            ));
        };
        let legacy_token: CandidateToken = serde_json::from_str(&candidate.provider_token)
            .map_err(|error| {
                ProviderError::new(
                    "providerProtocol",
                    format!("Monochrome legacy candidate token is invalid: {error}"),
                    true,
                )
            })?;

        let part_path = output_path.with_extension("flac.part");
        if part_path.exists() {
            fs::remove_file(&part_path).map_err(io_provider_error)?;
        }
        job.bytes_transferred.store(0, Ordering::Release);
        job.total_bytes.store(0, Ordering::Release);

        match self.download_lossless(&legacy_token.track_id, output_path, job) {
            Ok(()) => Ok(()),
            Err(legacy_error) => Err(combine_download_fallback_errors(direct_error, legacy_error)),
        }
    }

    fn download_tracks_lossless(
        &self,
        track_id: &str,
        output_path: &Path,
        job: &MonochromeJob,
    ) -> Result<(), ProviderError> {
        let url = tracks_stream_url(&self.inner.tracks_base_url, track_id)?;
        self.download_url(url, output_path, job, true)
    }

    fn download_lossless(
        &self,
        track_id: &str,
        output_path: &Path,
        job: &MonochromeJob,
    ) -> Result<(), ProviderError> {
        let playback = self.playback_data(track_id)?;
        let manifest = decode_manifest(&playback.manifest)?;
        if !manifest
            .encryption_type
            .as_deref()
            .unwrap_or("NONE")
            .eq_ignore_ascii_case("NONE")
        {
            return Err(ProviderError::new(
                "encryptedCandidate",
                "Monochrome returned an encrypted audio resource.",
                false,
            ));
        }
        let codec = manifest
            .codecs
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let mime = manifest
            .mime_type
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !codec.contains("flac") && !mime.contains("flac") {
            return Err(ProviderError::new(
                "losslessUnavailable",
                "Monochrome did not return a FLAC resource for this track.",
                false,
            ));
        }
        let source_url = manifest.urls.first().ok_or_else(|| {
            ProviderError::new(
                "downloadUrlUnavailable",
                "Monochrome returned no download URL for this track.",
                true,
            )
        })?;
        let parsed = url::Url::parse(source_url).map_err(|_| {
            ProviderError::new(
                "invalidDownloadUrl",
                "Monochrome returned an invalid download URL.",
                false,
            )
        })?;
        if parsed.scheme() != "https" {
            return Err(ProviderError::new(
                "invalidDownloadUrl",
                "Monochrome returned a non-HTTPS download URL.",
                false,
            ));
        }
        self.download_url(parsed, output_path, job, false)
    }

    fn download_url(
        &self,
        parsed: url::Url,
        output_path: &Path,
        job: &MonochromeJob,
        verify_flac_header: bool,
    ) -> Result<(), ProviderError> {
        if job.cancelled.load(Ordering::Acquire) {
            return Err(cancelled_error());
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(io_provider_error)?;
        }
        let part_path = output_path.with_extension("flac.part");
        let mut last_error = None;

        'stream_attempts: for stream_attempt in 0..MAX_DOWNLOAD_STREAM_ATTEMPTS {
            if job.cancelled.load(Ordering::Acquire) {
                let _ = fs::remove_file(&part_path);
                return Err(cancelled_error());
            }

            let resume_from = fs::metadata(&part_path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            let mut request = self
                .inner
                .http
                .get(parsed.clone())
                .timeout(DOWNLOAD_TIMEOUT);
            if resume_from > 0 {
                request = request.header(RANGE, format!("bytes={resume_from}-"));
            }

            let mut response = match request.send() {
                Ok(response) => response,
                Err(error) => {
                    let error = ProviderError::new(
                        "downloadFailed",
                        format!("Could not start the Monochrome download: {error}"),
                        true,
                    );
                    if stream_attempt + 1 < MAX_DOWNLOAD_STREAM_ATTEMPTS {
                        last_error = Some(error);
                        thread::sleep(download_retry_delay(stream_attempt));
                        continue;
                    }
                    return Err(error);
                }
            };
            if response.status() == StatusCode::RANGE_NOT_SATISFIABLE && resume_from > 0 {
                let _ = fs::remove_file(&part_path);
                job.bytes_transferred.store(0, Ordering::Release);
                if stream_attempt + 1 < MAX_DOWNLOAD_STREAM_ATTEMPTS {
                    continue;
                }
            }
            if !response.status().is_success() {
                let retryable = retryable_download_status(response.status());
                let error = ProviderError::new(
                    "downloadFailed",
                    format!("Monochrome download returned HTTP {}.", response.status()),
                    retryable,
                );
                if retryable && stream_attempt + 1 < MAX_DOWNLOAD_STREAM_ATTEMPTS {
                    last_error = Some(error);
                    thread::sleep(download_retry_delay(stream_attempt));
                    continue;
                }
                return Err(error);
            }

            let resumed = resume_from > 0 && response.status() == StatusCode::PARTIAL_CONTENT;
            let write_offset = if resumed { resume_from } else { 0 };
            if resume_from > 0 && !resumed {
                let _ = fs::remove_file(&part_path);
            }
            job.bytes_transferred.store(
                i64::try_from(write_offset).unwrap_or(i64::MAX),
                Ordering::Release,
            );

            let total = response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(content_range_total)
                .or_else(|| {
                    response.content_length().and_then(|length| {
                        write_offset
                            .checked_add(length)
                            .and_then(|value| i64::try_from(value).ok())
                    })
                });
            if let Some(total) = total {
                job.total_bytes.store(total, Ordering::Release);
            }

            let mut output = if resumed {
                fs::OpenOptions::new()
                    .append(true)
                    .open(&part_path)
                    .map_err(io_provider_error)?
            } else {
                fs::File::create(&part_path).map_err(io_provider_error)?
            };
            let mut buffer = [0_u8; 64 * 1024];
            let mut first_chunk = write_offset == 0;
            last_error = None;
            loop {
                if job.cancelled.load(Ordering::Acquire) {
                    drop(output);
                    let _ = fs::remove_file(&part_path);
                    return Err(cancelled_error());
                }
                let read = match response.read(&mut buffer) {
                    Ok(read) => read,
                    Err(error) => {
                        last_error = Some(download_read_error(error));
                        break;
                    }
                };
                if read == 0 {
                    break;
                }
                if first_chunk && verify_flac_header {
                    first_chunk = false;
                    if read < 4 || &buffer[..4] != b"fLaC" {
                        drop(output);
                        let _ = fs::remove_file(&part_path);
                        let error = ProviderError::new(
                            "providerProtocol",
                            "Monochrome track service did not return a FLAC stream.",
                            true,
                        );
                        if stream_attempt + 1 < MAX_DOWNLOAD_STREAM_ATTEMPTS {
                            last_error = Some(error);
                            thread::sleep(download_retry_delay(stream_attempt));
                            continue 'stream_attempts;
                        }
                        return Err(error);
                    }
                }
                output
                    .write_all(&buffer[..read])
                    .map_err(io_provider_error)?;
                job.bytes_transferred
                    .fetch_add(i64::try_from(read).unwrap_or(i64::MAX), Ordering::AcqRel);
            }

            output.flush().map_err(io_provider_error)?;
            drop(output);

            if let Some(error) = last_error.take() {
                if stream_attempt + 1 < MAX_DOWNLOAD_STREAM_ATTEMPTS {
                    thread::sleep(download_retry_delay(stream_attempt));
                    continue;
                }
                return Err(error);
            }

            let transferred = job.bytes_transferred.load(Ordering::Acquire);
            let expected = job.total_bytes.load(Ordering::Acquire);
            if expected > 0 && transferred < expected {
                if stream_attempt + 1 < MAX_DOWNLOAD_STREAM_ATTEMPTS {
                    thread::sleep(download_retry_delay(stream_attempt));
                    continue;
                }
                let error = ProviderError::new(
                    "downloadTransportFailed",
                    format!(
                        "Monochrome download ended early after {transferred} of {expected} bytes."
                    ),
                    true,
                );
                last_error = Some(error);
                continue;
            }
            if verify_flac_header && first_chunk && write_offset == 0 {
                let _ = fs::remove_file(&part_path);
                let error = ProviderError::new(
                    "downloadFailed",
                    "Monochrome track service returned an empty download.",
                    true,
                );
                if stream_attempt + 1 < MAX_DOWNLOAD_STREAM_ATTEMPTS {
                    last_error = Some(error);
                    thread::sleep(download_retry_delay(stream_attempt));
                    continue;
                }
                return Err(error);
            }

            fs::rename(&part_path, output_path).map_err(io_provider_error)?;
            return Ok(());
        }

        Err(last_error.unwrap_or_else(|| {
            ProviderError::new(
                "downloadTransportFailed",
                "Monochrome download did not complete after multiple stream attempts.",
                true,
            )
        }))
    }

    fn playback_data(&self, track_id: &str) -> Result<PlaybackData, ProviderError> {
        let mut last_error = None;
        for quality in ["LOSSLESS", "HI_RES_LOSSLESS"] {
            match self.api_get::<PlaybackData>(
                "/track/",
                &[("id", track_id.to_owned()), ("quality", quality.to_owned())],
            ) {
                Ok(data) if !is_dash_manifest(&data) => return Ok(data),
                Ok(_) => {
                    last_error = Some(ProviderError::new(
                        "dashManifestUnsupported",
                        format!(
                            "Monochrome returned a segmented MPEG-DASH stream for {quality}; Refrain requires a direct lossless FLAC resource."
                        ),
                        false,
                    ));
                }
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            ProviderError::new(
                "losslessUnavailable",
                "Monochrome could not resolve a lossless stream for this track.",
                true,
            )
        }))
    }

    fn enrich_tracks_candidates_with_stream_quality(
        &self,
        candidates: &mut [AcquisitionCandidate],
    ) {
        let probes = candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                if candidate.sample_rate_hz.is_some() && candidate.bit_depth.is_some() {
                    return None;
                }
                let token: CandidateToken = serde_json::from_str(&candidate.provider_token).ok()?;
                (token.source == CandidateSource::Tracks).then_some((index, token.track_id))
            })
            .collect::<Vec<_>>();

        for chunk in probes.chunks(QUALITY_PROBE_CONCURRENCY) {
            let results = thread::scope(|scope| {
                chunk
                    .iter()
                    .map(|(index, track_id)| {
                        let provider = self.clone();
                        let track_id = track_id.clone();
                        (
                            *index,
                            scope.spawn(move || provider.probe_tracks_stream_quality(&track_id)),
                        )
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .filter_map(|(index, handle)| {
                        handle.join().ok().flatten().map(|quality| (index, quality))
                    })
                    .collect::<Vec<_>>()
            });

            for (index, (sample_rate_hz, bit_depth)) in results {
                if let Some(candidate) = candidates.get_mut(index) {
                    candidate.sample_rate_hz.get_or_insert(sample_rate_hz);
                    candidate.bit_depth.get_or_insert(bit_depth);
                }
            }
        }
    }

    fn probe_tracks_stream_quality(&self, track_id: &str) -> Option<(i64, i64)> {
        let url = tracks_stream_url(&self.inner.tracks_base_url, track_id).ok()?;
        let mut response = self
            .inner
            .http
            .get(url)
            .header(
                ACCEPT,
                "audio/flac,application/octet-stream;q=0.9,*/*;q=0.1",
            )
            .header(RANGE, "bytes=0-41")
            .timeout(QUALITY_PROBE_TIMEOUT)
            .send()
            .ok()?;
        if !response.status().is_success() {
            return None;
        }

        let mut prefix = [0_u8; 42];
        response.read_exact(&mut prefix).ok()?;
        parse_flac_streaminfo_quality(&prefix)
    }
}

impl AcquisitionProvider for MonochromeProvider {
    fn id(&self) -> &'static str {
        MONOCHROME_PROVIDER_ID
    }

    fn health(&self) -> Result<ProviderHealth, ProviderError> {
        if self.playback_cooldown_active() {
            return Ok(ProviderHealth {
                available: false,
                version: None,
                message: Some(
                    "Monochrome playback is temporarily unavailable after repeated stream failures; trying the next provider."
                        .to_owned(),
                ),
            });
        }
        match self.tracks_get::<TracksSearchData>(
            "/search/tracks",
            &[
                ("q", "refrain provider health check".to_owned()),
                ("limit", "1".to_owned()),
            ],
        ) {
            Ok(_) => Ok(ProviderHealth {
                available: true,
                version: None,
                message: Some("Monochrome track service is reachable.".to_owned()),
            }),
            Err(tracks_error) => match self.api_get::<SearchData>(
                "/search/",
                &[("s", "refrain provider health check".to_owned())],
            ) {
                Ok(_) => Ok(ProviderHealth {
                    available: true,
                    version: None,
                    message: Some(
                        "Monochrome track service is unavailable; legacy API fallback is reachable."
                            .to_owned(),
                    ),
                }),
                Err(legacy_error) => {
                    let error = ProviderError::new(
                        "providerOffline",
                        format!(
                            "{} Legacy fallback: {}",
                            tracks_error.message, legacy_error.message
                        ),
                        true,
                    );
                    self.mark_playback_degraded(&error);
                    Ok(ProviderHealth {
                        available: false,
                        version: None,
                        message: Some(error.message),
                    })
                }
            },
        }
    }

    fn search(&self, query: &TrackQuery) -> Result<Vec<AcquisitionCandidate>, ProviderError> {
        match self.search_tracks_streamer(query) {
            Ok(tracks) => {
                let tracks = tracks
                    .into_iter()
                    .filter(|track| track.playable.unwrap_or(true))
                    .collect::<Vec<_>>();
                if tracks.is_empty() {
                    return match self.search_legacy_tracks(query) {
                        Ok(tracks) => legacy_tracks_to_candidates(query, tracks),
                        Err(_) => Ok(Vec::new()),
                    };
                }
                let mut candidates = tracks_to_candidates(query, tracks)?;
                self.enrich_tracks_candidates_with_stream_quality(&mut candidates);
                Ok(candidates)
            }
            Err(tracks_error) => match self.search_legacy_tracks(query) {
                Ok(tracks) => legacy_tracks_to_candidates(query, tracks),
                Err(legacy_error) => Err(ProviderError::new(
                    "providerOffline",
                    format!(
                        "{} Legacy fallback: {}",
                        tracks_error.message, legacy_error.message
                    ),
                    true,
                )),
            },
        }
    }

    fn acquire(&self, request: &AcquisitionRequest) -> Result<ProviderJob, ProviderError> {
        let token: CandidateToken = serde_json::from_str(&request.candidate.provider_token)
            .map_err(|error| {
                ProviderError::new(
                    "invalidCandidateToken",
                    format!("Monochrome candidate token is invalid: {error}"),
                    false,
                )
            })?;
        let provider_job_id = format!(
            "monochrome-{}-{}",
            request.library_track_id,
            self.inner.next_job_id.fetch_add(1, Ordering::Relaxed)
        );
        let job = Arc::new(MonochromeJob::new());
        self.inner
            .jobs
            .lock()
            .map_err(|_| {
                ProviderError::new(
                    "providerStateUnavailable",
                    "Monochrome download state is unavailable.",
                    true,
                )
            })?
            .insert(provider_job_id.clone(), Arc::clone(&job));
        let output_path = Path::new(&request.staging_path).join(format!("{}.flac", token.track_id));
        self.start_download(token, request.query.clone(), output_path, job);
        Ok(ProviderJob { provider_job_id })
    }

    fn status(&self, provider_job_id: &str) -> Result<ProviderJobStatus, ProviderError> {
        let jobs = self.inner.jobs.lock().map_err(|_| {
            ProviderError::new(
                "providerStateUnavailable",
                "Monochrome download state is unavailable.",
                true,
            )
        })?;
        jobs.get(provider_job_id)
            .ok_or_else(|| {
                ProviderError::new(
                    "providerJobMissing",
                    "Monochrome download state is no longer available.",
                    true,
                )
            })?
            .status()
    }

    fn cancel(&self, provider_job_id: &str) -> Result<(), ProviderError> {
        let jobs = self.inner.jobs.lock().map_err(|_| {
            ProviderError::new(
                "providerStateUnavailable",
                "Monochrome download state is unavailable.",
                true,
            )
        })?;
        if let Some(job) = jobs.get(provider_job_id) {
            job.cancelled.store(true, Ordering::Release);
            job.set_status(ProviderJobStatus::Cancelled);
        }
        Ok(())
    }
}

fn tracks_to_candidates(
    query: &TrackQuery,
    mut tracks: Vec<TracksSearchTrack>,
) -> Result<Vec<AcquisitionCandidate>, ProviderError> {
    tracks.sort_by_key(|track| std::cmp::Reverse(tracks_track_match_score(query, track)));
    let mut seen_recordings = HashSet::new();
    let mut seen_isrcs = HashSet::new();
    tracks
        .into_iter()
        .filter(|track| {
            let recording_id = track.recording_id.as_ref().and_then(id_string);
            let isrc = track
                .isrc
                .as_deref()
                .map(normalized_identifier)
                .filter(|value| !value.is_empty());
            if recording_id
                .as_ref()
                .is_some_and(|value| seen_recordings.contains(value))
                || isrc
                    .as_ref()
                    .is_some_and(|value| seen_isrcs.contains(value))
            {
                return false;
            }
            if let Some(recording_id) = recording_id {
                seen_recordings.insert(recording_id);
            }
            if let Some(isrc) = isrc {
                seen_isrcs.insert(isrc);
            }
            true
        })
        .take(MAX_SEARCH_RESULTS)
        .map(tracks_track_to_candidate)
        .collect()
}

fn normalized_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

fn configured_base_urls() -> Vec<String> {
    env::var(INSTANCE_OVERRIDE_ENV)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.trim_end_matches('/').to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|urls| !urls.is_empty())
        .unwrap_or_else(|| {
            DEFAULT_API_URLS
                .iter()
                .map(|url| (*url).to_owned())
                .collect()
        })
}

fn invalid_json_message(
    content_type: Option<&str>,
    body: &[u8],
    error: &serde_json::Error,
) -> String {
    let content_type = content_type.unwrap_or("unknown content type");
    let preview = String::from_utf8_lossy(&body[..body.len().min(180)])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if preview.is_empty() {
        format!(
            "Monochrome track service returned an empty or invalid JSON response ({content_type}): {error}"
        )
    } else {
        format!(
            "Monochrome track service returned invalid JSON ({content_type}): {error}. Response starts with: {preview}"
        )
    }
}

fn tracks_stream_url(base_url: &str, track_id: &str) -> Result<url::Url, ProviderError> {
    let mut url = url::Url::parse(base_url).map_err(|error| {
        ProviderError::new(
            "providerConfigurationInvalid",
            format!("Monochrome track service URL is invalid: {error}"),
            false,
        )
    })?;
    if url.scheme() != "https" {
        return Err(ProviderError::new(
            "providerConfigurationInvalid",
            "Monochrome track service must use HTTPS.",
            false,
        ));
    }
    url.set_query(None);
    url.set_fragment(None);
    let mut segments = url.path_segments_mut().map_err(|_| {
        ProviderError::new(
            "providerConfigurationInvalid",
            "Monochrome track service URL cannot be used for track downloads.",
            false,
        )
    })?;
    segments.pop_if_empty();
    segments.push("track");
    segments.push(track_id);
    drop(segments);
    Ok(url)
}

fn legacy_tracks_to_candidates(
    query: &TrackQuery,
    mut tracks: Vec<MonochromeTrack>,
) -> Result<Vec<AcquisitionCandidate>, ProviderError> {
    tracks.retain(|track| {
        track.stream_ready.unwrap_or(true)
            && track
                .audio_quality
                .as_deref()
                .is_none_or(|quality| quality.to_ascii_uppercase().contains("LOSSLESS"))
    });
    tracks.sort_by_key(|track| std::cmp::Reverse(track_match_score(query, track)));
    tracks
        .into_iter()
        .take(MAX_SEARCH_RESULTS)
        .map(track_to_candidate)
        .collect()
}

fn tracks_track_to_candidate(
    track: TracksSearchTrack,
) -> Result<AcquisitionCandidate, ProviderError> {
    let track_id = track
        .track_id
        .as_ref()
        .and_then(id_string)
        .or_else(|| track.id.as_ref().and_then(id_string))
        .ok_or_else(|| {
            ProviderError::new(
                "providerProtocol",
                "Monochrome track service returned a track without a usable ID.",
                false,
            )
        })?;
    let artists = tracks_track_artists(&track);
    let isrc = track.isrc.clone();
    let recording_id = track.recording_id.as_ref().and_then(id_string);
    let release_id = track.release_id.as_ref().and_then(id_string);
    let sample_rate_hz = optional_positive_i64(track.sample_rate_hz.as_ref());
    let bit_depth = optional_positive_i64(track.bit_depth.as_ref());
    let title = track.title;
    let provider_token = serde_json::to_string(&CandidateToken {
        track_id,
        source: CandidateSource::Tracks,
    })
    .map_err(|error| ProviderError::new("providerProtocol", error.to_string(), false))?;
    Ok(AcquisitionCandidate {
        provider: Some("monochrome".to_owned()),
        provider_token,
        source: None,
        file_name: Some(format!("{title}.flac")),
        title: Some(title),
        artists,
        album: track.album_title.or(track.release_title),
        duration_ms: track.duration.map(tracks_duration_to_ms),
        format: Some("FLAC".to_owned()),
        size_bytes: None,
        bitrate_kbps: None,
        sample_rate_hz,
        bit_depth,
        confidence: None,
        isrc,
        recording_id,
        release_id,
    })
}

fn track_to_candidate(track: MonochromeTrack) -> Result<AcquisitionCandidate, ProviderError> {
    let track_id = id_string(&track.id).ok_or_else(|| {
        ProviderError::new(
            "providerProtocol",
            "Monochrome returned a track without a usable ID.",
            false,
        )
    })?;
    let artists = if track.artists.is_empty() {
        track
            .artist
            .as_ref()
            .map(|artist| vec![artist.name.clone()])
            .unwrap_or_default()
    } else {
        track
            .artists
            .iter()
            .map(|artist| artist.name.clone())
            .collect()
    };
    let isrc = track.isrc.clone();
    let (sample_rate_hz, bit_depth) = monochrome_track_quality(&track);
    let title = track.title;
    let provider_token = serde_json::to_string(&CandidateToken {
        track_id,
        source: CandidateSource::LegacyApi,
    })
    .map_err(|error| ProviderError::new("providerProtocol", error.to_string(), false))?;
    Ok(AcquisitionCandidate {
        provider: Some("monochrome".to_owned()),
        provider_token,
        source: None,
        file_name: Some(format!("{title}.flac")),
        title: Some(title),
        artists,
        album: track.album.and_then(|album| album.title),
        duration_ms: track.duration.map(|seconds| seconds.saturating_mul(1_000)),
        format: Some("FLAC".to_owned()),
        size_bytes: None,
        bitrate_kbps: None,
        sample_rate_hz,
        bit_depth,
        confidence: None,
        isrc,
        recording_id: None,
        release_id: None,
    })
}

fn monochrome_track_quality(track: &MonochromeTrack) -> (Option<i64>, Option<i64>) {
    let sample_rate_hz = optional_positive_i64(track.sample_rate_hz.as_ref());
    let bit_depth = optional_positive_i64(track.bit_depth.as_ref());
    if sample_rate_hz.is_some() || bit_depth.is_some() {
        return (sample_rate_hz, bit_depth);
    }

    match track
        .audio_quality
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_uppercase)
        .as_deref()
    {
        Some("LOSSLESS") => (Some(44_100), Some(16)),
        _ => (None, None),
    }
}

fn optional_positive_i64(value: Option<&Value>) -> Option<i64> {
    value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
                .or_else(|| {
                    value
                        .as_str()
                        .and_then(|value| value.trim().parse::<i64>().ok())
                })
        })
        .filter(|value| *value > 0)
}

fn parse_flac_streaminfo_quality(bytes: &[u8]) -> Option<(i64, i64)> {
    if bytes.len() < 42 || &bytes[..4] != b"fLaC" {
        return None;
    }
    let block_type = bytes[4] & 0x7f;
    let block_length =
        (usize::from(bytes[5]) << 16) | (usize::from(bytes[6]) << 8) | usize::from(bytes[7]);
    if block_type != 0 || block_length < 34 {
        return None;
    }

    let sample_rate_hz =
        (u32::from(bytes[18]) << 12) | (u32::from(bytes[19]) << 4) | (u32::from(bytes[20]) >> 4);
    let bit_depth = (((bytes[20] & 0x01) << 4) | (bytes[21] >> 4)).saturating_add(1);
    if sample_rate_hz == 0 || bit_depth == 0 {
        return None;
    }

    Some((i64::from(sample_rate_hz), i64::from(bit_depth)))
}

fn tracks_track_match_score(query: &TrackQuery, track: &TracksSearchTrack) -> i32 {
    let mut score = 0;
    if normalized_text(&track.title) == normalized_text(&query.title) {
        score += 8;
    }
    let artists = tracks_track_artists(track);
    if artists.iter().any(|artist| {
        query
            .artists
            .iter()
            .any(|expected| normalized_text(artist) == normalized_text(expected))
    }) {
        score += 6;
    }
    if let Some(expected) = query.album.as_deref()
        && track
            .album_title
            .as_deref()
            .or(track.release_title.as_deref())
            .is_some_and(|actual| normalized_text(expected) == normalized_text(actual))
    {
        score += 2;
    }
    if let (Some(expected), Some(actual)) = (query.duration_ms, track.duration) {
        let difference = (expected - tracks_duration_to_ms(actual)).abs();
        if difference <= 2_000 {
            score += 3;
        } else if difference <= 5_000 {
            score += 1;
        }
    }
    if let (Some(expected), Some(actual)) = (query.isrc.as_deref(), track.isrc.as_deref())
        && expected.eq_ignore_ascii_case(actual)
    {
        score += 12;
    }
    score
}

fn tracks_track_artists(track: &TracksSearchTrack) -> Vec<String> {
    if !track.artist_names.is_empty() {
        return track.artist_names.clone();
    }
    track
        .artists
        .iter()
        .filter_map(|artist| artist.name.clone().or_else(|| artist.display_name.clone()))
        .collect()
}

fn tracks_duration_to_ms(duration: i64) -> i64 {
    if duration > 1_000 {
        duration
    } else {
        duration.saturating_mul(1_000)
    }
}

fn track_match_score(query: &TrackQuery, track: &MonochromeTrack) -> i32 {
    let mut score = 0;
    if normalized_text(&track.title) == normalized_text(&query.title) {
        score += 8;
    }
    let track_artists = if track.artists.is_empty() {
        track
            .artist
            .iter()
            .map(|artist| &artist.name)
            .collect::<Vec<_>>()
    } else {
        track
            .artists
            .iter()
            .map(|artist| &artist.name)
            .collect::<Vec<_>>()
    };
    if track_artists.iter().any(|artist| {
        query
            .artists
            .iter()
            .any(|expected| normalized_text(artist) == normalized_text(expected))
    }) {
        score += 6;
    }
    if let (Some(expected), Some(actual)) = (
        query.album.as_deref(),
        track
            .album
            .as_ref()
            .and_then(|album| album.title.as_deref()),
    ) && normalized_text(expected) == normalized_text(actual)
    {
        score += 2;
    }
    if let (Some(expected), Some(actual_seconds)) = (query.duration_ms, track.duration) {
        let difference = (expected - actual_seconds.saturating_mul(1_000)).abs();
        if difference <= 2_000 {
            score += 3;
        } else if difference <= 5_000 {
            score += 1;
        }
    }
    if let (Some(expected), Some(actual)) = (query.isrc.as_deref(), track.isrc.as_deref())
        && expected.eq_ignore_ascii_case(actual)
    {
        score += 12;
    }
    score
}

fn normalized_text(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn id_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn decode_manifest(encoded: &str) -> Result<PlaybackManifest, ProviderError> {
    let bytes = general_purpose::STANDARD
        .decode(encoded)
        .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(encoded))
        .map_err(|error| {
            ProviderError::new(
                "providerProtocol",
                format!("Monochrome returned an invalid stream manifest: {error}"),
                false,
            )
        })?;
    if bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        == Some(b'<')
    {
        return Err(ProviderError::new(
            "dashManifestUnsupported",
            "Monochrome returned a segmented MPEG-DASH stream; Refrain requires a direct lossless FLAC resource.",
            false,
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| {
        ProviderError::new(
            "providerProtocol",
            format!("Monochrome returned an unreadable stream manifest: {error}"),
            false,
        )
    })
}

fn is_dash_manifest(playback: &PlaybackData) -> bool {
    playback
        .manifest_mime_type
        .as_deref()
        .is_some_and(|mime_type| mime_type.eq_ignore_ascii_case("application/dash+xml"))
}

fn cancelled_error() -> ProviderError {
    ProviderError::new("cancelled", "Monochrome download was cancelled.", true)
}

fn io_provider_error(error: std::io::Error) -> ProviderError {
    ProviderError::new(
        "downloadIoFailed",
        format!("Could not write the Monochrome download: {error}"),
        true,
    )
}

fn download_read_error(error: std::io::Error) -> ProviderError {
    ProviderError::new(
        "downloadTransportFailed",
        format!("The Monochrome download stream was interrupted: {error}"),
        true,
    )
}

fn retryable_download_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn should_try_legacy_download_fallback(error: &ProviderError) -> bool {
    matches!(
        error.code.as_str(),
        "downloadFailed" | "downloadTransportFailed" | "providerProtocol" | "providerOffline"
    )
}

fn is_playback_outage_error(error: &ProviderError) -> bool {
    match error.code.as_str() {
        "providerOffline" | "downloadTransportFailed" | "providerProtocol" => true,
        "downloadFailed" | "monochromeDownloadFailed" => {
            error.retryable || error.message.contains("HTTP 403")
        }
        _ => false,
    }
}

fn legacy_fallback_candidate_matches(query: &TrackQuery, candidate: &AcquisitionCandidate) -> bool {
    let title_matches = candidate
        .title
        .as_deref()
        .is_some_and(|title| normalized_text(title) == normalized_text(&query.title));
    let artist_matches = candidate.artists.iter().any(|artist| {
        query
            .artists
            .iter()
            .any(|expected| normalized_text(artist) == normalized_text(expected))
    });
    let duration_matches = match (query.duration_ms, candidate.duration_ms) {
        (Some(expected), Some(actual)) => (expected - actual).abs() <= 5_000,
        _ => true,
    };
    let isrc_compatible = match (
        query.isrc.as_deref().map(normalized_identifier),
        candidate.isrc.as_deref().map(normalized_identifier),
    ) {
        (Some(expected), Some(actual)) => expected == actual,
        _ => true,
    };

    title_matches && artist_matches && duration_matches && isrc_compatible
}

fn combine_download_fallback_errors(
    direct_error: ProviderError,
    legacy_error: ProviderError,
) -> ProviderError {
    ProviderError::new(
        "monochromeDownloadFailed",
        format!(
            "Direct Monochrome download failed: {} Legacy fallback failed: {}",
            direct_error.message, legacy_error.message
        ),
        direct_error.retryable || legacy_error.retryable,
    )
}

fn download_retry_delay(attempt: usize) -> Duration {
    let shift = u32::try_from(attempt.min(4)).unwrap_or(4);
    Duration::from_millis(500_u64.saturating_mul(1_u64 << shift).min(8_000))
}

fn content_range_total(value: &str) -> Option<i64> {
    value
        .rsplit_once('/')
        .and_then(|(_, total)| total.parse::<i64>().ok())
        .filter(|total| *total >= 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    fn query() -> TrackQuery {
        TrackQuery {
            library_track_id: 1,
            title: "Introduction to the Snow".to_owned(),
            artists: vec!["Miracle Musical".to_owned()],
            album: Some("Hawaii: Part II".to_owned()),
            duration_ms: Some(101_000),
            isrc: Some("QM4DW1700001".to_owned()),
            version_kind: None,
            version_detail: None,
        }
    }

    #[test]
    fn exact_monochrome_track_scores_above_partial_match() {
        let exact: MonochromeTrack = serde_json::from_value(serde_json::json!({
            "id": 123,
            "title": "Introduction to the Snow",
            "duration": 101,
            "streamReady": true,
            "audioQuality": "LOSSLESS",
            "isrc": "QM4DW1700001",
            "artist": { "name": "Miracle Musical" },
            "artists": [{ "name": "Miracle Musical" }],
            "album": { "title": "Hawaii: Part II" }
        }))
        .unwrap();
        let partial: MonochromeTrack = serde_json::from_value(serde_json::json!({
            "id": 456,
            "title": "Introduction to the Snow (Cover)",
            "duration": 105,
            "streamReady": true,
            "audioQuality": "LOSSLESS",
            "artist": { "name": "Someone Else" },
            "album": { "title": "Covers" }
        }))
        .unwrap();
        assert!(track_match_score(&query(), &exact) > track_match_score(&query(), &partial));
    }

    #[test]
    fn decodes_lossless_monochrome_manifest() {
        let manifest = serde_json::json!({
            "urls": ["https://example.test/audio.flac"],
            "mimeType": "audio/flac",
            "codecs": "flac",
            "encryptionType": "NONE"
        });
        let encoded = general_purpose::STANDARD.encode(serde_json::to_vec(&manifest).unwrap());
        let decoded = decode_manifest(&encoded).unwrap();
        assert_eq!(decoded.urls, vec!["https://example.test/audio.flac"]);
        assert_eq!(decoded.codecs.as_deref(), Some("flac"));
        assert_eq!(decoded.encryption_type.as_deref(), Some("NONE"));
    }

    #[test]
    fn rejects_dash_manifest_without_reporting_it_as_broken_json() {
        let encoded =
            general_purpose::STANDARD.encode(b"<?xml version='1.0'?><MPD><Period /></MPD>");

        let error = decode_manifest(&encoded).unwrap_err();

        assert_eq!(error.code, "dashManifestUnsupported");
        assert!(!error.retryable);
        assert!(error.message.contains("MPEG-DASH"));
    }

    #[test]
    fn recognizes_dash_playback_response_from_manifest_mime_type() {
        let playback: PlaybackData = serde_json::from_value(serde_json::json!({
            "manifest": "ignored",
            "manifestMimeType": "application/dash+xml"
        }))
        .unwrap();

        assert!(is_dash_manifest(&playback));
    }

    #[test]
    fn parses_tracks_streamer_search_result_into_lossless_candidate() {
        let track: TracksSearchTrack = serde_json::from_value(serde_json::json!({
            "trackId": "153952818261528576",
            "title": "Introduction to the Snow",
            "duration": 101000,
            "playable": true,
            "isrc": "QM4DW1700001",
            "recordingId": "recording-1",
            "artistNames": ["Miracle Musical"],
            "releaseId": "153952817441148928",
            "albumTitle": "Hawaii: Part II",
            "sampleRate": "96000",
            "bitDepth": 24
        }))
        .unwrap();

        let candidate = tracks_track_to_candidate(track).unwrap();
        let token: CandidateToken = serde_json::from_str(&candidate.provider_token).unwrap();

        assert_eq!(token.track_id, "153952818261528576");
        assert_eq!(token.source, CandidateSource::Tracks);
        assert_eq!(candidate.title.as_deref(), Some("Introduction to the Snow"));
        assert_eq!(candidate.artists, vec!["Miracle Musical"]);
        assert_eq!(candidate.album.as_deref(), Some("Hawaii: Part II"));
        assert_eq!(candidate.duration_ms, Some(101_000));
        assert_eq!(candidate.format.as_deref(), Some("FLAC"));
        assert_eq!(candidate.sample_rate_hz, Some(96_000));
        assert_eq!(candidate.bit_depth, Some(24));
        assert_eq!(candidate.isrc.as_deref(), Some("QM4DW1700001"));
        assert_eq!(candidate.recording_id.as_deref(), Some("recording-1"));
        assert_eq!(candidate.release_id.as_deref(), Some("153952817441148928"));
    }

    #[test]
    fn maps_legacy_lossless_quality_to_cd_audio_metadata() {
        let track: MonochromeTrack = serde_json::from_value(serde_json::json!({
            "id": 123,
            "title": "Introduction to the Snow",
            "duration": 101,
            "streamReady": true,
            "audioQuality": "LOSSLESS",
            "isrc": "QM4DW1700001",
            "artist": { "name": "Miracle Musical" },
            "album": { "title": "Hawaii: Part II" }
        }))
        .unwrap();

        let candidate = track_to_candidate(track).unwrap();

        assert_eq!(candidate.sample_rate_hz, Some(44_100));
        assert_eq!(candidate.bit_depth, Some(16));
    }

    #[test]
    fn parses_sample_rate_and_bit_depth_from_flac_streaminfo_prefix() {
        let mut prefix = [0_u8; 42];
        prefix[..4].copy_from_slice(b"fLaC");
        prefix[4] = 0x80;
        prefix[7] = 34;

        let sample_rate = 96_000_u32;
        let bit_depth_minus_one = 23_u8;
        prefix[18] = (sample_rate >> 12) as u8;
        prefix[19] = (sample_rate >> 4) as u8;
        prefix[20] = ((sample_rate & 0x0f) as u8) << 4;
        prefix[20] |= bit_depth_minus_one >> 4;
        prefix[21] = (bit_depth_minus_one & 0x0f) << 4;

        assert_eq!(parse_flac_streaminfo_quality(&prefix), Some((96_000, 24)));
    }

    #[test]
    fn rejects_non_flac_quality_probe_payloads() {
        assert_eq!(parse_flac_streaminfo_quality(b"not a FLAC response"), None);
    }

    #[test]
    fn parses_tracks_streamer_result_when_id_and_track_id_are_both_present() {
        let track: TracksSearchTrack = serde_json::from_value(serde_json::json!({
            "id": "181548135879852032",
            "trackId": "181548135879852032",
            "title": "Provider",
            "artistIds": ["154901656494936064"],
            "artistNames": ["Sleep Token"]
        }))
        .unwrap();

        let candidate = tracks_track_to_candidate(track).unwrap();
        let token: CandidateToken = serde_json::from_str(&candidate.provider_token).unwrap();

        assert_eq!(token.track_id, "181548135879852032");
        assert_eq!(token.source, CandidateSource::Tracks);
    }

    #[test]
    fn collapses_duplicate_catalog_entries_for_the_same_recording() {
        let tracks = serde_json::from_value::<TracksSearchData>(serde_json::json!({
            "tracks": [
                {
                    "id": "175188016124649472",
                    "trackId": "175188016124649472",
                    "title": "Lagi Na Lang",
                    "artistNames": ["Sugarcane"],
                    "duration": 249725,
                    "isrc": "PHW012400262",
                    "recordingId": "212456142083723264",
                    "releaseId": "169623492306694144",
                    "playable": true
                },
                {
                    "id": "252346243236810752",
                    "trackId": "252346243236810752",
                    "title": "Lagi Na Lang",
                    "artistNames": ["Sugarcane"],
                    "duration": 249725,
                    "isrc": "PHW012400262",
                    "recordingId": "212456142083723264",
                    "releaseId": "252346207702302720",
                    "playable": true
                }
            ]
        }))
        .unwrap()
        .tracks;

        let candidates = tracks_to_candidates(&query(), tracks).unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title.as_deref(), Some("Lagi Na Lang"));
    }

    #[test]
    fn legacy_candidate_tokens_remain_backward_compatible() {
        let token: CandidateToken = serde_json::from_str(r#"{"trackId":"123"}"#).unwrap();
        assert_eq!(token.track_id, "123");
        assert_eq!(token.source, CandidateSource::LegacyApi);
    }

    #[test]
    fn builds_tracks_streamer_url_without_allowing_track_id_path_injection() {
        let url = tracks_stream_url("https://tracks.monochrome.st", "123/../../oops").unwrap();
        assert_eq!(
            url.as_str(),
            "https://tracks.monochrome.st/track/123%2F..%2F..%2Foops"
        );
    }

    #[test]
    fn invalid_track_service_json_reports_content_type_and_body_preview() {
        let error = serde_json::from_slice::<Value>(b"<html>upstream error</html>").unwrap_err();
        let message = invalid_json_message(
            Some("text/html; charset=utf-8"),
            b"<html>\n  upstream error\n</html>",
            &error,
        );

        assert!(message.contains("text/html; charset=utf-8"));
        assert!(message.contains("<html> upstream error </html>"));
    }

    #[test]
    fn response_body_read_errors_are_reported_as_transport_failures() {
        let error = download_read_error(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "request or response body error",
        ));

        assert_eq!(error.code, "downloadTransportFailed");
        assert!(error.message.contains("response body"));
        assert!(error.retryable);
    }

    #[test]
    fn transient_download_http_statuses_are_retryable() {
        assert!(retryable_download_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(retryable_download_status(StatusCode::BAD_GATEWAY));
        assert!(retryable_download_status(
            StatusCode::from_u16(521).unwrap()
        ));
        assert!(!retryable_download_status(StatusCode::NOT_FOUND));
    }

    #[test]
    fn direct_download_failures_fall_back_even_when_http_error_is_not_retryable() {
        let forbidden = ProviderError::new(
            "downloadFailed",
            "Monochrome download returned HTTP 403 Forbidden.",
            false,
        );
        let local_io = ProviderError::new("downloadIoFailed", "disk full", true);

        assert!(should_try_legacy_download_fallback(&forbidden));
        assert!(!should_try_legacy_download_fallback(&local_io));
    }

    #[test]
    fn playback_outage_classifier_ignores_track_specific_or_local_failures() {
        let transport = ProviderError::new("downloadTransportFailed", "body error", true);
        let forbidden = ProviderError::new(
            "downloadFailed",
            "Monochrome download returned HTTP 403 Forbidden.",
            false,
        );
        let not_found = ProviderError::new(
            "downloadFailed",
            "Monochrome download returned HTTP 404 Not Found.",
            false,
        );
        let local_io = ProviderError::new("downloadIoFailed", "disk full", true);

        assert!(is_playback_outage_error(&transport));
        assert!(is_playback_outage_error(&forbidden));
        assert!(!is_playback_outage_error(&not_found));
        assert!(!is_playback_outage_error(&local_io));
    }

    #[test]
    fn playback_outage_cooldown_short_circuits_provider_health() {
        let provider = MonochromeProvider::with_client_and_urls(
            Client::new(),
            "https://example.invalid".into(),
            Vec::new(),
        );
        provider.mark_playback_degraded(&ProviderError::new(
            "downloadTransportFailed",
            "body error",
            true,
        ));

        let health = provider.health().unwrap();

        assert!(!health.available);
        assert!(health.message.unwrap().contains("next provider"));
    }

    #[test]
    fn fresh_session_clears_monochrome_playback_cooldown() {
        let provider = MonochromeProvider::with_client_and_urls(
            Client::new(),
            "https://example.invalid".into(),
            Vec::new(),
        );
        provider.mark_playback_degraded(&ProviderError::new(
            "downloadTransportFailed",
            "body error",
            true,
        ));
        assert!(provider.playback_cooldown_active());

        provider.reset_session_state();

        assert!(!provider.playback_cooldown_active());
    }

    #[test]
    fn legacy_download_fallback_requires_compatible_recording_identity() {
        let matching = AcquisitionCandidate {
            provider: Some("monochrome".into()),
            provider_token: r#"{"trackId":"legacy-1"}"#.into(),
            source: None,
            file_name: Some("Introduction to the Snow.flac".into()),
            title: Some("Introduction to the Snow".into()),
            artists: vec!["Miracle Musical".into()],
            album: Some("Hawaii: Part II".into()),
            duration_ms: Some(101_000),
            format: Some("FLAC".into()),
            size_bytes: None,
            bitrate_kbps: None,
            sample_rate_hz: None,
            bit_depth: None,
            confidence: None,
            isrc: Some("QM4DW1700001".into()),
            recording_id: None,
            release_id: None,
        };
        let mut wrong_isrc = matching.clone();
        wrong_isrc.isrc = Some("WRONG0000000".into());

        assert!(legacy_fallback_candidate_matches(&query(), &matching));
        assert!(!legacy_fallback_candidate_matches(&query(), &wrong_isrc));
    }

    #[test]
    fn download_retry_backoff_grows_and_caps() {
        assert_eq!(download_retry_delay(0), Duration::from_millis(500));
        assert_eq!(download_retry_delay(1), Duration::from_secs(1));
        assert_eq!(download_retry_delay(2), Duration::from_secs(2));
        assert_eq!(download_retry_delay(4), Duration::from_secs(8));
        assert_eq!(download_retry_delay(8), Duration::from_secs(8));
    }

    #[test]
    fn track_candidates_collapse_same_isrc_across_different_recording_ids() {
        let query = TrackQuery {
            library_track_id: 1,
            title: "That's What I Get".into(),
            artists: vec!["Orange Dog Club".into()],
            album: Some("Album".into()),
            duration_ms: Some(298_000),
            isrc: Some("QZWFE2361369".into()),
            version_kind: None,
            version_detail: None,
        };
        let first: TracksSearchTrack = serde_json::from_value(serde_json::json!({
            "trackId": "track-1",
            "title": "That's What I Get",
            "duration": 298000,
            "playable": true,
            "isrc": "QZWFE2361369",
            "recordingId": "recording-1",
            "artistNames": ["Orange Dog Club"],
            "releaseId": "release-1",
            "albumTitle": "Album"
        }))
        .unwrap();
        let second: TracksSearchTrack = serde_json::from_value(serde_json::json!({
            "trackId": "track-2",
            "title": "That's What I Get",
            "duration": 298000,
            "playable": true,
            "isrc": "QZWFE2361369",
            "recordingId": "recording-2",
            "artistNames": ["Orange Dog Club"],
            "releaseId": "release-2",
            "albumTitle": "Album"
        }))
        .unwrap();

        let candidates = tracks_to_candidates(&query, vec![first, second]).unwrap();

        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn parses_total_size_from_partial_content_range() {
        assert_eq!(
            content_range_total("bytes 1048576-31149347/31149348"),
            Some(31_149_348)
        );
        assert_eq!(content_range_total("bytes */31149348"), Some(31_149_348));
        assert_eq!(content_range_total("garbage"), None);
    }
}
