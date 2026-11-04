use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::providers::openai_compatible::{OpenAiWireProvider, ReasoningWirePolicy};

pub(crate) struct NanoGpt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NanoGptImageCapabilities {
    pub sizes: Vec<String>,
    pub quality: Vec<String>,
    pub max_count: Option<u32>,
}

pub(crate) fn image_capabilities(body: &serde_json::Value, model: &str) -> Result<NanoGptImageCapabilities, crate::common::AdapterError> {
    let item = body.get("data").and_then(serde_json::Value::as_array)
        .and_then(|items| items.iter().find(|item| item.get("id").and_then(serde_json::Value::as_str) == Some(model)))
        .ok_or(crate::common::AdapterError::MalformedResponse)?;
    let parameters = item.get("supported_parameters");
    let strings = |key: &str| parameters.and_then(|parameters| parameters.get(key)).and_then(serde_json::Value::as_array)
        .map(|values| values.iter().filter_map(serde_json::Value::as_str).map(str::to_owned).collect()).unwrap_or_default();
    Ok(NanoGptImageCapabilities {
        sizes: strings("resolutions"), quality: strings("quality"),
        max_count: parameters.and_then(|parameters| parameters.get("fixed_image_count").or_else(|| parameters.get("max_images")))
            .and_then(serde_json::Value::as_u64).and_then(|count| u32::try_from(count).ok()).filter(|count| *count > 0),
    })
}

/// NanoGPT publishes image models separately from its text catalog.
pub(crate) async fn list_models<S: lettuce_settings::SecretStore + ?Sized>(
    store: &S,
    network: &lettuce_network::JsonClient,
    account: &lettuce_models::ProviderAccount,
) -> Result<Vec<crate::RemoteModel>, crate::common::AdapterError> {
    use crate::common::{Credentials, decode_json, generation_policy, load_auth, load_secret_headers, parse_openai_model_list, AuthPlan};
    let mut models = crate::providers::openai_compatible::list_models(&NanoGpt, store, network, account).await?;
    let base = account.endpoint.as_deref().or(DESCRIPTOR.default_endpoint).ok_or(crate::common::AdapterError::Rejected)?.trim_end_matches('/');
    let path = if base.ends_with("/v1") { "/image-models?detailed=true" } else { "/v1/image-models?detailed=true" };
    let credentials = Credentials::from(account);
    let response = network.get_json(base, path, &crate::common::ACCEPT_ONLY,
        load_auth(AuthPlan::Bearer, store, &credentials).await?, load_secret_headers(store, &credentials).await?, generation_policy(&credentials)).await?;
    let body = decode_json(&response)?;
    if body.get("data").and_then(serde_json::Value::as_array).is_none() { return Err(crate::common::AdapterError::MalformedResponse); }
    for mut image in parse_openai_model_list(&body) {
        image.output_modalities = Some(vec!["image".into()]);
        if let Some(existing) = models.iter_mut().find(|model| model.id == image.id) { *existing = image; }
        else { models.push(image); }
    }
    Ok(models)
}

impl OpenAiWireProvider for NanoGpt {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn includes_stream_usage(&self) -> bool {
        true
    }

    fn models_path(&self, endpoint: &str, _config: &lettuce_models::ProviderConfig) -> Option<std::borrow::Cow<'static, str>> {
        Some(std::borrow::Cow::Borrowed(if endpoint.trim_end_matches('/').ends_with("/v1") { "/models?detailed=true" } else { "/v1/models?detailed=true" }))
    }

    fn reasoning_policy(&self) -> ReasoningWirePolicy {
        ReasoningWirePolicy::MaxTokens
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "nanogpt",
    display_name: "NanoGPT",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: Some("https://nano-gpt.com/api"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags::PENALTIES,
    extra_body_keys: &[],
};
