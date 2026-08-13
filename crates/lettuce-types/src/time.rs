use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimestampMillis(i64);

impl TimestampMillis {
    pub const UNIX_EPOCH: Self = Self(0);

    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }

    pub fn now() -> Result<Self, TimeError> {
        Self::try_from(SystemTime::now())
    }
}

impl TryFrom<SystemTime> for TimestampMillis {
    type Error = TimeError;

    fn try_from(value: SystemTime) -> Result<Self, Self::Error> {
        match value.duration_since(UNIX_EPOCH) {
            Ok(duration) => millis_from_duration(duration).map(Self),
            Err(error) => {
                let millis = millis_from_duration(error.duration())?;
                millis.checked_neg().map(Self).ok_or(TimeError::OutOfRange)
            }
        }
    }
}

fn millis_from_duration(duration: Duration) -> Result<i64, TimeError> {
    i64::try_from(duration.as_millis()).map_err(|_| TimeError::OutOfRange)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TimeError {
    #[error("timestamp is outside the supported millisecond range")]
    OutOfRange,
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::TimestampMillis;

    #[test]
    fn converts_on_both_sides_of_epoch() {
        assert_eq!(
            TimestampMillis::try_from(UNIX_EPOCH + Duration::from_millis(42)),
            Ok(TimestampMillis::new(42))
        );
        assert_eq!(
            TimestampMillis::try_from(UNIX_EPOCH - Duration::from_millis(42)),
            Ok(TimestampMillis::new(-42))
        );
    }
}
