use lettuce_types::TimestampMillis;
use serde::{Deserialize, Serialize};

use crate::ValidationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CompanionClockSettings {
    #[serde(default)]
    pub time_awareness_enabled: bool,
    #[serde(default)]
    pub time_override: CompanionTimeOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompanionTimeOverride {
    #[default]
    Live,
    Frozen {
        anchor_at: TimestampMillis,
    },
    Ticking {
        anchor_at: TimestampMillis,
        set_at: TimestampMillis,
    },
}

impl CompanionClockSettings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        let invalid = match self.time_override {
            CompanionTimeOverride::Live => false,
            CompanionTimeOverride::Frozen { anchor_at } => anchor_at.get() < 0,
            CompanionTimeOverride::Ticking { anchor_at, set_at } => {
                anchor_at.get() < 0 || set_at.get() < 0
            }
        };
        if invalid {
            return Err(ValidationError::InvalidValue {
                field: "companion_clock.time_override",
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn effective_now(&self, real_now: TimestampMillis) -> TimestampMillis {
        if !self.time_awareness_enabled {
            return real_now;
        }
        match self.time_override {
            CompanionTimeOverride::Live => real_now,
            CompanionTimeOverride::Frozen { anchor_at } => anchor_at,
            CompanionTimeOverride::Ticking { anchor_at, set_at } => TimestampMillis::new(
                anchor_at
                    .get()
                    .saturating_add(real_now.get().saturating_sub(set_at.get()).max(0)),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_frozen_and_ticking_clocks_follow_the_legacy_effective_time() {
        let mut clock = CompanionClockSettings {
            time_awareness_enabled: false,
            time_override: CompanionTimeOverride::Frozen {
                anchor_at: TimestampMillis::new(100),
            },
        };
        assert_eq!(clock.effective_now(TimestampMillis::new(500)).get(), 500);
        clock.time_awareness_enabled = true;
        assert_eq!(clock.effective_now(TimestampMillis::new(500)).get(), 100);
        clock.time_override = CompanionTimeOverride::Ticking {
            anchor_at: TimestampMillis::new(100),
            set_at: TimestampMillis::new(500),
        };
        assert_eq!(clock.effective_now(TimestampMillis::new(550)).get(), 150);
        assert_eq!(clock.effective_now(TimestampMillis::new(450)).get(), 100);
        assert_eq!(clock.validate(), Ok(()));
        clock.time_override = CompanionTimeOverride::Frozen {
            anchor_at: TimestampMillis::new(-1),
        };
        assert_eq!(
            clock.validate(),
            Err(ValidationError::InvalidValue {
                field: "companion_clock.time_override"
            })
        );
        let old: CompanionClockSettings = serde_json::from_str("{}").expect("old settings");
        assert_eq!(old, CompanionClockSettings::default());
    }
}
