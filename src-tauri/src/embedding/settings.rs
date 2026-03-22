use tauri::AppHandle;

#[derive(Debug, Clone, Default)]
pub(crate) struct EmbeddingPreferences {
    pub(crate) preferred_source_version: Option<String>,
    pub(crate) max_tokens: Option<usize>,
    pub(crate) keep_model_loaded: bool,
}

#[derive(Debug, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AdvancedSettingsSnapshot {
    embedding_model_version: Option<String>,
    embedding_max_tokens: Option<u64>,
    embedding_keep_model_loaded: Option<bool>,
}

#[derive(Debug, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SettingsSnapshot {
    advanced_settings: Option<AdvancedSettingsSnapshot>,
}

pub(crate) fn read_embedding_preferences(app: &AppHandle) -> EmbeddingPreferences {
    let parsed = crate::storage_manager::settings::internal_read_settings(app)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<SettingsSnapshot>(&raw).ok())
        .unwrap_or_default();

    let advanced = parsed.advanced_settings.unwrap_or_default();
    EmbeddingPreferences {
        preferred_source_version: advanced.embedding_model_version,
        max_tokens: advanced.embedding_max_tokens.map(|v| v as usize),
        keep_model_loaded: advanced.embedding_keep_model_loaded.unwrap_or(false),
    }
}
