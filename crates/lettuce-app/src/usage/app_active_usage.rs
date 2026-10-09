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
    flushing: Mutex<()>,
}

impl AppActiveUsageTracker {
    #[must_use]
    pub fn new(now: TimestampMillis) -> Self {
        Self {
            state: Mutex::new(TrackerState {
                active_since: Some(now.get()),
                pending: BTreeMap::new(),
            }),
            flushing: Mutex::new(()),
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
        let _flushing = self
            .flushing
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    pub fn days<S: AppUsageRepository + ?Sized>(
        &self,
        store: &S,
        now: TimestampMillis,
    ) -> Result<Vec<lettuce_usage::AppUsageDay>, AppUsageError> {
        let _flushing = self
            .flushing
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut days = store
            .app_usage_days()?
            .into_iter()
            .map(|day| (day.day, day.active_ms))
            .collect::<BTreeMap<_, _>>();
        let state = self.lock();
        let mut pending = state.pending.clone();
        if let Some(since) = state.active_since {
            accrue(&mut pending, since, now.get());
        }
        for (day, active_ms) in pending {
            let total = days.entry(day).or_default();
            *total = total.checked_add(active_ms).ok_or(AppUsageError::Storage)?;
        }
        Ok(days
            .into_iter()
            .map(|(day, active_ms)| lettuce_usage::AppUsageDay { day, active_ms })
            .collect())
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

/// The first valid instant of the local day after `day`; where a clock
/// change skips local midnight, the first minute that exists that day.
fn next_day_start(day: chrono::NaiveDate) -> Option<i64> {
    next_day_start_in(&Local, day)
}

fn next_day_start_in<Tz: TimeZone>(zone: &Tz, day: chrono::NaiveDate) -> Option<i64> {
    let next = day.succ_opt()?.and_time(NaiveTime::MIN);
    (0..=MINUTES_PER_DAY).find_map(|minute| {
        zone.from_local_datetime(&(next + chrono::Duration::minutes(minute)))
            .earliest()
            .map(|time| time.timestamp_millis())
    })
}

const MINUTES_PER_DAY: i64 = 24 * 60;

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

    /// UTC, except that local time skips from midnight to 01:00 on
    /// 2026-03-11, as a clock change at midnight does.
    #[derive(Debug, Clone, Copy)]
    struct MidnightGap;

    impl TimeZone for MidnightGap {
        type Offset = chrono::FixedOffset;

        fn from_offset(_offset: &Self::Offset) -> Self {
            Self
        }

        fn offset_from_local_date(
            &self,
            _local: &chrono::NaiveDate,
        ) -> chrono::LocalResult<Self::Offset> {
            chrono::LocalResult::Single(chrono::FixedOffset::east_opt(0).expect("utc"))
        }

        fn offset_from_local_datetime(
            &self,
            local: &chrono::NaiveDateTime,
        ) -> chrono::LocalResult<Self::Offset> {
            let gap_start = chrono::NaiveDate::from_ymd_opt(2026, 3, 11)
                .expect("date")
                .and_time(NaiveTime::MIN);
            if *local >= gap_start && *local < gap_start + chrono::Duration::hours(1) {
                chrono::LocalResult::None
            } else {
                chrono::LocalResult::Single(chrono::FixedOffset::east_opt(0).expect("utc"))
            }
        }

        fn offset_from_utc_date(&self, _utc: &chrono::NaiveDate) -> Self::Offset {
            chrono::FixedOffset::east_opt(0).expect("utc")
        }

        fn offset_from_utc_datetime(&self, _utc: &chrono::NaiveDateTime) -> Self::Offset {
            chrono::FixedOffset::east_opt(0).expect("utc")
        }
    }

    #[test]
    fn a_skipped_midnight_starts_the_day_at_its_first_valid_instant() {
        let day = chrono::NaiveDate::from_ymd_opt(2026, 3, 10).expect("date");
        let expected = chrono::NaiveDate::from_ymd_opt(2026, 3, 11)
            .expect("date")
            .and_hms_opt(1, 0, 0)
            .expect("time")
            .and_utc()
            .timestamp_millis();
        assert_eq!(next_day_start_in(&MidnightGap, day), Some(expected));
    }

    /// Counts what is written and waits inside each write, so two flushes
    /// overlap.
    #[derive(Default)]
    struct SlowStore {
        written: std::sync::atomic::AtomicU64,
    }

    impl AppUsageRepository for SlowStore {
        fn add_app_usage(
            &self,
            _day: &str,
            active_ms: u64,
            _now: TimestampMillis,
        ) -> Result<(), AppUsageError> {
            std::thread::sleep(std::time::Duration::from_millis(20));
            self.written.fetch_add(active_ms, Ordering::SeqCst);
            Ok(())
        }

        fn app_usage_days(&self) -> Result<Vec<lettuce_usage::AppUsageDay>, AppUsageError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn concurrent_flushes_write_the_time_once() {
        let store = std::sync::Arc::new(SlowStore::default());
        let midnight = local_midnight();
        let tracker = std::sync::Arc::new(AppActiveUsageTracker::new(TimestampMillis::new(
            midnight + 1_000,
        )));
        tracker.on_focus_changed(false, TimestampMillis::new(midnight + 9_000));
        let flushes = (0..4)
            .map(|_| {
                let store = std::sync::Arc::clone(&store);
                let tracker = std::sync::Arc::clone(&tracker);
                std::thread::spawn(move || {
                    tracker
                        .flush(store.as_ref(), TimestampMillis::new(midnight + 10_000))
                        .expect("flush")
                })
            })
            .collect::<Vec<_>>();
        let reported = flushes
            .into_iter()
            .map(|flush| flush.join().expect("flush thread"))
            .sum::<u64>();
        assert_eq!(reported, 8_000);
        assert_eq!(store.written.load(Ordering::SeqCst), 8_000);
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

#[cfg(test)]
mod slice12_tests {
    use super::*;

    #[test]
    fn days_include_live_and_pending_time_without_writing_or_double_counting() {
        let database = lettuce_database::Database::open_in_memory().expect("database");
        let tracker = AppActiveUsageTracker::new(TimestampMillis::new(1000));
        let first = tracker
            .days(&database, TimestampMillis::new(4000))
            .expect("days");
        assert_eq!(first.iter().map(|day| day.active_ms).sum::<u64>(), 3000);
        assert!(database.app_usage_days().expect("persisted").is_empty());
        assert_eq!(
            tracker
                .days(&database, TimestampMillis::new(4000))
                .expect("replay"),
            first
        );
        tracker.on_focus_changed(false, TimestampMillis::new(5000));
        assert_eq!(
            tracker
                .days(&database, TimestampMillis::new(9000))
                .expect("blurred")
                .iter()
                .map(|day| day.active_ms)
                .sum::<u64>(),
            4000
        );
        tracker
            .flush(&database, TimestampMillis::new(9000))
            .expect("flush");
        assert_eq!(
            tracker
                .days(&database, TimestampMillis::new(9000))
                .expect("after flush")
                .iter()
                .map(|day| day.active_ms)
                .sum::<u64>(),
            4000
        );
    }
}
