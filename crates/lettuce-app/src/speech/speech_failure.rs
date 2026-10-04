use std::time::Duration;

use lettuce_jobs::{JobErrorCode, JobSnapshot};
use lettuce_types::TimestampMillis;

/// Attempts a speech job gets before a transient failure ends it.
pub const SPEECH_MAX_ATTEMPTS: u32 = 5;
const RETRY_BASE_DELAY: Duration = Duration::from_secs(2);
const RETRY_MAX_DELAY: Duration = Duration::from_secs(60);

pub const SPEECH_MODEL_REQUIRED_WHISPER: &str = "speech-model-required-whisper";
pub const SPEECH_MODEL_REQUIRED_KOKORO: &str = "speech-model-required-kokoro";
pub const SPEECH_SECRET_MISSING: &str = "speech-secret-missing";
pub const SPEECH_VOICE_MISSING: &str = "speech-voice-missing";
pub const SPEECH_ESPEAK_MISSING: &str = "speech-runtime-missing-espeak";
pub const SPEECH_RETRIES_EXHAUSTED: &str = "speech-retries-exhausted";

/// The job error a speech job ends with: its code, whether it is worth
/// retrying and its label.
pub(crate) type SpeechJobError = (JobErrorCode, bool, &'static str);

pub(crate) const RETRIES_EXHAUSTED: SpeechJobError = (
    JobErrorCode::ResourceUnavailable,
    false,
    SPEECH_RETRIES_EXHAUSTED,
);

/// Whether a failed attempt may be followed by another one.
#[must_use]
pub fn speech_retry_allowed(failed_attempt: u32) -> bool {
    failed_attempt < SPEECH_MAX_ATTEMPTS
}

/// How long a speech job waits after its `failed_attempt`th failure:
/// exponential from two seconds, at most a minute.
#[must_use]
pub fn speech_retry_delay(failed_attempt: u32) -> Duration {
    let doublings = failed_attempt.saturating_sub(1).min(16);
    RETRY_BASE_DELAY
        .saturating_mul(1_u32 << doublings)
        .min(RETRY_MAX_DELAY)
}

/// When a speech job queued after a failed attempt may run again; `None` for
/// a job that has not failed yet.
#[must_use]
pub fn speech_not_before(job: &JobSnapshot) -> Option<TimestampMillis> {
    let failed = job.attempt.get();
    if failed == 0 {
        return None;
    }
    let delay = i64::try_from(speech_retry_delay(failed).as_millis()).unwrap_or(i64::MAX);
    Some(TimestampMillis::new(
        job.updated_at.get().saturating_add(delay),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_double_up_to_the_cap() {
        assert_eq!(speech_retry_delay(1), Duration::from_secs(2));
        assert_eq!(speech_retry_delay(2), Duration::from_secs(4));
        assert_eq!(speech_retry_delay(5), Duration::from_secs(32));
        assert_eq!(speech_retry_delay(6), Duration::from_secs(60));
        assert_eq!(speech_retry_delay(u32::MAX), Duration::from_secs(60));
    }

    #[test]
    fn the_last_attempt_does_not_retry() {
        assert!(speech_retry_allowed(SPEECH_MAX_ATTEMPTS - 1));
        assert!(!speech_retry_allowed(SPEECH_MAX_ATTEMPTS));
    }
}
