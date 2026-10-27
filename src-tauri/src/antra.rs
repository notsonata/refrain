use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use reqwest::{
    StatusCode,
    blocking::{Client, Response},
    header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    acquisition::{AcquisitionProvider, ProviderError},
    domain::{
        AcquisitionCandidate, AcquisitionRequest, ProviderHealth, ProviderJob, ProviderJobStatus,
        TrackQuery,
    },
    security::{AntraTokenStore, KeyringAntraTokenStore},
};

const ANTRA_PROVIDER_ID: &str = "antra";
const ANTRA_AUTH_BASE_URL: &str = "https://antra.hoshi.cfd";
const SEARCH_TIMEOUT: Duration = Duration::from_secs(12);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const ENDPOINT_CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_DOWNLOAD_ATTEMPTS: usize = 4;
const REMUX_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamFormat {
    Flac,
    Mp4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum MirrorSource {
    Tidal,
    Qobuz,
}

impl MirrorSource {
    fn label(self) -> &'static str {
        match self {
            Self::Tidal => "Tidal",
            Self::Qobuz => "Qobuz",
        }
    }

    fn endpoint(self, mirrors: &MirrorEndpoints) -> Option<&str> {
        match self {
            Self::Tidal => mirrors.tidal.as_deref(),
            Self::Qobuz => mirrors.qobuz.as_deref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateToken {
    source: MirrorSource,
    track_id: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct EndpointManifest {
    #[serde(default)]
    mirrors: MirrorEndpoints,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct MirrorEndpoints {
    #[serde(default)]
    tidal: Option<String>,
    #[serde(default)]
    qobuz: Option<String>,
}

impl MirrorEndpoints {
    fn normalize(&mut self) {
        for endpoint in [&mut self.tidal, &mut self.qobuz] {
            if let Some(value) = endpoint {
                let trimmed = value.trim().trim_end_matches('/').to_owned();
                if trimmed.is_empty() {
                    *endpoint = None;
                } else {
                    *value = trimmed;
                }
            }
        }
    }

    fn has_lossless_source(&self) -> bool {
        self.tidal.is_some() || self.qobuz.is_some()
    }
}

#[derive(Debug, Clone)]
struct CachedEndpoints {
    value: EndpointManifest,
    fetched_at: Instant,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AntraAccountStatus {
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AntraDeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    pub expires_in: i64,
    pub interval: i64,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_url: String,
    expires_in: i64,
    interval: i64,
    #[serde(default)]
    error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AntraDeviceLoginStatus {
    pub status: String,
    pub configured: bool,
    pub username: Option<String>,
    pub tier: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeviceTokenResponse {
    status: String,
    #[serde(default)]
    token: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    tier: String,
    #[serde(default)]
    error: String,
}

#[derive(Clone)]
pub struct AntraProvider {
    inner: Arc<AntraInner>,
}

struct AntraInner {
    http: Client,
    auth_base_url: String,
    tokens: Arc<dyn AntraTokenStore>,
    token_cache: Mutex<Option<Option<String>>>,
    endpoints: Mutex<Option<CachedEndpoints>>,
    jobs: Mutex<HashMap<String, Arc<AntraJob>>>,
    next_job_id: AtomicU64,
    download_gate: Mutex<()>,
}

struct AntraJob {
    status: Mutex<ProviderJobStatus>,
    cancelled: AtomicBool,
    bytes_transferred: AtomicI64,
    total_bytes: AtomicI64,
}

impl AntraJob {
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
                    "Antra download state is unavailable.",
                    true,
                )
            })
    }
}

impl AntraProvider {
    pub fn new() -> Result<Self, ProviderError> {
        Self::with_parts(
            ANTRA_AUTH_BASE_URL.to_owned(),
            Arc::new(KeyringAntraTokenStore),
        )
    }

    fn with_parts(
        auth_base_url: String,
        tokens: Arc<dyn AntraTokenStore>,
    ) -> Result<Self, ProviderError> {
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(provider_transport_error)?;
        Ok(Self {
            inner: Arc::new(AntraInner {
                http,
                auth_base_url: auth_base_url.trim_end_matches('/').to_owned(),
                tokens,
                token_cache: Mutex::new(None),
                endpoints: Mutex::new(None),
                jobs: Mutex::new(HashMap::new()),
                next_job_id: AtomicU64::new(1),
                download_gate: Mutex::new(()),
            }),
        })
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

    pub fn account_status(&self) -> Result<AntraAccountStatus, ProviderError> {
        Ok(AntraAccountStatus {
            configured: self.load_token()?.is_some(),
        })
    }

    pub fn start_device_login(&self) -> Result<AntraDeviceCode, ProviderError> {
        let response = self
            .inner
            .http
            .post(format!("{}/api/device/code", self.inner.auth_base_url))
            .header(CONTENT_TYPE, "application/json")
            .json(&json!({}))
            .timeout(Duration::from_secs(20))
            .send()
            .map_err(provider_transport_error)?;
        if response.status() != StatusCode::OK {
            return Err(response_error("deviceLoginFailed", response, true));
        }
        let body: DeviceCodeResponse = response.json().map_err(provider_protocol_error)?;
        if !body.error.trim().is_empty() {
            return Err(ProviderError::new("deviceLoginFailed", body.error, true));
        }
        Ok(AntraDeviceCode {
            device_code: body.device_code,
            user_code: body.user_code,
            verification_url: body.verification_url,
            expires_in: body.expires_in,
            interval: body.interval.max(1),
        })
    }

    pub fn poll_device_login(
        &self,
        device_code: &str,
    ) -> Result<AntraDeviceLoginStatus, ProviderError> {
        let response = self
            .inner
            .http
            .post(format!("{}/api/device/token", self.inner.auth_base_url))
            .header(CONTENT_TYPE, "application/json")
            .json(&json!({
                "device_code": device_code,
                "device_name": "Refrain"
            }))
            .timeout(Duration::from_secs(20))
            .send()
            .map_err(provider_transport_error)?;

        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return Ok(AntraDeviceLoginStatus {
                status: "pending".into(),
                configured: false,
                username: None,
                tier: None,
                error: None,
            });
        }
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(AntraDeviceLoginStatus {
                status: "error".into(),
                configured: false,
                username: None,
                tier: None,
                error: Some("Login request expired. Start Antra sign-in again.".into()),
            });
        }
        if response.status() != StatusCode::OK {
            return Err(response_error("deviceLoginFailed", response, true));
        }

        let body: DeviceTokenResponse = response.json().map_err(provider_protocol_error)?;
        let mut configured = self.load_token()?.is_some();
        if body.status == "approved" && !body.token.trim().is_empty() {
            self.inner
                .tokens
                .set_token(body.token.trim())
                .map_err(credential_error)?;
            if let Ok(mut cached) = self.inner.token_cache.lock() {
                *cached = Some(Some(body.token.trim().to_owned()));
            }
            self.invalidate_endpoints();
            configured = true;
        }
        Ok(AntraDeviceLoginStatus {
            status: body.status,
            configured,
            username: nonempty(body.username),
            tier: nonempty(body.tier),
            error: nonempty(body.error),
        })
    }

    pub fn sign_out(&self) -> Result<AntraAccountStatus, ProviderError> {
        self.inner.tokens.clear_token().map_err(credential_error)?;
        if let Ok(mut cached) = self.inner.token_cache.lock() {
            *cached = Some(None);
        }
        self.invalidate_endpoints();
        Ok(AntraAccountStatus { configured: false })
    }

    fn load_token(&self) -> Result<Option<String>, ProviderError> {
        if let Ok(cached) = self.inner.token_cache.lock()
            && let Some(token) = cached.as_ref()
        {
            return Ok(token.clone());
        }
        let token = self
            .inner
            .tokens
            .get_token()
            .map_err(credential_error)
            .map(|token| token.and_then(nonempty))?;
        if let Ok(mut cached) = self.inner.token_cache.lock() {
            *cached = Some(token.clone());
        }
        Ok(token)
    }

    fn required_token(&self) -> Result<String, ProviderError> {
        self.load_token()?.ok_or_else(|| {
            ProviderError::new(
                "antraAuthRequired",
                "Sign in to Antra in Settings before using the Antra provider.",
                false,
            )
        })
    }

    fn invalidate_endpoints(&self) {
        if let Ok(mut endpoints) = self.inner.endpoints.lock() {
            *endpoints = None;
        }
    }

    fn endpoint_manifest(&self) -> Result<EndpointManifest, ProviderError> {
        if let Ok(endpoints) = self.inner.endpoints.lock()
            && let Some(cached) = endpoints.as_ref()
            && cached.fetched_at.elapsed() < ENDPOINT_CACHE_TTL
        {
            return Ok(cached.value.clone());
        }

        let token = self.required_token()?;
        let response = self
            .inner
            .http
            .get(format!(
                "{}/api/desktop/endpoints",
                self.inner.auth_base_url
            ))
            .header("X-API-Key", token)
            .header(ACCEPT, "application/json")
            .timeout(Duration::from_secs(15))
            .send()
            .map_err(provider_transport_error)?;
        match response.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                return Err(ProviderError::new(
                    "antraAuthRequired",
                    "Antra rejected the saved device token. Sign in again in Settings.",
                    false,
                ));
            }
            StatusCode::TOO_MANY_REQUESTS => {
                return Err(ProviderError::new(
                    "providerRateLimited",
                    "Antra rate limited the endpoint request.",
                    true,
                ));
            }
            status if !status.is_success() => {
                return Err(response_error("providerOffline", response, true));
            }
            _ => {}
        }

        let mut manifest: EndpointManifest = response.json().map_err(provider_protocol_error)?;
        manifest.mirrors.normalize();
        if !manifest.mirrors.has_lossless_source() {
            return Err(ProviderError::new(
                "providerUnavailable",
                "Antra did not return a Tidal or Qobuz lossless mirror.",
                true,
            ));
        }
        if let Ok(mut endpoints) = self.inner.endpoints.lock() {
            *endpoints = Some(CachedEndpoints {
                value: manifest.clone(),
                fetched_at: Instant::now(),
            });
        }
        Ok(manifest)
    }

    fn search_source(
        &self,
        source: MirrorSource,
        base: &str,
        token: &str,
        query: &TrackQuery,
    ) -> Result<Vec<AcquisitionCandidate>, ProviderError> {
        if let Some(isrc) = query
            .isrc
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let url = format!("{}/api/search/isrc/{}", base.trim_end_matches('/'), isrc);
            let response = self
                .inner
                .http
                .get(url)
                .header("X-API-Key", token)
                .header(ACCEPT, "application/json")
                .timeout(SEARCH_TIMEOUT)
                .send()
                .map_err(provider_transport_error)?;
            match response.status() {
                StatusCode::OK => {
                    let payload: Value = response.json().map_err(provider_protocol_error)?;
                    let mut candidate = exact_candidate_from_payload(source, query, &payload)?;
                    if source == MirrorSource::Tidal
                        && candidate.duration_ms.is_none()
                        && query.duration_ms.is_some()
                    {
                        self.enrich_tidal_duration(base, token, &mut candidate)?;
                    }
                    return Ok(vec![candidate]);
                }
                StatusCode::NOT_FOUND => {}
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                    return Err(ProviderError::new(
                        "antraAuthRequired",
                        "Antra rejected the saved device token. Sign in again in Settings.",
                        false,
                    ));
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    return Err(ProviderError::new(
                        "providerRateLimited",
                        format!("Antra {} search was rate limited.", source.label()),
                        true,
                    ));
                }
                status if status.is_server_error() => {
                    return Err(response_error("providerOffline", response, true));
                }
                _ => {}
            }
        }

        self.text_search(source, base, token, query)
    }

    fn enrich_tidal_duration(
        &self,
        base: &str,
        token: &str,
        candidate: &mut AcquisitionCandidate,
    ) -> Result<(), ProviderError> {
        let candidate_token: CandidateToken =
            serde_json::from_str(&candidate.provider_token).map_err(provider_protocol_error)?;
        validate_track_id(&candidate_token.track_id)?;
        let response = self
            .inner
            .http
            .get(format!(
                "{}/api/meta/track/{}",
                base.trim_end_matches('/'),
                candidate_token.track_id
            ))
            .header("X-API-Key", token)
            .header(ACCEPT, "application/json")
            .timeout(SEARCH_TIMEOUT)
            .send()
            .map_err(provider_transport_error)?;
        if response.status() != StatusCode::OK {
            return Ok(());
        }
        let payload: Value = response.json().map_err(provider_protocol_error)?;
        candidate.duration_ms = payload
            .get("duration_ms")
            .and_then(Value::as_i64)
            .or_else(|| {
                payload
                    .get("duration")
                    .and_then(Value::as_i64)
                    .map(|value| value * 1_000)
            });
        Ok(())
    }

    fn text_search(
        &self,
        source: MirrorSource,
        base: &str,
        token: &str,
        query: &TrackQuery,
    ) -> Result<Vec<AcquisitionCandidate>, ProviderError> {
        let mut request = match source {
            MirrorSource::Qobuz => self.inner.http.get(url_with_query(
                base,
                "/api/search",
                &[
                    ("title", query.title.as_str()),
                    (
                        "artist",
                        query.artists.first().map(String::as_str).unwrap_or(""),
                    ),
                    ("limit", "5"),
                ],
            )?),
            MirrorSource::Tidal => {
                let search = format!(
                    "{} {}",
                    query.title,
                    query.artists.first().map(String::as_str).unwrap_or("")
                );
                self.inner
                    .http
                    .get(url_with_query(base, "/search/", &[("s", search.trim())])?)
            }
        };
        request = request
            .header("X-API-Key", token)
            .header(ACCEPT, "application/json")
            .timeout(SEARCH_TIMEOUT);
        let response = request.send().map_err(provider_transport_error)?;
        match response.status() {
            StatusCode::OK => {}
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                return Err(ProviderError::new(
                    "antraAuthRequired",
                    "Antra rejected the saved device token. Sign in again in Settings.",
                    false,
                ));
            }
            StatusCode::TOO_MANY_REQUESTS => {
                return Err(ProviderError::new(
                    "providerRateLimited",
                    format!("Antra {} search was rate limited.", source.label()),
                    true,
                ));
            }
            status if status.is_server_error() => {
                return Err(response_error("providerOffline", response, true));
            }
            _ => return Ok(Vec::new()),
        }
        let payload: Value = response.json().map_err(provider_protocol_error)?;
        Ok(text_candidates_from_payload(source, query, &payload))
    }

    fn start_download(&self, token: CandidateToken, output_path: String, job: Arc<AntraJob>) {
        let provider = self.clone();
        thread::spawn(move || {
            let _gate = match provider.inner.download_gate.lock() {
                Ok(gate) => gate,
                Err(_) => {
                    job.set_status(ProviderJobStatus::Failed {
                        code: "providerStateUnavailable".into(),
                        message: "Antra download coordinator is unavailable.".into(),
                        retryable: true,
                    });
                    return;
                }
            };
            if job.cancelled.load(Ordering::Acquire) {
                job.set_status(ProviderJobStatus::Cancelled);
                return;
            }
            job.set_status(ProviderJobStatus::Running);
            match provider.download_with_retries(&token, Path::new(&output_path), &job) {
                Ok(()) if job.cancelled.load(Ordering::Acquire) => {
                    let _ = fs::remove_file(&output_path);
                    job.set_status(ProviderJobStatus::Cancelled)
                }
                Ok(()) => job.set_status(ProviderJobStatus::Succeeded {
                    output_paths: vec![output_path],
                }),
                Err(_) if job.cancelled.load(Ordering::Acquire) => {
                    let _ = fs::remove_file(&output_path);
                    job.set_status(ProviderJobStatus::Cancelled)
                }
                Err(error) => job.set_status(ProviderJobStatus::Failed {
                    code: error.code,
                    message: error.message,
                    retryable: error.retryable,
                }),
            }
        });
    }

    fn download_with_retries(
        &self,
        candidate: &CandidateToken,
        output_path: &Path,
        job: &AntraJob,
    ) -> Result<(), ProviderError> {
        let part_path = output_path.with_extension("flac.part");
        let mut last_error = None;
        for attempt in 0..MAX_DOWNLOAD_ATTEMPTS {
            if job.cancelled.load(Ordering::Acquire) {
                let _ = fs::remove_file(&part_path);
                return Err(cancelled_error());
            }
            match self.download_once(candidate, &part_path, job) {
                Ok(()) if job.cancelled.load(Ordering::Acquire) => {
                    let _ = fs::remove_file(&part_path);
                    return Err(cancelled_error());
                }
                Ok(()) => {
                    fs::rename(&part_path, output_path).map_err(io_provider_error)?;
                    if job.cancelled.load(Ordering::Acquire) {
                        let _ = fs::remove_file(output_path);
                        return Err(cancelled_error());
                    }
                    return Ok(());
                }
                Err(error) => {
                    let retry =
                        should_retry_same_candidate(&error) && attempt + 1 < MAX_DOWNLOAD_ATTEMPTS;
                    let _ = fs::remove_file(&part_path);
                    last_error = Some(error);
                    if retry {
                        thread::sleep(download_retry_delay(attempt));
                        continue;
                    }
                    break;
                }
            }
        }
        Err(last_error.unwrap_or_else(|| {
            ProviderError::new(
                "antraDownloadFailed",
                "Antra did not complete the download.",
                true,
            )
        }))
    }

    fn download_once(
        &self,
        candidate: &CandidateToken,
        part_path: &Path,
        job: &AntraJob,
    ) -> Result<(), ProviderError> {
        validate_track_id(&candidate.track_id)?;
        let manifest = self.endpoint_manifest()?;
        let base = candidate
            .source
            .endpoint(&manifest.mirrors)
            .ok_or_else(|| {
                ProviderError::new(
                    "providerUnavailable",
                    format!(
                        "Antra has no {} mirror available.",
                        candidate.source.label()
                    ),
                    true,
                )
            })?;
        let token = self.required_token()?;
        let response = self
            .inner
            .http
            .get(format!(
                "{}/api/stream/{}",
                base.trim_end_matches('/'),
                candidate.track_id
            ))
            .header("X-API-Key", token)
            .header(
                ACCEPT,
                "audio/flac,application/octet-stream;q=0.9,*/*;q=0.1",
            )
            .timeout(DOWNLOAD_TIMEOUT)
            .send()
            .map_err(|error| {
                ProviderError::new(
                    "downloadTransportFailed",
                    format!("Antra download request failed: {error}"),
                    true,
                )
            })?;
        let response = ensure_download_response(response, candidate.source)?;
        let format = stream_flac_to_part(response, part_path, job, candidate.source)?;
        if job.cancelled.load(Ordering::Acquire) {
            return Err(cancelled_error());
        }
        if format == StreamFormat::Mp4 {
            remux_mp4_flac(part_path, job, candidate.source)?;
        }
        Ok(())
    }
}

impl AcquisitionProvider for AntraProvider {
    fn id(&self) -> &'static str {
        ANTRA_PROVIDER_ID
    }

    fn health(&self) -> Result<ProviderHealth, ProviderError> {
        if self.load_token()?.is_none() {
            return Ok(ProviderHealth {
                available: false,
                version: None,
                message: Some("Sign in to Antra in Settings to enable this provider.".into()),
            });
        }
        match self.endpoint_manifest() {
            Ok(manifest) => {
                let mut sources = Vec::new();
                if manifest.mirrors.tidal.is_some() {
                    sources.push("Tidal");
                }
                if manifest.mirrors.qobuz.is_some() {
                    sources.push("Qobuz");
                }
                Ok(ProviderHealth {
                    available: !sources.is_empty(),
                    version: None,
                    message: Some(format!(
                        "Antra authenticated; lossless mirrors available: {}.",
                        sources.join(", ")
                    )),
                })
            }
            Err(error) if error.code == "antraAuthRequired" => Ok(ProviderHealth {
                available: false,
                version: None,
                message: Some(error.message),
            }),
            Err(error) => Err(error),
        }
    }

    fn search(&self, query: &TrackQuery) -> Result<Vec<AcquisitionCandidate>, ProviderError> {
        let manifest = self.endpoint_manifest()?;
        let token = self.required_token()?;
        let mut candidates = Vec::new();
        let mut last_error = None;
        for source in [MirrorSource::Tidal, MirrorSource::Qobuz] {
            let Some(base) = source.endpoint(&manifest.mirrors) else {
                continue;
            };
            match self.search_source(source, base, &token, query) {
                Ok(mut source_candidates) => candidates.append(&mut source_candidates),
                Err(error) => last_error = Some(error),
            }
        }
        if candidates.is_empty()
            && let Some(error) = last_error
        {
            return Err(error);
        }
        Ok(candidates)
    }

    fn acquire(&self, request: &AcquisitionRequest) -> Result<ProviderJob, ProviderError> {
        let token: CandidateToken = serde_json::from_str(&request.candidate.provider_token)
            .map_err(|error| {
                ProviderError::new(
                    "invalidCandidateToken",
                    format!("Antra candidate token is invalid: {error}"),
                    false,
                )
            })?;
        validate_track_id(&token.track_id)?;
        fs::create_dir_all(&request.staging_path).map_err(io_provider_error)?;
        let provider_job_id = format!(
            "antra-{}-{}",
            request.library_track_id,
            self.inner.next_job_id.fetch_add(1, Ordering::Relaxed)
        );
        let job = Arc::new(AntraJob::new());
        self.inner
            .jobs
            .lock()
            .map_err(|_| {
                ProviderError::new(
                    "providerStateUnavailable",
                    "Antra download state is unavailable.",
                    true,
                )
            })?
            .insert(provider_job_id.clone(), Arc::clone(&job));
        let output_path = Path::new(&request.staging_path)
            .join(format!(
                "antra-{}-{}.flac",
                token.source.label().to_lowercase(),
                token.track_id
            ))
            .to_string_lossy()
            .to_string();
        self.start_download(token, output_path, job);
        Ok(ProviderJob { provider_job_id })
    }

    fn status(&self, provider_job_id: &str) -> Result<ProviderJobStatus, ProviderError> {
        let jobs = self.inner.jobs.lock().map_err(|_| {
            ProviderError::new(
                "providerStateUnavailable",
                "Antra download state is unavailable.",
                true,
            )
        })?;
        jobs.get(provider_job_id)
            .ok_or_else(|| {
                ProviderError::new(
                    "providerJobMissing",
                    "Antra download state is no longer available.",
                    true,
                )
            })?
            .status()
    }

    fn cancel(&self, provider_job_id: &str) -> Result<(), ProviderError> {
        let jobs = self.inner.jobs.lock().map_err(|_| {
            ProviderError::new(
                "providerStateUnavailable",
                "Antra download state is unavailable.",
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

fn exact_candidate_from_payload(
    source: MirrorSource,
    query: &TrackQuery,
    payload: &Value,
) -> Result<AcquisitionCandidate, ProviderError> {
    let track_id =
        value_string(payload.get("track_id").or_else(|| payload.get("id"))).ok_or_else(|| {
            ProviderError::new(
                "providerProtocol",
                format!(
                    "Antra {} result did not include a track ID.",
                    source.label()
                ),
                true,
            )
        })?;
    let title = value_text(payload.get("title")).unwrap_or_else(|| query.title.clone());
    let artist = value_text(payload.get("artist"))
        .or_else(|| {
            payload
                .get("artist")
                .and_then(|artist| value_text(artist.get("name")))
        })
        .or_else(|| query.artists.first().cloned());
    let album = value_text(payload.get("album"))
        .or_else(|| {
            payload
                .get("album")
                .and_then(|album| value_text(album.get("title")))
        })
        .or_else(|| query.album.clone());
    let duration_ms = payload
        .get("duration_ms")
        .and_then(Value::as_i64)
        .or_else(|| {
            payload
                .get("duration")
                .and_then(Value::as_i64)
                .map(|value| value * 1_000)
        });
    let (bit_depth, sample_rate_hz) = candidate_quality_from_payload(source, payload);
    candidate_for(
        source,
        track_id,
        title,
        artist.into_iter().collect(),
        album,
        duration_ms,
        bit_depth,
        sample_rate_hz,
        value_text(payload.get("isrc")).or_else(|| query.isrc.clone()),
    )
}

fn text_candidates_from_payload(
    source: MirrorSource,
    query: &TrackQuery,
    payload: &Value,
) -> Vec<AcquisitionCandidate> {
    let items = match source {
        MirrorSource::Qobuz => payload.get("results").and_then(Value::as_array),
        MirrorSource::Tidal => payload
            .get("data")
            .and_then(|data| data.get("items"))
            .and_then(Value::as_array),
    };
    let Some(items) = items else {
        return Vec::new();
    };
    items
        .iter()
        .take(5)
        .filter_map(|item| {
            let track_id = value_string(item.get("track_id").or_else(|| item.get("id")))?;
            let title = value_text(item.get("title"))?;
            let artist = match source {
                MirrorSource::Qobuz => value_text(item.get("artist")),
                MirrorSource::Tidal => item
                    .get("artist")
                    .and_then(|artist| value_text(artist.get("name"))),
            }
            .or_else(|| query.artists.first().cloned());
            let album = match source {
                MirrorSource::Qobuz => value_text(item.get("album")),
                MirrorSource::Tidal => item
                    .get("album")
                    .and_then(|album| value_text(album.get("title"))),
            };
            let duration_ms = item.get("duration_ms").and_then(Value::as_i64).or_else(|| {
                item.get("duration")
                    .and_then(Value::as_i64)
                    .map(|value| value * 1_000)
            });
            let (bit_depth, sample_rate) = candidate_quality_from_payload(source, item);
            candidate_for(
                source,
                track_id,
                title,
                artist.into_iter().collect(),
                album,
                duration_ms,
                bit_depth,
                sample_rate,
                value_text(item.get("isrc")),
            )
            .ok()
        })
        .collect()
}

fn candidate_quality_from_payload(
    source: MirrorSource,
    payload: &Value,
) -> (Option<i64>, Option<i64>) {
    let bit_depth = payload.get("bitDepth").and_then(Value::as_i64);
    let sample_rate = payload.get("sampleRate").and_then(Value::as_i64);
    if source != MirrorSource::Tidal {
        return (bit_depth, sample_rate);
    }
    if tidal_item_is_hi_res(payload) {
        return (bit_depth.or(Some(24)), sample_rate.or(Some(96_000)));
    }
    if tidal_item_is_lossless(payload) {
        return (bit_depth.or(Some(16)), sample_rate.or(Some(44_100)));
    }
    (bit_depth, sample_rate)
}

#[allow(clippy::too_many_arguments)]
fn candidate_for(
    source: MirrorSource,
    track_id: String,
    title: String,
    artists: Vec<String>,
    album: Option<String>,
    duration_ms: Option<i64>,
    bit_depth: Option<i64>,
    sample_rate_hz: Option<i64>,
    isrc: Option<String>,
) -> Result<AcquisitionCandidate, ProviderError> {
    let provider_token = serde_json::to_string(&CandidateToken {
        source,
        track_id: track_id.clone(),
    })
    .map_err(provider_protocol_error)?;
    Ok(AcquisitionCandidate {
        provider: Some(ANTRA_PROVIDER_ID.into()),
        provider_token,
        source: Some(source.label().into()),
        file_name: Some(format!("{title}.flac")),
        title: Some(title),
        artists,
        album,
        duration_ms,
        format: Some("FLAC".into()),
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

fn tidal_item_is_hi_res(item: &Value) -> bool {
    let quality = value_text(item.get("audioQuality"))
        .unwrap_or_default()
        .to_ascii_uppercase();
    if quality.contains("HI_RES") || quality.contains("HIRES") {
        return true;
    }
    item.get("mediaMetadata")
        .and_then(|metadata| metadata.get("tags"))
        .and_then(Value::as_array)
        .is_some_and(|tags| {
            tags.iter().any(|tag| {
                value_text(Some(tag))
                    .unwrap_or_default()
                    .to_ascii_uppercase()
                    .contains("HI_RES")
            })
        })
}

fn tidal_item_is_lossless(item: &Value) -> bool {
    value_text(item.get("audioQuality"))
        .unwrap_or_default()
        .to_ascii_uppercase()
        .contains("LOSSLESS")
}

fn ensure_download_response(
    response: Response,
    source: MirrorSource,
) -> Result<Response, ProviderError> {
    match response.status() {
        status if status.is_success() => Ok(response),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::new(
            "antraAuthRequired",
            "Antra rejected the saved device token. Sign in again in Settings.",
            false,
        )),
        StatusCode::TOO_MANY_REQUESTS => Err(ProviderError::new(
            "providerRateLimited",
            format!("Antra {} download was rate limited.", source.label()),
            true,
        )),
        StatusCode::CONFLICT => Err(ProviderError::new(
            "losslessUnavailable",
            format!(
                "Antra {} did not have a compatible lossless stream.",
                source.label()
            ),
            false,
        )),
        status if status.is_server_error() => {
            Err(response_error("downloadTransportFailed", response, true))
        }
        _ => Err(response_error("antraDownloadFailed", response, false)),
    }
}

fn stream_flac_to_part(
    mut response: Response,
    part_path: &Path,
    job: &AntraJob,
    source: MirrorSource,
) -> Result<StreamFormat, ProviderError> {
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_owned();
    let expected = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    job.total_bytes.store(expected, Ordering::Release);
    job.bytes_transferred.store(0, Ordering::Release);

    let mut output = fs::File::create(part_path).map_err(io_provider_error)?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut stream_format = None;
    let mut prefix = [0_u8; 8];
    let mut prefix_len = 0;
    loop {
        if job.cancelled.load(Ordering::Acquire) {
            return Err(cancelled_error());
        }
        let read = response.read(&mut buffer).map_err(|error| {
            ProviderError::new(
                "downloadTransportFailed",
                format!("Antra download stream was interrupted: {error}"),
                true,
            )
        })?;
        if read == 0 {
            break;
        }
        if stream_format.is_none() {
            let take = (prefix.len() - prefix_len).min(read);
            prefix[prefix_len..prefix_len + take].copy_from_slice(&buffer[..take]);
            prefix_len += take;
            if let Some(format) = classify_stream_format(&prefix[..prefix_len]) {
                stream_format = Some(format);
            } else if prefix_len == prefix.len() {
                tracing::warn!(
                    source = source.label(),
                    content_type,
                    "Antra mirror returned a non-FLAC payload"
                );
                return Err(ProviderError::new(
                    "antraInvalidStream",
                    format!(
                        "Antra {} returned a non-FLAC stream ({content_type}).",
                        source.label()
                    ),
                    true,
                ));
            }
        }
        output
            .write_all(&buffer[..read])
            .map_err(io_provider_error)?;
        job.bytes_transferred
            .fetch_add(i64::try_from(read).unwrap_or(i64::MAX), Ordering::AcqRel);
    }
    output.flush().map_err(io_provider_error)?;
    if job.cancelled.load(Ordering::Acquire) {
        return Err(cancelled_error());
    }
    if stream_format.is_none() {
        return Err(ProviderError::new(
            "antraDownloadFailed",
            "Antra returned an empty or unrecognized audio stream.",
            true,
        ));
    }
    let transferred = job.bytes_transferred.load(Ordering::Acquire);
    if expected > 0 && transferred < expected {
        return Err(ProviderError::new(
            "downloadTransportFailed",
            format!("Antra download ended early after {transferred} of {expected} bytes."),
            true,
        ));
    }
    Ok(stream_format.expect("stream format set after first nonempty chunk"))
}

fn classify_stream_format(prefix: &[u8]) -> Option<StreamFormat> {
    if prefix.starts_with(b"fLaC") {
        Some(StreamFormat::Flac)
    } else if prefix.len() >= 8 && &prefix[4..8] == b"ftyp" {
        Some(StreamFormat::Mp4)
    } else {
        None
    }
}

// Some Tidal lossless streams arrive in an MP4 container. Only copy an existing
// FLAC audio stream; ffmpeg's FLAC muxer rejects AAC and other codecs here.
fn remux_mp4_flac(
    part_path: &Path,
    job: &AntraJob,
    source: MirrorSource,
) -> Result<(), ProviderError> {
    if job.cancelled.load(Ordering::Acquire) {
        return Err(cancelled_error());
    }
    let output_path = part_path.with_extension("remux.part");
    let _ = fs::remove_file(&output_path);

    let mut child = Command::new(ffmpeg_command())
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(part_path)
        .args(["-map", "0:a:0", "-c:a", "copy", "-f", "flac"])
        .arg(&output_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            ProviderError::new(
                "antraInvalidStream",
                if error.kind() == std::io::ErrorKind::NotFound {
                    format!(
                        "Antra {} returned MP4 audio. Install ffmpeg to extract its FLAC stream.",
                        source.label()
                    )
                } else {
                    format!(
                        "Antra {} FLAC extraction could not start: {error}",
                        source.label()
                    )
                },
                true,
            )
        })?;

    let started = Instant::now();
    let result = loop {
        if job.cancelled.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(cancelled_error());
        }
        if started.elapsed() >= REMUX_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            break Err(ProviderError::new(
                "antraInvalidStream",
                format!("Antra {} FLAC extraction timed out.", source.label()),
                true,
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status.success()),
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(io_provider_error(error));
            }
        }
    };

    let result = result.and_then(|success| {
        if job.cancelled.load(Ordering::Acquire) {
            return Err(cancelled_error());
        }
        if !success {
            return Err(ProviderError::new(
                "antraInvalidStream",
                format!(
                    "Antra {} returned MP4 audio without an extractable FLAC stream.",
                    source.label()
                ),
                true,
            ));
        }
        let mut header = [0; 4];
        fs::File::open(&output_path)
            .and_then(|mut file| file.read_exact(&mut header))
            .map_err(io_provider_error)?;
        if header != *b"fLaC" {
            return Err(ProviderError::new(
                "antraInvalidStream",
                format!(
                    "Antra {} MP4 extraction did not produce FLAC.",
                    source.label()
                ),
                true,
            ));
        }
        // Windows does not replace an existing destination on rename.
        fs::remove_file(part_path).map_err(io_provider_error)?;
        fs::rename(&output_path, part_path).map_err(io_provider_error)
    });
    if result.is_err() {
        let _ = fs::remove_file(&output_path);
    }
    result
}

fn cancelled_error() -> ProviderError {
    ProviderError::new("antraCancelled", "Antra download was cancelled.", false)
}

fn ffmpeg_command() -> &'static str {
    if cfg!(target_os = "macos") && Path::new("/opt/homebrew/bin/ffmpeg").is_file() {
        "/opt/homebrew/bin/ffmpeg"
    } else if cfg!(target_os = "macos") && Path::new("/usr/local/bin/ffmpeg").is_file() {
        "/usr/local/bin/ffmpeg"
    } else {
        "ffmpeg"
    }
}

fn should_retry_same_candidate(error: &ProviderError) -> bool {
    error.retryable && error.code != "antraInvalidStream"
}

fn validate_track_id(value: &str) -> Result<(), ProviderError> {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Ok(())
    } else {
        Err(ProviderError::new(
            "invalidCandidateToken",
            "Antra candidate track ID is invalid.",
            false,
        ))
    }
}

fn value_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => nonempty(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::to_owned)
        .and_then(nonempty)
}

fn url_with_query(
    base: &str,
    path: &str,
    query: &[(&str, &str)],
) -> Result<url::Url, ProviderError> {
    let mut url = url::Url::parse(&format!("{}{}", base.trim_end_matches('/'), path))
        .map_err(provider_protocol_error)?;
    {
        let mut pairs = url.query_pairs_mut();
        for (name, value) in query {
            pairs.append_pair(name, value);
        }
    }
    Ok(url)
}

fn nonempty(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn download_retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(750 * (1_u64 << attempt.min(3)))
}

fn credential_error(error: crate::security::CredentialStoreError) -> ProviderError {
    ProviderError::new(
        "credentialStoreFailed",
        format!("Antra credential store failed: {error}"),
        false,
    )
}

fn provider_transport_error(error: reqwest::Error) -> ProviderError {
    ProviderError::new(
        "providerOffline",
        format!("Antra request failed: {error}"),
        true,
    )
}

fn provider_protocol_error(error: impl std::fmt::Display) -> ProviderError {
    ProviderError::new(
        "providerProtocol",
        format!("Antra returned an invalid response: {error}"),
        true,
    )
}

fn io_provider_error(error: std::io::Error) -> ProviderError {
    ProviderError::new(
        "stagingIoFailed",
        format!("Antra staging write failed: {error}"),
        true,
    )
}

fn response_error(code: &str, response: Response, retryable: bool) -> ProviderError {
    ProviderError::new(
        code,
        format!("Antra returned HTTP {}.", response.status()),
        retryable,
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        process::{Command, Stdio},
        sync::atomic::{AtomicU64, Ordering},
    };

    use serde_json::json;

    use crate::{acquisition::ProviderError, domain::TrackQuery};

    use super::{
        AntraJob, CandidateToken, EndpointManifest, MirrorSource, StreamFormat,
        classify_stream_format, exact_candidate_from_payload, ffmpeg_command, remux_mp4_flac,
        should_retry_same_candidate,
    };

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("refrain-antra-{name}-{}-{id}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn part(&self) -> PathBuf {
            self.0.join("track.flac.part")
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn make_mp4(path: &std::path::Path, codec: &str) -> bool {
        let output = Command::new(ffmpeg_command())
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=0.2",
                "-c:a",
                codec,
            ])
            .args(if codec == "flac" {
                &["-strict", "-2"][..]
            } else {
                &[]
            })
            .args(["-f", "mp4", "-y"])
            .arg(path)
            .stdin(Stdio::null())
            .status()
            .unwrap()
            .success();
        assert!(output, "ffmpeg could not generate {codec} MP4 test fixture");
        true
    }

    fn ffmpeg_available() -> bool {
        Command::new(ffmpeg_command())
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[test]
    fn stream_prefix_classifies_native_flac_mp4_and_rejects_partial_or_unknown() {
        assert_eq!(classify_stream_format(b"fLaC"), Some(StreamFormat::Flac));
        assert_eq!(
            classify_stream_format(b"\0\0\0\x18ftypisom"),
            Some(StreamFormat::Mp4)
        );
        for payload in [b"fLa".as_slice(), b"\0\0\0\x18fty", b"not audio"] {
            assert_eq!(classify_stream_format(payload), None);
        }
    }

    #[test]
    fn mp4_with_flac_remuxes_without_reencoding() {
        if !ffmpeg_available() {
            eprintln!("skipping MP4 remux test: ffmpeg is unavailable");
            return;
        }
        let dir = TestDir::new("flac-mp4");
        let part = dir.part();
        make_mp4(&part, "flac");
        assert_eq!(
            classify_stream_format(&fs::read(&part).unwrap()[..8]),
            Some(StreamFormat::Mp4)
        );

        remux_mp4_flac(&part, &AntraJob::new(), MirrorSource::Tidal).unwrap();

        assert!(fs::read(&part).unwrap().starts_with(b"fLaC"));
        assert!(!part.with_extension("remux.part").exists());
    }

    #[test]
    fn mp4_with_aac_is_rejected_and_temporary_output_cleaned() {
        if !ffmpeg_available() {
            eprintln!("skipping AAC remux test: ffmpeg is unavailable");
            return;
        }
        let dir = TestDir::new("aac-mp4");
        let part = dir.part();
        make_mp4(&part, "aac");

        let error = remux_mp4_flac(&part, &AntraJob::new(), MirrorSource::Tidal).unwrap_err();

        assert_eq!(error.code, "antraInvalidStream");
        assert!(part.exists());
        assert!(!part.with_extension("remux.part").exists());
    }

    #[test]
    fn truncated_mp4_is_rejected_and_cancellation_preserves_original() {
        if !ffmpeg_available() {
            eprintln!("skipping truncated MP4 remux test: ffmpeg is unavailable");
            return;
        }
        let dir = TestDir::new("truncated-mp4");
        let part = dir.part();
        let header = b"\0\0\0\x18ftypisom";
        fs::write(&part, header).unwrap();
        let error = remux_mp4_flac(&part, &AntraJob::new(), MirrorSource::Tidal).unwrap_err();
        assert_eq!(error.code, "antraInvalidStream");
        assert_eq!(fs::read(&part).unwrap(), header);
        assert!(!part.with_extension("remux.part").exists());

        let job = AntraJob::new();
        job.cancelled.store(true, Ordering::Release);
        let error = remux_mp4_flac(&part, &job, MirrorSource::Tidal).unwrap_err();
        assert_eq!(error.code, "antraCancelled");
        assert_eq!(fs::read(&part).unwrap(), header);
    }

    fn query() -> TrackQuery {
        TrackQuery {
            library_track_id: 42,
            title: "Loverboy".into(),
            artists: vec!["A-Wall".into()],
            album: Some("Loverboy".into()),
            duration_ms: Some(224_000),
            isrc: Some("QZFZ41979172".into()),
            version_kind: None,
            version_detail: None,
        }
    }

    #[test]
    fn endpoint_manifest_deserializes_lossless_mirrors() {
        let manifest: EndpointManifest = serde_json::from_value(json!({
            "mirrors": {
                "tidal": "https://tidal.example/",
                "qobuz": "https://qobuz.example/",
                "deezer": "https://deezer.example/",
                "amazon": "https://amazon.example/",
                "apple": "https://apple.example/"
            }
        }))
        .unwrap();

        assert_eq!(
            manifest.mirrors.tidal.as_deref(),
            Some("https://tidal.example/")
        );
        assert_eq!(
            manifest.mirrors.qobuz.as_deref(),
            Some("https://qobuz.example/")
        );
    }

    #[test]
    fn exact_isrc_payload_becomes_lossless_candidate_with_quality() {
        let candidate = exact_candidate_from_payload(
            MirrorSource::Qobuz,
            &query(),
            &json!({
                "track_id": 99123,
                "title": "Loverboy",
                "artist": "A-Wall",
                "album": "Loverboy",
                "duration_ms": 224000,
                "bitDepth": 24,
                "sampleRate": 96000,
                "isrc": "QZFZ41979172"
            }),
        )
        .unwrap();

        assert_eq!(candidate.provider.as_deref(), Some("antra"));
        assert_eq!(candidate.source.as_deref(), Some("Qobuz"));
        assert_eq!(candidate.format.as_deref(), Some("FLAC"));
        assert_eq!(candidate.bit_depth, Some(24));
        assert_eq!(candidate.sample_rate_hz, Some(96_000));
        assert_eq!(candidate.isrc.as_deref(), Some("QZFZ41979172"));
        assert_eq!(candidate.title.as_deref(), Some("Loverboy"));
        assert_eq!(candidate.artists, vec!["A-Wall"]);
    }

    #[test]
    fn exact_tidal_hi_res_payload_exposes_hi_res_quality_for_ranking() {
        let candidate = exact_candidate_from_payload(
            MirrorSource::Tidal,
            &query(),
            &json!({
                "track_id": 99123,
                "title": "Loverboy",
                "artist": { "name": "A-Wall" },
                "album": { "title": "Loverboy" },
                "duration": 224,
                "audioQuality": "HI_RES_LOSSLESS",
                "isrc": "QZFZ41979172"
            }),
        )
        .unwrap();

        assert_eq!(candidate.bit_depth, Some(24));
        assert_eq!(candidate.sample_rate_hz, Some(96_000));
    }

    #[test]
    fn exact_tidal_lossless_payload_exposes_cd_quality_for_ranking() {
        let candidate = exact_candidate_from_payload(
            MirrorSource::Tidal,
            &query(),
            &json!({
                "track_id": 99123,
                "title": "Loverboy",
                "artist": { "name": "A-Wall" },
                "album": { "title": "Loverboy" },
                "duration": 224,
                "audioQuality": "LOSSLESS",
                "isrc": "QZFZ41979172"
            }),
        )
        .unwrap();

        assert_eq!(candidate.bit_depth, Some(16));
        assert_eq!(candidate.sample_rate_hz, Some(44_100));
    }

    #[test]
    fn candidate_token_round_trips_without_embedding_endpoint_or_auth() {
        let token = CandidateToken {
            source: MirrorSource::Tidal,
            track_id: "123456".into(),
        };
        let encoded = serde_json::to_string(&token).unwrap();
        let decoded: CandidateToken = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded, token);
        assert!(!encoded.contains("http"));
        assert!(!encoded.contains("token"));
    }

    #[test]
    fn non_flac_stream_moves_to_next_antra_candidate_without_same_mirror_retry() {
        let invalid_stream = ProviderError::new(
            "antraInvalidStream",
            "Antra Tidal returned a non-FLAC stream.",
            true,
        );
        let transport = ProviderError::new(
            "downloadTransportFailed",
            "temporary transport failure",
            true,
        );

        assert!(!should_retry_same_candidate(&invalid_stream));
        assert!(should_retry_same_candidate(&transport));
    }
}
