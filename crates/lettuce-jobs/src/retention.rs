//! Retention policy contains lifecycle records only. Pruning never removes a
//! domain outcome or any artifact named by an [`OutcomeRef`].

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub keep_terminal_for: Option<Duration>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_terminal_for: Some(Duration::from_secs(30 * 24 * 60 * 60)),
        }
    }
}
