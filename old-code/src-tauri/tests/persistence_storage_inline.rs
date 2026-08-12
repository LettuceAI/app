//! Gathered from inline tests in src/chat_manager/persistence/storage.rs.

use lettuceai_lib::chat_manager::persistence::storage::resolve_credential_for_model;
use lettuceai_lib::chat_manager::types::{
    AdvancedModelSettings, Model, ProviderCredential, Settings,
};
use serde_json::Value;

fn mk_model(provider_id: &str, provider_label: &str, name: &str) -> Model {
    Model {
        id: "model-1".to_string(),
        name: name.to_string(),
        provider_id: provider_id.to_string(),
        provider_credential_id: None,
        provider_label: provider_label.to_string(),
        display_name: name.to_string(),
        created_at: 0,
        input_scopes: vec!["text".to_string()],
        output_scopes: vec!["text".to_string()],
        advanced_model_settings: None,
        prompt_template_id: None,
        voice_config: None,
        system_prompt: None,
    }
}

fn mk_cred(
    id: &str,
    provider_id: &str,
    label: &str,
    default_model: Option<&str>,
) -> ProviderCredential {
    ProviderCredential {
        id: id.to_string(),
        provider_id: provider_id.to_string(),
        label: label.to_string(),
        api_key: Some("k".to_string()),
        base_url: Some("https://example.com".to_string()),
        default_model: default_model.map(str::to_string),
        headers: None,
        config: None,
    }
}

fn mk_settings(
    default_provider_credential_id: Option<&str>,
    provider_credentials: Vec<ProviderCredential>,
) -> Settings {
    Settings {
        default_provider_credential_id: default_provider_credential_id.map(str::to_string),
        default_model_id: None,
        provider_credentials,
        models: vec![],
        app_state: Value::Null,
        advanced_model_settings: AdvancedModelSettings::default(),
        advanced_settings: None,
        prompt_template_id: None,
        system_prompt: None,
        migration_version: 0,
    }
}

#[test]
fn resolves_single_candidate() {
    let model = mk_model("custom", "local", "glm-auto");
    let settings = mk_settings(None, vec![mk_cred("c1", "custom", "local", None)]);
    let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
    assert_eq!(picked.as_deref(), Some("c1"));
}

#[test]
fn resolves_to_default_provider_credential_when_present() {
    let model = mk_model("custom", "local", "glm-auto");
    let settings = mk_settings(
        Some("c2"),
        vec![
            mk_cred("c1", "custom", "local", None),
            mk_cred("c2", "custom", "modal", None),
        ],
    );
    let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
    assert_eq!(picked.as_deref(), Some("c2"));
}

#[test]
fn resolves_by_provider_label_when_multiple_candidates_exist() {
    let model = mk_model("custom", "local", "glm-auto");
    let settings = mk_settings(
        None,
        vec![
            mk_cred("c1", "custom", "modal", None),
            mk_cred("c2", "custom", "local", None),
        ],
    );
    let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
    assert_eq!(picked.as_deref(), Some("c2"));
}

#[test]
fn resolves_by_credential_default_model_when_label_does_not_match() {
    let model = mk_model("custom", "unknown", "glm-auto");
    let settings = mk_settings(
        None,
        vec![
            mk_cred("c1", "custom", "modal", None),
            mk_cred("c2", "custom", "local", Some("glm-auto")),
        ],
    );
    let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
    assert_eq!(picked.as_deref(), Some("c2"));
}

#[test]
fn returns_none_for_ambiguous_multiple_candidates() {
    let model = mk_model("custom", "", "glm-auto");
    let settings = mk_settings(
        None,
        vec![
            mk_cred("c1", "custom", "one", None),
            mk_cred("c2", "custom", "two", None),
        ],
    );
    let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
    assert!(picked.is_none());
}

#[test]
fn resolves_explicit_model_provider_credential_id_first() {
    let mut model = mk_model("custom", "local", "glm-auto");
    model.provider_credential_id = Some("c2".to_string());
    let settings = mk_settings(
        None,
        vec![
            mk_cred("c1", "custom", "local", Some("glm-auto")),
            mk_cred("c2", "custom", "modal", None),
        ],
    );
    let picked = resolve_credential_for_model(&settings, &model).map(|c| c.id.clone());
    assert_eq!(picked.as_deref(), Some("c2"));
}
