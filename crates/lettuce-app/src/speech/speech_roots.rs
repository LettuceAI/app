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

/// Resolves device-local locations, including roots relocated with a custom models folder.
pub(crate) fn retained_model_roots(
    device: &lettuce_settings::DeviceSettings,
    app_folder: &Path,
) -> lettuce_settings::RetainedModelRoots {
    let roots = &device.retained_model_roots;
    let chosen = |saved: &Option<String>, default: PathBuf| {
        Some(
            saved
                .clone()
                .unwrap_or_else(|| default.to_string_lossy().into_owned()),
        )
    };
    lettuce_settings::RetainedModelRoots {
        whisper: chosen(&roots.whisper, whisper_models_root(app_folder)),
        kokoro: chosen(&roots.kokoro, kokoro_root(app_folder)),
        embedding: chosen(&roots.embedding, crate::embedding_models_root(app_folder)),
        thymos: chosen(&roots.thymos, crate::companion_emotion_root(app_folder)),
    }
}

pub(crate) fn rebind_memory_model_manifests(
    roots: &lettuce_settings::RetainedModelRoots,
    old: &Path,
    new: &Path,
    mut record: impl FnMut(&Path, &[u8], &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    if let Some(root) = roots.embedding.as_deref().map(Path::new)
        && let Ok(relative) = root.strip_prefix(old)
        && root.exists()
    {
        let destination = lettuce_model_hub::EmbeddingInstallStore::new(new.join(relative));
        for mut manifest in lettuce_model_hub::EmbeddingInstallStore::new(root)
            .installed()
            .map_err(|error| error.to_string())?
        {
            for artifact in [&mut manifest.model, &mut manifest.tokenizer]
                .into_iter()
                .chain(manifest.calibration.iter_mut())
            {
                if let Ok(relative) = artifact.path.strip_prefix(old) {
                    artifact.path = new.join(relative);
                }
            }
            let source_path = root
                .join(manifest.family.install_dir())
                .join("manifest.json");
            let target_path = new
                .join(relative)
                .join(manifest.family.install_dir())
                .join("manifest.json");
            let before = std::fs::read(source_path).map_err(|error| error.to_string())?;
            let after = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
            record(&target_path, &before, &after)?;
            destination
                .record(&manifest)
                .map_err(|error| error.to_string())?;
        }
    }
    if let Some(root) = roots.thymos.as_deref().map(Path::new)
        && let Ok(relative) = root.strip_prefix(old)
        && root.exists()
    {
        let source = lettuce_model_hub::CompanionEmotionInstallStore::open(root)
            .map_err(|error| error.to_string())?;
        if let Some(mut manifest) = source.installed().map_err(|error| error.to_string())? {
            let before =
                std::fs::read(root.join("installed.json")).map_err(|error| error.to_string())?;
            for artifact in [
                &mut manifest.model,
                &mut manifest.tokenizer,
                &mut manifest.labels,
            ] {
                if let Ok(relative) = artifact.path.strip_prefix(old) {
                    artifact.path = new.join(relative);
                }
            }
            manifest.verify().map_err(|error| error.to_string())?;
            let after = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
            record(&new.join(relative).join("installed.json"), &before, &after)?;
        }
        lettuce_model_hub::CompanionEmotionInstallStore::open(new.join(relative))
            .map_err(|error| error.to_string())?
            .rebind_from(root)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
