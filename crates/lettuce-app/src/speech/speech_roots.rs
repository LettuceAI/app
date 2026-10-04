use std::path::{Path, PathBuf};

/// The app data folder's retained Whisper model directory.
#[must_use]
pub fn whisper_models_root(app_folder: &Path) -> PathBuf {
    app_folder.join("models").join("whisper")
}

/// The app data folder's Kokoro models and voices directory.
#[must_use]
pub fn kokoro_root(app_folder: &Path) -> PathBuf {
    app_folder.join("kokoro")
}

/// Where dictation recordings are written while they are captured.
#[must_use]
pub fn dictation_scratch_root(app_folder: &Path) -> PathBuf {
    app_folder.join("dictation")
}
