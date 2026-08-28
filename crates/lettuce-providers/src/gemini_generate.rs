use std::borrow::Cow;

use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceRequest, InferenceUsage,
    InferenceWarningCode, MessagePart, MessageRole, ProviderContextPart, ProviderNeutralContext,
};
use lettuce_inference::InferenceRuntimePort;
use lettuce_models::{
    ProviderAccount, ProviderConfig, ReasoningEffort, ReasoningMode, ResolvedChatParameters,
    ResolvedChatProfile,
};
use lettuce_network::{
    JsonClient, JsonQueryParameter, JsonResponse, JsonResponseStream, JsonStaticHeader,
    MAX_REQUEST_BYTES,
};
use lettuce_settings::{HeaderName, SecretStore};
use serde::{Deserialize, Serialize};

use crate::common::{
    AdapterError, AuthPlan, Credentials, RemoteModel, decode_json, generation_policy, load_auth,
    load_secret_headers, max_output_tokens, reject_unsupported_features, validate_common_request,
    validate_prompt_caching, validate_supported_reasoning,
};
use crate::descriptor::ProviderDescriptor;
use crate::gemini_cache::{GeminiCache, PreparedCache};

pub(crate) const GEMINI_HEADERS: [JsonStaticHeader; 2] = [
    JsonStaticHeader {
        name: "accept",
        value: "application/json",
    },
    JsonStaticHeader {
        name: "user-agent",
        value: concat!("LettuceAI/", env!("CARGO_PKG_VERSION")),
    },
];

pub(crate) trait GeminiWireProvider: Sync {
    fn descriptor(&self) -> &'static ProviderDescriptor;

    fn api_base<'a>(&self, endpoint: &'a str) -> Cow<'a, str>;

    fn generate_path(&self, model: &str) -> Result<String, AdapterError>;

    fn models_path(&self) -> Option<&'static str> {
        None
    }

    fn parse_models(&self, payload: &serde_json::Value) -> Vec<RemoteModel> {
        payload
            .get("models")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let name = item.get("name")?.as_str()?;
                        Some(RemoteModel {
                            id: name.strip_prefix("models/").unwrap_or(name).to_owned(),
                            display_name: item
                                .get("displayName")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned),
                            description: item
                                .get("description")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned),
                            context_length: item
                                .get("inputTokenLimit")
                                .and_then(serde_json::Value::as_u64),
                            input_modalities: None,
                            output_modalities: None,
                            supported_endpoints: None,
                            input_price: None,
                            output_price: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn auth(&self, _config: &ProviderConfig) -> Result<AuthPlan, AdapterError> {
        HeaderName::new("x-goog-api-key")
            .map(AuthPlan::Header)
            .map_err(|_| AdapterError::Rejected)
    }

    fn static_headers(&self) -> &'static [JsonStaticHeader] {
        &GEMINI_HEADERS
    }

    fn validate_parameters(&self, parameters: &ResolvedChatParameters) -> Result<(), AdapterError> {
        validate_supported_reasoning(parameters)?;
        if parameters.reasoning_mode != Some(ReasoningMode::Enabled) {
            reject_unsupported_features(parameters)?;
        } else if parameters.reasoning_budget_tokens > Some(i32::MAX as u32) {
            return Err(AdapterError::Rejected);
        }
        validate_prompt_caching(self.descriptor().prompt_caching, parameters)
    }
}

pub(crate) fn validate_model_id(model: &str) -> Result<&str, AdapterError> {
    if model.is_empty()
        || model.len() > 256
        || !model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(AdapterError::Rejected);
    }
    Ok(model)
}

pub(crate) async fn run<S: SecretStore + ?Sized>(
    provider: &dyn GeminiWireProvider,
    cache: &GeminiCache,
    secret_store: &S,
    network: &JsonClient,
    runtime: &dyn InferenceRuntimePort,
    request: InferenceRequest,
) -> Result<InferenceOutcome, AdapterError> {
    validate_common_request(&request)?;
    let profile = &request.profile.chat_profile;
    if !matches!(profile.provider_config, ProviderConfig::Standard) {
        return Err(AdapterError::Rejected);
    }
    provider.validate_parameters(&profile.parameters)?;
    let endpoint = profile
        .endpoint
        .as_deref()
        .or(provider.descriptor().default_endpoint)
        .ok_or(AdapterError::Rejected)?;
    let base = provider.api_base(endpoint);
    let path = provider.generate_path(&profile.external_model_id)?;
    let streaming = request.stream_sink.is_some();
    if streaming && (!profile.streaming_enabled || !provider.descriptor().streaming) {
        return Err(AdapterError::Rejected);
    }
    let uncached = build_request(profile, &request.context)?;
    let prepared = crate::streaming::await_cancelable(
        runtime,
        request.cancellation,
        cache.prepare(provider, secret_store, network, profile, &base, &uncached),
    )
    .await?;
    let body = match &prepared {
        Some(prepared) => encode_request(&uncached.with_cache(prepared.name.clone())?)?,
        None => encode_request(&uncached)?,
    };
    if streaming {
        let stream_path = path
            .strip_suffix(":generateContent")
            .map(|prefix| format!("{prefix}:streamGenerateContent"))
            .ok_or(AdapterError::Rejected)?;
        let mut response = crate::streaming::await_cancelable(
            runtime,
            request.cancellation,
            send_generate_stream(
                provider,
                secret_store,
                network,
                profile,
                &base,
                &stream_path,
                body,
            ),
        )
        .await?;
        if let Some(PreparedCache { key, .. }) = prepared
            && response.status == 404
        {
            cache.evict(key);
            tracing::warn!(
                provider = "gemini",
                cache_result = "resource_not_found",
                "cached Gemini resource disappeared; retrying the full streaming request"
            );
            response = crate::streaming::await_cancelable(
                runtime,
                request.cancellation,
                send_generate_stream(
                    provider,
                    secret_store,
                    network,
                    profile,
                    &base,
                    &stream_path,
                    encode_request(&uncached)?,
                ),
            )
            .await?;
        }
        return crate::streaming::consume_stream(
            response,
            crate::stream_framing::StreamFormat::Sse,
            crate::stream_normalize::StreamProtocol::Gemini,
            runtime,
            &request,
        )
        .await;
    }
    let mut response = crate::streaming::await_cancelable(
        runtime,
        request.cancellation,
        send_generate(provider, secret_store, network, profile, &base, &path, body),
    )
    .await?;
    if let Some(PreparedCache { key, .. }) = prepared
        && response.status == 404
    {
        cache.evict(key);
        tracing::warn!(
            provider = "gemini",
            cache_result = "resource_not_found",
            "cached Gemini resource disappeared; retrying the full request"
        );
        response = crate::streaming::await_cancelable(
            runtime,
            request.cancellation,
            send_generate(
                provider,
                secret_store,
                network,
                profile,
                &base,
                &path,
                encode_request(&uncached)?,
            ),
        )
        .await?;
    }
    parse_response(response)
}

async fn send_generate_stream<S: SecretStore + ?Sized>(
    provider: &dyn GeminiWireProvider,
    secret_store: &S,
    network: &JsonClient,
    profile: &ResolvedChatProfile,
    base: &str,
    path: &str,
    body: Vec<u8>,
) -> Result<JsonResponseStream, AdapterError> {
    const SSE_QUERY: [JsonQueryParameter; 1] = [JsonQueryParameter {
        name: "alt",
        value: "sse",
    }];
    let credentials = Credentials::from(profile);
    let auth = load_auth(
        provider.auth(&profile.provider_config)?,
        secret_store,
        &credentials,
    )
    .await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    network
        .post_json_stream_with_query(
            base,
            path,
            body,
            &SSE_QUERY,
            provider.static_headers(),
            auth,
            secret_headers,
            generation_policy(&credentials),
        )
        .await
        .map_err(Into::into)
}

async fn send_generate<S: SecretStore + ?Sized>(
    provider: &dyn GeminiWireProvider,
    secret_store: &S,
    network: &JsonClient,
    profile: &ResolvedChatProfile,
    base: &str,
    path: &str,
    body: Vec<u8>,
) -> Result<JsonResponse, AdapterError> {
    let credentials = Credentials::from(profile);
    let auth = load_auth(
        provider.auth(&profile.provider_config)?,
        secret_store,
        &credentials,
    )
    .await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    network
        .post_json(
            base,
            path,
            body,
            provider.static_headers(),
            auth,
            secret_headers,
            generation_policy(&credentials),
        )
        .await
        .map_err(Into::into)
}

pub(crate) async fn list_models<S: SecretStore + ?Sized>(
    provider: &dyn GeminiWireProvider,
    secret_store: &S,
    network: &JsonClient,
    account: &ProviderAccount,
) -> Result<Vec<RemoteModel>, AdapterError> {
    if !matches!(account.config, ProviderConfig::Standard) {
        return Err(AdapterError::Rejected);
    }
    let path = provider.models_path().ok_or(AdapterError::Rejected)?;
    let endpoint = account
        .endpoint
        .as_deref()
        .or(provider.descriptor().default_endpoint)
        .ok_or(AdapterError::Rejected)?;
    let base = provider.api_base(endpoint);
    let credentials = Credentials::from(account);
    let auth = load_auth(provider.auth(&account.config)?, secret_store, &credentials).await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    let response = network
        .get_json(
            &base,
            path,
            provider.static_headers(),
            auth,
            secret_headers,
            generation_policy(&credentials),
        )
        .await?;
    Ok(provider.parse_models(&decode_json(&response)?))
}

fn build_request(
    profile: &ResolvedChatProfile,
    context: &ProviderNeutralContext,
) -> Result<GenerateRequest, AdapterError> {
    let mut system_chunks: Vec<String> = Vec::new();
    let mut contents: Vec<Content> = Vec::new();
    for message in &context.messages {
        let mut text = String::new();
        for part in &message.parts {
            match part {
                ProviderContextPart::Text { text: fragment } => text.push_str(fragment),
                ProviderContextPart::MediaAsset { .. } => return Err(AdapterError::Rejected),
            }
        }
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        match message.role {
            MessageRole::System | MessageRole::Scene => system_chunks.push(trimmed.to_owned()),
            MessageRole::User => contents.push(Content {
                role: "user",
                parts: vec![TextPart { text }],
            }),
            MessageRole::Assistant => contents.push(Content {
                role: "model",
                parts: vec![TextPart { text }],
            }),
        }
    }
    if contents.is_empty() {
        return Err(AdapterError::Rejected);
    }
    let parameters = &profile.parameters;
    let request = GenerateRequest {
        contents,
        system_instruction: (!system_chunks.is_empty()).then(|| Content {
            role: "user",
            parts: vec![TextPart {
                text: system_chunks.join("\n\n"),
            }],
        }),
        generation_config: GenerationConfig {
            temperature: parameters.temperature,
            top_p: parameters.top_p,
            max_output_tokens: max_output_tokens(parameters),
            top_k: parameters.top_k,
            thinking_config: gemini_thinking_config(&profile.external_model_id, parameters),
        },
        cached_content: None,
    };
    Ok(request)
}

fn encode_request(request: &GenerateRequest) -> Result<Vec<u8>, AdapterError> {
    let body = serde_json::to_vec(&request).map_err(|_| AdapterError::Rejected)?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err(AdapterError::Rejected);
    }
    Ok(body)
}

fn parse_response(response: JsonResponse) -> Result<InferenceOutcome, AdapterError> {
    if let Some(error) = AdapterError::from_response(&response) {
        return Err(error);
    }
    let provider_request_id = response.request_id.clone();
    let parsed: GenerateResponse =
        serde_json::from_slice(&response.body).map_err(|_| AdapterError::MalformedResponse)?;
    let mut warnings = Vec::new();
    if parsed
        .prompt_feedback
        .as_ref()
        .is_some_and(|feedback| feedback.block_reason.is_some())
    {
        push(&mut warnings, InferenceWarningCode::SafetyTransformed);
    }
    let mut candidates = Vec::with_capacity(parsed.candidates.len());
    let mut finish_reason = FinishReason::Stop;
    let mut provider_finish_reason = None;
    let mut has_content = false;
    for (position, candidate) in parsed.candidates.into_iter().enumerate() {
        if position == 0 {
            provider_finish_reason.clone_from(&candidate.finish_reason);
        }
        let ordinal = u16::try_from(candidate.index.unwrap_or(position as u64))
            .map_err(|_| AdapterError::MalformedResponse)?;
        let mut text = String::new();
        let mut reasoning = String::new();
        for part in candidate
            .content
            .map(|content| content.parts)
            .unwrap_or_default()
        {
            if part.thought.unwrap_or(false) {
                reasoning.push_str(part.text.as_deref().unwrap_or_default());
            } else if let Some(fragment) = part.text {
                text.push_str(&fragment);
            }
        }
        match candidate.finish_reason.as_deref() {
            None | Some("STOP") | Some("FINISH_REASON_UNSPECIFIED") => {}
            Some("MAX_TOKENS") => {
                push(&mut warnings, InferenceWarningCode::Truncated);
                if position == 0 {
                    finish_reason = FinishReason::Length;
                }
            }
            Some(
                "SAFETY"
                | "RECITATION"
                | "LANGUAGE"
                | "BLOCKLIST"
                | "PROHIBITED_CONTENT"
                | "SPII"
                | "IMAGE_SAFETY"
                | "IMAGE_PROHIBITED_CONTENT"
                | "IMAGE_RECITATION"
                | "IMAGE_OTHER"
                | "OTHER",
            ) => push(&mut warnings, InferenceWarningCode::SafetyTransformed),
            Some(_) => push(&mut warnings, InferenceWarningCode::ProviderDegraded),
        }
        has_content |= !text.trim().is_empty() || !reasoning.trim().is_empty();
        candidates.push(InferenceCandidate {
            ordinal,
            parts: {
                let mut parts = Vec::new();
                if !reasoning.is_empty() {
                    parts.push(MessagePart::ReasoningSummary { text: reasoning });
                }
                if !text.is_empty() {
                    parts.push(MessagePart::Text { text });
                }
                parts
            },
            provider_replay: None,
        });
    }
    if !has_content
        && !warnings.contains(&InferenceWarningCode::SafetyTransformed)
        && !warnings.contains(&InferenceWarningCode::ProviderDegraded)
    {
        return Err(AdapterError::EmptyResponse);
    }
    if candidates.is_empty() {
        candidates.push(InferenceCandidate {
            ordinal: 0,
            parts: Vec::new(),
            provider_replay: None,
        });
    }
    Ok(InferenceOutcome {
        candidates,
        usage: parsed.usage_metadata.and_then(|usage| {
            Some(InferenceUsage {
                input_tokens: usage.prompt_token_count?,
                output_tokens: usage.candidates_token_count?,
            })
        }),
        finish_reason,
        provider_finish_reason,
        provider_request_id,
        warning_codes: warnings,
    })
}

fn push(warnings: &mut Vec<InferenceWarningCode>, warning: InferenceWarningCode) {
    if !warnings.contains(&warning) {
        warnings.push(warning);
    }
}

#[derive(Clone, Serialize)]
pub(crate) struct GenerateRequest {
    pub(crate) contents: Vec<Content>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    pub(crate) system_instruction: Option<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
    #[serde(rename = "cachedContent", skip_serializing_if = "Option::is_none")]
    cached_content: Option<String>,
}

impl GenerateRequest {
    pub(crate) fn cache_partition(&self) -> Option<(&[Content], &Content)> {
        let (live, prefix) = self.contents.split_last()?;
        (!prefix.is_empty()).then_some((prefix, live))
    }

    fn with_cache(&self, name: String) -> Result<Self, AdapterError> {
        let (_, live) = self.cache_partition().ok_or(AdapterError::Rejected)?;
        Ok(Self {
            contents: vec![live.clone()],
            system_instruction: None,
            generation_config: self.generation_config.clone(),
            cached_content: Some(name),
        })
    }
}

#[derive(Clone, Serialize)]
pub(crate) struct Content {
    role: &'static str,
    parts: Vec<TextPart>,
}

#[derive(Clone, Serialize)]
struct TextPart {
    text: String,
}

#[derive(Clone, Serialize)]
struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(rename = "topP", skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
    #[serde(rename = "topK", skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(rename = "thinkingConfig", skip_serializing_if = "Option::is_none")]
    thinking_config: Option<ThinkingConfig>,
}

#[derive(Clone, Serialize)]
struct ThinkingConfig {
    #[serde(rename = "includeThoughts")]
    include_thoughts: bool,
    #[serde(rename = "thinkingBudget", skip_serializing_if = "Option::is_none")]
    thinking_budget: Option<i32>,
    #[serde(rename = "thinkingLevel", skip_serializing_if = "Option::is_none")]
    thinking_level: Option<&'static str>,
}

fn gemini_thinking_config(
    model: &str,
    parameters: &ResolvedChatParameters,
) -> Option<ThinkingConfig> {
    if parameters.reasoning_mode != Some(ReasoningMode::Enabled) {
        return None;
    }
    let normalized = model.trim().to_ascii_lowercase();
    let budget_model = normalized.contains("gemini-2.5") || normalized.contains("robotics-er-1.5");
    let level_model = normalized.contains("gemini-3");
    Some(ThinkingConfig {
        include_thoughts: true,
        thinking_budget: budget_model.then(|| {
            parameters
                .reasoning_budget_tokens
                .map_or(-1, |budget| budget as i32)
        }),
        thinking_level: level_model
            .then(|| parameters.reasoning_effort.map(gemini_reasoning_level))
            .flatten(),
    })
}

const fn gemini_reasoning_level(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::Low => "LOW",
        ReasoningEffort::Medium => "MEDIUM",
        ReasoningEffort::High => "HIGH",
    }
}

#[derive(Deserialize)]
struct GenerateResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
    #[serde(rename = "promptFeedback")]
    prompt_feedback: Option<PromptFeedback>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Deserialize)]
struct Candidate {
    index: Option<u64>,
    content: Option<CandidateContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct CandidateContent {
    #[serde(default)]
    parts: Vec<CandidatePart>,
}

#[derive(Deserialize)]
struct CandidatePart {
    text: Option<String>,
    thought: Option<bool>,
}

#[derive(Deserialize)]
struct PromptFeedback {
    #[serde(rename = "blockReason")]
    block_reason: Option<String>,
}

#[derive(Deserialize)]
struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<u64>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(body: &str) -> JsonResponse {
        JsonResponse {
            status: 200,
            body: body.as_bytes().to_vec(),
            request_id: None,
            retry_after: None,
        }
    }

    fn reasoning_parameters() -> ResolvedChatParameters {
        ResolvedChatParameters {
            reasoning_mode: Some(ReasoningMode::Enabled),
            reasoning_effort: Some(ReasoningEffort::Medium),
            reasoning_budget_tokens: Some(8192),
            ..crate::integration_tests::parameters()
        }
    }

    #[test]
    fn maps_reasoning_by_gemini_model_family() {
        let parameters = reasoning_parameters();
        assert_eq!(
            serde_json::to_value(
                gemini_thinking_config("gemini-2.5-pro", &parameters).expect("2.5 config")
            )
            .expect("serialize"),
            serde_json::json!({
                "includeThoughts": true,
                "thinkingBudget": 8192,
            })
        );
        assert_eq!(
            serde_json::to_value(
                gemini_thinking_config("gemini-3-pro", &parameters).expect("3 config")
            )
            .expect("serialize"),
            serde_json::json!({
                "includeThoughts": true,
                "thinkingLevel": "MEDIUM",
            })
        );
        assert_eq!(
            serde_json::to_value(
                gemini_thinking_config("future-gemini", &parameters).expect("unknown config")
            )
            .expect("serialize"),
            serde_json::json!({ "includeThoughts": true })
        );
    }

    #[test]
    fn gemini_budget_models_request_provider_auto_budget_when_unspecified() {
        let mut parameters = reasoning_parameters();
        parameters.reasoning_effort = None;
        parameters.reasoning_budget_tokens = None;
        assert_eq!(
            serde_json::to_value(
                gemini_thinking_config("robotics-er-1.5-preview", &parameters)
                    .expect("robotics config")
            )
            .expect("serialize"),
            serde_json::json!({
                "includeThoughts": true,
                "thinkingBudget": -1,
            })
        );
    }

    #[test]
    fn preserves_thought_parts_and_reads_native_usage() {
        let outcome = parse_response(response(
            r#"{"candidates":[{"content":{"parts":[{"text":"hidden","thought":true},{"text":"vis"},{"text":"ible"}],"role":"model"},"finishReason":"MAX_TOKENS"}],"usageMetadata":{"promptTokenCount":9,"candidatesTokenCount":4,"totalTokenCount":13}}"#,
        ))
        .expect("response");
        assert_eq!(
            outcome.candidates[0].parts,
            vec![
                MessagePart::ReasoningSummary {
                    text: "hidden".to_owned()
                },
                MessagePart::Text {
                    text: "visible".to_owned()
                }
            ]
        );
        assert_eq!(outcome.finish_reason, FinishReason::Length);
        assert_eq!(
            outcome.usage,
            Some(InferenceUsage {
                input_tokens: 9,
                output_tokens: 4
            })
        );
    }

    #[test]
    fn blocked_prompts_and_safety_finishes_are_safety_outcomes() {
        let outcome = parse_response(response(
            r#"{"promptFeedback":{"blockReason":"PROHIBITED_CONTENT"}}"#,
        ))
        .expect("blocked prompt");
        assert!(outcome.candidates[0].parts.is_empty());
        assert_eq!(
            outcome.warning_codes,
            vec![InferenceWarningCode::SafetyTransformed]
        );
        let outcome = parse_response(response(
            r#"{"candidates":[{"finishReason":"SAFETY","index":0}]}"#,
        ))
        .expect("safety finish");
        assert_eq!(
            outcome.warning_codes,
            vec![InferenceWarningCode::SafetyTransformed]
        );
        assert_eq!(
            parse_response(response(r#"{"candidates":[]}"#)),
            Err(AdapterError::EmptyResponse)
        );
    }

    #[test]
    fn model_ids_are_bounded_path_segments() {
        assert!(validate_model_id("gemini-2.5-pro").is_ok());
        assert_eq!(validate_model_id("a/b"), Err(AdapterError::Rejected));
        assert_eq!(validate_model_id("a:b"), Err(AdapterError::Rejected));
        assert_eq!(validate_model_id(""), Err(AdapterError::Rejected));
    }
}
