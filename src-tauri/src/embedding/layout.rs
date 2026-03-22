use std::fs;
use std::path::Path;

use super::specs::{
    MODEL_FILES_V1, MODEL_FILES_V2_LOCAL, MODEL_FILES_V2_LOCAL_LEGACY, MODEL_FILES_V3_LOCAL,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct InstalledSources {
    pub(crate) has_v1: bool,
    pub(crate) has_v2: bool,
    pub(crate) has_v3: bool,
}

pub(crate) fn detect_installed_sources(model_dir: &Path) -> InstalledSources {
    let has_v1 = MODEL_FILES_V1
        .iter()
        .all(|filename| model_dir.join(filename).exists());

    let has_v2 = MODEL_FILES_V2_LOCAL
        .iter()
        .all(|filename| model_dir.join(filename).exists())
        || MODEL_FILES_V2_LOCAL_LEGACY
            .iter()
            .all(|filename| model_dir.join(filename).exists());

    let has_v3 = MODEL_FILES_V3_LOCAL
        .iter()
        .all(|filename| model_dir.join(filename).exists());

    InstalledSources {
        has_v1,
        has_v2,
        has_v3,
    }
}

pub(crate) fn migrate_legacy_layout(model_dir: &Path) -> Result<(), String> {
    // Layout v2 migration: materialize v2-tokenizer.json from shared tokenizer.json
    fs::create_dir_all(model_dir)
        .map_err(|e| format!("failed to ensure embedding model dir exists: {}", e))?;

    let legacy_tokenizer = model_dir.join("tokenizer.json");

    let v2_model = model_dir.join("v2-model.onnx");
    let v2_data = model_dir.join("v2-model.onnx.data");
    let v2_tokenizer = model_dir.join("v2-tokenizer.json");
    if v2_model.exists() && v2_data.exists() && legacy_tokenizer.exists() && !v2_tokenizer.exists()
    {
        fs::copy(&legacy_tokenizer, &v2_tokenizer)
            .map_err(|e| format!("failed to migrate v2 tokenizer: {}", e))?;
    }

    Ok(())
}
