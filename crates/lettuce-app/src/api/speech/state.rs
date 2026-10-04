use std::sync::{Arc, Mutex, MutexGuard, atomic::AtomicBool};

use super::dictation::DictationState;

/// The speech API's per-process state: the microphone capture in progress
/// and whether the retained legacy Whisper folder was looked at.
pub(crate) struct SpeechApiState {
    dictation: DictationState,
    legacy_whisper_admitted: AtomicBool,
    legacy_whisper_admission: Arc<Mutex<()>>,
}

impl Default for SpeechApiState {
    fn default() -> Self {
        Self {
            dictation: DictationState::default(),
            legacy_whisper_admitted: AtomicBool::new(false),
            legacy_whisper_admission: Arc::new(Mutex::new(())),
        }
    }
}

impl SpeechApiState {
    pub(super) const fn dictation(&self) -> &DictationState {
        &self.dictation
    }

    pub(super) const fn legacy_whisper_admitted(&self) -> &AtomicBool {
        &self.legacy_whisper_admitted
    }

    pub(super) fn legacy_whisper_admission(&self) -> MutexGuard<'_, ()> {
        self.legacy_whisper_admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
