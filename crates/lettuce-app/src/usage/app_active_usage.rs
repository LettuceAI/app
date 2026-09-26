use std::{collections::BTreeMap, sync::Mutex};

use chrono::{Local, NaiveTime, TimeZone};
use lettuce_types::TimestampMillis;
use lettuce_usage::{AppUsageError, AppUsageRepository};

#[derive(Debug, Default)]
struct TrackerState {
    active_since: Option<i64>,
    /// Counted time not yet written, per local day (`YYYY-MM-DD`).
    pending: BTreeMap<String, u64>,
}

/// Counts the time the app window is focused and adds it to the install's
/// per-day usage when flushed, each local day's share to its own day.
#[derive(Debug)]
pub struct AppActiveUsageTracker {
    state: Mutex<TrackerState>,
}

impl AppActiveUsageTracker {
    #[must_use]
    pub fn new(now: TimestampMillis) -> Self {
        Self {
            state: Mutex::new(TrackerState {
                active_since: Some(now.get()),
                pending: BTreeMap::new(),
            }),
        }
    }

    pub fn on_focus_changed(&self, focused: bool, now: TimestampMillis) {
        let mut state = self.lock();
        match (focused, state.active_since) {
            (true, None) => state.active_since = Some(now.get()),
            (false, Some(since)) => {
                accrue(&mut state.pending, since, now.get());
                state.active_since = None;
            }
            _ => {}
        }
    }

    /// Adds the time counted since the last flush to each local day it fell
    /// on; a day whose write fails keeps its time pending for the next flush.
    pub fn flush<S: AppUsageRepository + ?Sized>(
        &self,
        store: &S,
        now: TimestampMillis,
    ) -> Result<u64, AppUsageError> {
        let pending = {
            let mut state = self.lock();
            if let Some(since) = state.active_since {
                accrue(&mut state.pending, since, now.get());
                state.active_since = Some(now.get().max(since));
            }
            state.pending.clone()
        };
        let mut written = 0_u64;
        for (day, active_ms) in pending {
            store.add_app_usage(&day, active_ms, now)?;
            let mut state = self.lock();
            if let Some(left) = state.pending.get_mut(&day) {
                *left = left.saturating_sub(active_ms);
                if *left == 0 {
                    state.pending.remove(&day);
                }
            }
            written = written.saturating_add(active_ms);
        }
        Ok(written)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, TrackerState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn local_day(millis: i64) -> Option<chrono::NaiveDate> {
    Local
        .timestamp_millis_opt(millis)
        .earliest()
        .map(|time| time.date_naive())
}

/// The first millisecond of the local day after `day`.
fn next_day_start(day: chrono::NaiveDate) -> Option<i64> {
    let next = day.succ_opt()?.and_time(NaiveTime::MIN);
    Local
        .from_local_datetime(&next)
        .earliest()
        .map(|time| time.timestamp_millis())
}

/// Adds `since..until` to the pending time of each local day it spans.
fn accrue(pending: &mut BTreeMap<String, u64>, since: i64, until: i64) {
    let mut start = since;
    while start < until {
        let Some(day) = local_day(start) else {
            return;
        };
        let end = next_day_start(day).map_or(until, |next| next.clamp(start + 1, until));
        let share = u64::try_from(end - start).unwrap_or(0);
        if share > 0 {
            let total = pending
                .entry(day.format("%Y-%m-%d").to_string())
                .or_default();
            *total = total.saturating_add(share);
        }
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[test]
    fn focused_time_accumulates_into_the_install_counters() {
        let database = lettuce_database::Database::open_in_memory().expect("database");
        let tracker = AppActiveUsageTracker::new(TimestampMillis::new(1_000));
        tracker.on_focus_changed(false, TimestampMillis::new(4_000));
        tracker.on_focus_changed(false, TimestampMillis::new(9_000));
        tracker.on_focus_changed(true, TimestampMillis::new(10_000));
        assert_eq!(
            tracker
                .flush(&database, TimestampMillis::new(12_000))
                .expect("flush"),
            5_000
        );
        assert_eq!(
            tracker
                .flush(&database, TimestampMillis::new(13_000))
                .expect("flush again"),
            1_000
        );
        let days = lettuce_usage::AppUsageRepository::app_usage_days(&database).expect("days");
        assert_eq!(days.iter().map(|day| day.active_ms).sum::<u64>(), 6_000);
        assert_eq!(
            tracker
                .flush(&database, TimestampMillis::new(13_000))
                .expect("nothing new"),
            0
        );
    }

    fn local_midnight() -> i64 {
        let day = chrono::NaiveDate::from_ymd_opt(2026, 3, 10).expect("date");
        next_day_start(day).expect("midnight")
    }

    #[test]
    fn a_stretch_across_midnight_is_split_between_its_days() {
        let database = lettuce_database::Database::open_in_memory().expect("database");
        let midnight = local_midnight();
        let tracker = AppActiveUsageTracker::new(TimestampMillis::new(midnight - 60_000));
        assert_eq!(
            tracker
                .flush(&database, TimestampMillis::new(midnight + 30_000))
                .expect("flush"),
            90_000
        );
        let days = lettuce_usage::AppUsageRepository::app_usage_days(&database).expect("days");
        assert_eq!(
            days.iter()
                .map(|day| (day.day.as_str(), day.active_ms))
                .collect::<Vec<_>>(),
            vec![("2026-03-10", 60_000), ("2026-03-11", 30_000)]
        );
    }

    struct FlakyStore {
        failing: AtomicBool,
        written: Mutex<Vec<(String, u64)>>,
    }

    impl AppUsageRepository for FlakyStore {
        fn add_app_usage(
            &self,
            day: &str,
            active_ms: u64,
            _now: TimestampMillis,
        ) -> Result<(), AppUsageError> {
            if self.failing.load(Ordering::SeqCst) {
                return Err(AppUsageError::Storage);
            }
            self.written
                .lock()
                .expect("written")
                .push((day.to_owned(), active_ms));
            Ok(())
        }

        fn app_usage_days(&self) -> Result<Vec<lettuce_usage::AppUsageDay>, AppUsageError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn a_failed_flush_keeps_the_time_for_the_next_one() {
        let store = FlakyStore {
            failing: AtomicBool::new(true),
            written: Mutex::new(Vec::new()),
        };
        let midnight = local_midnight();
        let tracker = AppActiveUsageTracker::new(TimestampMillis::new(midnight - 10_000));
        tracker.on_focus_changed(false, TimestampMillis::new(midnight + 5_000));
        assert_eq!(
            tracker.flush(&store, TimestampMillis::new(midnight + 6_000)),
            Err(AppUsageError::Storage)
        );
        store.failing.store(false, Ordering::SeqCst);
        assert_eq!(
            tracker
                .flush(&store, TimestampMillis::new(midnight + 7_000))
                .expect("retried"),
            15_000
        );
        assert_eq!(
            *store.written.lock().expect("written"),
            vec![
                ("2026-03-10".to_owned(), 10_000),
                ("2026-03-11".to_owned(), 5_000)
            ]
        );
    }
}
