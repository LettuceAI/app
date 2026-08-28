use std::{borrow::Cow, collections::HashSet};

use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceRequest, InferenceUsage,
    InferenceWarningCode, MessagePart, MessageRole, ProviderContextPart, ProviderNeutralContext,
};
use lettuce_inference::InferenceRuntimePort;
use lettuce_models::{
    PromptCacheRetention, PromptCaching, ProviderAccount, ProviderConfig, ReasoningEffort,
    ReasoningMode, ResolvedChatParameters, ResolvedChatProfile,
};
use lettuce_network::{JsonClient, JsonResponse, JsonStaticHeader, MAX_REQUEST_BYTES};
use lettuce_settings::SecretStore;
use serde::{Deserialize, Serialize};

pub(crate) use crate::common::{ACCEPT_ONLY, AdapterError, AuthPlan, NO_HEADERS, STANDARD_HEADERS};
use crate::common::{
    Credentials, RemoteModel, decode_json, generation_policy, load_auth, load_secret_headers,
    max_output_tokens, parse_openai_model_list, reject_unsupported_features, skip_image_data,
    validate_common_request, validate_prompt_caching, validate_supported_reasoning,
};
use crate::descriptor::ProviderDescriptor;

const VERSIONED_CHAT_PATH: &str = "/v1/chat/completions";
const CHAT_PATH: &str = "/chat/completions";

/// One OpenAI-envelope provider policy. Every method has the standard
/// behavior as its default; a provider overrides what differs for it and may
/// delegate to another provider's implementation the way the legacy adapters
/// did (`OpenAi.chat_path(...)` and so on).
pub(crate) trait OpenAiWireProvider: Sync {
    fn descriptor(&self) -> &'static ProviderDescriptor;

    fn accepts(&self, config: &ProviderConfig) -> bool {
        matches!(config, ProviderConfig::Standard)
    }

    fn default_endpoint(&self) -> Option<&'static str> {
        self.descriptor().default_endpoint
    }

    fn normalize_endpoint<'a>(&self, endpoint: &'a str) -> Cow<'a, str> {
        Cow::Borrowed(endpoint)
    }

    fn chat_path(
        &self,
        endpoint: &str,
        _config: &ProviderConfig,
    ) -> Result<Cow<'static, str>, AdapterError> {
        Ok(Cow::Borrowed(versioned_chat_path(endpoint)))
    }

    fn models_path(&self, endpoint: &str, _config: &ProviderConfig) -> Option<Cow<'static, str>> {
        Some(Cow::Borrowed(
            if endpoint.trim_end_matches('/').ends_with("/v1") {
                "/models"
            } else {
                "/v1/models"
            },
        ))
    }

    fn parse_models(
        &self,
        payload: &serde_json::Value,
        _config: &ProviderConfig,
    ) -> Vec<RemoteModel> {
        parse_openai_model_list(payload)
    }

    fn role(&self, role: MessageRole, _config: &ProviderConfig) -> Option<Cow<'static, str>> {
        standard_role(role).map(Cow::Borrowed)
    }

    fn merges_same_role(&self, _config: &ProviderConfig) -> bool {
        false
    }

    fn auth(&self, _config: &ProviderConfig) -> Result<AuthPlan, AdapterError> {
        Ok(AuthPlan::Bearer)
    }

    fn static_headers(&self) -> &'static [JsonStaticHeader] {
        &STANDARD_HEADERS
    }

    fn validate_parameters(&self, parameters: &ResolvedChatParameters) -> Result<(), AdapterError> {
        if self.reasoning_policy() == ReasoningWirePolicy::Unsupported {
            reject_unsupported_features(parameters)?;
        } else {
            validate_supported_reasoning(parameters)?;
        }
        validate_prompt_caching(self.descriptor().prompt_caching, parameters)
    }

    fn reasoning_policy(&self) -> ReasoningWirePolicy {
        ReasoningWirePolicy::Unsupported
    }

    fn wire_parameters(&self, parameters: &ResolvedChatParameters) -> WireParameters {
        standard_parameters(parameters)
    }

    fn extend_body(
        &self,
        _parameters: &ResolvedChatParameters,
        _body: &mut serde_json::Map<String, serde_json::Value>,
    ) {
    }

    fn includes_stream_usage(&self) -> bool {
        false
    }

    fn supports_streaming(&self, _config: &ProviderConfig) -> bool {
        self.descriptor().streaming
    }
}

pub(crate) fn versioned_chat_path(endpoint: &str) -> &'static str {
    if endpoint.trim_end_matches('/').ends_with("/v1") {
        CHAT_PATH
    } else {
        VERSIONED_CHAT_PATH
    }
}

pub(crate) fn standard_role(role: MessageRole) -> Option<&'static str> {
    match role {
        MessageRole::System | MessageRole::Scene => Some("system"),
        MessageRole::User => Some("user"),
        MessageRole::Assistant => Some("assistant"),
    }
}

pub(crate) fn standard_parameters(parameters: &ResolvedChatParameters) -> WireParameters {
    WireParameters {
        temperature: parameters.temperature,
        top_p: parameters.top_p,
        max_tokens: Some(max_output_tokens(parameters)),
        context_length: parameters.context_length,
        frequency_penalty: parameters.frequency_penalty,
        presence_penalty: parameters.presence_penalty,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReasoningWirePolicy {
    Unsupported,
    MaxCompletionTokens,
    MaxTokens,
    OpenRouter,
    EnableThinking,
    Zai,
}

pub(crate) async fn run<S: SecretStore + ?Sized>(
    provider: &dyn OpenAiWireProvider,
    secret_store: &S,
    network: &JsonClient,
    runtime: &dyn InferenceRuntimePort,
    request: InferenceRequest,
) -> Result<InferenceOutcome, AdapterError> {
    validate_common_request(&request)?;
    let profile = &request.profile.chat_profile;
    let config = &profile.provider_config;
    if !provider.accepts(config) {
        return Err(AdapterError::Rejected);
    }
    provider.validate_parameters(&profile.parameters)?;
    let endpoint = profile
        .endpoint
        .as_deref()
        .or_else(|| provider.default_endpoint())
        .ok_or(AdapterError::Rejected)?;
    let endpoint = provider.normalize_endpoint(endpoint);
    let path = provider.chat_path(&endpoint, config)?;
    let mut messages = wire_messages(&request.context, provider, config)?;
    if provider.merges_same_role(config) {
        messages = merge_same_role(messages);
    }
    let streaming = request.stream_sink.is_some();
    if streaming && (!profile.streaming_enabled || !provider.supports_streaming(config)) {
        return Err(AdapterError::Rejected);
    }
    let body = encode_request(provider, profile, messages, streaming)?;
    let credentials = Credentials::from(profile);
    let auth = load_auth(provider.auth(config)?, secret_store, &credentials).await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    if streaming {
        let response = crate::streaming::await_cancelable(runtime, request.cancellation, async {
            network
                .post_json_stream(
                    &endpoint,
                    &path,
                    body,
                    provider.static_headers(),
                    auth,
                    secret_headers,
                    generation_policy(&credentials),
                )
                .await
                .map_err(Into::into)
        })
        .await?;
        crate::streaming::consume_stream(
            response,
            crate::stream_framing::StreamFormat::Sse,
            crate::stream_normalize::StreamProtocol::OpenAi,
            runtime,
            &request,
        )
        .await
    } else {
        let response = crate::streaming::await_cancelable(runtime, request.cancellation, async {
            network
                .post_json(
                    &endpoint,
                    &path,
                    body,
                    provider.static_headers(),
                    auth,
                    secret_headers,
                    generation_policy(&credentials),
                )
                .await
                .map_err(Into::into)
        })
        .await?;
        parse_response(response)
    }
}

pub(crate) async fn list_models<S: SecretStore + ?Sized>(
    provider: &dyn OpenAiWireProvider,
    secret_store: &S,
    network: &JsonClient,
    account: &ProviderAccount,
) -> Result<Vec<RemoteModel>, AdapterError> {
    if !provider.accepts(&account.config) {
        return Err(AdapterError::Rejected);
    }
    let endpoint = account
        .endpoint
        .as_deref()
        .or_else(|| provider.default_endpoint())
        .ok_or(AdapterError::Rejected)?;
    let endpoint = provider.normalize_endpoint(endpoint);
    let path = provider
        .models_path(&endpoint, &account.config)
        .ok_or(AdapterError::Rejected)?;
    let credentials = Credentials::from(account);
    let auth = load_auth(provider.auth(&account.config)?, secret_store, &credentials).await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    let response = network
        .get_json(
            &endpoint,
            &path,
            provider.static_headers(),
            auth,
            secret_headers,
            generation_policy(&credentials),
        )
        .await?;
    Ok(provider.parse_models(&decode_json(&response)?, &account.config))
}

fn wire_messages(
    context: &ProviderNeutralContext,
    provider: &dyn OpenAiWireProvider,
    config: &ProviderConfig,
) -> Result<Vec<WireMessage>, AdapterError> {
    context
        .messages
        .iter()
        .map(|message| {
            let role = provider
                .role(message.role, config)
                .ok_or(AdapterError::Rejected)?;
            let mut content = String::new();
            for part in &message.parts {
                match part {
                    ProviderContextPart::Text { text } => content.push_str(text),
                    ProviderContextPart::MediaAsset { .. }
                    | ProviderContextPart::ToolCall(_)
                    | ProviderContextPart::ToolResult(_) => return Err(AdapterError::Rejected),
                }
            }
            Ok(WireMessage {
                role,
                content,
                cache_control: None,
            })
        })
        .collect()
}

fn merge_same_role(messages: Vec<WireMessage>) -> Vec<WireMessage> {
    let mut merged: Vec<WireMessage> = Vec::with_capacity(messages.len());
    for message in messages {
        match merged.last_mut() {
            Some(last) if last.role == message.role => {
                if !last.content.is_empty() {
                    last.content.push_str("\n\n");
                }
                last.content.push_str(&message.content);
            }
            _ => merged.push(message),
        }
    }
    merged
}

fn encode_request(
    provider: &dyn OpenAiWireProvider,
    profile: &ResolvedChatProfile,
    mut messages: Vec<WireMessage>,
    streaming: bool,
) -> Result<Vec<u8>, AdapterError> {
    if provider.descriptor().prompt_caching == crate::descriptor::PromptCachingSupport::CacheControl
    {
        apply_cache_control(&profile.parameters, &mut messages);
    }
    let parameters = provider.wire_parameters(&profile.parameters);
    let request = OpenAiRequest {
        model: profile.external_model_id.clone(),
        messages,
        stream: streaming,
        stream_options: (streaming && provider.includes_stream_usage()).then_some(StreamOptions {
            include_usage: true,
        }),
        temperature: parameters.temperature,
        top_p: parameters.top_p,
        max_tokens: parameters.max_tokens,
        context_length: parameters.context_length,
        frequency_penalty: parameters.frequency_penalty,
        presence_penalty: parameters.presence_penalty,
    };
    let mut value = serde_json::to_value(&request).map_err(|_| AdapterError::Rejected)?;
    let object = value.as_object_mut().ok_or(AdapterError::Rejected)?;
    apply_reasoning(provider.reasoning_policy(), &profile.parameters, object)?;
    provider.extend_body(&profile.parameters, object);
    let body = serde_json::to_vec(&value).map_err(|_| AdapterError::Rejected)?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err(AdapterError::Rejected);
    }
    Ok(body)
}

fn apply_reasoning(
    policy: ReasoningWirePolicy,
    parameters: &ResolvedChatParameters,
    body: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    let enabled = parameters.reasoning_mode == Some(ReasoningMode::Enabled);
    if !enabled {
        if policy == ReasoningWirePolicy::Zai {
            body.insert(
                "thinking".to_owned(),
                serde_json::json!({ "type": "disabled" }),
            );
        }
        return Ok(());
    }

    let total = parameters.total_completion_allowance.map_or_else(
        || {
            max_output_tokens(parameters)
                .checked_add(parameters.reasoning_budget_tokens.unwrap_or(0))
                .ok_or(AdapterError::Rejected)
        },
        Ok,
    )?;
    let effort = parameters.reasoning_effort.map(reasoning_effort);
    match policy {
        ReasoningWirePolicy::Unsupported => {}
        ReasoningWirePolicy::MaxCompletionTokens => {
            body.remove("max_tokens");
            body.insert("max_completion_tokens".to_owned(), total.into());
            if let Some(effort) = effort {
                body.insert("reasoning_effort".to_owned(), effort.into());
            }
        }
        ReasoningWirePolicy::MaxTokens => {
            body.insert("max_tokens".to_owned(), total.into());
            if let Some(effort) = effort {
                body.insert("reasoning_effort".to_owned(), effort.into());
            }
        }
        ReasoningWirePolicy::OpenRouter => {
            body.remove("max_tokens");
            body.insert("max_completion_tokens".to_owned(), total.into());
            let reasoning = if let Some(effort) = effort {
                serde_json::json!({ "effort": effort })
            } else if let Some(budget) = parameters.reasoning_budget_tokens {
                serde_json::json!({ "max_tokens": budget })
            } else {
                serde_json::json!({ "enabled": true })
            };
            body.insert("reasoning".to_owned(), reasoning);
        }
        ReasoningWirePolicy::EnableThinking => {
            body.insert("max_tokens".to_owned(), total.into());
            if let Some(effort) = effort {
                body.insert("reasoning_effort".to_owned(), effort.into());
            }
            body.insert("enable_thinking".to_owned(), true.into());
            if let Some(budget) = parameters.reasoning_budget_tokens {
                body.insert("thinking_budget".to_owned(), budget.into());
            }
        }
        ReasoningWirePolicy::Zai => {
            body.insert("max_tokens".to_owned(), total.into());
            body.insert(
                "thinking".to_owned(),
                serde_json::json!({ "type": "enabled" }),
            );
            if let Some(effort) = effort {
                body.insert("reasoning_effort".to_owned(), effort.into());
            }
        }
    }
    Ok(())
}

const fn reasoning_effort(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
    }
}

fn apply_cache_control(parameters: &ResolvedChatParameters, messages: &mut [WireMessage]) {
    let Some(PromptCaching::Enabled { retention }) = parameters.prompt_caching else {
        return;
    };
    let control = WireCacheControl {
        kind: "ephemeral",
        ttl: (retention == PromptCacheRetention::OneHour).then_some("1h"),
    };
    if let Some(system) = messages
        .iter_mut()
        .find(|message| matches!(message.role.as_ref(), "system" | "developer"))
    {
        system.cache_control = Some(control);
    }
    if let Some(user) = messages
        .iter_mut()
        .rev()
        .find(|message| message.role == "user")
    {
        user.cache_control = Some(control);
    }
}

fn parse_response(response: JsonResponse) -> Result<InferenceOutcome, AdapterError> {
    if let Some(error) = AdapterError::from_response(&response) {
        return Err(error);
    }
    let header_request_id = response.request_id.clone();
    let parsed: OpenAiResponse =
        serde_json::from_slice(&response.body).map_err(|_| AdapterError::MalformedResponse)?;
    if parsed.choices.is_empty() {
        return Err(AdapterError::EmptyResponse);
    }

    let mut candidates = Vec::with_capacity(parsed.choices.len());
    let mut warnings = Vec::new();
    let mut finish_reason = FinishReason::Stop;
    let mut provider_finish_reason = None;
    let mut has_content = false;
    let mut ordinals = HashSet::with_capacity(parsed.choices.len());
    for (position, choice) in parsed.choices.into_iter().enumerate() {
        if position == 0 {
            provider_finish_reason.clone_from(&choice.finish_reason);
        }
        let index = choice.index.unwrap_or(position as u64);
        let ordinal = u16::try_from(index).map_err(|_| AdapterError::MalformedResponse)?;
        if !ordinals.insert(ordinal) {
            return Err(AdapterError::MalformedResponse);
        }
        let message = choice.message.ok_or(AdapterError::MalformedResponse)?;
        let raw_text = message
            .content
            .map(MessageContent::into_text)
            .unwrap_or_default();
        let (text, tagged_reasoning) = crate::stream_normalize::split_complete_thinking(&raw_text);
        let explicit_reasoning = [message.reasoning, message.reasoning_content]
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect::<Vec<_>>();
        let reasoning = crate::stream_normalize::merge_complete_reasoning(
            tagged_reasoning,
            explicit_reasoning.iter().map(String::as_str),
        );
        has_content |= !text.trim().is_empty() || !reasoning.trim().is_empty();
        let mut parts = Vec::new();
        if !reasoning.is_empty() {
            parts.push(MessagePart::ReasoningSummary { text: reasoning });
        }
        if !text.is_empty() {
            parts.push(MessagePart::Text { text });
        }
        if position == 0 && choice.finish_reason.as_deref() == Some("length") {
            finish_reason = FinishReason::Length;
        }
        match choice.finish_reason.as_deref() {
            Some("length") => {
                push_warning(&mut warnings, InferenceWarningCode::Truncated);
            }
            Some("content_filter") => {
                push_warning(&mut warnings, InferenceWarningCode::SafetyTransformed);
            }
            Some("tool_calls") | Some("function_call") => {
                push_warning(&mut warnings, InferenceWarningCode::ProviderDegraded);
            }
            Some("stop") => {}
            _ => push_warning(&mut warnings, InferenceWarningCode::ProviderDegraded),
        }
        candidates.push(InferenceCandidate {
            ordinal,
            parts,
            tool_calls: Vec::new(),
            provider_replay: None,
        });
    }
    if !has_content
        && !warnings.contains(&InferenceWarningCode::SafetyTransformed)
        && !warnings.contains(&InferenceWarningCode::ProviderDegraded)
    {
        return Err(AdapterError::EmptyResponse);
    }
    Ok(InferenceOutcome {
        candidates,
        usage: parsed.usage.and_then(|usage| {
            Some(InferenceUsage {
                input_tokens: usage.input()?,
                output_tokens: usage.output()?,
            })
        }),
        finish_reason,
        provider_finish_reason,
        provider_request_id: header_request_id,
        warning_codes: warnings,
    })
}

fn push_warning(warnings: &mut Vec<InferenceWarningCode>, warning: InferenceWarningCode) {
    if !warnings.contains(&warning) {
        warnings.push(warning);
    }
}

pub(crate) struct WireMessage {
    role: Cow<'static, str>,
    content: String,
    cache_control: Option<WireCacheControl>,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct WireCacheControl {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<&'static str>,
}

#[derive(Serialize)]
struct CachedTextContent<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    text: &'a str,
    cache_control: WireCacheControl,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WireParameters {
    pub(crate) temperature: Option<f64>,
    pub(crate) top_p: Option<f64>,
    pub(crate) max_tokens: Option<u32>,
    pub(crate) context_length: Option<u32>,
    pub(crate) frequency_penalty: Option<f64>,
    pub(crate) presence_penalty: Option<f64>,
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<WireMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f64>,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

impl Serialize for WireMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("OpenAiMessage", 2)?;
        state.serialize_field("role", self.role.as_ref())?;
        if let Some(cache_control) = self.cache_control {
            state.serialize_field(
                "content",
                &[CachedTextContent {
                    kind: "text",
                    text: &self.content,
                    cache_control,
                }],
            )?;
        } else {
            state.serialize_field("content", &self.content)?;
        }
        state.end()
    }
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    index: Option<u64>,
    message: Option<OpenAiResponseMessage>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiResponseMessage {
    content: Option<MessageContent>,
    reasoning: Option<serde_json::Value>,
    reasoning_content: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Deserialize)]
struct ContentPart {
    text: Option<String>,
    content: Option<String>,
}

impl MessageContent {
    fn into_text(self) -> String {
        let fragments = match self {
            Self::Text(text) => vec![text],
            Self::Parts(parts) => parts
                .into_iter()
                .filter_map(|part| part.text.or(part.content))
                .collect(),
        };
        fragments
            .into_iter()
            .filter(|fragment| !skip_image_data(fragment))
            .collect()
    }
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<u64>,
    input_tokens: Option<u64>,
    prompt_eval_count: Option<u64>,
    completion_tokens: Option<u64>,
    output_tokens: Option<u64>,
    eval_count: Option<u64>,
}

impl OpenAiUsage {
    fn input(&self) -> Option<u64> {
        self.prompt_eval_count
            .or(self.prompt_tokens)
            .or(self.input_tokens)
    }

    fn output(&self) -> Option<u64> {
        self.eval_count
            .or(self.completion_tokens)
            .or(self.output_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_conversations::PortError;
    use lettuce_conversations::{ProviderFailure, ProviderFailureKind};
    use lettuce_models::{ReasoningEffort, ReasoningMode};

    fn reasoning_parameters() -> ResolvedChatParameters {
        ResolvedChatParameters {
            visible_max_output_tokens: Some(100),
            reasoning_mode: Some(ReasoningMode::Enabled),
            reasoning_effort: Some(ReasoningEffort::Medium),
            reasoning_budget_tokens: Some(20),
            total_completion_allowance: Some(120),
            ..crate::integration_tests::parameters()
        }
    }

    fn base_body() -> serde_json::Map<String, serde_json::Value> {
        serde_json::json!({ "max_tokens": 100 })
            .as_object()
            .expect("object")
            .clone()
    }

    #[test]
    fn openai_reasoning_switches_to_total_completion_allowance() {
        let mut body = base_body();
        apply_reasoning(
            ReasoningWirePolicy::MaxCompletionTokens,
            &reasoning_parameters(),
            &mut body,
        )
        .expect("reasoning wire");
        assert_eq!(
            serde_json::Value::Object(body),
            serde_json::json!({
                "max_completion_tokens": 120,
                "reasoning_effort": "medium",
            })
        );
    }

    #[test]
    fn openrouter_uses_one_nested_reasoning_control() {
        let mut body = base_body();
        apply_reasoning(
            ReasoningWirePolicy::OpenRouter,
            &reasoning_parameters(),
            &mut body,
        )
        .expect("reasoning wire");
        assert_eq!(
            serde_json::Value::Object(body),
            serde_json::json!({
                "max_completion_tokens": 120,
                "reasoning": { "effort": "medium" },
            })
        );

        let mut parameters = reasoning_parameters();
        parameters.reasoning_effort = None;
        let mut body = base_body();
        apply_reasoning(ReasoningWirePolicy::OpenRouter, &parameters, &mut body)
            .expect("reasoning wire");
        assert_eq!(body["reasoning"], serde_json::json!({ "max_tokens": 20 }));
    }

    #[test]
    fn qwen_and_moonshot_emit_explicit_thinking_fields() {
        let mut body = base_body();
        apply_reasoning(
            ReasoningWirePolicy::EnableThinking,
            &reasoning_parameters(),
            &mut body,
        )
        .expect("reasoning wire");
        assert_eq!(body["max_tokens"], 120);
        assert_eq!(body["enable_thinking"], true);
        assert_eq!(body["thinking_budget"], 20);
        assert_eq!(body["reasoning_effort"], "medium");
    }

    #[test]
    fn zai_always_emits_an_explicit_thinking_state() {
        let mut enabled = base_body();
        apply_reasoning(
            ReasoningWirePolicy::Zai,
            &reasoning_parameters(),
            &mut enabled,
        )
        .expect("reasoning wire");
        assert_eq!(
            enabled["thinking"],
            serde_json::json!({ "type": "enabled" })
        );
        assert_eq!(enabled["max_tokens"], 120);

        let mut disabled_parameters = reasoning_parameters();
        disabled_parameters.reasoning_mode = Some(ReasoningMode::Disabled);
        disabled_parameters.reasoning_effort = None;
        disabled_parameters.reasoning_budget_tokens = None;
        disabled_parameters.total_completion_allowance = Some(100);
        let mut disabled = base_body();
        apply_reasoning(
            ReasoningWirePolicy::Zai,
            &disabled_parameters,
            &mut disabled,
        )
        .expect("reasoning wire");
        assert_eq!(
            disabled["thinking"],
            serde_json::json!({ "type": "disabled" })
        );
        assert!(disabled.get("reasoning_effort").is_none());
    }

    fn response(body: &str) -> JsonResponse {
        JsonResponse {
            status: 200,
            body: body.as_bytes().to_vec(),
            request_id: None,
            retry_after: None,
        }
    }

    #[test]
    fn maps_ordered_text_choices_and_optional_usage_without_response_id_coupling() {
        let outcome = parse_response(response(
            r#"{
                "id":"response-canary",
                "choices":[
                    {"index":7,"message":{"role":"assistant","content":"first"},"finish_reason":"stop"},
                    {"index":2,"message":{"role":"assistant","content":"second"},"finish_reason":"length"}
                ],
                "usage":{"prompt_tokens":7,"completion_tokens":3}
            }"#,
        ))
        .expect("valid response");
        assert_eq!(outcome.finish_reason, FinishReason::Stop);
        assert_eq!(outcome.candidates.len(), 2);
        assert_eq!(outcome.candidates[0].ordinal, 7);
        assert_eq!(outcome.candidates[1].ordinal, 2);
        assert_eq!(
            outcome.candidates[0].parts,
            vec![MessagePart::Text {
                text: "first".to_owned()
            }]
        );
        assert_eq!(
            outcome.usage,
            Some(InferenceUsage {
                input_tokens: 7,
                output_tokens: 3
            })
        );
        assert_eq!(outcome.warning_codes, vec![InferenceWarningCode::Truncated]);
        assert_eq!(outcome.provider_finish_reason.as_deref(), Some("stop"));
        assert!(outcome.provider_request_id.is_none());
    }

    #[test]
    fn preserves_header_request_id_separately_from_response_body_id() {
        let outcome = parse_response(JsonResponse {
            status: 200,
            body: br#"{"id":"response-id","choices":[{"message":{"content":"ok"},"finish_reason":"stop"}]}"#.to_vec(),
            request_id: Some("request-id".to_owned()),
            retry_after: None,
        })
        .expect("valid response");
        assert_eq!(outcome.provider_request_id.as_deref(), Some("request-id"));
        assert_eq!(outcome.provider_finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn tolerates_missing_choice_index_partial_usage_and_blank_content() {
        let outcome = parse_response(response(
            r#"{"choices":[{"message":{"content":"a"},"finish_reason":"stop"},{"message":{"content":"b"}}],"usage":{"total_tokens":5}}"#,
        ))
        .expect("index-less choices");
        assert_eq!(outcome.candidates[0].ordinal, 0);
        assert_eq!(outcome.candidates[1].ordinal, 1);
        assert!(outcome.usage.is_none());
        let outcome = parse_response(response(
            r#"{"choices":[{"index":0,"message":{"content":"a"}}],"usage":{"input_tokens":2,"output_tokens":3}}"#,
        ))
        .expect("aliased usage");
        assert_eq!(
            outcome.usage,
            Some(InferenceUsage {
                input_tokens: 2,
                output_tokens: 3
            })
        );
        assert_eq!(
            parse_response(response(
                r#"{"choices":[{"index":0,"message":{"content":"   "},"finish_reason":"stop"}]}"#,
            )),
            Err(AdapterError::EmptyResponse)
        );
    }

    #[test]
    fn joins_text_parts_when_content_is_an_array() {
        let outcome = parse_response(response(
            r#"{"choices":[{"index":0,"message":{"content":[{"type":"text","text":"first "},{"type":"image_url","image_url":{"url":"data:image/png;base64,AAAA"}},{"type":"text","text":"data:image/png;base64,BBBB"},{"type":"text","text":"second"}]},"finish_reason":"stop"}]}"#,
        ))
        .expect("array content");
        assert_eq!(
            outcome.candidates[0].parts,
            vec![MessagePart::Text {
                text: "first second".to_owned()
            }]
        );
    }

    #[test]
    fn merges_consecutive_same_role_messages_with_blank_lines() {
        let merged = merge_same_role(vec![
            WireMessage {
                role: Cow::Borrowed("system"),
                content: "a".to_owned(),
                cache_control: None,
            },
            WireMessage {
                role: Cow::Borrowed("system"),
                content: String::new(),
                cache_control: None,
            },
            WireMessage {
                role: Cow::Borrowed("system"),
                content: "b".to_owned(),
                cache_control: None,
            },
            WireMessage {
                role: Cow::Borrowed("user"),
                content: "c".to_owned(),
                cache_control: None,
            },
            WireMessage {
                role: Cow::Borrowed("assistant"),
                content: String::new(),
                cache_control: None,
            },
            WireMessage {
                role: Cow::Borrowed("assistant"),
                content: "d".to_owned(),
                cache_control: None,
            },
        ]);
        let rendered: Vec<(&str, &str)> = merged
            .iter()
            .map(|message| (message.role.as_ref(), message.content.as_str()))
            .collect();
        assert_eq!(
            rendered,
            vec![("system", "a\n\n\n\nb"), ("user", "c"), ("assistant", "d")]
        );
    }

    #[test]
    fn classifies_provider_degraded_finish_reasons_without_turning_a_response_into_error() {
        let outcome = parse_response(response(
            r#"{"choices":[{"index":0,"message":{"content":null},"finish_reason":"tool_calls"}],"id":"id"}"#,
        ))
        .expect("tool-call response remains an outcome");
        assert_eq!(outcome.finish_reason, FinishReason::Stop);
        assert!(outcome.candidates[0].parts.is_empty());
        assert_eq!(
            outcome.warning_codes,
            vec![InferenceWarningCode::ProviderDegraded]
        );
        assert!(outcome.usage.is_none());
    }

    #[test]
    fn buffered_openai_preserves_native_and_tagged_reasoning() {
        let outcome = parse_response(response(
            r#"{"choices":[{"index":0,"message":{"content":"<think>tagged</think>visible","reasoning_content":"native"},"finish_reason":"stop"}]}"#,
        ))
        .expect("reasoning response");
        assert_eq!(
            outcome.candidates[0].parts,
            vec![
                MessagePart::ReasoningSummary {
                    text: "tagged\n\nnative".to_owned()
                },
                MessagePart::Text {
                    text: "visible".to_owned()
                }
            ]
        );
    }

    #[test]
    fn rejects_malformed_success_and_maps_http_categories_without_exposing_body() {
        assert_eq!(
            parse_response(response("not-json")),
            Err(AdapterError::MalformedResponse)
        );
        let error = parse_response(JsonResponse {
            status: 401,
            body: b"secret-prompt-canary".to_vec(),
            request_id: None,
            retry_after: None,
        })
        .expect_err("provider rejection");
        assert!(matches!(
            error,
            AdapterError::Provider(ProviderFailure {
                kind: ProviderFailureKind::CredentialRejected,
                status: 401,
                ..
            })
        ));
        assert!(!format!("{error:?}").contains("secret-prompt-canary"));
        assert!(matches!(PortError::from(error), PortError::Provider(_)));

        let error = parse_response(JsonResponse {
            status: 429,
            body: br#"{"error":{"code":"rate_limit","message":"try later"}}"#.to_vec(),
            request_id: Some("request-id".to_owned()),
            retry_after: None,
        })
        .expect_err("provider rejection");
        let AdapterError::Provider(failure) = &error else {
            panic!("expected provider failure");
        };
        assert_eq!(failure.kind, ProviderFailureKind::Unavailable);
        assert_eq!(failure.status, 429);
        assert_eq!(failure.code.as_deref(), Some("rate_limit"));
        assert_eq!(failure.message.as_deref(), Some("try later"));
        assert_eq!(failure.request_id.as_deref(), Some("request-id"));
        assert!(!format!("{error:?}").contains("try later"));
    }

    #[test]
    fn validates_choice_indices_and_status_categories() {
        for body in [
            r#"{"choices":[{"index":1,"message":{"content":"a"}},{"index":1,"message":{"content":"b"}}]}"#,
            r#"{"choices":[{"index":65536,"message":{"content":"a"}}]}"#,
        ] {
            assert_eq!(
                parse_response(response(body)),
                Err(AdapterError::MalformedResponse)
            );
        }
        for (status, kind) in [
            (401, ProviderFailureKind::CredentialRejected),
            (403, ProviderFailureKind::CredentialRejected),
            (408, ProviderFailureKind::Unavailable),
            (429, ProviderFailureKind::Unavailable),
            (422, ProviderFailureKind::RequestRejected),
        ] {
            let actual = parse_response(JsonResponse {
                status,
                body: b"provider-error-canary".to_vec(),
                request_id: None,
                retry_after: None,
            })
            .expect_err("status must be classified");
            assert!(matches!(
                &actual,
                AdapterError::Provider(ProviderFailure {
                    kind: actual_kind,
                    status: actual_status,
                    ..
                }) if *actual_kind == kind && *actual_status == status
            ));
            assert!(matches!(PortError::from(actual), PortError::Provider(_)));
        }
    }
}
