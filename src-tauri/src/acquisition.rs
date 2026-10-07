use std::{error::Error, fmt, fs, path::Path, sync::Arc, thread, time::Duration};

use lofty::file::AudioFile;

use crate::{
    db::{Database, DatabaseError},
    domain::{
        AcquisitionCandidate, AcquisitionJob, AcquisitionRequest, ProviderHealth, ProviderJob,
        ProviderJobStatus, TrackQuery,
    },
};

const MAX_ATTEMPTS: usize = 3;
const MAX_CHAIN_ATTEMPTS: usize = 3;
const MAX_CONCURRENT_ACQUISITIONS: usize = 3;
const MAX_STATUS_POLLS: usize = 3_600;
const STATUS_POLL_DELAY: Duration = Duration::from_millis(250);
const AUTO_CANDIDATE_MIN_CONFIDENCE: u8 = 75;
const REVIEW_CANDIDATE_MIN_CONFIDENCE: u8 = 55;
const AUTO_CANDIDATE_MIN_MARGIN: u8 = 8;
const MAX_REVIEW_CANDIDATES: usize = 3;

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
    #[cfg(test)]
    ProviderUnavailable(String),
    InvalidState(String),
}

impl fmt::Display for AcquisitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "acquisition database error: {error}"),
            Self::Io(error) => write!(formatter, "acquisition filesystem error: {error}"),
            Self::Provider(error) => write!(formatter, "acquisition provider error: {error}"),
            #[cfg(test)]
            Self::ProviderUnavailable(message) => formatter.write_str(message),
            Self::InvalidState(message) => formatter.write_str(message),
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
    pub fn provider_health<P: AcquisitionProvider + ?Sized>(
        &self,
        provider: &P,
    ) -> Result<ProviderHealth, AcquisitionError> {
        provider.health().map_err(AcquisitionError::from)
    }

    #[cfg(test)]
    pub fn run_missing<P: AcquisitionProvider + ?Sized>(
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
            let Some(current_job) = database.acquisition_job(job.id)? else {
                continue;
            };
            if current_job.status != "queued" {
                continue;
            }
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

    pub fn run_missing_chain(
        &self,
        database: &Database,
        app_data_dir: &Path,
        providers: &[Arc<dyn AcquisitionProvider>],
        is_cancelled: impl Fn() -> bool + Sync,
    ) -> Result<Vec<AcquisitionJob>, AcquisitionError> {
        let primary = providers.first().ok_or_else(|| {
            AcquisitionError::InvalidState("At least one acquisition provider is required.".into())
        })?;
        let queued = database.queue_missing_acquisition_jobs(primary.id())?;
        self.run_chain_jobs_bounded(database, app_data_dir, providers, queued, &is_cancelled)
    }

    pub(crate) fn run_library_track_chain(
        &self,
        database: &Database,
        app_data_dir: &Path,
        providers: &[Arc<dyn AcquisitionProvider>],
        library_track_id: i64,
        mut is_cancelled: impl FnMut() -> bool,
    ) -> Result<AcquisitionJob, AcquisitionError> {
        let primary = providers.first().ok_or_else(|| {
            AcquisitionError::InvalidState("At least one acquisition provider is required.".into())
        })?;
        let job = database
            .queue_acquisition_job(library_track_id, primary.id())?
            .ok_or_else(|| {
                AcquisitionError::InvalidState(
                    "Track is not currently tracked or already has a present local file.".into(),
                )
            })?;
        self.run_chain_job(database, app_data_dir, providers, job.id, &mut is_cancelled)?;
        database
            .acquisition_job(job.id)?
            .ok_or_else(|| AcquisitionError::InvalidState("acquisition job disappeared".into()))
    }

    pub(crate) fn retry_failed_chain(
        &self,
        database: &Database,
        app_data_dir: &Path,
        providers: &[Arc<dyn AcquisitionProvider>],
        is_cancelled: impl Fn() -> bool + Sync,
    ) -> Result<Vec<AcquisitionJob>, AcquisitionError> {
        let primary = providers.first().ok_or_else(|| {
            AcquisitionError::InvalidState("At least one acquisition provider is required.".into())
        })?;
        let failed = database.failed_acquisition_jobs()?;
        let mut queued = Vec::new();
        for failed_job in failed {
            if is_cancelled() {
                break;
            }
            let Some(job) =
                database.queue_acquisition_job(failed_job.library_track_id, primary.id())?
            else {
                continue;
            };
            queued.push(job);
        }
        self.run_chain_jobs_bounded(database, app_data_dir, providers, queued, &is_cancelled)
    }

    fn run_chain_jobs_bounded<F>(
        &self,
        database: &Database,
        app_data_dir: &Path,
        providers: &[Arc<dyn AcquisitionProvider>],
        jobs: Vec<AcquisitionJob>,
        is_cancelled: &F,
    ) -> Result<Vec<AcquisitionJob>, AcquisitionError>
    where
        F: Fn() -> bool + Sync,
    {
        if jobs.is_empty() {
            return Ok(Vec::new());
        }

        let worker_count = jobs.len().min(MAX_CONCURRENT_ACQUISITIONS);
        let next_job = std::sync::atomic::AtomicUsize::new(0);
        let (sender, receiver) = std::sync::mpsc::channel();

        thread::scope(|scope| {
            for _ in 0..worker_count {
                let sender = sender.clone();
                let jobs = &jobs;
                let next_job = &next_job;
                scope.spawn(move || {
                    loop {
                        let index = next_job.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(job) = jobs.get(index) else {
                            break;
                        };
                        let result = (|| -> Result<Option<AcquisitionJob>, AcquisitionError> {
                            let Some(current_job) = database.acquisition_job(job.id)? else {
                                return Ok(None);
                            };
                            if current_job.status != "queued" {
                                return Ok(None);
                            }

                            let mut cancelled = || is_cancelled();
                            if cancelled() {
                                database.finish_acquisition_job(job.id, "cancelled", None, None)?;
                            } else {
                                self.run_chain_job(
                                    database,
                                    app_data_dir,
                                    providers,
                                    job.id,
                                    &mut cancelled,
                                )?;
                            }
                            Ok(database.acquisition_job(job.id)?)
                        })();
                        if sender.send((index, result)).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(sender);
        });

        let mut ordered = std::iter::repeat_with(|| None)
            .take(jobs.len())
            .collect::<Vec<_>>();
        for (index, result) in receiver {
            ordered[index] = Some(result);
        }

        let mut completed = Vec::with_capacity(jobs.len());
        for result in ordered.into_iter().flatten() {
            if let Some(job) = result? {
                completed.push(job);
            }
        }
        Ok(completed)
    }

    pub(crate) fn run_selected_candidate<P: AcquisitionProvider + ?Sized>(
        &self,
        database: &Database,
        app_data_dir: &Path,
        provider: &P,
        job_id: i64,
        provider_token: &str,
        mut is_cancelled: impl FnMut() -> bool,
    ) -> Result<AcquisitionJob, AcquisitionError> {
        let job = database.acquisition_job(job_id)?.ok_or_else(|| {
            AcquisitionError::InvalidState(format!("acquisition job {job_id} not found"))
        })?;
        let query = database
            .acquisition_track_query(job.library_track_id)?
            .ok_or_else(|| {
                AcquisitionError::InvalidState("library track for acquisition was not found".into())
            })?;
        let candidate = job
            .candidates
            .iter()
            .find(|candidate| {
                candidate.provider_token == provider_token
                    && candidate
                        .provider
                        .as_deref()
                        .is_none_or(|candidate_provider| candidate_provider == provider.id())
            })
            .cloned()
            .ok_or_else(|| {
                AcquisitionError::InvalidState(
                    "Selected acquisition candidate is no longer available.".into(),
                )
            })?;
        database.set_acquisition_provider(job_id, provider.id())?;
        let _ = run_candidates(
            database,
            app_data_dir,
            provider,
            &job,
            &query,
            (vec![candidate], Vec::new()),
            &mut is_cancelled,
        )?;
        database
            .acquisition_job(job_id)?
            .ok_or_else(|| AcquisitionError::InvalidState("acquisition job disappeared".into()))
    }

    fn run_chain_job(
        &self,
        database: &Database,
        app_data_dir: &Path,
        providers: &[Arc<dyn AcquisitionProvider>],
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
        for chain_attempt in 0..MAX_CHAIN_ATTEMPTS {
            let mut review_candidates = Vec::new();
            let mut automatic_failure: Option<(String, ProviderError)> = None;
            let mut last_error = ProviderError::new(
                "noCandidatesAcrossProviders",
                "No configured acquisition provider returned a suitable match.",
                false,
            );

            for provider in providers {
                if is_cancelled() {
                    database.finish_acquisition_job(job_id, "cancelled", None, None)?;
                    return Ok(());
                }

                database.set_acquisition_provider(job_id, provider.id())?;
                database.set_acquisition_stage(job_id, "searching")?;

                match self.provider_health(provider.as_ref()) {
                    Ok(health) if health.available => {}
                    Ok(health) => {
                        last_error = ProviderError::new(
                            "providerUnavailable",
                            health
                                .message
                                .unwrap_or_else(|| format!("{} is unavailable", provider.id())),
                            true,
                        );
                        tracing::debug!(
                            job_id,
                            provider = provider.id(),
                            error_code = %last_error.code,
                            error = %last_error.message,
                            "acquisition provider unavailable"
                        );
                        continue;
                    }
                    Err(AcquisitionError::Provider(error)) => {
                        tracing::debug!(
                            job_id,
                            provider = provider.id(),
                            error_code = %error.code,
                            retryable = error.retryable,
                            error = %error.message,
                            "acquisition provider health check failed"
                        );
                        last_error = error;
                        continue;
                    }
                    Err(error) => return Err(error),
                }

                let mut candidates = match provider.search(&query) {
                    Ok(candidates) => candidates,
                    Err(error) => {
                        tracing::debug!(
                            job_id,
                            provider = provider.id(),
                            error_code = %error.code,
                            retryable = error.retryable,
                            error = %error.message,
                            "acquisition provider search failed"
                        );
                        last_error = error;
                        continue;
                    }
                };
                for candidate in &mut candidates {
                    candidate.provider = Some(provider.id().into());
                }
                let candidates = rank_candidates(&query, candidates);
                if candidates.is_empty() {
                    continue;
                }

                let exact_matches = candidates
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| compatible_exact_isrc_candidate(&query, candidate))
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>();

                let plausible = candidates
                    .iter()
                    .filter(|candidate| {
                        candidate_confidence_value(candidate) >= REVIEW_CANDIDATE_MIN_CONFIDENCE
                    })
                    .take(MAX_REVIEW_CANDIDATES)
                    .cloned()
                    .collect::<Vec<_>>();

                let automatic = if !exact_matches.is_empty() {
                    exact_matches
                        .iter()
                        .map(|&index| candidates[index].clone())
                        .collect()
                } else if !plausible.is_empty() && !requires_manual_resolution(&plausible) {
                    plausible
                        .iter()
                        .filter(|candidate| {
                            candidate_confidence_value(candidate) >= AUTO_CANDIDATE_MIN_CONFIDENCE
                        })
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                };

                if !automatic.is_empty() {
                    let failure = run_candidates(
                        database,
                        app_data_dir,
                        provider.as_ref(),
                        &job,
                        &query,
                        (automatic, Vec::new()),
                        is_cancelled,
                    )?;
                    let current = database.acquisition_job(job_id)?.ok_or_else(|| {
                        AcquisitionError::InvalidState("acquisition job disappeared".into())
                    })?;
                    if matches!(current.status.as_str(), "staged" | "cancelled") {
                        return Ok(());
                    }
                    if let Some(error) = failure {
                        tracing::debug!(
                            job_id,
                            provider = provider.id(),
                            error_code = %error.code,
                            retryable = error.retryable,
                            error = %error.message,
                            "automatic acquisition candidate failed"
                        );
                        last_error = error.clone();
                        automatic_failure.get_or_insert_with(|| (provider.id().to_owned(), error));
                    }
                    continue;
                }

                review_candidates.extend(candidates);
            }

            if let Some((provider, error)) = automatic_failure {
                if error.retryable && chain_attempt + 1 < MAX_CHAIN_ATTEMPTS {
                    thread::sleep(chain_retry_delay(chain_attempt));
                    continue;
                }
                database.set_acquisition_provider(job_id, &provider)?;
                tracing::warn!(
                    job_id,
                    provider = %provider,
                    error_code = %error.code,
                    retryable = error.retryable,
                    error = %error.message,
                    "acquisition job failed after provider chain"
                );
                database.finish_acquisition_job(
                    job_id,
                    "failed",
                    Some(&error.code),
                    Some(&error.message),
                )?;
                return Ok(());
            }

            if !review_candidates.is_empty() {
                database.require_acquisition_resolution(job_id, &review_candidates)?;
                return Ok(());
            }

            if last_error.retryable && chain_attempt + 1 < MAX_CHAIN_ATTEMPTS {
                thread::sleep(chain_retry_delay(chain_attempt));
                continue;
            }

            tracing::warn!(
                job_id,
                provider = %database
                    .acquisition_job(job_id)?
                    .map(|job| job.provider)
                    .unwrap_or_default(),
                error_code = %last_error.code,
                retryable = last_error.retryable,
                error = %last_error.message,
                "acquisition job failed after provider chain"
            );
            database.finish_acquisition_job(
                job_id,
                "failed",
                Some(&last_error.code),
                Some(&last_error.message),
            )?;
            return Ok(());
        }

        Ok(())
    }

    #[cfg(test)]
    fn run_job<P: AcquisitionProvider + ?Sized>(
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
        database.set_acquisition_stage(job_id, "searching")?;
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

        if database
            .acquisition_job(job_id)?
            .is_some_and(|current| current.status == "cancelled")
        {
            return Ok(());
        }

        if candidates.is_empty() {
            database.finish_acquisition_job(
                job_id,
                "failed",
                Some("noCandidates"),
                Some("No matching files were found for this track. Retry later."),
            )?;
            return Ok(());
        }

        let mut candidates = rank_candidates(&query, candidates);

        let compatible_exact_isrc = candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| compatible_exact_isrc_candidate(&query, candidate))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if compatible_exact_isrc.len() == 1 {
            let selected = candidates.remove(compatible_exact_isrc[0]);
            let deferred_candidates = candidates
                .into_iter()
                .filter(|candidate| {
                    candidate_confidence_value(candidate) >= REVIEW_CANDIDATE_MIN_CONFIDENCE
                })
                .take(MAX_REVIEW_CANDIDATES)
                .collect();
            return run_candidates(
                database,
                app_data_dir,
                provider,
                &job,
                &query,
                (vec![selected], deferred_candidates),
                is_cancelled,
            )
            .map(|_| ());
        }

        let candidates = candidates
            .into_iter()
            .filter(|candidate| {
                candidate_confidence_value(candidate) >= REVIEW_CANDIDATE_MIN_CONFIDENCE
            })
            .take(MAX_REVIEW_CANDIDATES)
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            database.finish_acquisition_job(
                job_id,
                "failed",
                Some("lowConfidenceCandidates"),
                Some("No provider candidate matched this track closely enough. Search again or try later."),
            )?;
            return Ok(());
        }

        if requires_manual_resolution(&candidates) {
            database.require_acquisition_resolution(job_id, &candidates)?;
            return Ok(());
        }

        let (automatic_candidates, deferred_candidates): (Vec<_>, Vec<_>) =
            candidates.into_iter().partition(|candidate| {
                candidate_confidence_value(candidate) >= AUTO_CANDIDATE_MIN_CONFIDENCE
            });

        run_candidates(
            database,
            app_data_dir,
            provider,
            &job,
            &query,
            (automatic_candidates, deferred_candidates),
            is_cancelled,
        )
        .map(|_| ())
    }
}

fn run_candidates<P: AcquisitionProvider + ?Sized>(
    database: &Database,
    app_data_dir: &Path,
    provider: &P,
    job: &AcquisitionJob,
    query: &TrackQuery,
    candidate_plan: (Vec<AcquisitionCandidate>, Vec<AcquisitionCandidate>),
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<Option<ProviderError>, AcquisitionError> {
    let job_id = job.id;
    let (candidates, deferred_candidates) = candidate_plan;

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
        if database
            .acquisition_job(job_id)?
            .is_some_and(|current| current.status == "cancelled")
        {
            return Ok(None);
        }
        if is_cancelled() {
            database.finish_acquisition_job(job_id, "cancelled", None, None)?;
            return Ok(None);
        }

        prepare_staging_directory(&staging_dir)?;
        let staging_path = staging_dir.to_string_lossy().into_owned();
        let attempt = i64::try_from(index + 1).unwrap_or(i64::MAX);
        database.begin_acquisition_attempt(job_id, attempt, &candidate, &staging_path)?;
        let request = AcquisitionRequest {
            library_track_id: job.library_track_id,
            query: query.clone(),
            candidate: candidate.clone(),
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
            ProviderOutcome::Staged(output_paths) => {
                let candidate = enrich_candidate_from_outputs(candidate, &output_paths);
                database.update_acquisition_candidate(job_id, &candidate)?;
                database.finish_acquisition_job(job_id, "staged", None, None)?;
                return Ok(None);
            }
            ProviderOutcome::Cancelled => {
                database.finish_acquisition_job(job_id, "cancelled", None, None)?;
                return Ok(None);
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

    if last_error.retryable && !deferred_candidates.is_empty() {
        database.require_acquisition_resolution(job_id, &deferred_candidates)?;
        return Ok(None);
    }

    database.finish_acquisition_job(
        job_id,
        "failed",
        Some(&last_error.code),
        Some(&last_error.message),
    )?;
    Ok(Some(last_error))
}

fn chain_retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(500_u64.saturating_mul(1_u64 << attempt.min(2)))
}

fn requires_manual_resolution(candidates: &[AcquisitionCandidate]) -> bool {
    let first = candidate_confidence_value(&candidates[0]);
    if first < AUTO_CANDIDATE_MIN_CONFIDENCE {
        return true;
    }
    if candidates.len() <= 1 {
        return false;
    }
    let runner_up = candidates
        .iter()
        .skip(1)
        .map(candidate_confidence_value)
        .max()
        .unwrap_or(0);
    first.saturating_sub(runner_up) < AUTO_CANDIDATE_MIN_MARGIN
}

pub(crate) fn rank_candidates(
    query: &TrackQuery,
    mut candidates: Vec<AcquisitionCandidate>,
) -> Vec<AcquisitionCandidate> {
    for candidate in &mut candidates {
        candidate.confidence = Some(candidate_confidence(query, candidate));
    }
    candidates.sort_by(|left, right| {
        let left_exact = compatible_exact_isrc_candidate(query, left);
        let right_exact = compatible_exact_isrc_candidate(query, right);
        right_exact.cmp(&left_exact).then_with(|| {
            if left_exact && right_exact {
                candidate_quality_cmp(right, left)
                    .then_with(|| {
                        candidate_confidence_value(right).cmp(&candidate_confidence_value(left))
                    })
                    .then_with(|| left.provider_token.cmp(&right.provider_token))
            } else {
                candidate_confidence_value(right)
                    .cmp(&candidate_confidence_value(left))
                    .then_with(|| candidate_quality_cmp(right, left))
                    .then_with(|| left.provider_token.cmp(&right.provider_token))
            }
        })
    });
    candidates
}

fn candidate_quality_cmp(
    left: &AcquisitionCandidate,
    right: &AcquisitionCandidate,
) -> std::cmp::Ordering {
    candidate_format_priority(left)
        .cmp(&candidate_format_priority(right))
        .then_with(|| {
            left.bit_depth
                .unwrap_or_default()
                .cmp(&right.bit_depth.unwrap_or_default())
        })
        .then_with(|| {
            left.sample_rate_hz
                .unwrap_or_default()
                .cmp(&right.sample_rate_hz.unwrap_or_default())
        })
        .then_with(|| {
            left.bitrate_kbps
                .unwrap_or_default()
                .cmp(&right.bitrate_kbps.unwrap_or_default())
        })
}

fn candidate_format_priority(candidate: &AcquisitionCandidate) -> u8 {
    match candidate
        .format
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "flac" => 3,
        "alac" | "wav" | "wave" | "aiff" | "aif" | "ape" | "wv" => 2,
        "" => 0,
        _ => 1,
    }
}

fn candidate_confidence(query: &TrackQuery, candidate: &AcquisitionCandidate) -> u8 {
    let mut confidence = 0_i32;
    if let Some(title) = candidate.title.as_deref() {
        if normalized_text(title) == normalized_text(&query.title) {
            confidence += 30;
        } else {
            confidence -= 45;
        }
    }
    if !candidate.artists.is_empty() {
        if candidate_artists_match(query, candidate) {
            confidence += 25;
        } else {
            confidence -= 35;
        }
    }
    if let (Some(expected), Some(actual)) = (query.album.as_deref(), candidate.album.as_deref())
        && normalized_text(expected) == normalized_text(actual)
    {
        confidence += 10;
    }
    if let (Some(expected), Some(actual)) = (query.duration_ms, candidate.duration_ms) {
        let difference = (expected - actual).abs();
        if difference <= 2_000 {
            confidence += 20;
        } else if difference <= 5_000 {
            confidence += 14;
        } else if difference <= 10_000 {
            confidence += 6;
        } else {
            confidence -= 25;
        }
    }

    match (
        normalized_isrc(query.isrc.as_deref()),
        normalized_isrc(candidate.isrc.as_deref()),
    ) {
        (Some(expected), Some(actual)) if expected == actual => confidence += 40,
        (Some(_), Some(_)) => confidence -= 45,
        _ => {}
    }

    confidence.clamp(0, 100) as u8
}

fn candidate_confidence_value(candidate: &AcquisitionCandidate) -> u8 {
    candidate.confidence.unwrap_or_default()
}

fn compatible_exact_isrc_candidate(query: &TrackQuery, candidate: &AcquisitionCandidate) -> bool {
    if !exact_isrc_match(query.isrc.as_deref(), candidate.isrc.as_deref()) {
        return false;
    }

    if candidate
        .title
        .as_deref()
        .is_some_and(|title| !exact_isrc_titles_compatible(&query.title, title))
    {
        return false;
    }

    if !candidate.artists.is_empty() && !candidate_artists_match(query, candidate) {
        return false;
    }

    if let (Some(expected), Some(actual)) = (query.duration_ms, candidate.duration_ms)
        && (expected - actual).abs() > 10_000
    {
        return false;
    }

    true
}

fn exact_isrc_titles_compatible(expected: &str, actual: &str) -> bool {
    let expected = normalized_text(expected);
    let actual = normalized_text(actual);
    if expected == actual {
        return true;
    }
    if expected.len().min(actual.len()) < 6 {
        return false;
    }
    expected.starts_with(&actual) || actual.starts_with(&expected)
}

fn candidate_artists_match(query: &TrackQuery, candidate: &AcquisitionCandidate) -> bool {
    candidate.artists.iter().any(|artist| {
        let direct = query
            .artists
            .iter()
            .any(|expected| normalized_text(artist) == normalized_text(expected));
        direct
            || artist
                .split([',', ';', '&'])
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .any(|part| {
                    query
                        .artists
                        .iter()
                        .any(|expected| normalized_text(part) == normalized_text(expected))
                })
    })
}

fn exact_isrc_match(expected: Option<&str>, actual: Option<&str>) -> bool {
    matches!(
        (normalized_isrc(expected), normalized_isrc(actual)),
        (Some(expected), Some(actual)) if expected == actual
    )
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

fn normalized_text(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn enrich_candidate_from_outputs(
    mut candidate: AcquisitionCandidate,
    output_paths: &[String],
) -> AcquisitionCandidate {
    for output_path in output_paths {
        let path = Path::new(output_path);
        let Ok(tagged_file) = lofty::read_from_path(path) else {
            continue;
        };
        let properties = tagged_file.properties();
        if let Some(value) = properties.audio_bitrate() {
            candidate.bitrate_kbps = Some(i64::from(value));
        }
        if let Some(value) = properties.sample_rate() {
            candidate.sample_rate_hz = Some(i64::from(value));
        }
        if let Some(value) = properties.bit_depth() {
            candidate.bit_depth = Some(i64::from(value));
        }
        if let Ok(metadata) = fs::metadata(path) {
            candidate.size_bytes = Some(i64::try_from(metadata.len()).unwrap_or(i64::MAX));
        }
        if let Some(extension) = path
            .extension()
            .and_then(|extension| extension.to_str())
            .filter(|extension| !extension.is_empty())
        {
            candidate.format = Some(extension.to_owned());
        }
        break;
    }
    candidate
}

enum ProviderOutcome {
    Staged(Vec<String>),
    Cancelled,
    Failed(ProviderError),
}

fn wait_for_provider_job<P: AcquisitionProvider + ?Sized>(
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
            Ok(ProviderJobStatus::Succeeded { output_paths }) => {
                return ProviderOutcome::Staged(output_paths);
            }
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
        sync::atomic::{AtomicU64, AtomicUsize, Ordering},
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
        search_calls: usize,
        unavailable: bool,
    }

    #[derive(Clone)]
    struct FakeProvider {
        id: &'static str,
        state: Arc<Mutex<FakeState>>,
    }

    impl Default for FakeProvider {
        fn default() -> Self {
            Self {
                id: "fake",
                state: Arc::new(Mutex::new(FakeState::default())),
            }
        }
    }

    impl FakeProvider {
        fn with_candidates(tokens: &[&str]) -> Self {
            Self::named_with_candidates("fake", tokens)
        }

        fn named_with_candidates(id: &'static str, tokens: &[&str]) -> Self {
            let provider = Self {
                id,
                ..Self::default()
            };
            provider.state.lock().unwrap().candidates = tokens
                .iter()
                .map(|token| AcquisitionCandidate {
                    provider: None,
                    provider_token: (*token).into(),
                    source: None,
                    file_name: None,
                    title: Some("Missing Song".into()),
                    artists: vec!["Artist".into()],
                    album: Some("Album".into()),
                    duration_ms: Some(180_000),
                    format: Some("flac".into()),
                    size_bytes: None,
                    bitrate_kbps: None,
                    sample_rate_hz: None,
                    bit_depth: None,
                    confidence: None,
                    isrc: None,
                    recording_id: None,
                    release_id: None,
                })
                .collect();
            provider
        }
    }

    impl AcquisitionProvider for FakeProvider {
        fn id(&self) -> &'static str {
            self.id
        }

        fn health(&self) -> Result<ProviderHealth, ProviderError> {
            let unavailable = self.state.lock().unwrap().unavailable;
            Ok(ProviderHealth {
                available: !unavailable,
                version: Some("test".into()),
                message: unavailable.then(|| format!("{} unavailable", self.id)),
            })
        }

        fn search(&self, _query: &TrackQuery) -> Result<Vec<AcquisitionCandidate>, ProviderError> {
            let mut state = self.state.lock().unwrap();
            state.search_calls += 1;
            Ok(state.candidates.clone())
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

    #[derive(Default)]
    struct ConcurrencyProbeProvider {
        active: AtomicUsize,
        max_active: AtomicUsize,
        next_job: AtomicUsize,
    }

    impl AcquisitionProvider for ConcurrencyProbeProvider {
        fn id(&self) -> &'static str {
            "parallel-probe"
        }

        fn health(&self) -> Result<ProviderHealth, ProviderError> {
            Ok(ProviderHealth {
                available: true,
                version: Some("test".into()),
                message: None,
            })
        }

        fn search(&self, _query: &TrackQuery) -> Result<Vec<AcquisitionCandidate>, ProviderError> {
            Ok(vec![AcquisitionCandidate {
                provider: None,
                provider_token: "parallel-candidate".into(),
                source: None,
                file_name: None,
                title: Some("Missing Song".into()),
                artists: vec!["Artist".into()],
                album: Some("Album".into()),
                duration_ms: Some(180_000),
                format: Some("flac".into()),
                size_bytes: None,
                bitrate_kbps: None,
                sample_rate_hz: None,
                bit_depth: None,
                confidence: None,
                isrc: None,
                recording_id: None,
                release_id: None,
            }])
        }

        fn acquire(&self, _request: &AcquisitionRequest) -> Result<ProviderJob, ProviderError> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(active, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(80));
            self.active.fetch_sub(1, Ordering::SeqCst);
            let job = self.next_job.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(ProviderJob {
                provider_job_id: format!("parallel-job-{job}"),
            })
        }

        fn status(&self, provider_job_id: &str) -> Result<ProviderJobStatus, ProviderError> {
            Ok(ProviderJobStatus::Succeeded {
                output_paths: vec![format!("/{provider_job_id}.flac")],
            })
        }

        fn cancel(&self, _provider_job_id: &str) -> Result<(), ProviderError> {
            Ok(())
        }
    }

    fn database(name: &str) -> (TestPath, Database) {
        let path = TestPath::new(name);
        fs::create_dir_all(&path.0).unwrap();
        let database = Database::open(path.0.join("refrain.sqlite3")).unwrap();
        (path, database)
    }

    fn write_test_wav(path: &Path, sample_rate: u32, bit_depth: u16) {
        let channels = 2_u16;
        let bytes_per_sample = u32::from(bit_depth) / 8;
        let block_align = u32::from(channels) * bytes_per_sample;
        let data_len = block_align * 32;
        let byte_rate = sample_rate * block_align;
        let mut bytes = Vec::with_capacity(44 + usize::try_from(data_len).unwrap());
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&(block_align as u16).to_le_bytes());
        bytes.extend_from_slice(&bit_depth.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.resize(44 + usize::try_from(data_len).unwrap(), 0);
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn completed_audio_enriches_candidate_with_actual_file_quality() {
        let path = TestPath::new("candidate-quality");
        fs::create_dir_all(&path.0).unwrap();
        let output = path.0.join("download.wav");
        write_test_wav(&output, 96_000, 24);
        let candidate = FakeProvider::with_candidates(&["quality"])
            .state
            .lock()
            .unwrap()
            .candidates[0]
            .clone();

        let enriched =
            enrich_candidate_from_outputs(candidate, &[output.to_string_lossy().into_owned()]);

        assert_eq!(enriched.format.as_deref(), Some("wav"));
        assert_eq!(enriched.sample_rate_hz, Some(96_000));
        assert_eq!(enriched.bit_depth, Some(24));
        assert!(enriched.size_bytes.is_some_and(|value| value > 44));
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
        database
            .set_source_collection_tracking(collection_id, true)
            .unwrap();
        library_track_id
    }

    fn seed_missing_tracks(database: &Database, count: usize) -> Vec<i64> {
        let account_id = database
            .upsert_source_account(&SourceAccount {
                provider: "spotify".into(),
                provider_account_id: "parallel-listener".into(),
                display_name: Some("Parallel Listener".into()),
                image_url: None,
                client_id: "client".into(),
            })
            .unwrap();
        let collection_id = database
            .upsert_source_collection(&SourceCollection {
                source_account_id: account_id,
                provider_collection_id: "parallel-playlist".into(),
                kind: "playlist".into(),
                name: "Parallel Playlist".into(),
                snapshot_id: None,
                owner_provider_id: None,
                is_accessible: true,
                access_issue: None,
                image_url: None,
                external_url: None,
                album_metadata: None,
            })
            .unwrap();

        let mut library_track_ids = Vec::with_capacity(count);
        let mut entries = Vec::with_capacity(count);
        for index in 0..count {
            let mut track = source_track(&format!("parallel-{index}"));
            track.isrc = Some(format!("TEST{index:08}"));
            let source_track_id = database.upsert_source_track(&track).unwrap();
            let library_track_id = database
                .create_library_track_from_source(source_track_id)
                .unwrap();
            database
                .persist_track_link(source_track_id, library_track_id, "existing", 10_000)
                .unwrap();
            library_track_ids.push(library_track_id);
            entries.push(CollectionEntry {
                position: i64::try_from(index).unwrap(),
                source_track_id: Some(source_track_id),
                provider_item_uri: None,
                item_type: "track".into(),
                added_at: None,
                unavailable_reason: None,
            });
        }
        database
            .replace_collection_entries(collection_id, &entries)
            .unwrap();
        database
            .set_source_collection_tracking(collection_id, true)
            .unwrap();
        library_track_ids
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
    fn bulk_acquisition_runs_multiple_tracks_in_parallel_with_a_bounded_pool() {
        let (path, database) = database("bounded-parallel-acquisition");
        seed_missing_tracks(&database, MAX_CONCURRENT_ACQUISITIONS + 2);
        let provider = Arc::new(ConcurrencyProbeProvider::default());
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![provider.clone()];

        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        assert_eq!(jobs.len(), MAX_CONCURRENT_ACQUISITIONS + 2);
        assert!(jobs.iter().all(|job| job.status == "staged"));
        let max_active = provider.max_active.load(Ordering::SeqCst);
        assert!(max_active >= 2, "expected overlapping acquisition work");
        assert!(max_active <= MAX_CONCURRENT_ACQUISITIONS);
    }

    #[test]
    fn interrupted_running_acquisition_is_recovered_and_requeued_on_next_sync() {
        let (path, database) = database("interrupted-acquisition-recovery");
        seed_missing_track(&database, false);
        let job = database
            .queue_missing_acquisition_jobs("monochrome")
            .unwrap()
            .remove(0);
        let candidate = AcquisitionCandidate {
            provider: Some("monochrome".into()),
            provider_token: r#"{"trackId":"123","source":"tracks"}"#.into(),
            source: None,
            file_name: Some("Missing Song.flac".into()),
            title: Some("Missing Song".into()),
            artists: vec!["Artist".into()],
            album: Some("Album".into()),
            duration_ms: Some(180_000),
            format: Some("FLAC".into()),
            size_bytes: None,
            bitrate_kbps: None,
            sample_rate_hz: None,
            bit_depth: None,
            confidence: Some(100),
            isrc: Some("TEST12345678".into()),
            recording_id: None,
            release_id: None,
        };
        database
            .begin_acquisition_attempt(
                job.id,
                1,
                &candidate,
                &path.0.join("runtime/acquisition").to_string_lossy(),
            )
            .unwrap();
        database
            .set_acquisition_provider_job_id(job.id, "monochrome-stale-job")
            .unwrap();

        assert_eq!(database.recover_interrupted_acquisition_jobs().unwrap(), 1);
        let recovered = database.acquisition_job(job.id).unwrap().unwrap();
        assert_eq!(recovered.status, "failed");
        assert_eq!(recovered.stage.as_deref(), Some("failed"));
        assert_eq!(
            recovered.error_code.as_deref(),
            Some("acquisitionInterrupted")
        );
        assert_eq!(recovered.provider_job_id, None);

        let requeued = database
            .queue_missing_acquisition_jobs("monochrome")
            .unwrap();
        assert_eq!(requeued.len(), 1);
        assert_eq!(requeued[0].status, "queued");
        assert_eq!(requeued[0].stage.as_deref(), Some("queued"));
    }

    #[test]
    fn clearing_acquisition_session_discards_prior_failed_jobs_and_requeues_cleanly() {
        let (_path, database) = database("fresh-acquisition-session");
        seed_missing_track(&database, false);
        let job = database
            .queue_missing_acquisition_jobs("monochrome")
            .unwrap()
            .remove(0);
        database
            .finish_acquisition_job(
                job.id,
                "failed",
                Some("downloadTransportFailed"),
                Some("old session failure"),
            )
            .unwrap();

        assert_eq!(database.acquisition_jobs_page(0, 10).unwrap().total, 1);
        assert_eq!(database.clear_acquisition_jobs().unwrap(), 1);
        assert_eq!(database.acquisition_jobs_page(0, 10).unwrap().total, 0);

        let requeued = database
            .queue_missing_acquisition_jobs("monochrome")
            .unwrap();
        assert_eq!(requeued.len(), 1);
        assert_eq!(requeued[0].status, "queued");
        assert_eq!(requeued[0].stage.as_deref(), Some("queued"));
        assert_eq!(requeued[0].error_code, None);
        assert_eq!(requeued[0].error_message, None);
        assert!(requeued[0].candidates.is_empty());
    }

    #[test]
    fn staging_projection_includes_tracked_missing_track_before_job_exists() {
        let (_path, database) = database("staging-projection");
        let library_track_id = seed_missing_track(&database, false);

        let page = database.staging_items_page(0, 10).unwrap();

        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].library_track_id, library_track_id);
        assert_eq!(page.items[0].job_id, None);
        assert_eq!(page.items[0].stage, "needsLocalCopy");

        database.set_source_collection_tracking(1, false).unwrap();
        assert_eq!(database.staging_items_page(0, 10).unwrap().total, 0);
    }

    #[test]
    fn excluding_a_staging_track_disables_every_linked_source_reference() {
        let (_path, database) = database("staging-exclude-tracking");
        let library_track_id = seed_missing_track(&database, true);

        assert_eq!(database.staging_items_page(0, 10).unwrap().total, 1);

        database
            .exclude_library_track_from_tracking(library_track_id)
            .unwrap();

        assert!(database.tracked_source_track_ids().unwrap().is_empty());
        assert_eq!(database.staging_items_page(0, 10).unwrap().total, 0);
    }

    #[test]
    fn staging_projection_scores_legacy_resolution_candidates_without_confidence() {
        let (_path, database) = database("legacy-resolution-confidence");
        seed_missing_track(&database, false);
        let jobs = database.queue_missing_acquisition_jobs("fake").unwrap();
        let provider = FakeProvider::with_candidates(&["legacy-candidate"]);
        let candidate = provider.state.lock().unwrap().candidates[0].clone();
        assert_eq!(candidate.confidence, None);
        database
            .require_acquisition_resolution(jobs[0].id, &[candidate])
            .unwrap();

        let staging = database.staging_items_page(0, 10).unwrap();

        assert_eq!(staging.items[0].candidates[0].confidence, Some(85));
    }

    #[test]
    fn retryable_failure_advances_to_a_distinct_candidate() {
        let (path, database) = database("retry");
        seed_missing_track(&database, false);
        let provider = FakeProvider::with_candidates(&["candidate-a", "candidate-b"]);
        {
            let mut state = provider.state.lock().unwrap();
            state.candidates[0].title = Some("Missing Song".into());
            state.candidates[0].artists = vec!["Artist".into()];
            state.candidates[0].album = Some("Album".into());
            state.candidates[0].duration_ms = Some(180_000);
            state.candidates[1].title = Some("Missing Song".into());
            state.candidates[1].artists = vec!["Artist".into()];
            state.candidates[1].album = Some("Different Album".into());
            state.candidates[1].duration_ms = Some(180_000);
        }
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
    fn retryable_failure_does_not_surface_unrelated_fallback_for_resolution() {
        let (path, database) = database("retry-low-confidence");
        seed_missing_track(&database, false);
        let provider = FakeProvider::with_candidates(&["candidate-a", "candidate-b"]);
        {
            let mut state = provider.state.lock().unwrap();
            state.candidates[0].title = Some("Missing Song".into());
            state.candidates[0].artists = vec!["Artist".into()];
            state.candidates[0].album = Some("Album".into());
            state.candidates[0].duration_ms = Some(180_000);
            state.candidates[1].title = Some("Different Song".into());
            state.candidates[1].artists = vec!["Different Artist".into()];
            state.candidates[1].album = Some("Different Album".into());
            state.candidates[1].duration_ms = Some(240_000);
        }
        provider
            .state
            .lock()
            .unwrap()
            .acquire_results
            .push_back(Err(ProviderError::new("busy", "try another", true)));

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "failed");
        assert_eq!(jobs[0].stage.as_deref(), Some("failed"));
        assert_eq!(jobs[0].error_code.as_deref(), Some("busy"));
        assert!(jobs[0].candidates.is_empty());
        assert_eq!(
            provider.state.lock().unwrap().acquired_tokens,
            vec!["candidate-a"]
        );
    }

    #[test]
    fn multiple_candidates_pause_for_manual_resolution() {
        let (path, database) = database("manual-resolution");
        seed_missing_track(&database, false);
        let provider = FakeProvider::with_candidates(&["candidate-a", "candidate-b"]);

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, "failed");
        assert_eq!(jobs[0].stage.as_deref(), Some("needsResolution"));
        assert_eq!(jobs[0].error_code.as_deref(), Some("needsResolution"));
        assert!(provider.state.lock().unwrap().acquired_tokens.is_empty());

        let staging = database.staging_items_page(0, 10).unwrap();
        assert_eq!(staging.items[0].candidates.len(), 2);
    }

    #[test]
    fn unique_exact_isrc_candidate_is_selected_without_manual_resolution() {
        let (path, database) = database("exact-isrc-auto");
        seed_missing_track(&database, false);
        let provider = FakeProvider::with_candidates(&["metadata-match", "exact-isrc"]);
        provider.state.lock().unwrap().candidates[1].isrc = Some("TEST12345678".into());

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "staged");
        assert_eq!(jobs[0].stage.as_deref(), Some("downloaded"));
        assert_eq!(
            provider.state.lock().unwrap().acquired_tokens,
            vec!["exact-isrc"]
        );
    }

    #[test]
    fn exact_isrc_does_not_override_contradictory_title_metadata() {
        let (path, database) = database("exact-isrc-title-conflict");
        seed_missing_track(&database, false);
        let provider = FakeProvider::with_candidates(&["wrong-title"]);
        {
            let mut state = provider.state.lock().unwrap();
            state.candidates[0].title = Some("Different Song".into());
            state.candidates[0].isrc = Some("TEST12345678".into());
        }

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "failed");
        assert_eq!(
            jobs[0].error_code.as_deref(),
            Some("lowConfidenceCandidates")
        );
        assert!(provider.state.lock().unwrap().acquired_tokens.is_empty());
    }

    #[test]
    fn manual_resolution_filters_noise_and_orders_plausible_candidates() {
        let (path, database) = database("rank-resolution");
        seed_missing_track(&database, false);
        let provider = FakeProvider::with_candidates(&["junk", "review", "stronger"]);
        {
            let mut state = provider.state.lock().unwrap();
            state.candidates[0].title = Some("Completely Different".into());
            state.candidates[0].artists = vec!["Someone Else".into()];
            state.candidates[0].album = Some("Wrong Album".into());
            state.candidates[0].duration_ms = Some(260_000);

            state.candidates[1].album = None;
            state.candidates[1].duration_ms = None;

            state.candidates[2].duration_ms = Some(188_000);
        }

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();

        assert_eq!(jobs[0].stage.as_deref(), Some("needsResolution"));
        assert_eq!(jobs[0].candidates.len(), 2);
        assert_eq!(jobs[0].candidates[0].provider_token, "stronger");
        assert_eq!(jobs[0].candidates[1].provider_token, "review");
        assert!(provider.state.lock().unwrap().acquired_tokens.is_empty());
    }

    #[test]
    fn low_confidence_single_candidate_fails_without_manual_resolution() {
        let (path, database) = database("single-low-confidence");
        seed_missing_track(&database, false);
        let provider = FakeProvider::with_candidates(&["wrong-candidate"]);
        provider.state.lock().unwrap().candidates[0] = AcquisitionCandidate {
            provider: None,
            provider_token: "wrong-candidate".into(),
            source: None,
            file_name: Some("Different Song.flac".into()),
            title: Some("Different Song".into()),
            artists: vec!["Different Artist".into()],
            album: Some("Different Album".into()),
            duration_ms: Some(240_000),
            format: Some("flac".into()),
            size_bytes: None,
            bitrate_kbps: None,
            sample_rate_hz: None,
            bit_depth: None,
            confidence: None,
            isrc: None,
            recording_id: None,
            release_id: None,
        };

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, "failed");
        assert_eq!(jobs[0].stage.as_deref(), Some("failed"));
        assert_eq!(
            jobs[0].error_code.as_deref(),
            Some("lowConfidenceCandidates")
        );
        assert!(jobs[0].candidates.is_empty());
        assert!(provider.state.lock().unwrap().acquired_tokens.is_empty());
    }

    #[test]
    fn selected_resolution_candidate_downloads_without_researching() {
        let (path, database) = database("manual-resolution-download");
        seed_missing_track(&database, false);
        let provider = FakeProvider::with_candidates(&["candidate-a", "candidate-b"]);

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();
        assert_eq!(jobs[0].stage.as_deref(), Some("needsResolution"));

        let resolved = AcquisitionCoordinator
            .run_selected_candidate(
                &database,
                &path.0,
                &provider,
                jobs[0].id,
                "candidate-b",
                || false,
            )
            .unwrap();

        assert_eq!(resolved.status, "staged");
        assert_eq!(resolved.stage.as_deref(), Some("downloaded"));
        assert_eq!(
            provider.state.lock().unwrap().acquired_tokens,
            vec!["candidate-b"]
        );
    }

    #[test]
    fn failed_job_is_requeued_on_next_run_when_track_is_still_missing() {
        let (path, database) = database("retry-failed-job");
        seed_missing_track(&database, false);
        let provider = FakeProvider::default();

        let first_run = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();
        assert_eq!(first_run.len(), 1);
        assert_eq!(first_run[0].status, "failed");
        assert_eq!(first_run[0].error_code.as_deref(), Some("noCandidates"));
        assert_eq!(
            first_run[0].error_message.as_deref(),
            Some("No matching files were found for this track. Retry later.")
        );
        let job_id = first_run[0].id;

        provider.state.lock().unwrap().candidates = vec![AcquisitionCandidate {
            provider: None,
            provider_token: "candidate-a".into(),
            source: None,
            file_name: None,
            title: Some("Missing Song".into()),
            artists: vec!["Artist".into()],
            album: Some("Album".into()),
            duration_ms: Some(180_000),
            format: Some("flac".into()),
            size_bytes: None,
            bitrate_kbps: None,
            sample_rate_hz: None,
            bit_depth: None,
            confidence: None,
            isrc: None,
            recording_id: None,
            release_id: None,
        }];

        let second_run = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();

        assert_eq!(second_run.len(), 1);
        assert_eq!(second_run[0].id, job_id);
        assert_eq!(second_run[0].status, "staged");
        assert_eq!(database.acquisition_jobs_page(0, 10).unwrap().total, 1);
    }

    #[test]
    fn provider_switch_requeues_unresolved_job_for_new_provider() {
        let (_path, database) = database("provider-switch");
        seed_missing_track(&database, false);
        let jobs = database.queue_missing_acquisition_jobs("sockseek").unwrap();
        assert_eq!(jobs.len(), 1);
        database
            .require_acquisition_resolution(
                jobs[0].id,
                &[AcquisitionCandidate {
                    provider: Some("sockseek".into()),
                    provider_token: "old-sockseek-candidate".into(),
                    source: None,
                    file_name: None,
                    title: Some("Missing Song".into()),
                    artists: vec!["Artist".into()],
                    album: Some("Album".into()),
                    duration_ms: Some(180_000),
                    format: Some("flac".into()),
                    size_bytes: None,
                    bitrate_kbps: None,
                    sample_rate_hz: None,
                    bit_depth: None,
                    confidence: None,
                    isrc: None,
                    recording_id: None,
                    release_id: None,
                }],
            )
            .unwrap();

        let requeued = database
            .queue_missing_acquisition_jobs("monochrome")
            .unwrap();

        assert_eq!(requeued.len(), 1);
        assert_eq!(requeued[0].id, jobs[0].id);
        assert_eq!(requeued[0].provider, "monochrome");
        assert_eq!(requeued[0].status, "queued");
        assert_eq!(requeued[0].stage.as_deref(), Some("queued"));
        assert!(requeued[0].candidates.is_empty());
    }

    #[test]
    fn provider_chain_falls_through_to_lower_priority_match() {
        let (path, database) = database("provider-chain-fallback");
        seed_missing_track(&database, false);
        let first = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["weak-match"],
        ));
        {
            let mut state = first.state.lock().unwrap();
            state.candidates[0].title = Some("Different Song".into());
            state.candidates[0].artists = vec!["Different Artist".into()];
            state.candidates[0].album = Some("Different Album".into());
            state.candidates[0].duration_ms = Some(260_000);
        }
        let second = Arc::new(FakeProvider::named_with_candidates(
            "sockseek",
            &["good-match"],
        ));
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![first.clone(), second.clone()];

        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "staged");
        assert_eq!(jobs[0].provider, "sockseek");
        assert_eq!(first.state.lock().unwrap().search_calls, 1);
        assert!(first.state.lock().unwrap().acquired_tokens.is_empty());
        assert_eq!(second.state.lock().unwrap().search_calls, 1);
        assert_eq!(
            second.state.lock().unwrap().acquired_tokens,
            vec!["good-match"]
        );
    }

    #[test]
    fn provider_chain_skips_unavailable_provider_and_continues() {
        let (path, database) = database("provider-chain-unavailable");
        seed_missing_track(&database, false);
        let first = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["unavailable-match"],
        ));
        first.state.lock().unwrap().unavailable = true;
        let second = Arc::new(FakeProvider::named_with_candidates(
            "sockseek",
            &["available-match"],
        ));
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![first.clone(), second.clone()];

        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "staged");
        assert_eq!(jobs[0].provider, "sockseek");
        assert_eq!(first.state.lock().unwrap().search_calls, 0);
        assert_eq!(second.state.lock().unwrap().search_calls, 1);
    }

    #[test]
    fn provider_chain_stops_after_higher_priority_provider_stages_match() {
        let (path, database) = database("provider-chain-priority-stop");
        seed_missing_track(&database, false);
        let first = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["priority-match"],
        ));
        let second = Arc::new(FakeProvider::named_with_candidates(
            "sockseek",
            &["unused-match"],
        ));
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![first.clone(), second.clone()];

        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "staged");
        assert_eq!(jobs[0].provider, "monochrome");
        assert_eq!(first.state.lock().unwrap().search_calls, 1);
        assert_eq!(
            first.state.lock().unwrap().acquired_tokens,
            vec!["priority-match"]
        );
        assert_eq!(second.state.lock().unwrap().search_calls, 0);
    }

    #[test]
    fn provider_chain_falls_through_after_retryable_download_failure() {
        let (path, database) = database("provider-chain-download-fallback");
        seed_missing_track(&database, false);
        let first = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["mono-match"],
        ));
        first
            .state
            .lock()
            .unwrap()
            .acquire_results
            .push_back(Err(ProviderError::new(
                "downloadTransportFailed",
                "temporary transport failure",
                true,
            )));
        let second = Arc::new(FakeProvider::named_with_candidates(
            "sockseek",
            &["sock-match"],
        ));
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![first.clone(), second.clone()];

        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "staged");
        assert_eq!(jobs[0].provider, "sockseek");
        assert_eq!(first.state.lock().unwrap().search_calls, 1);
        assert_eq!(second.state.lock().unwrap().search_calls, 1);
        assert_eq!(
            second.state.lock().unwrap().acquired_tokens,
            vec!["sock-match"]
        );
    }

    #[test]
    fn retryable_transport_failure_retries_the_provider_chain_automatically() {
        let (path, database) = database("provider-chain-auto-retry");
        seed_missing_track(&database, false);
        let provider = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["exact-match"],
        ));
        {
            let mut state = provider.state.lock().unwrap();
            state.candidates[0].isrc = Some("TEST12345678".into());
            state.acquire_results.push_back(Err(ProviderError::new(
                "downloadTransportFailed",
                "temporary transport failure",
                true,
            )));
            state.acquire_results.push_back(Err(ProviderError::new(
                "downloadTransportFailed",
                "temporary transport failure",
                true,
            )));
        }
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![provider.clone()];

        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "staged");
        assert_eq!(jobs[0].provider, "monochrome");
        assert_eq!(provider.state.lock().unwrap().search_calls, 3);
        assert_eq!(
            provider.state.lock().unwrap().acquired_tokens,
            vec!["exact-match", "exact-match", "exact-match"]
        );
    }

    #[test]
    fn exact_match_transport_failure_does_not_become_manual_resolution() {
        let (path, database) = database("provider-chain-exact-download-failure");
        seed_missing_track(&database, false);
        let first = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["exact-match"],
        ));
        {
            let mut state = first.state.lock().unwrap();
            state.candidates[0].isrc = Some("TEST12345678".into());
            for _ in 0..MAX_CHAIN_ATTEMPTS {
                state.acquire_results.push_back(Err(ProviderError::new(
                    "downloadTransportFailed",
                    "temporary transport failure",
                    true,
                )));
            }
        }
        let second = Arc::new(FakeProvider::named_with_candidates(
            "sockseek",
            &["weak-fallback"],
        ));
        {
            let mut state = second.state.lock().unwrap();
            state.candidates[0].title = Some("Different Song".into());
            state.candidates[0].artists = vec!["Different Artist".into()];
            state.candidates[0].album = Some("Different Album".into());
            state.candidates[0].duration_ms = Some(260_000);
        }
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![first.clone(), second];

        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "failed");
        assert_eq!(jobs[0].stage.as_deref(), Some("failed"));
        assert_eq!(
            jobs[0].error_code.as_deref(),
            Some("downloadTransportFailed")
        );
        assert!(jobs[0].candidates.is_empty());
        assert_eq!(first.state.lock().unwrap().search_calls, MAX_CHAIN_ATTEMPTS);
    }

    #[test]
    fn multiple_compatible_exact_isrc_candidates_auto_select_one_recording() {
        let (path, database) = database("provider-chain-exact-isrc-tie");
        seed_missing_track(&database, false);
        let first = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["release-a", "release-b"],
        ));
        {
            let mut state = first.state.lock().unwrap();
            for candidate in &mut state.candidates {
                candidate.isrc = Some("TEST12345678".into());
            }
            state.candidates[0].release_id = Some("release-1".into());
            state.candidates[1].release_id = Some("release-2".into());
        }
        let second = Arc::new(FakeProvider::named_with_candidates("sockseek", &["unused"]));
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![first.clone(), second.clone()];

        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "staged");
        assert_eq!(jobs[0].provider, "monochrome");
        assert_eq!(first.state.lock().unwrap().acquired_tokens.len(), 1);
        assert_eq!(second.state.lock().unwrap().search_calls, 0);
    }

    #[test]
    fn exact_isrc_transport_failure_tries_another_exact_candidate_before_provider_fallback() {
        let (path, database) = database("provider-chain-exact-isrc-retry-sibling");
        seed_missing_track(&database, false);
        let first = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["release-a", "release-b"],
        ));
        {
            let mut state = first.state.lock().unwrap();
            for candidate in &mut state.candidates {
                candidate.isrc = Some("TEST12345678".into());
            }
            state.acquire_results.push_back(Err(ProviderError::new(
                "downloadTransportFailed",
                "temporary transport failure",
                true,
            )));
        }
        let second = Arc::new(FakeProvider::named_with_candidates("sockseek", &["unused"]));
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![first.clone(), second.clone()];

        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        assert_eq!(jobs[0].status, "staged");
        assert_eq!(jobs[0].provider, "monochrome");
        assert_eq!(
            first.state.lock().unwrap().acquired_tokens,
            vec!["release-a", "release-b"]
        );
        assert_eq!(second.state.lock().unwrap().search_calls, 0);
    }

    #[test]
    fn equal_match_confidence_prefers_flac_and_higher_quality() {
        let query = TrackQuery {
            library_track_id: 1,
            title: "Missing Song".into(),
            artists: vec!["Artist".into()],
            album: Some("Album".into()),
            duration_ms: Some(180_000),
            isrc: None,
            version_kind: None,
            version_detail: None,
        };
        let provider = FakeProvider::with_candidates(&["mp3", "flac-low", "flac-high"]);
        let mut candidates = provider.state.lock().unwrap().candidates.clone();
        candidates[0].format = Some("mp3".into());
        candidates[0].bitrate_kbps = Some(320);
        candidates[1].format = Some("flac".into());
        candidates[1].sample_rate_hz = Some(44_100);
        candidates[1].bit_depth = Some(16);
        candidates[2].format = Some("flac".into());
        candidates[2].sample_rate_hz = Some(96_000);
        candidates[2].bit_depth = Some(24);

        let ranked = rank_candidates(&query, candidates);

        assert_eq!(ranked[0].provider_token, "flac-high");
        assert_eq!(ranked[1].provider_token, "flac-low");
        assert_eq!(ranked[2].provider_token, "mp3");
    }

    #[test]
    fn antra_prefers_hi_res_flac_then_cd_flac() {
        let query = TrackQuery {
            library_track_id: 1,
            title: "Missing Song".into(),
            artists: vec!["Artist".into()],
            album: Some("Album".into()),
            duration_ms: Some(180_000),
            isrc: None,
            version_kind: None,
            version_detail: None,
        };
        let provider = FakeProvider::named_with_candidates(
            "antra",
            &["cd-flac", "unknown-flac", "hi-res-flac"],
        );
        let mut candidates = provider.state.lock().unwrap().candidates.clone();
        candidates[0].format = Some("flac".into());
        candidates[0].sample_rate_hz = Some(44_100);
        candidates[0].bit_depth = Some(16);
        candidates[1].format = Some("flac".into());
        candidates[2].format = Some("flac".into());
        candidates[2].sample_rate_hz = Some(96_000);
        candidates[2].bit_depth = Some(24);

        let ranked = rank_candidates(&query, candidates);

        assert_eq!(ranked[0].provider_token, "hi-res-flac");
        assert_eq!(ranked[1].provider_token, "cd-flac");
        assert_eq!(ranked[2].provider_token, "unknown-flac");
    }

    #[test]
    fn exact_isrc_title_variant_prefers_hi_res_antra_candidate() {
        let query = TrackQuery {
            library_track_id: 452,
            title: "Don't Stop Me Now - ...Revisited".into(),
            artists: vec!["Queen".into()],
            album: Some("Bohemian Rhapsody (The Original Soundtrack)".into()),
            duration_ms: Some(217_824),
            isrc: Some("GBUM71805977".into()),
            version_kind: None,
            version_detail: None,
        };
        let provider = FakeProvider::named_with_candidates("antra", &["tidal", "qobuz"]);
        let mut candidates = provider.state.lock().unwrap().candidates.clone();
        for candidate in &mut candidates {
            candidate.title = Some("Don't Stop Me Now".into());
            candidate.artists = vec!["Queen".into()];
            candidate.isrc = Some("GBUM71805977".into());
            candidate.format = Some("FLAC".into());
            candidate.bit_depth = Some(24);
        }
        candidates[0].source = Some("Tidal".into());
        candidates[0].album = Some("Bohemian Rhapsody (The Original Soundtrack)".into());
        candidates[0].duration_ms = Some(218_000);
        candidates[0].sample_rate_hz = Some(44_100);
        candidates[1].source = Some("Qobuz".into());
        candidates[1].album = Some("Bohemian Rhapsody".into());
        candidates[1].duration_ms = Some(217_000);
        candidates[1].sample_rate_hz = Some(96_000);

        let ranked = rank_candidates(&query, candidates);

        assert!(compatible_exact_isrc_candidate(&query, &ranked[0]));
        assert!(compatible_exact_isrc_candidate(&query, &ranked[1]));
        assert_eq!(ranked[0].provider_token, "qobuz");
        assert_eq!(ranked[1].provider_token, "tidal");
    }

    #[test]
    fn provider_chain_keeps_manual_candidates_grouped_by_provider_order() {
        let (path, database) = database("provider-chain-manual-groups");
        seed_missing_track(&database, false);
        let first = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["mono-a", "mono-b"],
        ));
        let second = Arc::new(FakeProvider::named_with_candidates(
            "sockseek",
            &["sock-a", "sock-b"],
        ));
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![first, second];

        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        assert_eq!(jobs[0].stage.as_deref(), Some("needsResolution"));
        let grouped = jobs[0]
            .candidates
            .iter()
            .map(|candidate| {
                (
                    candidate.provider.as_deref(),
                    candidate.provider_token.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            grouped,
            vec![
                (Some("monochrome"), "mono-a"),
                (Some("monochrome"), "mono-b"),
                (Some("sockseek"), "sock-a"),
                (Some("sockseek"), "sock-b"),
            ]
        );
    }

    #[test]
    fn provider_chain_keeps_all_outputs_for_manual_resolution_when_none_auto_match() {
        let (path, database) = database("provider-chain-all-manual-outputs");
        seed_missing_track(&database, false);
        let first = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["mono-weak"],
        ));
        let second = Arc::new(FakeProvider::named_with_candidates(
            "sockseek",
            &["sock-weak"],
        ));
        for provider in [&first, &second] {
            let mut state = provider.state.lock().unwrap();
            state.candidates[0].title = Some("Different Song".into());
            state.candidates[0].artists = vec!["Different Artist".into()];
            state.candidates[0].album = Some("Different Album".into());
            state.candidates[0].duration_ms = Some(260_000);
        }
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![first, second];

        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        assert_eq!(jobs[0].stage.as_deref(), Some("needsResolution"));
        assert_eq!(jobs[0].candidates.len(), 2);
        assert_eq!(
            jobs[0].candidates[0].provider.as_deref(),
            Some("monochrome")
        );
        assert_eq!(jobs[0].candidates[0].provider_token, "mono-weak");
        assert_eq!(jobs[0].candidates[1].provider.as_deref(), Some("sockseek"));
        assert_eq!(jobs[0].candidates[1].provider_token, "sock-weak");
    }

    #[test]
    fn selected_manual_candidate_switches_to_its_provider() {
        let (path, database) = database("provider-chain-selected-provider");
        seed_missing_track(&database, false);
        let first = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["mono-a", "mono-b"],
        ));
        let second = Arc::new(FakeProvider::named_with_candidates(
            "sockseek",
            &["sock-a", "sock-b"],
        ));
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![first, second.clone()];
        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();
        database
            .set_acquisition_provider(jobs[0].id, "monochrome")
            .unwrap();

        let resolved = AcquisitionCoordinator
            .run_selected_candidate(
                &database,
                &path.0,
                second.as_ref(),
                jobs[0].id,
                "sock-b",
                || false,
            )
            .unwrap();

        assert_eq!(resolved.status, "staged");
        assert_eq!(resolved.provider, "sockseek");
        assert_eq!(second.state.lock().unwrap().acquired_tokens, vec!["sock-b"]);
    }

    #[test]
    fn rejecting_duplicate_token_only_removes_candidate_from_selected_provider() {
        let (path, database) = database("provider-chain-reject-provider-token");
        seed_missing_track(&database, false);
        let first = Arc::new(FakeProvider::named_with_candidates(
            "monochrome",
            &["shared", "mono-b"],
        ));
        let second = Arc::new(FakeProvider::named_with_candidates(
            "sockseek",
            &["shared", "sock-b"],
        ));
        let providers: Vec<Arc<dyn AcquisitionProvider>> = vec![first, second];
        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &path.0, &providers, || false)
            .unwrap();

        let remaining = database
            .reject_acquisition_candidate(jobs[0].id, "sockseek", "shared")
            .unwrap();

        assert!(remaining.iter().any(|candidate| {
            candidate.provider.as_deref() == Some("monochrome")
                && candidate.provider_token == "shared"
        }));
        assert!(!remaining.iter().any(|candidate| {
            candidate.provider.as_deref() == Some("sockseek")
                && candidate.provider_token == "shared"
        }));
    }

    #[test]
    fn rejecting_legacy_candidate_uses_job_provider_when_provenance_is_missing() {
        let (_path, database) = database("legacy-provider-reject");
        seed_missing_track(&database, false);
        let jobs = database
            .queue_missing_acquisition_jobs("monochrome")
            .unwrap();
        database
            .require_acquisition_resolution(
                jobs[0].id,
                &[AcquisitionCandidate {
                    provider: None,
                    provider_token: "legacy-token".into(),
                    source: None,
                    file_name: None,
                    title: Some("Missing Song".into()),
                    artists: vec!["Artist".into()],
                    album: Some("Album".into()),
                    duration_ms: Some(180_000),
                    format: Some("flac".into()),
                    size_bytes: None,
                    bitrate_kbps: None,
                    sample_rate_hz: None,
                    bit_depth: None,
                    confidence: Some(80),
                    isrc: None,
                    recording_id: None,
                    release_id: None,
                }],
            )
            .unwrap();

        let remaining = database
            .reject_acquisition_candidate(jobs[0].id, "monochrome", "legacy-token")
            .unwrap();

        assert!(remaining.is_empty());
    }

    #[test]
    fn resync_requeues_needs_resolution_for_a_fresh_provider_search() {
        let (_path, database) = database("needs-resolution-resync");
        seed_missing_track(&database, false);
        let jobs = database
            .queue_missing_acquisition_jobs("monochrome")
            .unwrap();
        database
            .require_acquisition_resolution(
                jobs[0].id,
                &[AcquisitionCandidate {
                    provider: Some("monochrome".into()),
                    provider_token: "stale-candidate".into(),
                    source: None,
                    file_name: None,
                    title: Some("Missing Song".into()),
                    artists: vec!["Artist".into()],
                    album: Some("Album".into()),
                    duration_ms: Some(180_000),
                    format: Some("flac".into()),
                    size_bytes: None,
                    bitrate_kbps: None,
                    sample_rate_hz: None,
                    bit_depth: None,
                    confidence: Some(80),
                    isrc: None,
                    recording_id: None,
                    release_id: None,
                }],
            )
            .unwrap();

        let requeued = database
            .queue_missing_acquisition_jobs("monochrome")
            .unwrap();

        assert_eq!(requeued.len(), 1);
        assert_eq!(requeued[0].id, jobs[0].id);
        assert_eq!(requeued[0].status, "queued");
        assert_eq!(requeued[0].stage.as_deref(), Some("queued"));
        assert!(requeued[0].candidates.is_empty());
    }

    #[test]
    fn retry_failed_includes_needs_resolution_jobs() {
        let (_path, database) = database("needs-resolution-retry");
        seed_missing_track(&database, false);
        let jobs = database
            .queue_missing_acquisition_jobs("monochrome")
            .unwrap();
        database
            .require_acquisition_resolution(jobs[0].id, &[])
            .unwrap();

        let failed = database.failed_acquisition_jobs().unwrap();

        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].id, jobs[0].id);
        assert_eq!(failed[0].stage.as_deref(), Some("needsResolution"));
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
    fn failed_provider_job_stays_in_staging_instead_of_issues() {
        let (path, database) = database("issue");
        seed_missing_track(&database, false);
        let provider = FakeProvider::default();

        let jobs = AcquisitionCoordinator
            .run_missing(&database, &path.0, &provider, || false)
            .unwrap();
        assert_eq!(jobs[0].status, "failed");

        let issues = crate::issues::list_issues(&database, 0, 100).unwrap();
        assert!(
            !issues
                .items
                .iter()
                .any(|issue| issue.kind == crate::domain::IssueKind::AcquisitionFailed)
        );

        let staging = database.staging_items_page(0, 100).unwrap();
        assert_eq!(staging.total, 1);
        assert_eq!(staging.items[0].job_id, Some(jobs[0].id));
        assert_eq!(staging.items[0].stage, "failed");
    }
}
