use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use crate::domain::AppSettings;
use tauri::Manager;

#[derive(Clone)]
pub struct SyncScheduler {
    configuration: Arc<Mutex<(bool, Option<i64>)>>,
}

impl SyncScheduler {
    pub fn new(settings: &AppSettings) -> Self {
        Self {
            configuration: Arc::new(Mutex::new((
                settings.sync_on_startup,
                settings.sync_interval_minutes,
            ))),
        }
    }

    pub fn reconfigure(&self, settings: &AppSettings) {
        if let Ok(mut configuration) = self.configuration.lock() {
            *configuration = (settings.sync_on_startup, settings.sync_interval_minutes);
        }
    }

    pub fn start(&self, app: tauri::AppHandle) {
        let configuration = Arc::clone(&self.configuration);
        thread::Builder::new()
            .name("refrain-sync-scheduler".into())
            .spawn(move || {
                let started = Instant::now();
                let mut current_configuration = configuration
                    .lock()
                    .map(|config| *config)
                    .unwrap_or((false, None));
                let mut schedule = SyncSchedule::new(
                    current_configuration.0,
                    current_configuration.1,
                    Duration::ZERO,
                );
                let (done_tx, done_rx) = mpsc::channel();
                let mut running = false;
                let mut startup = schedule.startup_action();
                loop {
                    let now = started.elapsed();
                    let current = configuration
                        .lock()
                        .map(|config| *config)
                        .unwrap_or((false, None));
                    if current != current_configuration {
                        schedule.reconfigure(current.0, current.1, now);
                        current_configuration = current;
                    }

                    if done_rx.try_recv().is_ok() {
                        running = false;
                    }

                    let action = if let Some(action) = startup.take() {
                        Some(action)
                    } else if running {
                        schedule.tick(now, true)
                    } else {
                        schedule.finish_run().or_else(|| schedule.tick(now, false))
                    };

                    let pipeline_busy = app
                        .try_state::<crate::app::AppState>()
                        .is_some_and(|state| state.sync_pipeline.is_busy());
                    if let Some(action) = action
                        && pipeline_busy
                    {
                        schedule.defer(action);
                    } else if let Some(action) = action
                        && !running
                    {
                        running = true;
                        let app = app.clone();
                        let done_tx = done_tx.clone();
                        tauri::async_runtime::spawn(async move {
                            let trigger = match action {
                                ScheduleAction::Startup => "startup",
                                ScheduleAction::Periodic => "scheduled",
                            };
                            if let Err(error) = crate::reconciliation::run_automatic_full_sync(app, trigger.into()).await {
                                tracing::warn!(%error, "automatic full synchronization did not complete");
                            }
                            let _ = done_tx.send(());
                        });
                    }

                    thread::sleep(Duration::from_millis(250));
                }
            })
            .expect("could not start synchronization scheduler thread");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScheduleAction {
    Startup,
    Periodic,
}

#[derive(Debug)]
struct SyncSchedule {
    startup_enabled: bool,
    startup_emitted: bool,
    interval: Option<Duration>,
    next_tick: Option<Duration>,
    pending: Option<ScheduleAction>,
}

impl SyncSchedule {
    fn new(startup_enabled: bool, interval_minutes: Option<i64>, now: Duration) -> Self {
        let interval = interval_duration(interval_minutes);
        Self {
            startup_enabled,
            startup_emitted: false,
            interval,
            next_tick: interval.map(|interval| now + interval),
            pending: None,
        }
    }

    fn startup_action(&mut self) -> Option<ScheduleAction> {
        if self.startup_emitted {
            return None;
        }
        self.startup_emitted = true;
        self.startup_enabled.then_some(ScheduleAction::Startup)
    }

    fn reconfigure(
        &mut self,
        startup_enabled: bool,
        interval_minutes: Option<i64>,
        now: Duration,
    ) {
        self.startup_enabled = startup_enabled;
        self.interval = interval_duration(interval_minutes);
        self.next_tick = self.interval.map(|interval| now + interval);
        if self.pending != Some(ScheduleAction::Startup) {
            self.pending = None;
        }
    }

    fn tick(&mut self, now: Duration, busy: bool) -> Option<ScheduleAction> {
        let interval = self.interval?;
        let deadline = self.next_tick?;
        if now < deadline {
            return None;
        }
        self.next_tick = Some(now + interval);
        if busy {
            self.defer(ScheduleAction::Periodic);
            None
        } else {
            Some(ScheduleAction::Periodic)
        }
    }

    fn defer(&mut self, action: ScheduleAction) {
        if self.pending.is_none() || action == ScheduleAction::Startup {
            self.pending = Some(action);
        }
    }

    fn finish_run(&mut self) -> Option<ScheduleAction> {
        self.pending.take()
    }
}

fn interval_duration(minutes: Option<i64>) -> Option<Duration> {
    minutes
        .filter(|minutes| *minutes > 0)
        .map(|minutes| Duration::from_secs(minutes.saturating_mul(60) as u64))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{ScheduleAction, SyncSchedule};

    #[test]
    fn startup_trigger_is_emitted_once_only_when_enabled() {
        let mut disabled = SyncSchedule::new(false, None, Duration::ZERO);
        assert_eq!(disabled.startup_action(), None);

        let mut enabled = SyncSchedule::new(true, None, Duration::ZERO);
        assert_eq!(enabled.startup_action(), Some(ScheduleAction::Startup));
        assert_eq!(enabled.startup_action(), None);
    }

    #[test]
    fn elapsed_ticks_coalesce_to_one_pending_run_while_busy() {
        let mut schedule = SyncSchedule::new(false, Some(10), Duration::ZERO);
        assert_eq!(schedule.tick(Duration::from_secs(600), true), None);
        assert_eq!(schedule.tick(Duration::from_secs(1200), true), None);
        assert_eq!(schedule.tick(Duration::from_secs(1800), true), None);
        assert_eq!(schedule.finish_run(), Some(ScheduleAction::Periodic));
        assert_eq!(schedule.finish_run(), None);
    }

    #[test]
    fn settings_reconfiguration_restarts_interval_and_clears_disabled_pending_tick() {
        let mut schedule = SyncSchedule::new(false, Some(10), Duration::ZERO);
        schedule.tick(Duration::from_secs(600), true);
        schedule.reconfigure(false, Some(25), Duration::from_secs(720));
        assert_eq!(schedule.finish_run(), None);
        assert_eq!(schedule.tick(Duration::from_secs(2219), false), None);
        assert_eq!(schedule.tick(Duration::from_secs(2220), false), Some(ScheduleAction::Periodic));
    }

    #[test]
    fn settings_updates_change_periodic_deadline() {
        let mut schedule = SyncSchedule::new(false, Some(10), Duration::ZERO);
        schedule.reconfigure(false, Some(5), Duration::from_secs(180));
        assert_eq!(schedule.tick(Duration::from_secs(479), false), None);
        assert_eq!(schedule.tick(Duration::from_secs(480), false), Some(ScheduleAction::Periodic));
    }
}
