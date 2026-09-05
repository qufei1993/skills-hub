use chrono::{DateTime, Duration, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use super::types::DeviceSyncConfig;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleState {
    Disabled,
    Initializing,
    Scheduled,
    Backoff,
    Paused,
    Running,
    Waiting,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScheduleSummary {
    pub state: ScheduleState,
    pub next_at: Option<i64>,
}

struct ScheduleSnapshot {
    key: (String, String, Option<SyncSchedule>),
    completion: Option<String>,
    next_at: i64,
    retrying: bool,
}

#[derive(Clone, Default)]
pub struct SchedulerRuntime(Arc<Mutex<Option<ScheduleSnapshot>>>);

impl SchedulerRuntime {
    fn publish(
        &self,
        config: &DeviceSyncConfig,
        clock: &ScheduleClock,
        completion: Option<String>,
    ) {
        if let Ok(mut snapshot) = self.0.lock() {
            *snapshot = Some(ScheduleSnapshot {
                key: (
                    config.remote_url.clone(),
                    config.branch.clone(),
                    config.auto_sync_schedule.clone(),
                ),
                completion,
                next_at: clock.next_at,
                retrying: clock.failures > 0,
            });
        }
    }

    pub fn status(
        &self,
        store: &crate::core::skill_store::SkillStore,
        conflicts: bool,
        running: bool,
    ) -> anyhow::Result<ScheduleSummary> {
        let config = store.get_device_sync_config()?;
        let completion = store.get_setting("device_sync_last_run")?;
        Ok(self.summary(
            config.as_ref(),
            completion.as_deref(),
            conflicts,
            running,
            Local::now().timestamp_millis(),
        ))
    }

    fn summary(
        &self,
        config: Option<&DeviceSyncConfig>,
        completion: Option<&str>,
        conflicts: bool,
        running: bool,
        now: i64,
    ) -> ScheduleSummary {
        let without_time = |state| ScheduleSummary {
            state,
            next_at: None,
        };
        let Some(config) = config.filter(|c| c.auto_sync && c.auto_sync_schedule.is_some()) else {
            return without_time(ScheduleState::Disabled);
        };
        if running {
            return without_time(ScheduleState::Running);
        }
        if conflicts {
            return without_time(ScheduleState::Paused);
        }
        let Ok(snapshot) = self.0.lock() else {
            return without_time(ScheduleState::Initializing);
        };
        let key = (
            config.remote_url.clone(),
            config.branch.clone(),
            config.auto_sync_schedule.clone(),
        );
        let Some(snapshot) = snapshot
            .as_ref()
            .filter(|s| s.key == key && s.completion.as_deref() == completion)
        else {
            return without_time(ScheduleState::Initializing);
        };
        if snapshot.next_at <= now {
            return without_time(ScheduleState::Waiting);
        }
        ScheduleSummary {
            state: if snapshot.retrying {
                ScheduleState::Backoff
            } else {
                ScheduleState::Scheduled
            },
            next_at: Some(snapshot.next_at),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum SyncSchedule {
    Interval { minutes: u32 },
    Daily { time: String },
}

impl SyncSchedule {
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            Self::Interval { minutes } => anyhow::ensure!(
                (5..=43200).contains(minutes),
                "sync interval must be an integer between 5 and 43200 minutes"
            ),
            Self::Daily { time } => {
                parse_time(time)?;
            }
        }
        Ok(())
    }

    fn next(&self, now: i64) -> i64 {
        match self {
            Self::Interval { minutes } => now + i64::from(*minutes) * 60000,
            Self::Daily { time } => next_daily(Local.timestamp_millis_opt(now).unwrap(), time),
        }
    }
}

fn parse_time(time: &str) -> anyhow::Result<(u32, u32)> {
    anyhow::ensure!(
        time.len() == 5
            && time.as_bytes()[2] == b':'
            && time
                .bytes()
                .enumerate()
                .all(|(i, c)| i == 2 || c.is_ascii_digit()),
        "daily sync time must be HH:MM"
    );
    let hour: u32 = time[..2].parse()?;
    let minute: u32 = time[3..].parse()?;
    anyhow::ensure!(hour < 24 && minute < 60, "invalid daily sync time");
    Ok((hour, minute))
}

fn next_daily<T: TimeZone>(now: DateTime<T>, time: &str) -> i64 {
    let (hour, minute) = parse_time(time).expect("validated schedule");
    let zone = now.timezone();
    for days in 0..3 {
        let date = now.date_naive() + Duration::days(days);
        let target = date.and_hms_opt(hour, minute, 0).unwrap();
        // Spring-forward gaps use the next valid minute; fall-back runs only once.
        for shift in 0..180 {
            if let Some(candidate) = zone
                .from_local_datetime(&(target + Duration::minutes(shift)))
                .earliest()
            {
                if candidate.timestamp_millis() > now.timestamp_millis() {
                    return candidate.timestamp_millis();
                }
                break;
            }
        }
    }
    now.timestamp_millis() + 86400000
}

pub struct ScheduleClock {
    next_at: i64,
    failures: u32,
}

impl ScheduleClock {
    pub fn new(schedule: &SyncSchedule, now: i64) -> Self {
        Self {
            next_at: schedule.next(now),
            failures: 0,
        }
    }

    pub fn due(&self, now: i64) -> bool {
        now >= self.next_at
    }

    pub fn complete(&mut self, schedule: &SyncSchedule, now: i64, success: bool) {
        if success {
            self.failures = 0;
            self.next_at = schedule.next(now);
        } else {
            self.failures = (self.failures + 1).min(8);
            let base = match schedule {
                SyncSchedule::Interval { minutes } => i64::from(*minutes) * 60000,
                SyncSchedule::Daily { .. } => 300000,
            };
            self.next_at = now + (base * (1_i64 << self.failures)).min(base.max(3600000));
        }
    }
}

pub fn start(app: tauri::AppHandle, store: crate::core::skill_store::SkillStore) {
    use tauri::{Emitter, Manager};
    let runtime = SchedulerRuntime::default();
    app.manage(runtime.clone());
    std::thread::spawn(move || {
        let mut active = None;
        let mut clock: Option<ScheduleClock> = None;
        let mut observed_completion = None;
        loop {
            // Only metadata is read here. Credentials are accessed only inside a due sync.
            let config = store
                .get_device_sync_config()
                .ok()
                .flatten()
                .filter(|c| c.auto_sync);
            let key = config.as_ref().and_then(|c| {
                c.auto_sync_schedule
                    .as_ref()
                    .map(|s| (c.remote_url.clone(), c.branch.clone(), s.clone()))
            });
            let now = Local::now().timestamp_millis();
            if key != active {
                clock = key
                    .as_ref()
                    .filter(|(_, _, s)| s.validate().is_ok())
                    .map(|(_, _, s)| ScheduleClock::new(s, now));
                active = key;
                observed_completion = store.get_setting("device_sync_last_run").ok().flatten();
            }
            if let (Some(config), Some((_, _, schedule)), Some(clock)) =
                (&config, &active, &mut clock)
            {
                let completion = store.get_setting("device_sync_last_run").ok().flatten();
                if completion != observed_completion {
                    if let Some((status, finished)) = completion
                        .as_ref()
                        .and_then(|v| serde_json::from_str::<(String, i64)>(v).ok())
                    {
                        if matches!(schedule, SyncSchedule::Interval { .. }) {
                            clock.complete(schedule, finished, status == "success");
                        }
                    }
                    observed_completion = completion;
                }
                runtime.publish(config, clock, observed_completion.clone());
                if clock.due(now) {
                    let result = (|| -> anyhow::Result<Option<super::types::SyncRunResult>> {
                        let workspace = app.path().app_data_dir()?.join("device-sync");
                        let central =
                            crate::core::central_repo::resolve_central_repo_path(&app, &store)?;
                        let credentials = super::credentials::SystemCredentialStore;
                        super::DeviceSyncService::new(&store, &credentials, workspace, central)
                            .sync_scheduled(config)
                    })();
                    let changed = result
                        .as_ref()
                        .ok()
                        .and_then(|r| r.as_ref())
                        .is_some_and(|r| r.changes != super::types::SyncChangeSummary::default());
                    let attempted = !matches!(result, Ok(None));
                    match result {
                        Ok(None) => {}
                        Ok(Some(result)) => clock.complete(
                            schedule,
                            Local::now().timestamp_millis(),
                            result.status == "success",
                        ),
                        Err(_) => {
                            clock.complete(schedule, Local::now().timestamp_millis(), false);
                            log::warn!("scheduled device sync failed; retry delayed");
                        }
                    }
                    observed_completion = store.get_setting("device_sync_last_run").ok().flatten();
                    runtime.publish(config, clock, observed_completion.clone());
                    if attempted {
                        let _ = app.emit("device-sync-completed", changed);
                    }
                }
            } else if let Ok(mut snapshot) = runtime.0.lock() {
                *snapshot = None;
            }
            std::thread::sleep(std::time::Duration::from_secs(15));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_uses_worker_deadline_and_hides_stale_or_paused_deadlines() {
        let runtime = SchedulerRuntime::default();
        let mut config = super::super::types::DeviceSyncConfig {
            auto_sync: true,
            auto_sync_schedule: Some(SyncSchedule::Interval { minutes: 5 }),
            ..Default::default()
        };
        let schedule = config.auto_sync_schedule.as_ref().unwrap();
        let mut clock = ScheduleClock::new(schedule, 1000);
        runtime.publish(&config, &clock, None);
        let read = |config: &super::super::types::DeviceSyncConfig, conflicts, running, now| {
            runtime.summary(Some(config), None, conflicts, running, now)
        };
        assert_eq!(read(&config, false, false, 1000).next_at, Some(301000));
        clock.complete(schedule, 2000, false);
        runtime.publish(&config, &clock, None);
        let retry = read(&config, false, false, 2000);
        assert_eq!(retry.state, ScheduleState::Backoff);
        assert_eq!(retry.next_at, Some(602000));
        assert_eq!(
            read(&config, true, false, 2000).state,
            ScheduleState::Paused
        );
        assert_eq!(read(&config, true, false, 2000).next_at, None);
        assert_eq!(
            read(&config, false, true, 2000).state,
            ScheduleState::Running
        );
        assert_eq!(
            read(&config, false, false, 602000).state,
            ScheduleState::Waiting
        );
        assert_eq!(
            runtime
                .summary(Some(&config), Some("new completion"), false, false, 2000)
                .next_at,
            None
        );
        config.branch = "another".into();
        assert_eq!(
            read(&config, false, false, 2000).state,
            ScheduleState::Initializing
        );
        config.auto_sync = false;
        assert_eq!(
            read(&config, false, false, 2000).state,
            ScheduleState::Disabled
        );
    }

    #[test]
    fn rejects_short_fractional_and_malformed_schedules() {
        for minutes in [0, 1, 4, 43201] {
            assert!(SyncSchedule::Interval { minutes }.validate().is_err());
        }
        assert!(SyncSchedule::Interval { minutes: 5 }.validate().is_ok());
        for time in ["", "9:00", "24:00", "12:60", "aa:bb"] {
            assert!(SyncSchedule::Daily { time: time.into() }
                .validate()
                .is_err());
        }
        assert!(
            serde_json::from_str::<SyncSchedule>(r#"{"mode":"interval","minutes":5.5}"#).is_err()
        );
    }

    #[test]
    fn interval_waits_from_completion_and_collapses_missed_ticks() {
        let schedule = SyncSchedule::Interval { minutes: 5 };
        let mut clock = ScheduleClock::new(&schedule, 1000);
        assert!(!clock.due(300999));
        assert!(clock.due(301000));
        clock.complete(&schedule, 900000, true);
        assert!(!clock.due(1199999));
        assert!(clock.due(1200000));
        assert!(clock.due(99999999));
        clock.complete(&schedule, 99999999, true);
        assert!(!clock.due(99999999));
    }

    #[test]
    fn failures_back_off_and_success_resets_delay() {
        let schedule = SyncSchedule::Interval { minutes: 5 };
        let mut clock = ScheduleClock::new(&schedule, 0);
        clock.complete(&schedule, 0, false);
        assert!(!clock.due(599999));
        assert!(clock.due(600000));
        clock.complete(&schedule, 600000, false);
        assert!(!clock.due(1799999));
        clock.complete(&schedule, 1800000, true);
        assert!(clock.due(2100000));
    }

    #[test]
    fn daily_sleep_catches_up_once_and_backoff_is_bounded() {
        let schedule = SyncSchedule::Daily {
            time: "09:00".into(),
        };
        let now = Local::now().timestamp_millis();
        let mut clock = ScheduleClock::new(&schedule, now);
        let resumed = now + 3 * 86400000;
        assert!(clock.due(resumed));
        clock.complete(&schedule, resumed, true);
        assert!(!clock.due(resumed));
        for _ in 0..20 {
            clock.complete(&schedule, resumed, false);
        }
        assert!(!clock.due(resumed + 3599999));
        assert!(clock.due(resumed + 3600000));
    }

    #[test]
    fn daily_uses_local_timezone_and_runs_once_per_day() {
        use chrono::{FixedOffset, TimeZone};
        let tz = FixedOffset::east_opt(8 * 3600).unwrap();
        let now = tz.with_ymd_and_hms(2026, 9, 5, 8, 59, 0).unwrap();
        let first = next_daily(now, "09:00");
        assert_eq!(
            first,
            tz.with_ymd_and_hms(2026, 9, 5, 9, 0, 0)
                .unwrap()
                .timestamp_millis()
        );
        let after = tz.with_ymd_and_hms(2026, 9, 5, 9, 1, 0).unwrap();
        assert_eq!(
            next_daily(after, "09:00"),
            tz.with_ymd_and_hms(2026, 9, 6, 9, 0, 0)
                .unwrap()
                .timestamp_millis()
        );
    }
}
