//! Model profiles read from USC `model_profile` cards and the old app's model
//! JSON.

use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelTransferError {
    #[error("Invalid model file: {0}")]
    InvalidJson(String),
    #[error("Model name is required.")]
    NameRequired,
    #[error("Provider ID is required.")]
    ProviderRequired,
    #[error("Unsupported model file.")]
    Unsupported,
}

/// A model read from a file, before its provider account is resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedModel {
    pub name: String,
    pub provider_id: String,
    pub provider_label: String,
    pub display_name: String,
    pub input_scopes: Vec<String>,
    pub output_scopes: Vec<String>,
    pub advanced_model_settings: Map<String, Value>,
}

/// Legacy scopes: only `text`, `image` and `audio`, once each and in that
/// order, `text` when none is left.
#[must_use]
pub fn legacy_model_scopes(value: Option<&Value>) -> Vec<String> {
    let listed = value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let scopes = ["text", "image", "audio"]
        .into_iter()
        .filter(|scope| listed.contains(scope))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if scopes.is_empty() {
        vec!["text".to_owned()]
    } else {
        scopes
    }
}

/// A model file: a USC `model_profile` card, else a model JSON object.
pub fn parse_model_import(json: &str) -> Result<ImportedModel, ModelTransferError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|error| ModelTransferError::InvalidJson(error.to_string()))?;
    let payload = (value.pointer("/schema/name").and_then(Value::as_str) == Some("USC")
        && value.get("kind").and_then(Value::as_str) == Some("model_profile"))
    .then(|| value.get("payload"))
    .flatten()
    .filter(|payload| crate::files::prompt_transfer::truthy(payload));
    let input = match payload {
        Some(payload) => payload,
        None if value.is_object() || value.is_array() => &value,
        None => return Err(ModelTransferError::Unsupported),
    };
    let trimmed = |key: &str| {
        input
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let name = trimmed("name").ok_or(ModelTransferError::NameRequired)?;
    let provider_id = trimmed("providerId").ok_or(ModelTransferError::ProviderRequired)?;
    Ok(ImportedModel {
        provider_label: trimmed("providerLabel").unwrap_or_else(|| provider_id.clone()),
        display_name: trimmed("displayName").unwrap_or_else(|| name.clone()),
        name,
        provider_id,
        input_scopes: legacy_model_scopes(input.get("inputScopes")),
        output_scopes: legacy_model_scopes(input.get("outputScopes")),
        advanced_model_settings: input
            .get("advancedModelSettings")
            .and_then(Value::as_object)
            .cloned()
            .map(|mut settings| {
                settings.remove("llamaLastRuntimeReport");
                settings
            })
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn both_model_files_read_as_the_same_model() {
        let settings = json!({"temperature": 0.5});
        let usc = json!({
            "schema": {"name": "USC", "version": "1.0"},
            "kind": "model_profile",
            "payload": {
                "id": "m1",
                "name": "gpt-4o",
                "displayName": "GPT-4o",
                "providerId": "openai",
                "providerLabel": "Work",
                "inputScopes": ["text", "image"],
                "outputScopes": ["text"],
                "advancedModelSettings": settings,
                "createdAt": 5
            }
        });
        let plain = usc["payload"].clone();
        for file in [usc, plain] {
            let imported = parse_model_import(&file.to_string()).expect("import");
            assert_eq!(imported.name, "gpt-4o");
            assert_eq!(imported.display_name, "GPT-4o");
            assert_eq!(imported.provider_id, "openai");
            assert_eq!(imported.provider_label, "Work");
            assert_eq!(imported.input_scopes, vec!["text", "image"]);
            assert_eq!(imported.output_scopes, vec!["text"]);
            assert_eq!(
                imported.advanced_model_settings,
                settings.as_object().cloned().expect("object")
            );
        }
    }

    #[test]
    fn a_model_file_is_normalized_like_the_old_import() {
        let imported = parse_model_import(
            r#"{"name": " m ", "providerId": " ollama ", "inputScopes": ["audio", "video", "text", "audio"], "outputScopes": []}"#,
        )
        .expect("import");
        assert_eq!(imported.name, "m");
        assert_eq!(imported.provider_label, "ollama");
        assert_eq!(imported.display_name, "m");
        assert_eq!(imported.input_scopes, vec!["text", "audio"]);
        assert_eq!(imported.output_scopes, vec!["text"]);
        assert_eq!(
            parse_model_import(r#"{"providerId": "x"}"#),
            Err(ModelTransferError::NameRequired)
        );
        assert_eq!(
            parse_model_import(r#"{"name": "x"}"#),
            Err(ModelTransferError::ProviderRequired)
        );
        assert_eq!(
            parse_model_import("[1]"),
            Err(ModelTransferError::NameRequired)
        );
        assert_eq!(
            parse_model_import("3"),
            Err(ModelTransferError::Unsupported)
        );
    }
}
