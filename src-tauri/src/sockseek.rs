use std::{
    collections::HashMap,
    error::Error,
    fmt, fs,
    io::Write,
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_shell::{
    ShellExt,
    process::{CommandChild, CommandEvent},
};
use tungstenite::{Message, connect};
use walkdir::WalkDir;

use crate::{
    acquisition::{AcquisitionProvider, ProviderError},
    domain::{
        AcquisitionCandidate, AcquisitionRequest, ProviderHealth, ProviderJob, ProviderJobStatus,
        TrackQuery,
    },
    security::{KeyringSoulseekCredentialStore, SoulseekCredentialStore, SoulseekCredentials},
};

pub const SOCKSEEK_VERSION: &str = "3.0.5";
const SOCKSEEK_PROVIDER_ID: &str = "sockseek";
const DAEMON_START_TIMEOUT: Duration = Duration::from_secs(12);
const SEARCH_TIMEOUT: Duration = Duration::from_secs(90);
const SEARCH_POLL_DELAY: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub enum SockseekError {
    Credentials(String),
    Runtime(String),
    Provider(ProviderError),
    Io(std::io::Error),
}

impl fmt::Display for SockseekError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Credentials(message) | Self::Runtime(message) => formatter.write_str(message),
            Self::Provider(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "Sockseek runtime filesystem error: {error}"),
        }
    }
}

impl Error for SockseekError {}

impl From<std::io::Error> for SockseekError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProviderError> for SockseekError {
    fn from(error: ProviderError) -> Self {
        Self::Provider(error)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SoulseekCredentialStatus {
    pub configured: bool,
    pub username: Option<String>,
}

struct SockseekProcess {
    child: CommandChild,
    provider: Arc<SockseekProvider>,
    event_stop: Arc<AtomicBool>,
}

pub struct SockseekManager {
    credentials: Arc<dyn SoulseekCredentialStore>,
    process: Mutex<Option<SockseekProcess>>,
}

impl Default for SockseekManager {
    fn default() -> Self {
        Self {
            credentials: Arc::new(KeyringSoulseekCredentialStore),
            process: Mutex::new(None),
        }
    }
}

impl SockseekManager {
    pub fn credential_status(&self) -> Result<SoulseekCredentialStatus, SockseekError> {
        let credentials = self
            .credentials
            .get_credentials()
            .map_err(|error| SockseekError::Credentials(error.to_string()))?;
        Ok(SoulseekCredentialStatus {
            configured: credentials.is_some(),
            username: credentials.map(|value| value.username),
        })
    }

    pub fn set_credentials(&self, username: &str, password: &str) -> Result<(), SockseekError> {
        validate_credential(username, "Soulseek username")?;
        validate_credential(password, "Soulseek password")?;
        self.shutdown();
        self.credentials
            .set_credentials(&SoulseekCredentials {
                username: username.trim().to_owned(),
                password: password.to_owned(),
            })
            .map_err(|error| SockseekError::Credentials(error.to_string()))
    }

    pub fn clear_credentials(&self) -> Result<(), SockseekError> {
        self.shutdown();
        self.credentials
            .clear_credentials()
            .map_err(|error| SockseekError::Credentials(error.to_string()))
    }

    pub fn provider(
        &self,
        app: &AppHandle,
        app_data_dir: &Path,
    ) -> Result<Arc<SockseekProvider>, SockseekError> {
        let mut process = self.process.lock().unwrap();
        if let Some(active) = process.as_ref()
            && active.provider.compatible_daemon().is_ok()
        {
            return Ok(Arc::clone(&active.provider));
        }

        if let Some(stale) = process.take() {
            stop_process(stale);
        }

        let credentials = self
            .credentials
            .get_credentials()
            .map_err(|error| SockseekError::Credentials(error.to_string()))?
            .ok_or_else(|| {
                SockseekError::Credentials(
                    "Configure Soulseek credentials in Settings before enabling acquisition."
                        .to_owned(),
                )
            })?;

        let started = start_sidecar(app, app_data_dir, &credentials)?;
        let provider = Arc::clone(&started.provider);
        *process = Some(started);
        Ok(provider)
    }

    pub fn shutdown(&self) {
        if let Some(process) = self.process.lock().unwrap().take() {
            stop_process(process);
        }
    }
}

impl Drop for SockseekManager {
    fn drop(&mut self) {
        if let Ok(process) = self.process.get_mut()
            && let Some(process) = process.take()
        {
            stop_process(process);
        }
    }
}

fn stop_process(process: SockseekProcess) {
    process.event_stop.store(true, Ordering::Release);
    if let Err(error) = process.child.kill() {
        tracing::debug!(%error, "Sockseek sidecar was already stopped");
    }
}

fn validate_credential(value: &str, label: &str) -> Result<(), SockseekError> {
    if value.trim().is_empty() {
        return Err(SockseekError::Credentials(format!("{label} is required.")));
    }
    if value.contains(['\r', '\n']) {
        return Err(SockseekError::Credentials(format!(
            "{label} cannot contain line breaks."
        )));
    }
    Ok(())
}

fn start_sidecar(
    app: &AppHandle,
    app_data_dir: &Path,
    credentials: &SoulseekCredentials,
) -> Result<SockseekProcess, SockseekError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);

    let runtime_dir = app_data_dir.join("runtime").join("sockseek");
    fs::create_dir_all(&runtime_dir)?;
    restrict_directory(&runtime_dir)?;
    let config_path = runtime_dir.join(format!("sockseek-{port}.conf"));
    write_runtime_config(&config_path, app_data_dir, credentials)?;

    let sidecar = app
        .shell()
        .sidecar("sockseek")
        .map_err(|error| {
            SockseekError::Runtime(format!(
                "Sockseek sidecar is unavailable: {error}. Fetch the pinned sidecar before running acquisition."
            ))
        })?
        .args([
            "daemon".to_owned(),
            "--config".to_owned(),
            config_path.to_string_lossy().into_owned(),
            "--server-ip".to_owned(),
            "127.0.0.1".to_owned(),
            "--server-port".to_owned(),
            port.to_string(),
        ]);
    let (mut events, child) = sidecar
        .spawn()
        .map_err(|error| SockseekError::Runtime(format!("Could not start Sockseek: {error}")))?;

    let redactions = Arc::new(vec![
        credentials.username.clone(),
        credentials.password.clone(),
    ]);
    tauri::async_runtime::spawn(async move {
        while let Some(event) = events.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => {
                    let message = redact(&String::from_utf8_lossy(&bytes), &redactions);
                    tracing::debug!(target: "sockseek", %message, "sidecar stdout");
                }
                CommandEvent::Stderr(bytes) => {
                    let message = redact(&String::from_utf8_lossy(&bytes), &redactions);
                    tracing::warn!(target: "sockseek", %message, "sidecar stderr");
                }
                CommandEvent::Error(error) => {
                    let message = redact(&error, &redactions);
                    tracing::warn!(target: "sockseek", %message, "sidecar process error");
                }
                CommandEvent::Terminated(payload) => {
                    tracing::info!(target: "sockseek", code = ?payload.code, "sidecar stopped");
                }
                _ => {}
            }
        }
    });

    let base_url = format!("http://127.0.0.1:{port}");
    let wake = Arc::new(EventWake::default());
    let provider = Arc::new(SockseekProvider::new(base_url.clone(), Arc::clone(&wake))?);
    let deadline = Instant::now() + DAEMON_START_TIMEOUT;
    let startup = loop {
        match provider.compatible_daemon() {
            Ok(()) => break Ok(()),
            Err(error) if Instant::now() < deadline && error.code == "providerOffline" => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => break Err(error),
        }
    };

    if let Err(error) = fs::remove_file(&config_path) {
        tracing::warn!(%error, path = %config_path.display(), "could not remove transient Sockseek config");
    }

    if let Err(error) = startup {
        let _ = child.kill();
        return Err(SockseekError::Provider(error));
    }

    let event_stop = Arc::new(AtomicBool::new(false));
    start_signalr_listener(base_url, wake, Arc::clone(&event_stop));

    Ok(SockseekProcess {
        child,
        provider,
        event_stop,
    })
}

fn write_runtime_config(
    path: &Path,
    app_data_dir: &Path,
    credentials: &SoulseekCredentials,
) -> Result<(), SockseekError> {
    let output_dir = app_data_dir.join("runtime").join("acquisition");
    fs::create_dir_all(&output_dir)?;
    let content = format!(
        "username = {}\npassword = {}\noutput-dir = {}\n",
        credentials.username,
        credentials.password,
        output_dir.display()
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(content.as_bytes())?;
    }
    #[cfg(not(unix))]
    {
        fs::write(path, content)?;
    }
    Ok(())
}

#[cfg(unix)]
fn restrict_directory(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn restrict_directory(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

fn redact(message: &str, secrets: &[String]) -> String {
    secrets
        .iter()
        .filter(|value| !value.is_empty())
        .fold(message.to_owned(), |redacted, secret| {
            redacted.replace(secret, "[REDACTED]")
        })
}

#[derive(Default)]
struct EventWake {
    generation: Mutex<u64>,
    changed: Condvar,
    progress: Mutex<HashMap<String, DownloadProgress>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    job_id: String,
    bytes_transferred: i64,
    total_bytes: i64,
}

impl EventWake {
    fn notify(&self, progress: Option<DownloadProgress>) {
        if let Some(progress) = progress {
            tracing::trace!(
                job_id = %progress.job_id,
                bytes_transferred = progress.bytes_transferred,
                total_bytes = progress.total_bytes,
                "Sockseek download progress"
            );
            self.progress
                .lock()
                .unwrap()
                .insert(progress.job_id.clone(), progress);
        }
        let mut generation = self.generation.lock().unwrap();
        *generation = generation.wrapping_add(1);
        self.changed.notify_all();
    }

    fn wait(&self, timeout: Duration) {
        let generation = self.generation.lock().unwrap();
        let observed = *generation;
        let _guard = self
            .changed
            .wait_timeout_while(generation, timeout, |current| *current == observed)
            .unwrap();
    }
}

fn start_signalr_listener(base_url: String, wake: Arc<EventWake>, stop: Arc<AtomicBool>) {
    thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            if let Err(error) = run_signalr_connection(&base_url, &wake, &stop)
                && !stop.load(Ordering::Acquire)
            {
                tracing::debug!(%error, "Sockseek SignalR connection dropped; HTTP polling remains active");
                wake.notify(None);
                thread::sleep(Duration::from_secs(1));
            }
        }
    });
}

fn run_signalr_connection(
    base_url: &str,
    wake: &EventWake,
    stop: &AtomicBool,
) -> Result<(), String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| error.to_string())?;
    let negotiation: SignalRNegotiate = client
        .post(format!(
            "{base_url}/api/events/negotiate?negotiateVersion=1"
        ))
        .send()
        .and_then(Response::error_for_status)
        .map_err(|error| error.to_string())?
        .json()
        .map_err(|error| error.to_string())?;

    let mut url = url::Url::parse(&base_url.replace("http://", "ws://"))
        .map_err(|error| error.to_string())?;
    url.set_path("/api/events");
    url.query_pairs_mut()
        .append_pair("id", &negotiation.connection_token);
    let (mut socket, _) = connect(url.as_str()).map_err(|error| error.to_string())?;
    socket
        .send(Message::Text(
            "{\"protocol\":\"json\",\"version\":1}\u{1e}".into(),
        ))
        .map_err(|error| error.to_string())?;

    let handshake = socket.read().map_err(|error| error.to_string())?;
    if let Message::Text(text) = handshake
        && !text.split('\u{1e}').any(|frame| frame.trim() == "{}")
    {
        return Err("Sockseek SignalR handshake was rejected".to_owned());
    }

    socket
        .send(Message::Text(
            "{\"type\":1,\"target\":\"SubscribeAll\",\"arguments\":[]}\u{1e}".into(),
        ))
        .map_err(|error| error.to_string())?;

    while !stop.load(Ordering::Acquire) {
        match socket.read().map_err(|error| error.to_string())? {
            Message::Text(text) => {
                for frame in text
                    .split('\u{1e}')
                    .filter(|frame| !frame.trim().is_empty())
                {
                    handle_signalr_frame(frame, wake);
                }
            }
            Message::Binary(bytes) => {
                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                    for frame in text
                        .split('\u{1e}')
                        .filter(|frame| !frame.trim().is_empty())
                    {
                        handle_signalr_frame(frame, wake);
                    }
                }
            }
            Message::Ping(payload) => {
                socket
                    .send(Message::Pong(payload))
                    .map_err(|error| error.to_string())?;
            }
            Message::Close(_) => return Err("Sockseek SignalR socket closed".to_owned()),
            _ => {}
        }
    }
    Ok(())
}

fn handle_signalr_frame(frame: &str, wake: &EventWake) {
    let Ok(value) = serde_json::from_str::<Value>(frame) else {
        return;
    };
    if value.get("type").and_then(Value::as_i64) != Some(1) {
        return;
    }

    let target = value
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let argument = value
        .get("arguments")
        .and_then(Value::as_array)
        .and_then(|arguments| arguments.first());
    match (target, argument) {
        ("workflowUpdateBatch", Some(batch)) => {
            let progress = batch
                .get("progress")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| serde_json::from_value::<DownloadProgress>(item.clone()).ok())
                .collect::<Vec<_>>();
            if progress.is_empty() {
                wake.notify(None);
            } else {
                for item in progress {
                    wake.notify(Some(item));
                }
            }
        }
        ("serverEvent", Some(envelope)) => {
            let progress = envelope
                .get("payload")
                .cloned()
                .and_then(|payload| serde_json::from_value::<DownloadProgress>(payload).ok());
            wake.notify(progress);
        }
        _ => wake.notify(None),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalRNegotiate {
    connection_token: String,
}

pub struct SockseekProvider {
    base_url: String,
    http: Client,
    wake: Arc<EventWake>,
    staging_by_job: Mutex<HashMap<String, PathBuf>>,
}

impl SockseekProvider {
    fn new(base_url: String, wake: Arc<EventWake>) -> Result<Self, SockseekError> {
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|error| SockseekError::Runtime(error.to_string()))?;
        Ok(Self {
            base_url,
            http,
            wake,
            staging_by_job: Mutex::new(HashMap::new()),
        })
    }

    fn compatible_daemon(&self) -> Result<(), ProviderError> {
        let info: ServerInfo = self.get_json("/api/server/info")?;
        if info.version != SOCKSEEK_VERSION {
            return Err(ProviderError::new(
                "providerVersionMismatch",
                format!(
                    "Refrain requires Sockseek {SOCKSEEK_VERSION}, but the sidecar reported {}.",
                    info.version
                ),
                false,
            ));
        }
        Ok(())
    }

    fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, ProviderError> {
        let response = self
            .http
            .get(format!("{}{path}", self.base_url))
            .send()
            .map_err(provider_transport_error)?;
        decode_response(response)
    }

    fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ProviderError> {
        let response = self
            .http
            .post(format!("{}{path}", self.base_url))
            .json(body)
            .send()
            .map_err(provider_transport_error)?;
        decode_response(response)
    }

    fn post_empty(&self, path: &str) -> Result<(), ProviderError> {
        let response = self
            .http
            .post(format!("{}{path}", self.base_url))
            .send()
            .map_err(provider_transport_error)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(provider_http_error(response))
        }
    }

    fn wait_for_search(&self, job_id: &str) -> Result<FileResultSnapshot, ProviderError> {
        let deadline = Instant::now() + SEARCH_TIMEOUT;
        loop {
            let snapshot: FileResultSnapshot =
                self.get_json(&format!("/api/jobs/{job_id}/results/files"))?;
            if snapshot.is_complete {
                return Ok(snapshot);
            }

            let detail: JobDetail = self.get_json(&format!("/api/jobs/{job_id}"))?;
            if is_terminal(&detail.summary.lifecycle_state) {
                return match normalized(&detail.summary.terminal_outcome).as_str() {
                    "succeeded" => Ok(snapshot),
                    "cancelled" => Err(ProviderError::new(
                        "searchCancelled",
                        "Sockseek search was cancelled.",
                        true,
                    )),
                    _ => Err(job_failure(&detail.summary, "Sockseek search failed.")),
                };
            }
            if Instant::now() >= deadline {
                return Err(ProviderError::new(
                    "searchTimeout",
                    "Sockseek search did not finish in time.",
                    true,
                ));
            }
            self.wake.wait(SEARCH_POLL_DELAY);
        }
    }
}

impl AcquisitionProvider for SockseekProvider {
    fn id(&self) -> &'static str {
        SOCKSEEK_PROVIDER_ID
    }

    fn health(&self) -> Result<ProviderHealth, ProviderError> {
        self.compatible_daemon()?;
        let status: ServerStatus = self.get_json("/api/server/status")?;
        Ok(ProviderHealth {
            available: status.soulseek_client.is_ready,
            version: Some(SOCKSEEK_VERSION.to_owned()),
            message: (!status.soulseek_client.is_ready).then(|| {
                format!(
                    "Sockseek is running, but Soulseek is not ready ({})",
                    status.soulseek_client.state
                )
            }),
        })
    }

    fn search(&self, query: &TrackQuery) -> Result<Vec<AcquisitionCandidate>, ProviderError> {
        let request = TrackSearchRequest {
            song_query: SongQuery {
                artist: query.artists.first().cloned(),
                title: Some(query.title.clone()),
                album: query.album.clone(),
                uri: None,
                length: query
                    .duration_ms
                    .map(|duration| i32::try_from((duration + 500) / 1_000).unwrap_or(i32::MAX)),
                artist_maybe_wrong: false,
            },
            include_full_results: false,
        };
        let summary: JobSummary = self.post_json("/api/jobs/search/tracks", &request)?;
        let snapshot = self.wait_for_search(&summary.job_id)?;

        snapshot
            .items
            .into_iter()
            .map(|item| {
                let provider_token = serde_json::to_string(&CandidateToken {
                    search_job_id: summary.job_id.clone(),
                    username: item.reference.username,
                    filename: item.reference.filename.clone(),
                })
                .map_err(|error| {
                    ProviderError::new("providerProtocol", error.to_string(), false)
                })?;
                Ok(AcquisitionCandidate {
                    provider_token,
                    title: Some(remote_basename(&item.reference.filename)),
                    artists: query.artists.clone(),
                    album: query.album.clone(),
                    duration_ms: item.length.map(|seconds| i64::from(seconds) * 1_000),
                    format: item.extension,
                    size_bytes: Some(item.size),
                })
            })
            .collect()
    }

    fn acquire(&self, request: &AcquisitionRequest) -> Result<ProviderJob, ProviderError> {
        let token: CandidateToken = serde_json::from_str(&request.candidate.provider_token)
            .map_err(|error| {
                ProviderError::new(
                    "invalidCandidateToken",
                    format!("Sockseek candidate token is invalid: {error}"),
                    false,
                )
            })?;
        let body = StartDownloadsRequest {
            files: vec![FileCandidateRef {
                username: token.username,
                filename: token.filename,
            }],
            options: SubmissionOptions {
                output_parent_dir: request.staging_path.clone(),
            },
        };
        let jobs: Vec<JobSummary> = self.post_json(
            &format!("/api/jobs/{}/downloads/files", token.search_job_id),
            &body,
        )?;
        let job = jobs.into_iter().next().ok_or_else(|| {
            ProviderError::new(
                "providerProtocol",
                "Sockseek did not create a download job for the selected candidate.",
                true,
            )
        })?;
        self.staging_by_job
            .lock()
            .unwrap()
            .insert(job.job_id.clone(), PathBuf::from(&request.staging_path));
        Ok(ProviderJob {
            provider_job_id: job.job_id,
        })
    }

    fn status(&self, provider_job_id: &str) -> Result<ProviderJobStatus, ProviderError> {
        let detail: JobDetail = self.get_json(&format!("/api/jobs/{provider_job_id}"))?;
        if !is_terminal(&detail.summary.lifecycle_state) {
            return match normalized(&detail.summary.lifecycle_state).as_str() {
                "pending" => Ok(ProviderJobStatus::Pending),
                _ => Ok(ProviderJobStatus::Running),
            };
        }

        match normalized(&detail.summary.terminal_outcome).as_str() {
            "succeeded" => {
                let output_paths = self
                    .staging_by_job
                    .lock()
                    .unwrap()
                    .get(provider_job_id)
                    .map(|path| staged_files(path))
                    .transpose()
                    .map_err(|error| {
                        ProviderError::new("stagingReadFailed", error.to_string(), true)
                    })?
                    .unwrap_or_default();
                if output_paths.is_empty() {
                    return Ok(ProviderJobStatus::Failed {
                        code: "stagingOutputMissing".to_owned(),
                        message: "Sockseek completed without producing a staged file.".to_owned(),
                        retryable: true,
                    });
                }
                Ok(ProviderJobStatus::Succeeded { output_paths })
            }
            "cancelled" => Ok(ProviderJobStatus::Cancelled),
            _ => {
                let failure = job_failure(&detail.summary, "Sockseek download failed.");
                Ok(ProviderJobStatus::Failed {
                    code: failure.code,
                    message: failure.message,
                    retryable: failure.retryable,
                })
            }
        }
    }

    fn cancel(&self, provider_job_id: &str) -> Result<(), ProviderError> {
        self.post_empty(&format!("/api/jobs/{provider_job_id}/cancel"))
    }

    fn wait_for_status_change(&self, _provider_job_id: &str, timeout: Duration) {
        self.wake.wait(timeout);
    }
}

fn staged_files(path: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut files = Vec::new();
    if !path.exists() {
        return Ok(files);
    }
    for entry in WalkDir::new(path) {
        let entry = entry.map_err(std::io::Error::other)?;
        if entry.file_type().is_file() {
            files.push(entry.path().to_string_lossy().into_owned());
        }
    }
    files.sort();
    Ok(files)
}

fn remote_basename(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_owned()
}

fn decode_response<T: DeserializeOwned>(response: Response) -> Result<T, ProviderError> {
    if response.status().is_success() {
        response.json().map_err(|error| {
            ProviderError::new(
                "providerProtocol",
                format!("Sockseek returned an invalid response: {error}"),
                true,
            )
        })
    } else {
        Err(provider_http_error(response))
    }
}

fn provider_transport_error(error: reqwest::Error) -> ProviderError {
    ProviderError::new(
        "providerOffline",
        format!("Could not reach the Sockseek sidecar: {error}"),
        true,
    )
}

fn provider_http_error(response: Response) -> ProviderError {
    let status = response.status();
    let message = response
        .json::<ApiError>()
        .map(|error| error.error)
        .unwrap_or_else(|_| format!("Sockseek returned HTTP {status}."));
    ProviderError::new("providerRequestFailed", message, status.is_server_error())
}

fn job_failure(summary: &JobSummary, fallback: &str) -> ProviderError {
    let reason = summary
        .failure_reason
        .as_deref()
        .unwrap_or("providerFailed");
    let normalized_reason = normalized(reason);
    let retryable = matches!(
        normalized_reason.as_str(),
        "outofdownloadretries" | "alldownloadsfailed" | "extractionfailed" | "other"
    );
    ProviderError::new(
        reason,
        summary
            .failure_message
            .clone()
            .unwrap_or_else(|| fallback.to_owned()),
        retryable,
    )
}

fn is_terminal(lifecycle: &str) -> bool {
    normalized(lifecycle) == "terminal"
}

fn normalized(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerInfo {
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerStatus {
    soulseek_client: SoulseekClientStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SoulseekClientStatus {
    state: String,
    is_ready: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackSearchRequest {
    song_query: SongQuery,
    include_full_results: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SongQuery {
    artist: Option<String>,
    title: Option<String>,
    album: Option<String>,
    uri: Option<String>,
    length: Option<i32>,
    artist_maybe_wrong: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileResultSnapshot {
    is_complete: bool,
    items: Vec<FileCandidate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileCandidate {
    #[serde(rename = "ref")]
    reference: FileCandidateRef,
    size: i64,
    length: Option<i32>,
    extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileCandidateRef {
    username: String,
    filename: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateToken {
    search_job_id: String,
    username: String,
    filename: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartDownloadsRequest {
    files: Vec<FileCandidateRef>,
    options: SubmissionOptions,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmissionOptions {
    output_parent_dir: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobDetail {
    summary: JobSummary,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobSummary {
    job_id: String,
    lifecycle_state: String,
    terminal_outcome: String,
    failure_reason: Option<String>,
    failure_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    error: String,
}
