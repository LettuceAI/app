use std::{borrow::Cow, collections::HashSet};

use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceRequest, InferenceUsage,
    InferenceWarningCode, MessagePart, MessageRole, ProposedToolCall, ProviderContextPart,
    ProviderNeutralContext, ToolChoice, ToolRequest,
};
use lettuce_inference::InferenceRuntimePort;
use lettuce_models::{
    CapabilityStatus, PromptCacheRetention, PromptCaching, ProviderAccount, ProviderConfig,
    ReasoningEffort, ReasoningMode, ResolvedChatParameters, ResolvedChatProfile,
};
use lettuce_network::{JsonClient, JsonResponse, JsonStaticHeader, MAX_REQUEST_BYTES};
use lettuce_settings::SecretStore;
use serde::{Deserialize, Serialize};

pub(crate) use crate::common::{ACCEPT_ONLY, AdapterError, AuthPlan, NO_HEADERS, STANDARD_HEADERS};
use crate::common::{
    Credentials, RemoteModel, decode_json, generation_policy, load_auth, load_secret_headers,
    max_output_tokens, parse_openai_model_list, reject_unsupported_features, skip_image_data,
    validate_common_request_with_tools, validate_prompt_caching, validate_supported_reasoning,
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

    fn extend_config_body(
        &self,
        _config: &ProviderConfig,
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

    fn tool_choice(
        &self,
        choice: &ToolChoice,
        _config: &ProviderConfig,
    ) -> Result<Option<serde_json::Value>, AdapterError> {
        Ok(Some(standard_tool_choice(choice)))
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
    validate_common_request_with_tools(&request)?;
    let profile = &request.profile.chat_profile;
    let config = &profile.provider_config;
    if !provider.accepts(config) {
        return Err(AdapterError::Rejected);
    }
    provider.validate_parameters(&profile.parameters)?;
    if request.tools.is_some() && profile.capabilities.tools == CapabilityStatus::Unsupported {
        return Err(AdapterError::Rejected);
    }
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
    let body = encode_request(
        provider,
        profile,
        config,
        messages,
        request.tools.as_ref(),
        streaming,
    )?;
    let credentials = Credentials::from(profile);
    let auth = load_auth(provider.auth(config)?, secret_store, &credentials).await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    let outcome = if streaming {
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
        .await?
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
        parse_response(response)?
    };
    validate_tool_outcome(&request, outcome)
}

fn validate_tool_outcome(
    request: &InferenceRequest,
    outcome: InferenceOutcome,
) -> Result<InferenceOutcome, AdapterError> {
    outcome
        .validate()
        .map_err(|_| AdapterError::MalformedResponse)?;
    let calls = outcome
        .candidates
        .iter()
        .flat_map(|candidate| candidate.tool_calls.iter())
        .collect::<Vec<_>>();
    let Some(tools) = &request.tools else {
        return if calls.is_empty() {
            Ok(outcome)
        } else {
            Err(AdapterError::MalformedResponse)
        };
    };
    if request.profile.tool_policy == lettuce_conversations::ToolPolicy::Required
        && calls.is_empty()
    {
        return Err(AdapterError::MalformedResponse);
    }
    for call in calls {
        if !tools
            .definitions
            .iter()
            .any(|definition| definition.name == call.name)
            || matches!(&tools.choice, ToolChoice::Named { name } if *name != call.name)
        {
            return Err(AdapterError::MalformedResponse);
        }
    }
    Ok(outcome)
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
    let mut messages = Vec::new();
    for message in &context.messages {
        let results = message
            .parts
            .iter()
            .filter_map(|part| match part {
                ProviderContextPart::ToolResult(result) => Some(result),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !results.is_empty() {
            if results.len() != message.parts.len() || message.role != MessageRole::User {
                return Err(AdapterError::Rejected);
            }
            for result in results {
                let provider_call_id = result
                    .provider_call_id
                    .as_deref()
                    .ok_or(AdapterError::Rejected)?;
                messages.push(WireMessage {
                    role: Cow::Borrowed("tool"),
                    content: Some(
                        serde_json::to_string(&result.output.value)
                            .map_err(|_| AdapterError::Rejected)?,
                    ),
                    tool_calls: Vec::new(),
                    tool_call_id: Some(provider_call_id.to_owned()),
                    cache_control: None,
                });
            }
            continue;
        }
        let role = provider
            .role(message.role, config)
            .ok_or(AdapterError::Rejected)?;
        let mut content = String::new();
        let mut tool_calls = Vec::new();
        for part in &message.parts {
            match part {
                ProviderContextPart::Text { text } => content.push_str(text),
                ProviderContextPart::ToolCall(call) => {
                    if message.role != MessageRole::Assistant {
                        return Err(AdapterError::Rejected);
                    }
                    tool_calls.push(WireToolCall {
                        id: call
                            .provider_call_id
                            .clone()
                            .ok_or(AdapterError::Rejected)?,
                        kind: "function",
                        function: WireFunctionCall {
                            name: call.name.clone(),
                            arguments: call.raw_arguments.clone().unwrap_or(
                                serde_json::to_string(&call.arguments)
                                    .map_err(|_| AdapterError::Rejected)?,
                            ),
                        },
                    });
                }
                ProviderContextPart::MediaAsset { .. } | ProviderContextPart::ToolResult(_) => {
                    return Err(AdapterError::Rejected);
                }
            }
        }
        messages.push(WireMessage {
            role,
            content: if tool_calls.is_empty() || !content.is_empty() {
                Some(content)
            } else {
                None
            },
            tool_calls,
            tool_call_id: None,
            cache_control: None,
        });
    }
    Ok(messages)
}

fn merge_same_role(messages: Vec<WireMessage>) -> Vec<WireMessage> {
    let mut merged: Vec<WireMessage> = Vec::with_capacity(messages.len());
    for message in messages {
        match merged.last_mut() {
            Some(last)
                if last.role == message.role
                    && last.tool_calls.is_empty()
                    && message.tool_calls.is_empty()
                    && last.tool_call_id.is_none()
                    && message.tool_call_id.is_none() =>
            {
                let last_content = last.content.get_or_insert_default();
                if !last_content.is_empty() {
                    last_content.push_str("\n\n");
                }
                last_content.push_str(message.content.as_deref().unwrap_or_default());
            }
            _ => merged.push(message),
        }
    }
    merged
}

fn standard_tool_choice(choice: &ToolChoice) -> serde_json::Value {
    match choice {
        ToolChoice::Auto => serde_json::Value::String("auto".to_owned()),
        ToolChoice::Required => serde_json::Value::String("required".to_owned()),
        ToolChoice::Named { name } => serde_json::json!({
            "type": "function",
            "function": { "name": name }
        }),
    }
}

fn encode_request(
    provider: &dyn OpenAiWireProvider,
    profile: &ResolvedChatProfile,
    config: &ProviderConfig,
    mut messages: Vec<WireMessage>,
    tools: Option<&ToolRequest>,
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
        tools: tools.map(|request| {
            request
                .definitions
                .iter()
                .map(|definition| WireToolDefinition {
                    kind: "function",
                    function: WireFunctionDefinition {
                        name: definition.name.clone(),
                        description: definition.description.clone(),
                        parameters: definition.parameters.clone(),
                    },
                })
                .collect()
        }),
        tool_choice: tools
            .map(|request| provider.tool_choice(&request.choice, config))
            .transpose()?
            .flatten(),
    };
    let mut value = serde_json::to_value(&request).map_err(|_| AdapterError::Rejected)?;
    let object = value.as_object_mut().ok_or(AdapterError::Rejected)?;
    if tools.is_some()
        && provider.descriptor().prompt_caching
            == crate::descriptor::PromptCachingSupport::CacheControl
    {
        apply_tool_cache_control(&profile.parameters, object);
    }
    apply_reasoning(provider.reasoning_policy(), &profile.parameters, object)?;
    provider.extend_body(&profile.parameters, object);
    provider.extend_config_body(config, &profile.parameters, object);
    let body = serde_json::to_vec(&value).map_err(|_| AdapterError::Rejected)?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err(AdapterError::Rejected);
    }
    Ok(body)
}

fn apply_tool_cache_control(
    parameters: &ResolvedChatParameters,
    body: &mut serde_json::Map<String, serde_json::Value>,
) {
    let Some(PromptCaching::Enabled { retention }) = parameters.prompt_caching else {
        return;
    };
    let Some(last_tool) = body
        .get_mut("tools")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|tools| tools.last_mut())
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    let mut control = serde_json::Map::from_iter([(
        "type".to_owned(),
        serde_json::Value::String("ephemeral".to_owned()),
    )]);
    if retention == PromptCacheRetention::OneHour {
        control.insert("ttl".to_owned(), "1h".into());
    }
    last_tool.insert("cache_control".to_owned(), control.into());
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
        .find(|message| message.role == "user" && message.tool_call_id.is_none())
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
        let tool_calls = message
            .tool_calls
            .into_iter()
            .map(parse_tool_call)
            .collect::<Result<Vec<_>, _>>()?;
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
        has_content |=
            !text.trim().is_empty() || !reasoning.trim().is_empty() || !tool_calls.is_empty();
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
            Some("tool_calls") if !tool_calls.is_empty() => {}
            Some("tool_calls") | Some("function_call") => {
                return Err(AdapterError::MalformedResponse);
            }
            Some("stop") => {}
            _ => push_warning(&mut warnings, InferenceWarningCode::ProviderDegraded),
        }
        candidates.push(InferenceCandidate {
            ordinal,
            parts,
            tool_calls,
            provider_replay: None,
        });
    }
    if !has_content
        && !warnings.contains(&InferenceWarningCode::SafetyTransformed)
        && !warnings.contains(&InferenceWarningCode::ProviderDegraded)
    {
        return Err(AdapterError::EmptyResponse);
    }
    let outcome = InferenceOutcome {
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
    };
    outcome
        .validate()
        .map_err(|_| AdapterError::MalformedResponse)?;
    Ok(outcome)
}

fn parse_tool_call(call: OpenAiResponseToolCall) -> Result<ProposedToolCall, AdapterError> {
    if call.kind.as_deref().is_some_and(|kind| kind != "function") {
        return Err(AdapterError::MalformedResponse);
    }
    let id = call.id.ok_or(AdapterError::MalformedResponse)?;
    let function = call.function.ok_or(AdapterError::MalformedResponse)?;
    let (arguments, raw_arguments) = match function.arguments {
        serde_json::Value::String(raw) => {
            let arguments =
                serde_json::from_str(&raw).map_err(|_| AdapterError::MalformedResponse)?;
            (arguments, Some(raw))
        }
        value @ serde_json::Value::Object(_) => (value, None),
        _ => return Err(AdapterError::MalformedResponse),
    };
    let call = ProposedToolCall {
        provider_call_id: Some(id),
        name: function.name,
        arguments,
        raw_arguments,
        provider_replay: None,
    };
    call.validate()
        .map_err(|_| AdapterError::MalformedResponse)?;
    Ok(call)
}

fn push_warning(warnings: &mut Vec<InferenceWarningCode>, warning: InferenceWarningCode) {
    if !warnings.contains(&warning) {
        warnings.push(warning);
    }
}

pub(crate) struct WireMessage {
    role: Cow<'static, str>,
    content: Option<String>,
    tool_calls: Vec<WireToolCall>,
    tool_call_id: Option<String>,
    cache_control: Option<WireCacheControl>,
}

impl WireMessage {
    #[cfg(test)]
    fn text(role: &'static str, content: impl Into<String>) -> Self {
        Self {
            role: Cow::Borrowed(role),
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
            cache_control: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct WireToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireFunctionCall,
}

#[derive(Debug, Clone, Serialize)]
struct WireFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct WireToolDefinition {
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireFunctionDefinition,
}

#[derive(Serialize)]
struct WireFunctionDefinition {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    parameters: serde_json::Value,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<WireToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
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
        let mut state = serializer.serialize_struct("OpenAiMessage", 4)?;
        state.serialize_field("role", self.role.as_ref())?;
        if let Some(cache_control) = self.cache_control {
            let content = self.content.as_deref().unwrap_or_default();
            state.serialize_field(
                "content",
                &[CachedTextContent {
                    kind: "text",
                    text: content,
                    cache_control,
                }],
            )?;
        } else {
            state.serialize_field("content", &self.content)?;
        }
        if !self.tool_calls.is_empty() {
            state.serialize_field("tool_calls", &self.tool_calls)?;
        }
        if let Some(tool_call_id) = &self.tool_call_id {
            state.serialize_field("tool_call_id", tool_call_id)?;
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
    #[serde(default)]
    tool_calls: Vec<OpenAiResponseToolCall>,
}

#[derive(Deserialize)]
struct OpenAiResponseToolCall {
    id: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    function: Option<OpenAiResponseFunctionCall>,
}

#[derive(Deserialize)]
struct OpenAiResponseFunctionCall {
    name: String,
    arguments: serde_json::Value,
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
    use lettuce_conversations::{
        ContextAttributions, ContextBudgetReport, PortError, ProviderFailure, ProviderFailureKind,
        ProviderNeutralMessage, ToolDefinition, ToolOutput, TranscriptToolCall,
        TranscriptToolResult,
    };
    use lettuce_models::{
        ChatProfileWarning, ModelCapabilities, ProviderProtocol, ReasoningEffort, ReasoningMode,
    };

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

    fn test_profile() -> ResolvedChatProfile {
        ResolvedChatProfile {
            model_profile_id: lettuce_types::ModelProfileId::new(),
            model_revision: lettuce_types::Revision::INITIAL,
            provider_account_id: lettuce_types::ProviderAccountId::new(),
            provider_account_revision: lettuce_types::Revision::INITIAL,
            secret_owner_id: lettuce_settings::SecretOwnerId::new(),
            external_model_id: "test-model".to_owned(),
            provider_kind: "openai".to_owned(),
            provider_protocol: ProviderProtocol::OpenAiCompatible,
            endpoint: Some("https://api.openai.com".to_owned()),
            provider_config: ProviderConfig::Standard,
            streaming_enabled: true,
            allow_invalid_tls: false,
            capabilities: ModelCapabilities::default(),
            parameters: crate::integration_tests::parameters(),
            api_key_ref: None,
            secret_headers: Vec::new(),
            warnings: Vec::<ChatProfileWarning>::new(),
        }
    }

    #[test]
    fn encodes_tool_definitions_choice_and_replay_messages_exactly() {
        let execution_id = lettuce_types::ToolExecutionId::new();
        let context = ProviderNeutralContext {
            messages: vec![
                ProviderNeutralMessage {
                    role: MessageRole::Assistant,
                    parts: vec![ProviderContextPart::ToolCall(TranscriptToolCall {
                        execution_id,
                        provider_call_id: Some("call-1".to_owned()),
                        name: "create_memory".to_owned(),
                        arguments: serde_json::json!({"content": "one"}),
                        raw_arguments: Some(r#"{"content":"one"}"#.to_owned()),
                        provider_replay: None,
                    })],
                },
                ProviderNeutralMessage {
                    role: MessageRole::User,
                    parts: vec![ProviderContextPart::ToolResult(TranscriptToolResult {
                        execution_id,
                        provider_call_id: Some("call-1".to_owned()),
                        name: "create_memory".to_owned(),
                        output: ToolOutput {
                            value: serde_json::json!({"ok": true}),
                            is_error: false,
                        },
                    })],
                },
            ],
            attributions: ContextAttributions::default(),
            budget: ContextBudgetReport::default(),
        };
        context.validate().expect("tool transcript");
        let tools = ToolRequest {
            definitions: vec![ToolDefinition {
                name: "create_memory".to_owned(),
                description: Some("Create one memory".to_owned()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {"content": {"type": "string"}},
                    "required": ["content"]
                }),
                version: 1,
            }],
            choice: ToolChoice::Named {
                name: "create_memory".to_owned(),
            },
        };
        let profile = test_profile();
        let messages = wire_messages(&context, &crate::openai::OpenAi, &ProviderConfig::Standard)
            .expect("messages");
        let body = encode_request(
            &crate::openai::OpenAi,
            &profile,
            &ProviderConfig::Standard,
            messages,
            Some(&tools),
            false,
        )
        .expect("request");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(
            body["messages"],
            serde_json::json!([
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "create_memory",
                            "arguments": "{\"content\":\"one\"}"
                        }
                    }]
                },
                {"role": "tool", "content": "{\"ok\":true}", "tool_call_id": "call-1"}
            ])
        );
        assert_eq!(
            body["tool_choice"],
            serde_json::json!({
                "type": "function",
                "function": {"name": "create_memory"}
            })
        );
        assert_eq!(body["tools"][0]["function"]["name"], "create_memory");
        assert!(body["tools"][0]["function"].get("version").is_none());
    }

    #[test]
    fn provider_tool_choice_policies_keep_their_legacy_wire_differences() {
        assert_eq!(
            crate::mistral::Mistral
                .tool_choice(&ToolChoice::Required, &ProviderConfig::Standard)
                .expect("mistral choice"),
            Some("any".into())
        );

        let cases = [
            (
                lettuce_models::CustomToolChoiceMode::Auto,
                Some("auto".into()),
            ),
            (
                lettuce_models::CustomToolChoiceMode::Required,
                Some("required".into()),
            ),
            (
                lettuce_models::CustomToolChoiceMode::None,
                Some("none".into()),
            ),
            (lettuce_models::CustomToolChoiceMode::Omit, None),
        ];
        for (mode, expected) in cases {
            let config = ProviderConfig::Custom(lettuce_models::CustomProviderConfig {
                tool_choice_mode: mode,
                ..Default::default()
            });
            assert_eq!(
                crate::custom::Custom
                    .tool_choice(&ToolChoice::Required, &config)
                    .expect("custom choice"),
                expected
            );
        }
        let passthrough = ProviderConfig::Custom(lettuce_models::CustomProviderConfig {
            tool_choice_mode: lettuce_models::CustomToolChoiceMode::Passthrough,
            ..Default::default()
        });
        assert_eq!(
            crate::custom::Custom
                .tool_choice(
                    &ToolChoice::Named {
                        name: "create_memory".to_owned(),
                    },
                    &passthrough,
                )
                .expect("passthrough"),
            Some(serde_json::json!({
                "type": "function",
                "function": {"name": "create_memory"}
            }))
        );
    }

    #[test]
    fn custom_provider_restores_opt_in_chat_template_kwargs() {
        let config = ProviderConfig::Custom(lettuce_models::CustomProviderConfig {
            send_chat_template_kwargs: true,
            ..Default::default()
        });
        let mut profile = test_profile();
        profile.provider_kind = "custom".to_owned();
        profile.provider_config = config.clone();
        let body = encode_request(
            &crate::custom::Custom,
            &profile,
            &config,
            vec![WireMessage::text("user", "hello")],
            None,
            false,
        )
        .expect("custom request");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(
            body["chat_template_kwargs"],
            serde_json::json!({"enable_thinking": false})
        );
    }

    #[test]
    fn cache_control_marks_only_the_final_tool_definition() {
        let mut parameters = crate::integration_tests::parameters();
        parameters.prompt_caching = Some(PromptCaching::Enabled {
            retention: PromptCacheRetention::OneHour,
        });
        let mut body = serde_json::json!({
            "tools": [
                {"type": "function", "function": {"name": "first"}},
                {"type": "function", "function": {"name": "last"}}
            ]
        })
        .as_object()
        .expect("object")
        .clone();

        apply_tool_cache_control(&parameters, &mut body);

        assert!(body["tools"][0].get("cache_control").is_none());
        assert_eq!(
            body["tools"][1]["cache_control"],
            serde_json::json!({"type": "ephemeral", "ttl": "1h"})
        );
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
            WireMessage::text("system", "a"),
            WireMessage::text("system", ""),
            WireMessage::text("system", "b"),
            WireMessage::text("user", "c"),
            WireMessage::text("assistant", ""),
            WireMessage::text("assistant", "d"),
        ]);
        let rendered: Vec<(&str, &str)> = merged
            .iter()
            .map(|message| {
                (
                    message.role.as_ref(),
                    message.content.as_deref().unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(
            rendered,
            vec![("system", "a\n\n\n\nb"), ("user", "c"), ("assistant", "d")]
        );
    }

    #[test]
    fn buffered_tool_only_response_is_a_typed_outcome() {
        let outcome = parse_response(response(
            r#"{"choices":[{"index":0,"message":{"content":null,"tool_calls":[{"id":"call-1","type":"function","function":{"name":"create_memory","arguments":"{\"content\":\"remember\"}"}}]},"finish_reason":"tool_calls"}],"id":"id"}"#,
        ))
        .expect("tool-call outcome");
        assert_eq!(outcome.finish_reason, FinishReason::Stop);
        assert!(outcome.candidates[0].parts.is_empty());
        assert_eq!(outcome.candidates[0].tool_calls.len(), 1);
        assert_eq!(outcome.candidates[0].tool_calls[0].name, "create_memory");
        assert!(outcome.warning_codes.is_empty());
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
