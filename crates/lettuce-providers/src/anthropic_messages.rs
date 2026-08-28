use std::borrow::Cow;

use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceRequest, InferenceUsage,
    InferenceWarningCode, MessagePart, MessageRole, ProposedToolCall, ProviderContextPart,
    ProviderNeutralContext, ToolChoice, ToolRequest,
};
use lettuce_inference::InferenceRuntimePort;
use lettuce_models::{
    CapabilityStatus, PromptCacheRetention, PromptCaching, ProviderAccount, ProviderConfig,
    ReasoningMode, ResolvedChatParameters, ResolvedChatProfile,
};
use lettuce_network::{JsonClient, JsonResponse, JsonStaticHeader, MAX_REQUEST_BYTES};
use lettuce_settings::SecretStore;
use serde::{Deserialize, Serialize};

use crate::common::{
    AdapterError, AuthPlan, Credentials, RemoteModel, decode_json, generation_policy, load_auth,
    load_secret_headers, max_output_tokens, reject_unsupported_features,
    validate_common_request_with_tools, validate_prompt_caching, validate_supported_reasoning,
};
use crate::descriptor::ProviderDescriptor;

pub(crate) const ANTHROPIC_HEADERS: [JsonStaticHeader; 3] = [
    JsonStaticHeader {
        name: "anthropic-version",
        value: "2023-06-01",
    },
    JsonStaticHeader {
        name: "accept",
        value: "application/json",
    },
    JsonStaticHeader {
        name: "user-agent",
        value: concat!("LettuceAI/", env!("CARGO_PKG_VERSION")),
    },
];

const ANTHROPIC_STREAM_HEADERS: [JsonStaticHeader; 3] = [
    JsonStaticHeader {
        name: "anthropic-version",
        value: "2023-06-01",
    },
    JsonStaticHeader {
        name: "accept",
        value: "text/event-stream",
    },
    JsonStaticHeader {
        name: "user-agent",
        value: concat!("LettuceAI/", env!("CARGO_PKG_VERSION")),
    },
];

pub(crate) trait AnthropicWireProvider: Sync {
    fn descriptor(&self) -> &'static ProviderDescriptor;

    fn accepts(&self, config: &ProviderConfig) -> bool {
        matches!(config, ProviderConfig::Standard)
    }

    fn default_endpoint(&self) -> Option<&'static str> {
        self.descriptor().default_endpoint
    }

    fn chat_path(
        &self,
        endpoint: &str,
        _config: &ProviderConfig,
    ) -> Result<Cow<'static, str>, AdapterError> {
        Ok(Cow::Borrowed(
            if endpoint.trim_end_matches('/').ends_with("/v1") {
                "/messages"
            } else {
                "/v1/messages"
            },
        ))
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
        payload
            .get("data")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        Some(RemoteModel {
                            id: item.get("id")?.as_str()?.to_owned(),
                            display_name: item
                                .get("display_name")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned),
                            description: None,
                            context_length: None,
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

    fn merges_same_role(&self, _config: &ProviderConfig) -> bool {
        false
    }

    fn roles(&self, _config: &ProviderConfig) -> (Cow<'static, str>, Cow<'static, str>) {
        (Cow::Borrowed("user"), Cow::Borrowed("assistant"))
    }

    fn auth(&self, _config: &ProviderConfig) -> Result<AuthPlan, AdapterError>;

    fn static_headers(&self) -> &'static [JsonStaticHeader] {
        &ANTHROPIC_HEADERS
    }

    fn validate_parameters(&self, parameters: &ResolvedChatParameters) -> Result<(), AdapterError> {
        validate_supported_reasoning(parameters)?;
        match parameters.reasoning_mode {
            Some(ReasoningMode::Enabled) if parameters.reasoning_budget_tokens.is_none() => {
                return Err(AdapterError::Rejected);
            }
            Some(ReasoningMode::Enabled)
                if parameters.total_completion_allowance.is_none()
                    && max_output_tokens(parameters)
                        .checked_add(parameters.reasoning_budget_tokens.unwrap_or_default())
                        .is_none() =>
            {
                return Err(AdapterError::Rejected);
            }
            Some(ReasoningMode::Enabled) => {}
            Some(ReasoningMode::Disabled) | None => reject_unsupported_features(parameters)?,
        }
        validate_prompt_caching(self.descriptor().prompt_caching, parameters)
    }

    fn supports_streaming(&self, _config: &ProviderConfig) -> bool {
        self.descriptor().streaming
    }
}

pub(crate) async fn run<S: SecretStore + ?Sized>(
    provider: &dyn AnthropicWireProvider,
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
    validate_tool_features(profile, request.tools.as_ref())?;
    let endpoint = profile
        .endpoint
        .as_deref()
        .or_else(|| provider.default_endpoint())
        .ok_or(AdapterError::Rejected)?;
    let path = provider.chat_path(endpoint, config)?;
    let streaming = request.stream_sink.is_some();
    if streaming && (!profile.streaming_enabled || !provider.supports_streaming(config)) {
        return Err(AdapterError::Rejected);
    }
    let body = encode_request(
        profile,
        &request.context,
        provider.merges_same_role(config),
        provider.roles(config),
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
                    endpoint,
                    &path,
                    body,
                    &ANTHROPIC_STREAM_HEADERS,
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
            crate::stream_normalize::StreamProtocol::Anthropic,
            runtime,
            &request,
        )
        .await
    } else {
        let response = crate::streaming::await_cancelable(runtime, request.cancellation, async {
            network
                .post_json(
                    endpoint,
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
    }?;
    validate_tool_outcome(&request, outcome)
}

fn validate_tool_features(
    profile: &ResolvedChatProfile,
    tools: Option<&ToolRequest>,
) -> Result<(), AdapterError> {
    if tools.is_none() {
        return Ok(());
    }
    if profile.capabilities.tools == CapabilityStatus::Unsupported {
        return Err(AdapterError::Rejected);
    }
    // Extended-thinking tool rounds require replaying Anthropic's signed thinking
    // blocks exactly. The provider-replay artifact is not materialized here yet.
    if profile.parameters.reasoning_mode == Some(ReasoningMode::Enabled) {
        return Err(AdapterError::Rejected);
    }
    Ok(())
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
    provider: &dyn AnthropicWireProvider,
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
    let path = provider
        .models_path(endpoint, &account.config)
        .ok_or(AdapterError::Rejected)?;
    let credentials = Credentials::from(account);
    let auth = load_auth(provider.auth(&account.config)?, secret_store, &credentials).await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    let response = network
        .get_json(
            endpoint,
            &path,
            provider.static_headers(),
            auth,
            secret_headers,
            generation_policy(&credentials),
        )
        .await?;
    Ok(provider.parse_models(&decode_json(&response)?, &account.config))
}

struct Turn {
    assistant: bool,
    content: Vec<WireContentBlock>,
}

fn encode_request(
    profile: &ResolvedChatProfile,
    context: &ProviderNeutralContext,
    merge_same_role: bool,
    (user_role, assistant_role): (Cow<'static, str>, Cow<'static, str>),
    tools: Option<&ToolRequest>,
    streaming: bool,
) -> Result<Vec<u8>, AdapterError> {
    let mut system_parts: Vec<String> = Vec::new();
    let mut turns: Vec<Turn> = Vec::new();
    for message in &context.messages {
        let mut content = Vec::new();
        let mut results = Vec::new();
        for part in &message.parts {
            match part {
                ProviderContextPart::Text { text } => content.push(WireContentBlock::Text {
                    text: text.clone(),
                    cache_control: None,
                }),
                ProviderContextPart::ToolCall(call) => {
                    let id = call
                        .provider_call_id
                        .clone()
                        .ok_or(AdapterError::Rejected)?;
                    content.push(WireContentBlock::ToolUse {
                        id,
                        name: call.name.clone(),
                        input: call.arguments.clone(),
                    });
                }
                ProviderContextPart::ToolResult(result) => {
                    let tool_use_id = result
                        .provider_call_id
                        .clone()
                        .ok_or(AdapterError::Rejected)?;
                    results.push(WireContentBlock::ToolResult {
                        tool_use_id,
                        content: serde_json::to_string(&result.output.value)
                            .map_err(|_| AdapterError::Rejected)?,
                        is_error: result.output.is_error,
                    });
                }
                ProviderContextPart::MediaAsset { .. } => return Err(AdapterError::Rejected),
            }
        }
        collapse_text_only(&mut content);
        if content.is_empty() && results.is_empty() {
            continue;
        }
        match message.role {
            MessageRole::System | MessageRole::Scene => {
                if !results.is_empty()
                    || content
                        .iter()
                        .any(|part| !matches!(part, WireContentBlock::Text { .. }))
                {
                    return Err(AdapterError::Rejected);
                }
                let text = content
                    .into_iter()
                    .filter_map(|part| match part {
                        WireContentBlock::Text { text, .. } => Some(text),
                        _ => None,
                    })
                    .collect::<String>();
                if !text.is_empty() {
                    system_parts.push(text);
                }
            }
            MessageRole::User | MessageRole::Assistant => {
                let assistant = message.role == MessageRole::Assistant;
                if assistant && !results.is_empty() {
                    return Err(AdapterError::Rejected);
                }
                if !assistant {
                    results.append(&mut content);
                    content = results;
                }
                match turns.last_mut() {
                    Some(last)
                        if merge_same_role
                            && last.assistant == assistant
                            && !contains_tool_block(&last.content)
                            && !contains_tool_block(&content) =>
                    {
                        merge_text_only(&mut last.content, content)?;
                    }
                    _ => turns.push(Turn { assistant, content }),
                }
            }
        }
    }
    validate_tool_sequence(&turns)?;
    let mut messages = turns
        .into_iter()
        .filter(|turn| !turn.content.is_empty())
        .map(|turn| WireMessage {
            role: if turn.assistant {
                assistant_role.clone()
            } else {
                user_role.clone()
            },
            content: turn.content,
        })
        .collect::<Vec<_>>();
    if messages.is_empty() {
        return Err(AdapterError::Rejected);
    }
    let parameters = &profile.parameters;
    let cache_control = match parameters.prompt_caching {
        Some(PromptCaching::Enabled { retention }) => Some(WireCacheControl {
            kind: "ephemeral",
            ttl: (retention == PromptCacheRetention::OneHour).then_some("1h"),
        }),
        Some(PromptCaching::Disabled) | None => None,
    };
    if let (Some(control), Some(last_user)) = (
        cache_control,
        messages
            .iter_mut()
            .rev()
            .find(|message| message.role == user_role),
    ) {
        if let Some(last_text) = last_user
            .content
            .iter_mut()
            .rev()
            .find_map(WireContentBlock::text_mut)
        {
            last_text.1.replace(control);
        }
    }
    let system = (!system_parts.is_empty()).then(|| {
        let text = system_parts.join("\n\n");
        cache_control.map_or(WireSystem::Text(text.clone()), |control| {
            WireSystem::Blocks(vec![WireTextBlock {
                kind: "text",
                text,
                cache_control: Some(control),
            }])
        })
    });
    let request = MessagesRequest {
        model: profile.external_model_id.clone(),
        messages,
        system,
        max_tokens: anthropic_max_tokens(parameters),
        stream: streaming,
        temperature: anthropic_temperature(parameters),
        top_p: parameters.top_p,
        top_k: parameters.top_k,
        thinking: anthropic_thinking(parameters),
        tools: tools.map(|request| {
            request
                .definitions
                .iter()
                .map(|definition| WireToolDefinition {
                    name: definition.name.clone(),
                    description: definition.description.clone(),
                    input_schema: definition.parameters.clone(),
                    cache_control: None,
                })
                .collect()
        }),
        tool_choice: tools.map(|request| match &request.choice {
            ToolChoice::Auto => WireToolChoice::Auto { kind: "auto" },
            ToolChoice::Required => WireToolChoice::Any { kind: "any" },
            ToolChoice::Named { name } => WireToolChoice::Named {
                kind: "tool",
                name: name.clone(),
            },
        }),
    };
    let mut request = request;
    if let (Some(control), Some(last_tool)) = (
        cache_control,
        request.tools.as_mut().and_then(|tools| tools.last_mut()),
    ) {
        last_tool.cache_control = Some(control);
    }
    let body = serde_json::to_vec(&request).map_err(|_| AdapterError::Rejected)?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err(AdapterError::Rejected);
    }
    Ok(body)
}

fn contains_tool_block(content: &[WireContentBlock]) -> bool {
    content
        .iter()
        .any(|part| !matches!(part, WireContentBlock::Text { .. }))
}

fn collapse_text_only(content: &mut Vec<WireContentBlock>) {
    if contains_tool_block(content) || content.len() < 2 {
        return;
    }
    let text = std::mem::take(content)
        .into_iter()
        .filter_map(|part| match part {
            WireContentBlock::Text { text, .. } => Some(text),
            _ => None,
        })
        .collect::<String>();
    content.push(WireContentBlock::Text {
        text,
        cache_control: None,
    });
}

fn merge_text_only(
    target: &mut [WireContentBlock],
    source: Vec<WireContentBlock>,
) -> Result<(), AdapterError> {
    let Some((target, _)) = target.last_mut().and_then(WireContentBlock::text_mut) else {
        return Err(AdapterError::Rejected);
    };
    let Some(source) = source.into_iter().next().and_then(|part| match part {
        WireContentBlock::Text { text, .. } => Some(text),
        _ => None,
    }) else {
        return Err(AdapterError::Rejected);
    };
    if !target.is_empty() {
        target.push_str("\n\n");
    }
    target.push_str(&source);
    Ok(())
}

fn validate_tool_sequence(turns: &[Turn]) -> Result<(), AdapterError> {
    let mut expected: Option<Vec<&str>> = None;
    for turn in turns {
        let result_ids = turn
            .content
            .iter()
            .filter_map(|part| match part {
                WireContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if let Some(mut expected_ids) = expected.take() {
            if turn.assistant || result_ids.len() != expected_ids.len() {
                return Err(AdapterError::Rejected);
            }
            expected_ids.sort_unstable();
            let mut actual = result_ids;
            actual.sort_unstable();
            if actual != expected_ids {
                return Err(AdapterError::Rejected);
            }
        } else if !result_ids.is_empty() {
            return Err(AdapterError::Rejected);
        }

        let call_ids = turn
            .content
            .iter()
            .filter_map(|part| match part {
                WireContentBlock::ToolUse { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !call_ids.is_empty() {
            if !turn.assistant {
                return Err(AdapterError::Rejected);
            }
            expected = Some(call_ids);
        }
    }
    if expected.is_some() {
        return Err(AdapterError::Rejected);
    }
    Ok(())
}

fn anthropic_max_tokens(parameters: &ResolvedChatParameters) -> u32 {
    parameters.total_completion_allowance.unwrap_or_else(|| {
        max_output_tokens(parameters) + parameters.reasoning_budget_tokens.unwrap_or(0)
    })
}

fn anthropic_temperature(parameters: &ResolvedChatParameters) -> Option<f64> {
    if parameters.reasoning_mode == Some(ReasoningMode::Enabled) {
        Some(1.0)
    } else {
        parameters.temperature
    }
}

fn anthropic_thinking(parameters: &ResolvedChatParameters) -> Option<ThinkingConfig> {
    parameters
        .reasoning_budget_tokens
        .and_then(|budget_tokens| {
            (parameters.reasoning_mode == Some(ReasoningMode::Enabled)).then_some(ThinkingConfig {
                kind: "enabled",
                budget_tokens,
            })
        })
}

fn parse_response(response: JsonResponse) -> Result<InferenceOutcome, AdapterError> {
    if let Some(error) = AdapterError::from_response(&response) {
        return Err(error);
    }
    let header_request_id = response.request_id.clone();
    let parsed: MessagesResponse =
        serde_json::from_slice(&response.body).map_err(|_| AdapterError::MalformedResponse)?;
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut tool_calls = Vec::new();
    let mut warnings = Vec::new();
    for block in parsed.content {
        match block.kind.as_str() {
            "text" => text.push_str(block.text.as_deref().unwrap_or_default()),
            "thinking" => reasoning.push_str(block.thinking.as_deref().unwrap_or_default()),
            "tool_use" => {
                let provider_call_id = block
                    .id
                    .filter(|id| !id.trim().is_empty())
                    .ok_or(AdapterError::MalformedResponse)?;
                let name = block
                    .name
                    .filter(|name| !name.trim().is_empty())
                    .ok_or(AdapterError::MalformedResponse)?;
                let arguments = block
                    .input
                    .filter(serde_json::Value::is_object)
                    .ok_or(AdapterError::MalformedResponse)?;
                tool_calls.push(ProposedToolCall {
                    provider_call_id: Some(provider_call_id),
                    name,
                    arguments,
                    raw_arguments: None,
                    provider_replay: None,
                });
            }
            "server_tool_use" => push(&mut warnings, InferenceWarningCode::ProviderDegraded),
            _ => {}
        }
    }
    if !tool_calls.is_empty() && parsed.stop_reason.as_deref() != Some("tool_use") {
        return Err(AdapterError::MalformedResponse);
    }
    let finish_reason = match parsed.stop_reason.as_deref() {
        Some("max_tokens") => {
            push(&mut warnings, InferenceWarningCode::Truncated);
            FinishReason::Length
        }
        Some("refusal") => {
            push(&mut warnings, InferenceWarningCode::SafetyTransformed);
            FinishReason::Stop
        }
        Some("tool_use") if tool_calls.is_empty() => {
            return Err(AdapterError::MalformedResponse);
        }
        Some("tool_use") => FinishReason::Stop,
        Some("pause_turn") => {
            push(&mut warnings, InferenceWarningCode::ProviderDegraded);
            FinishReason::Stop
        }
        _ => FinishReason::Stop,
    };
    if text.trim().is_empty()
        && reasoning.trim().is_empty()
        && tool_calls.is_empty()
        && !warnings.contains(&InferenceWarningCode::SafetyTransformed)
        && !warnings.contains(&InferenceWarningCode::ProviderDegraded)
    {
        return Err(AdapterError::EmptyResponse);
    }
    let outcome = InferenceOutcome {
        candidates: vec![InferenceCandidate {
            ordinal: 0,
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
            tool_calls,
            provider_replay: None,
        }],
        usage: parsed.usage.and_then(|usage| {
            Some(InferenceUsage {
                input_tokens: usage.input_tokens?,
                output_tokens: usage.output_tokens?,
            })
        }),
        finish_reason,
        provider_finish_reason: parsed.stop_reason,
        provider_request_id: header_request_id,
        warning_codes: warnings,
    };
    outcome
        .validate()
        .map_err(|_| AdapterError::MalformedResponse)?;
    Ok(outcome)
}

fn push(warnings: &mut Vec<InferenceWarningCode>, warning: InferenceWarningCode) {
    if !warnings.contains(&warning) {
        warnings.push(warning);
    }
}

#[derive(Serialize)]
struct MessagesRequest {
    model: String,
    messages: Vec<WireMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<WireSystem>,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<WireToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<WireToolChoice>,
}

#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    kind: &'static str,
    budget_tokens: u32,
}

#[derive(Serialize)]
struct WireMessage {
    role: Cow<'static, str>,
    content: Vec<WireContentBlock>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireContentBlock {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<WireCacheControl>,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

impl WireContentBlock {
    fn text_mut(&mut self) -> Option<(&mut String, &mut Option<WireCacheControl>)> {
        match self {
            Self::Text {
                text,
                cache_control,
            } => Some((text, cache_control)),
            Self::ToolUse { .. } | Self::ToolResult { .. } => None,
        }
    }
}

#[derive(Serialize)]
struct WireToolDefinition {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    input_schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<WireCacheControl>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum WireToolChoice {
    Auto {
        #[serde(rename = "type")]
        kind: &'static str,
    },
    Any {
        #[serde(rename = "type")]
        kind: &'static str,
    },
    Named {
        #[serde(rename = "type")]
        kind: &'static str,
        name: String,
    },
}

#[derive(Serialize)]
#[serde(untagged)]
enum WireSystem {
    Text(String),
    Blocks(Vec<WireTextBlock>),
}

#[derive(Serialize)]
struct WireTextBlock {
    #[serde(rename = "type")]
    kind: &'static str,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<WireCacheControl>,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct WireCacheControl {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<&'static str>,
}

#[derive(Deserialize)]
struct MessagesResponse {
    #[serde(default)]
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
    thinking: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_conversations::{
        ContextAttributions, ContextBudgetReport, ProviderFailure, ProviderFailureKind,
        ProviderNeutralMessage, ToolDefinition, ToolOutput, TranscriptToolCall,
        TranscriptToolResult,
    };
    use lettuce_models::{ChatProfileWarning, ModelCapabilities, ProviderProtocol};

    fn response(body: &str) -> JsonResponse {
        JsonResponse {
            status: 200,
            body: body.as_bytes().to_vec(),
            request_id: None,
            retry_after: None,
        }
    }

    fn test_profile() -> ResolvedChatProfile {
        ResolvedChatProfile {
            model_profile_id: lettuce_types::ModelProfileId::new(),
            model_revision: lettuce_types::Revision::INITIAL,
            provider_account_id: lettuce_types::ProviderAccountId::new(),
            provider_account_revision: lettuce_types::Revision::INITIAL,
            secret_owner_id: lettuce_settings::SecretOwnerId::new(),
            external_model_id: "test-model".to_owned(),
            provider_kind: "anthropic".to_owned(),
            provider_protocol: ProviderProtocol::Anthropic,
            endpoint: Some("https://api.anthropic.com".to_owned()),
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

    fn tool_request(choice: ToolChoice) -> ToolRequest {
        ToolRequest {
            definitions: vec![
                ToolDefinition {
                    name: "first".to_owned(),
                    description: None,
                    parameters: serde_json::json!({"type": "object"}),
                    version: 1,
                },
                ToolDefinition {
                    name: "create_memory".to_owned(),
                    description: Some("Create memory".to_owned()),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {"content": {"type": "string"}}
                    }),
                    version: 1,
                },
            ],
            choice,
        }
    }

    #[test]
    fn encodes_native_tool_replay_choice_and_cache_controls() {
        let execution_id = lettuce_types::ToolExecutionId::new();
        let context = ProviderNeutralContext {
            messages: vec![
                ProviderNeutralMessage {
                    role: MessageRole::System,
                    parts: vec![ProviderContextPart::Text {
                        text: "system".to_owned(),
                    }],
                },
                ProviderNeutralMessage {
                    role: MessageRole::User,
                    parts: vec![ProviderContextPart::Text {
                        text: "remember this".to_owned(),
                    }],
                },
                ProviderNeutralMessage {
                    role: MessageRole::Assistant,
                    parts: vec![
                        ProviderContextPart::Text {
                            text: "I'll save it".to_owned(),
                        },
                        ProviderContextPart::ToolCall(TranscriptToolCall {
                            execution_id,
                            provider_call_id: Some("toolu-1".to_owned()),
                            name: "create_memory".to_owned(),
                            arguments: serde_json::json!({"content": "one"}),
                            raw_arguments: Some(r#"{"content":"one"}"#.to_owned()),
                            provider_replay: None,
                        }),
                    ],
                },
                ProviderNeutralMessage {
                    role: MessageRole::User,
                    parts: vec![
                        ProviderContextPart::Text {
                            text: "continue".to_owned(),
                        },
                        ProviderContextPart::ToolResult(TranscriptToolResult {
                            execution_id,
                            provider_call_id: Some("toolu-1".to_owned()),
                            name: "create_memory".to_owned(),
                            output: ToolOutput {
                                value: serde_json::json!({"ok": true}),
                                is_error: false,
                            },
                        }),
                    ],
                },
            ],
            attributions: ContextAttributions::default(),
            budget: ContextBudgetReport::default(),
        };
        context.validate().expect("context");
        let mut profile = test_profile();
        profile.parameters.prompt_caching = Some(PromptCaching::Enabled {
            retention: PromptCacheRetention::OneHour,
        });
        let tools = tool_request(ToolChoice::Named {
            name: "create_memory".to_owned(),
        });

        let body = encode_request(
            &profile,
            &context,
            false,
            (Cow::Borrowed("user"), Cow::Borrowed("assistant")),
            Some(&tools),
            false,
        )
        .expect("request");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("json");

        assert_eq!(
            body["tool_choice"],
            serde_json::json!({"type": "tool", "name": "create_memory"})
        );
        assert!(body["tools"][0].get("cache_control").is_none());
        assert_eq!(
            body["tools"][1]["cache_control"],
            serde_json::json!({"type": "ephemeral", "ttl": "1h"})
        );
        assert_eq!(body["messages"][1]["content"][1]["type"], "tool_use");
        assert_eq!(
            body["messages"][1]["content"][1]["input"],
            serde_json::json!({"content": "one"})
        );
        assert_eq!(body["messages"][2]["content"][0]["type"], "tool_result");
        assert_eq!(body["messages"][2]["content"][0]["is_error"], false);
        assert_eq!(body["messages"][2]["content"][1]["text"], "continue");
        assert_eq!(
            body["messages"][2]["content"][1]["cache_control"],
            serde_json::json!({"type": "ephemeral", "ttl": "1h"})
        );
        assert_eq!(
            body["system"][0]["cache_control"],
            serde_json::json!({"type": "ephemeral", "ttl": "1h"})
        );
    }

    #[test]
    fn maps_all_anthropic_tool_choices() {
        for (choice, expected) in [
            (ToolChoice::Auto, serde_json::json!({"type": "auto"})),
            (ToolChoice::Required, serde_json::json!({"type": "any"})),
            (
                ToolChoice::Named {
                    name: "create_memory".to_owned(),
                },
                serde_json::json!({"type": "tool", "name": "create_memory"}),
            ),
        ] {
            let request = MessagesRequest {
                model: "m".to_owned(),
                messages: Vec::new(),
                system: None,
                max_tokens: 1,
                stream: false,
                temperature: None,
                top_p: None,
                top_k: None,
                thinking: None,
                tools: None,
                tool_choice: Some(match choice {
                    ToolChoice::Auto => WireToolChoice::Auto { kind: "auto" },
                    ToolChoice::Required => WireToolChoice::Any { kind: "any" },
                    ToolChoice::Named { name } => WireToolChoice::Named { kind: "tool", name },
                }),
            };
            let value = serde_json::to_value(request).expect("choice");
            assert_eq!(value["tool_choice"], expected);
        }
    }

    #[test]
    fn rejects_tools_with_unsupported_capability_or_unreplayable_thinking() {
        let tools = tool_request(ToolChoice::Auto);
        let mut profile = test_profile();
        profile.capabilities.tools = CapabilityStatus::Unsupported;
        assert_eq!(
            validate_tool_features(&profile, Some(&tools)),
            Err(AdapterError::Rejected)
        );

        profile.capabilities.tools = CapabilityStatus::Supported;
        profile.parameters.reasoning_mode = Some(ReasoningMode::Enabled);
        assert_eq!(
            validate_tool_features(&profile, Some(&tools)),
            Err(AdapterError::Rejected)
        );
        assert_eq!(validate_tool_features(&profile, None), Ok(()));
    }

    #[test]
    fn enabled_reasoning_uses_native_thinking_total_tokens_and_fixed_temperature() {
        let mut parameters = crate::integration_tests::parameters();
        parameters.temperature = Some(0.4);
        parameters.visible_max_output_tokens = Some(100);
        parameters.reasoning_mode = Some(ReasoningMode::Enabled);
        parameters.reasoning_budget_tokens = Some(20);
        parameters.total_completion_allowance = Some(120);

        assert_eq!(anthropic_max_tokens(&parameters), 120);
        assert_eq!(anthropic_temperature(&parameters), Some(1.0));
        assert_eq!(
            serde_json::to_value(anthropic_thinking(&parameters)).expect("serialize"),
            serde_json::json!({ "type": "enabled", "budget_tokens": 20 })
        );

        parameters.reasoning_mode = Some(ReasoningMode::Disabled);
        parameters.reasoning_budget_tokens = None;
        parameters.total_completion_allowance = Some(100);
        assert_eq!(anthropic_temperature(&parameters), Some(0.4));
        assert!(anthropic_thinking(&parameters).is_none());
    }

    #[test]
    fn rejects_an_inconsistent_resolved_completion_allowance() {
        let mut parameters = crate::integration_tests::parameters();
        parameters.visible_max_output_tokens = Some(100);
        parameters.reasoning_mode = Some(ReasoningMode::Enabled);
        parameters.reasoning_budget_tokens = Some(20);
        parameters.total_completion_allowance = Some(119);
        assert_eq!(
            crate::anthropic::Anthropic.validate_parameters(&parameters),
            Err(AdapterError::Rejected)
        );
    }

    #[test]
    fn joins_text_blocks_and_maps_stop_reasons() {
        let outcome = parse_response(response(
            r#"{"content":[{"type":"text","text":"a"},{"type":"thinking","thinking":"x"},{"type":"text","text":"b"}],"stop_reason":"max_tokens","usage":{"input_tokens":3,"output_tokens":5}}"#,
        ))
        .expect("response");
        assert_eq!(
            outcome.candidates[0].parts,
            vec![
                MessagePart::ReasoningSummary {
                    text: "x".to_owned()
                },
                MessagePart::Text {
                    text: "ab".to_owned()
                }
            ]
        );
        assert_eq!(outcome.finish_reason, FinishReason::Length);
        assert_eq!(
            outcome.usage,
            Some(InferenceUsage {
                input_tokens: 3,
                output_tokens: 5
            })
        );
        assert_eq!(outcome.warning_codes, vec![InferenceWarningCode::Truncated]);
    }

    #[test]
    fn parses_mixed_buffered_tool_use_without_degrading_it() {
        let outcome = parse_response(response(
            r#"{"content":[{"type":"thinking","thinking":"why"},{"type":"text","text":"saving"},{"type":"tool_use","id":"toolu-1","name":"create_memory","input":{"content":"one"}}],"stop_reason":"tool_use","usage":{"input_tokens":3,"output_tokens":5}}"#,
        ))
        .expect("tool response");

        assert_eq!(outcome.candidates[0].tool_calls.len(), 1);
        assert_eq!(
            outcome.candidates[0].tool_calls[0]
                .provider_call_id
                .as_deref(),
            Some("toolu-1")
        );
        assert_eq!(
            outcome.candidates[0].tool_calls[0].arguments,
            serde_json::json!({"content": "one"})
        );
        assert!(outcome.warning_codes.is_empty());
        assert_eq!(outcome.provider_finish_reason.as_deref(), Some("tool_use"));

        for malformed in [
            r#"{"content":[{"type":"tool_use","name":"x","input":{}}],"stop_reason":"tool_use"}"#,
            r#"{"content":[{"type":"tool_use","id":"id","name":"x","input":[]}],"stop_reason":"tool_use"}"#,
            r#"{"content":[],"stop_reason":"tool_use"}"#,
            r#"{"content":[{"type":"tool_use","id":"id","name":"x","input":{}}],"stop_reason":"end_turn"}"#,
        ] {
            assert_eq!(
                parse_response(response(malformed)),
                Err(AdapterError::MalformedResponse)
            );
        }
    }

    #[test]
    fn refusals_are_safety_outcomes_and_blank_text_is_empty() {
        let outcome = parse_response(response(r#"{"content":[],"stop_reason":"refusal"}"#))
            .expect("refusal outcome");
        assert!(outcome.candidates[0].parts.is_empty());
        assert_eq!(
            outcome.warning_codes,
            vec![InferenceWarningCode::SafetyTransformed]
        );
        assert_eq!(
            parse_response(response(
                r#"{"content":[{"type":"text","text":" "}],"stop_reason":"end_turn"}"#
            )),
            Err(AdapterError::EmptyResponse)
        );
        assert!(matches!(
            parse_response(JsonResponse {
                status: 401,
                body: b"key-canary".to_vec(),
                request_id: None,
                retry_after: None,
            }),
            Err(AdapterError::Provider(ProviderFailure {
                kind: ProviderFailureKind::CredentialRejected,
                status: 401,
                ..
            }))
        ));
    }
}
