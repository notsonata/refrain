use std::{error::Error, fmt, fs, path::Path, thread, time::Duration};

use crate::{
    db::{Database, DatabaseError},
    domain::{
        AcquisitionCandidate, AcquisitionJob, AcquisitionRequest, ProviderHealth, ProviderJob,
        ProviderJobStatus, TrackQuery,
    },
};

const MAX_ATTEMPTS: usize = 3;
const MAX_STATUS_POLLS: usize = 3_600;
const STATUS_POLL_DELAY: Duration = Duration::from_millis(250);

pub trait AcquisitionProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn health(&self) -> Result<ProviderHealth, ProviderError>;
    fn search(&self, query: &TrackQuery) -> Result<Vec<AcquisitionCandidate>, ProviderError>;
    fn acquire(&self, request: &AcquisitionRequest) -> Result<ProviderJob, ProviderError>;
    fn status(&self, provider_job_id: &str) -> Result<ProviderJobStatus, ProviderError>;
    fn cancel(&self, provider_job_id: &str) -> Result<(), ProviderError>;

    fn wait_for_status_change(&self, _provider_job_id: &str, timeout: Duration) {
        thread::sleep(timeout);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl ProviderError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ProviderError {}

#[derive(Debug)]
pub enum AcquisitionError {
    Database(DatabaseError),
    Io(std::io::Error),
    Provider(ProviderError),
    ProviderUnavailable(String),
    InvalidState(String),
}

impl fmt::Display for AcquisitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "acquisition database error: {error}"),
            Self::Io(error) => write!(formatter, "acquisition filesystem error: {error}"),
            Self::Provider(error) => write!(formatter, "acquisition provider error: {error}"),
            Self::ProviderUnavailable(message) | Self::InvalidState(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl Error for AcquisitionError {}

impl From<DatabaseError> for AcquisitionError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<std::io::Error> for AcquisitionError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProviderError> for AcquisitionError {
    fn from(error: ProviderError) -> Self {
        Self::Provider(error)
    }
}

#[derive(Debug, Default)]
pub struct AcquisitionCoordinator;

impl AcquisitionCoordinator {
    pub fn provider_health<P: AcquisitionProvider>(
        &self,
        provider: &P,
    ) -> Result<ProviderHealth, AcquisitionError> {
        provider.health().map_err(AcquisitionError::from)
    }

    pub fn run_missing<P: AcquisitionProvider>(
        &self,
        database: &Database,
        app_data_dir: &Path,
        provider: &P,
        mut is_cancelled: impl FnMut() -> bool,
    ) -> Result<Vec<AcquisitionJob>, AcquisitionError> {
        let health = self.provider_health(provider)?;
        if !health.available {
            return Err(AcquisitionError::ProviderUnavailable(
                health
                    .message
                    .unwrap_or_else(|| format!("{} is unavailable", provider.id())),
            ));
        }

        let queued = database.queue_missing_acquisition_jobs(provider.id())?;
        let mut completed = Vec::new();
        for job in queued
            .into_iter()
            .filter(|job| job.provider == provider.id())
        {
            if is_cancelled() {
                database.finish_acquisition_job(job.id, "cancelled", None, None)?;
            } else {
                self.run_job(database, app_data_dir, provider, job.id, &mut is_cancelled)?;
            }
            completed.push(database.acquisition_job(job.id)?.ok_or_else(|| {
                AcquisitionError::InvalidState("acquisition job disappeared".into())
            })?);
        }
        Ok(completed)
    }

    fn run_job<P: AcquisitionProvider>(
        &self,
        database: &Database,
        app_data_dir: &Path,
        provider: &P,
        job_id: i64,
        is_cancelled: &mut impl FnMut() -> bool,
    ) -> Result<(), AcquisitionError> {
        let job = database.acquisition_job(job_id)?.ok_or_else(|| {
            AcquisitionError::InvalidState(format!("acquisition job {job_id} not found"))
        })?;
        let query = database
            .acquisition_track_query(job.library_track_id)?
            .ok_or_else(|| {
                AcquisitionError::InvalidState("library track for acquisition was not found".into())
            })?;
        let candidates = match provider.search(&query) {
            Ok(candidates) => candidates,
            Err(error) => {
                database.finish_acquisition_job(
                    job_id,
                    "failed",
                    Some(&error.code),
                    Some(&error.message),
                )?;
                return Ok(());
            }
        };

        if candidates.is_empty() {
            database.finish_acquisition_job(
                job_id,
                "failed",
                Some("noCandidates"),
                Some("The acquisition provider returned no candidates."),
            )?;
            return Ok(());
        }

        let staging_dir = app_data_dir
            .join("runtime")
            .join("acquisition")
            .join(job_id.to_string());
        let mut last_error = ProviderError::new(
            "acquisitionFailed",
            "No acquisition candidate completed successfully.",
            false,
        );

        for (index, candidate) in candidates.into_iter().take(MAX_ATTEMPTS).enumerate() {
            if is_cancelled() {
                database.finish_acquisition_job(job_id, "cancelled", None, None)?;
                return Ok(());
            }

            prepare_staging_directory(&staging_dir)?;
            let staging_path = staging_dir.to_string_lossy().into_owned();
            let attempt = i64::try_from(index + 1).unwrap_or(i64::MAX);
            database.begin_acquisition_attempt(job_id, attempt, &candidate, &staging_path)?;
            let request = AcquisitionRequest {
                library_track_id: job.library_track_id,
                query: query.clone(),
                candidate,
                staging_path,
            };

            let provider_job = match provider.acquire(&request) {
                Ok(provider_job) => provider_job,
                Err(error) => {
                    let retryable = error.retryable;
                    last_error = error;
                    if retryable {
                        continue;
                    }
                    break;
                }
            };
            database.set_acquisition_provider_job_id(job_id, &provider_job.provider_job_id)?;

            match wait_for_provider_job(provider, &provider_job.provider_job_id, is_cancelled) {
                ProviderOutcome::Staged => {
                    database.finish_acquisition_job(job_id, "staged", None, None)?;
                    return Ok(());
                }
                ProviderOutcome::Cancelled => {
                    database.finish_acquisition_job(job_id, "cancelled", None, None)?;
                    return Ok(());
                }
                ProviderOutcome::Failed(error) => {
                    let retryable = error.retryable;
                    last_error = error;
                    if retryable {
                        let _ = provider.cancel(&provider_job.provider_job_id);
                        continue;
                    }
                    break;
                }
            }
        }

        database.finish_acquisition_job(
            job_id,
            "failed",
            Some(&last_error.code),
            Some(&last_error.message),
        )?;
        Ok(())
    }
}

enum ProviderOutcome {
    Staged,
    Cancelled,
    Failed(ProviderError),
}

fn wait_for_provider_job<P: AcquisitionProvider>(
    provider: &P,
    provider_job_id: &str,
    is_cancelled: &mut impl FnMut() -> bool,
) -> ProviderOutcome {
    for _ in 0..MAX_STATUS_POLLS {
        if is_cancelled() {
            let _ = provider.cancel(provider_job_id);
            return ProviderOutcome::Cancelled;
        }
        match provider.status(provider_job_id) {
            Ok(ProviderJobStatus::Succeeded { .. }) => return ProviderOutcome::Staged,
            Ok(ProviderJobStatus::Cancelled) => return ProviderOutcome::Cancelled,
            Ok(ProviderJobStatus::Failed {
                code,
                message,
                retryable,
            }) => {
                return ProviderOutcome::Failed(ProviderError::new(code, message, retryable));
            }
            Ok(ProviderJobStatus::Pending | ProviderJobStatus::Running) => {
                provider.wait_for_status_change(provider_job_id, STATUS_POLL_DELAY);
            }
            Err(error) => return ProviderOutcome::Failed(error),
        }
    }

    let _ = provider.cancel(provider_job_id);
    ProviderOutcome::Failed(ProviderError::new(
        "providerTimeout",
        "The acquisition provider did not finish in time.",
        true,
    ))
}

fn prepare_staging_directory(path: &Path) -> Result<(), std::io::Error> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        sync::{Arc, Mutex},
    };

    use crate::domain::{CollectionEntry, SourceAccount, SourceCollection, SourceTrack};

    use super::*;

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    struct TestPath(PathBuf);

    impl TestPath {
        fn new(name: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "refrain-acquisition-{name}-{}-{id}",
                std::process::id()
            )))
        }
    }

    impl Drop for TestPath {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
            let _ = fs::remove_file(self.0.with_extension("sqlite3"));
            let _ = fs::remove_file(self.0.with_extension("sqlite3-wal"));
            let _ = fs::remove_file(self.0.with_extension("sqlite3-shm"));
        }
    }

    #[derive(Default)]
    struct FakeState {
        candidates: Vec<AcquisitionCandidate>,
        acquire_results: VecDeque<Result<String, ProviderError>>,
        statuses: HashMap<String, VecDeque<Result<ProviderJobStatus, ProviderError>>>,
        acquired_tokens: Vec<String>,
        cancelled_jobs: Vec<String>,
    }

    #[derive(Clone, Default)]
    struct FakeProvider {
        state: Arc<Mutex<FakeState>>,
    }

    impl FakeProvider {
        fn with_candidates(tokens: &[&str]) -> Self {
            let provider = Self::default();
            provider.state.lock().unwrap().candidates = tokens
                .iter()
                .map(|token| AcquisitionCandidate {
                    provider_token: (*token).into(),
                    title: None,
                    artists: Vec::new(),
                    album: None,
                    duration_ms: None,
                    format: Some("flac".into()),
                    size_bytes: None,
                })
                .collect();
            provider
        }
    }

    impl AcquisitionProvider for FakeProvider {
        fn id(&self) -> &'static str {
            "fake"
        }

        fn health(&self) -> Result<ProviderHealth, ProviderError> {
            Ok(ProviderHealth {
                available: true,
                version: Some("test".into()),
                message: None,
            })
        }

        fn search(&self, _query: &TrackQuery) -> Result<Vec<AcquisitionCandidate>, ProviderError> {
            Ok(self.state.lock().unwrap().candidates.clone())
        }

        fn acquire(&self, request: &AcquisitionRequest) -> Result<ProviderJob, ProviderError> {
            let mut state = self.state.lock().unwrap();
            state
                .acquired_tokens
                .push(request.candidate.provider_token.clone());
            let result = state
                .acquire_results
                .pop_front()
                .unwrap_or_else(|| Ok(format!("job-{}", state.acquired_tokens.len())));
            result.map(|provider_job_id| ProviderJob { provider_job_id })
        }

        fn status(&self, provider_job_id: &str) -> Result<ProviderJobStatus, ProviderError> {
            let mut state = self.state.lock().unwrap();
            state
                .statuses
                .get_mut(provider_job_id)
                .and_then(VecDeque::pop_front)
                .unwrap_or_else(|| {
                    Ok(ProviderJobStatus::Succeeded {
                        output_paths: vec![format!("/{provider_job_id}.flac")],
                    })
                })
        }

        fn cancel(&self, provider_job_id: &str) -> Result<(), ProviderError> {
            self.state
                .lock()
                .unwrap()
                .cancelled_jobs
                .push(provider_job_id.into());
            Ok(())
        }
    }

    fn database(name: &str) -> (TestPath, Database) {
        let path = TestPath::new(name);
        fs::create_dir_all(&path.0).unwrap();
        let database = Database::open(path.0.join("refrain.sqlite3")).unwrap();
        (path, database)
    }

    fn seed_missing_track(database: &Database, duplicate_source_reference: bool) -> i64 {
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
                provider_collection_id: "playlist".into(),
                kind: "playlist".into(),
                name: "Playlist".into(),
                snapshot_id: None,
                owner_provider_id: None,
                is_accessible: true,
                access_issue: None,
                image_url: None,
                external_url: None,
                album_metadata: None,
            })
            .unwrap();
        let first_id = database
            .upsert_source_track(&source_track("first"))
            .unwrap();
        let library_track_id = database.create_library_track_from_source(first_id).unwrap();
        database
            .persist_track_link(first_id, library_track_id, "existing", 10_000)
            .unwrap();
        let mut entries = vec![CollectionEntry {
            position: 0,
            source_track_id: Some(first_id),
            provider_item_uri: None,
            item_type: "track".into(),
            added_at: None,
            unavailable_reason: None,
        }];
        if duplicate_source_reference {
            let second_id = database
                .upsert_source_track(&source_track("second"))
                .unwrap();
            database
                .persist_track_link(second_id, library_track_id, "metadata", 9_500)
                .unwrap();
            entries.push(CollectionEntry {
                position: 1,
                source_track_id: Some(second_id),
                provider_item_uri: None,
                item_type: "track".into(),
                added_at: None,
                unavailable_reason: None,
            });
        }
        database
            .replace_collection_entries(collection_id, &entries)
            .unwrap();
        library_track_id
    }

    fn source_track(id: &str) -> SourceTrack {
        SourceTrack {
            provider: "spotify".into(),
            provider_track_id: id.into(),
            uri: None,
            isrc: Some("TEST12345678".into()),
            title: "Missing Song".into(),
            normalized_title: "missing song".into(),
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

    #[test]
    fn duplicate_source_references_create_one_staged_job_without_importing_audio() {
        let (path, database) = database("dedupe");
        let library_track_id = seed_missing_track(&database, true);
        let provider = FakeProvider::with_candidates(&["candidate-a"]);

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].library_track_id, library_track_id);
        assert_eq!(jobs[0].status, "staged");
        assert_eq!(database.acquisition_jobs_page(0, 10).unwrap().total, 1);
        assert!(
            !database
                .library_track_has_preferred_present_file(library_track_id)
                .unwrap()
        );
    }

    #[test]
    fn retryable_failure_advances_to_a_distinct_candidate() {
        let (path, database) = database("retry");
        seed_missing_track(&database, false);
        let provider = FakeProvider::with_candidates(&["candidate-a", "candidate-b"]);
        provider
            .state
            .lock()
            .unwrap()
            .acquire_results
            .push_back(Err(ProviderError::new("busy", "try another", true)));

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "staged");
        assert_eq!(jobs[0].attempt, 2);
        assert_eq!(
            provider.state.lock().unwrap().acquired_tokens,
            vec!["candidate-a", "candidate-b"]
        );
    }

    #[test]
    fn cancellation_stops_provider_job_and_persists_cancelled_state() {
        let (path, database) = database("cancel");
        seed_missing_track(&database, false);
        let provider = FakeProvider::with_candidates(&["candidate-a"]);
        provider.state.lock().unwrap().statuses.insert(
            "job-1".into(),
            VecDeque::from([Ok(ProviderJobStatus::Running)]),
        );
        let calls = std::cell::Cell::new(0);

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || {
                let next = calls.get() + 1;
                calls.set(next);
                next > 2
            })
            .unwrap();

        assert_eq!(jobs[0].status, "cancelled");
        assert_eq!(provider.state.lock().unwrap().cancelled_jobs, vec!["job-1"]);
    }

    #[test]
    fn failed_provider_job_is_exposed_as_an_issue() {
        let (path, database) = database("issue");
        seed_missing_track(&database, false);
        let provider = FakeProvider::default();

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();
        assert_eq!(jobs[0].status, "failed");

        let issues = crate::issues::list_issues(&database, 0, 100).unwrap();
        assert!(
            issues
                .items
                .iter()
                .any(|issue| { issue.id == format!("acquisition:{}", jobs[0].id) })
        );
    }
}
