use lettuce_types::TimestampMillis;

/// Time the app was in use on one local calendar day (`YYYY-MM-DD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUsageDay {
    pub day: String,
    pub active_ms: u64,
}

#[must_use]
pub fn is_app_usage_day(day: &str) -> bool {
    let bytes = day.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

/// Per-install app usage; it never syncs or enters backups.
pub trait AppUsageRepository: Send + Sync {
    /// Adds to the day's total in one atomic step.
    fn add_app_usage(
        &self,
        day: &str,
        active_ms: u64,
        now: TimestampMillis,
    ) -> Result<(), AppUsageError>;

    /// Every recorded day, oldest first.
    fn app_usage_days(&self) -> Result<Vec<AppUsageDay>, AppUsageError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AppUsageError {
    #[error("app usage day is invalid")]
    InvalidDay,
    #[error("app usage storage failed")]
    Storage,
}
