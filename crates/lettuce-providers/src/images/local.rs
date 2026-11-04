//! Connection metadata and checkpoint discovery for self-hosted image servers.

use base64::Engine as _;
use lettuce_models::{ProviderAccount, ProviderProtocol};
use lettuce_network::{JsonAuth, JsonClient};
use lettuce_settings::{HeaderName, SecretStore, SecretValue};
use serde_json::Value;

use crate::common::{AdapterError, Credentials, decode_json, load_auth, load_secret_headers, probe_policy, AuthPlan};
use crate::descriptor::{ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport, RemoteModel};

const fn descriptor(kind: &'static str, name: &'static str, endpoint: &'static str) -> ProviderDescriptor {
    ProviderDescriptor {
        kind, display_name: name, protocol: ProviderProtocol::StableDiffusion, aliases: &[],
        default_endpoint: Some(endpoint), endpoint_editable: true,
        api_key: ApiKeyRequirement::Optional, auth_header: "authorization", streaming: false,
        lists_models: true, verifies_key: true, reasoning: ReasoningSupport::None,
        prompt_caching: PromptCachingSupport::None,
        parameters: ParameterFlags { temperature: false, top_p: false, max_output_tokens: false,
            context_length: false, frequency_penalty: false, presence_penalty: false,
            top_k: false, repetition_penalty: false, reasoning_budget: false },
        extra_body_keys: &[],
    }
}

pub(crate) const COMFYUI: ProviderDescriptor = descriptor("comfyui", "ComfyUI (Self-hosted)", "http://127.0.0.1:8188");
pub(crate) const AUTOMATIC1111: ProviderDescriptor = descriptor("automatic1111", "Automatic1111 (Self-hosted)", "http://127.0.0.1:7860");

pub(crate) fn api_base(account: &ProviderAccount) -> Result<&str, AdapterError> {
    let descriptor = crate::provider_descriptor(&account.provider_kind).ok_or(AdapterError::Rejected)?;
    let base = account.endpoint.as_deref().or(descriptor.default_endpoint).ok_or(AdapterError::Rejected)?.trim_end_matches('/');
    Ok(if account.provider_kind.eq_ignore_ascii_case("automatic1111") {
        base.strip_suffix("/sdapi/v1").unwrap_or(base)
    } else { base })
}

/// WebUI's --api-auth accepts user:password. A plain token is still usable
/// with reverse proxies that expect bearer authentication.
pub(crate) fn auth(kind: &str, key: Option<SecretValue>) -> Result<JsonAuth, AdapterError> {
    let Some(key) = key else { return Ok(JsonAuth::None); };
    if kind.eq_ignore_ascii_case("automatic1111") && key.with(|value| value.contains(':')) {
        let value = key.with(|value| format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(value)));
        Ok(JsonAuth::Header { name: HeaderName::new("authorization").map_err(|_| AdapterError::Rejected)?, value: SecretValue::new(value).map_err(|_| AdapterError::Rejected)? })
    } else { Ok(JsonAuth::Bearer(key)) }
}

pub(crate) async fn list_models<S: SecretStore + ?Sized>(store: &S, network: &JsonClient, account: &ProviderAccount) -> Result<Vec<RemoteModel>, AdapterError> {
    let comfy = account.provider_kind.eq_ignore_ascii_case("comfyui");
    let credentials = Credentials::from(account);
    let loaded = load_auth(AuthPlan::OptionalBearer, store, &credentials).await?;
    let key = match loaded { JsonAuth::Bearer(key) => Some(key), _ => None };
    let response = network.get_json(api_base(account)?, if comfy { "/object_info/CheckpointLoaderSimple" } else { "/sdapi/v1/sd-models" }, &crate::common::ACCEPT_ONLY,
        auth(&account.provider_kind, key)?, load_secret_headers(store, &credentials).await?, probe_policy(&credentials)).await?;
    let body: Value = decode_json(&response)?;
    let entries = if comfy {
        body.pointer("/CheckpointLoaderSimple/input/required/ckpt_name/0").and_then(Value::as_array)
    } else { body.as_array() }.ok_or(AdapterError::MalformedResponse)?;
    entries.iter().map(|entry| {
        let id = if comfy { entry.as_str() } else { entry.get("title").and_then(Value::as_str).or_else(|| entry.get("model_name").and_then(Value::as_str)) }.ok_or(AdapterError::MalformedResponse)?;
        Ok(RemoteModel { id: id.to_owned(), display_name: Some(id.to_owned()), description: None,
            context_length: None, input_modalities: Some(vec!["text".into(), "image".into()]),
            output_modalities: Some(vec!["image".into()]), supported_endpoints: None,
            input_price: None, cache_read_price: None, output_price: None })
    }).collect()
}
