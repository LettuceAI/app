use std::sync::Mutex;

use chrono::TimeZone;
use lettuce_types::TimestampMillis;
use lettuce_usage::{AppUsageError, AppUsageRepository};

#[derive(Debug, Default)]
struct TrackerState {
    active_since: Option<i64>,
    pending_ms: u64,
}

/// Counts the time the app window is focused and adds it to the install's
/// per-day usage when flushed.
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
                pending_ms: 0,
            }),
        }
    }

    pub fn on_focus_changed(&self, focused: bool, now: TimestampMillis) {
        let mut state = self.lock();
        match (focused, state.active_since) {
            (true, None) => state.active_since = Some(now.get()),
            (false, Some(since)) => {
                state.pending_ms = state.pending_ms.saturating_add(elapsed(since, now));
                state.active_since = None;
            }
            _ => {}
        }
    }

    /// Adds the time counted since the last flush to today's total; on
    /// failure the time stays pending for the next flush.
    pub fn flush<S: AppUsageRepository + ?Sized>(
        &self,
        store: &S,
        now: TimestampMillis,
    ) -> Result<u64, AppUsageError> {
        let pending = {
            let mut state = self.lock();
            if let Some(since) = state.active_since {
                state.pending_ms = state.pending_ms.saturating_add(elapsed(since, now));
                state.active_since = Some(now.get().max(since));
            }
            state.pending_ms
        };
        if pending == 0 {
            return Ok(0);
        }
        let day = chrono::Local
            .timestamp_millis_opt(now.get())
            .single()
            .ok_or(AppUsageError::InvalidDay)?
            .format("%Y-%m-%d")
            .to_string();
        store.add_app_usage(&day, pending, now)?;
        let mut state = self.lock();
        state.pending_ms = state.pending_ms.saturating_sub(pending);
        Ok(pending)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, TrackerState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn elapsed(since: i64, now: TimestampMillis) -> u64 {
    u64::try_from(now.get().saturating_sub(since)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
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
}
