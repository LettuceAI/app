use tauri::AppHandle;

use crate::chat_manager::persistence::storage::resolve_credential_for_model;
use crate::chat_manager::request as chat_request;
use crate::chat_manager::types::{ProviderCredential, Settings};
use crate::chat_manager::{
    entries::{in_chat_user_entry, relative_system_entry},
    request_builder::{effective_streaming_enabled, system_role_for},
    turn_builder::assemble_prompt_messages,
};
use crate::creation_helper::service::send_creation_api_request;
use crate::storage_manager::settings::internal_read_settings;

pub struct LlmContext {
    pub cred: ProviderCredential,
    pub api_key: String,
    pub provider_id: String,
    pub provider_label: String,
    pub model_id: String,
    pub model_name: String,
    pub streaming_enabled: bool,
}

pub fn load_context(app: &AppHandle) -> Result<LlmContext, String> {
    let settings_json =
        internal_read_settings(app)?.ok_or_else(|| "No settings found".to_string())?;
    let settings: Settings =
        serde_json::from_str(&settings_json).map_err(|e| format!("settings parse: {}", e))?;
    let advanced = settings.advanced_settings.as_ref();

    let model_id = advanced
        .and_then(|a| a.creation_helper_model_id.as_ref())
        .or(settings.default_model_id.as_ref())
        .ok_or_else(|| "No model configured".to_string())?
        .to_string();

    let streaming_enabled_setting = advanced
        .and_then(|a| a.creation_helper_streaming)
        .unwrap_or(true);

    let model = settings
        .models
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| "Model not found".to_string())?;
    let credential = resolve_credential_for_model(&settings, model)
        .ok_or_else(|| "No credentials found for provider".to_string())?;

    let provider_id = model.provider_id.clone();
    let model_name = model.name.clone();
    let provider_label = credential.label.clone();
    let api_key = credential.api_key.clone().unwrap_or_default();
    let cred: ProviderCredential = credential.clone();

    let streaming_enabled = effective_streaming_enabled(&cred, streaming_enabled_setting);

    Ok(LlmContext {
        cred,
        api_key,
        provider_id,
        provider_label,
        model_id,
        model_name,
        streaming_enabled,
    })
}

#[allow(dead_code)]
pub async fn call_text(
    app: &AppHandle,
    ctx: &LlmContext,
    session_id: &str,
    stream_request_id: &str,
    system: &str,
    user: &str,
    visible: bool,
) -> Result<String, String> {
    let system_role = system_role_for(&ctx.cred);
    let messages = assemble_prompt_messages(
        vec![
            relative_system_entry(
                "creation_internal_instructions",
                "Creation Internal Instructions",
                system,
            ),
            in_chat_user_entry(
                "runtime_creation_internal_input",
                "Creation Internal Input",
                user,
                0,
            ),
        ],
        Vec::new(),
        &system_role,
    );

    let streaming = visible && ctx.streaming_enabled;
    let request_id = if visible {
        stream_request_id.to_string()
    } else {
        format!("{}-internal", stream_request_id)
    };

    let resp = send_creation_api_request(
        app,
        session_id,
        &request_id,
        &ctx.provider_id,
        &ctx.cred,
        &ctx.api_key,
        &ctx.model_name,
        &messages,
        streaming,
        None,
    )
    .await?;

    if !resp.ok {
        let err = resp
            .data()
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("LLM call failed")
            .to_string();
        crate::creation_helper::service::record_creation_usage(
            app,
            resp.data(),
            session_id,
            &ctx.model_id,
            &ctx.model_name,
            &ctx.provider_id,
            &ctx.provider_label,
            &ctx.api_key,
            "",
            false,
            Some(err.clone()),
        )
        .await;
        return Err(err);
    }

    let text = chat_request::extract_text(resp.data(), Some(&ctx.provider_id)).unwrap_or_default();
    crate::creation_helper::service::record_creation_usage(
        app,
        resp.data(),
        session_id,
        &ctx.model_id,
        &ctx.model_name,
        &ctx.provider_id,
        &ctx.provider_label,
        &ctx.api_key,
        "",
        true,
        None,
    )
    .await;
    Ok(text)
}
