use std::borrow::Cow;

use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceRequest, InferenceUsage,
    InferenceWarningCode, MAX_TOOL_CALLS_PER_RESPONSE, MessagePart, MessageRole, ProposedToolCall,
    ProviderContextPart, ProviderNeutralContext, ToolChoice, ToolRequest,
};
use lettuce_inference::InferenceRuntimePort;
use lettuce_models::{
    CapabilityStatus, ProviderAccount, ProviderConfig, ReasoningEffort, ReasoningMode,
    ResolvedChatParameters, ResolvedChatProfile,
};
use lettuce_network::{
    JsonClient, JsonQueryParameter, JsonResponse, JsonResponseStream, JsonStaticHeader,
    MAX_REQUEST_BYTES,
};
use lettuce_settings::{HeaderName, SecretStore};
use serde::{Deserialize, Serialize};

use crate::common::{
    AdapterError, AuthPlan, Credentials, RemoteModel, decode_json, generation_policy, load_auth,
    load_secret_headers, max_output_tokens, reject_unsupported_features,
    validate_common_request_with_tools, validate_prompt_caching, validate_supported_reasoning,
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
    validate_common_request_with_tools(&request)?;
    let profile = &request.profile.chat_profile;
    if !matches!(profile.provider_config, ProviderConfig::Standard) {
        return Err(AdapterError::Rejected);
    }
    provider.validate_parameters(&profile.parameters)?;
    validate_tool_features(profile, request.tools.as_ref())?;
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
    let uncached = build_request(profile, &request.context, request.tools.as_ref())?;
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
        let outcome = crate::streaming::consume_stream(
            response,
            crate::stream_framing::StreamFormat::Sse,
            crate::stream_normalize::StreamProtocol::Gemini,
            runtime,
            &request,
        )
        .await?;
        return validate_tool_outcome(&request, outcome);
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
    validate_tool_outcome(&request, parse_response(response)?)
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
    // A signed Gemini function-call part must be replayed byte-for-byte. The
    // provider replay artifact is not materialized at this adapter boundary yet.
    if profile.parameters.reasoning_mode == Some(ReasoningMode::Enabled)
        || profile
            .external_model_id
            .trim()
            .to_ascii_lowercase()
            .contains("gemini-3")
    {
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
    if (request.profile.tool_policy == lettuce_conversations::ToolPolicy::Required
        || matches!(
            tools.choice,
            ToolChoice::Required | ToolChoice::Named { .. }
        ))
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
    tools: Option<&ToolRequest>,
) -> Result<GenerateRequest, AdapterError> {
    let mut system_chunks: Vec<String> = Vec::new();
    let mut contents: Vec<Content> = Vec::new();
    let mut expected_results: Option<Vec<lettuce_types::ToolExecutionId>> = None;
    for message in &context.messages {
        let mut parts = Vec::new();
        let mut call_ids = Vec::new();
        let mut call_provider_ids = Vec::new();
        let mut result_ids = Vec::new();
        for part in &message.parts {
            match part {
                ProviderContextPart::Text { text } if !text.is_empty() => {
                    parts.push(ContentPart::Text { text: text.clone() });
                }
                ProviderContextPart::Text { .. } => {}
                ProviderContextPart::ToolCall(call) => {
                    if message.role != MessageRole::Assistant || call.provider_replay.is_some() {
                        return Err(AdapterError::Rejected);
                    }
                    call_ids.push(call.execution_id);
                    call_provider_ids.push(call.provider_call_id.as_deref());
                    parts.push(ContentPart::FunctionCall {
                        function_call: FunctionCall {
                            id: call.provider_call_id.clone(),
                            name: call.name.clone(),
                            args: call.arguments.clone(),
                        },
                    });
                }
                ProviderContextPart::ToolResult(result) => {
                    if message.role != MessageRole::User {
                        return Err(AdapterError::Rejected);
                    }
                    result_ids.push(result.execution_id);
                    parts.push(ContentPart::FunctionResponse {
                        function_response: FunctionResponse {
                            id: result.provider_call_id.clone(),
                            name: result.name.clone(),
                            response: gemini_tool_response(result),
                        },
                    });
                }
                ProviderContextPart::MediaAsset { .. } => return Err(AdapterError::Rejected),
            }
        }
        if parts.is_empty() {
            continue;
        }
        match message.role {
            MessageRole::System | MessageRole::Scene => {
                if !call_ids.is_empty() || !result_ids.is_empty() {
                    return Err(AdapterError::Rejected);
                }
                let text = parts
                    .into_iter()
                    .filter_map(|part| match part {
                        ContentPart::Text { text } => Some(text),
                        _ => None,
                    })
                    .collect::<String>();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    system_chunks.push(trimmed.to_owned());
                }
                continue;
            }
            MessageRole::User => {
                if !call_ids.is_empty() {
                    return Err(AdapterError::Rejected);
                }
            }
            MessageRole::Assistant => {
                if !result_ids.is_empty() {
                    return Err(AdapterError::Rejected);
                }
            }
        }
        if let Some(mut expected) = expected_results.take() {
            if message.role != MessageRole::User || result_ids.len() != expected.len() {
                return Err(AdapterError::Rejected);
            }
            expected.sort_unstable();
            result_ids.sort_unstable();
            if result_ids != expected {
                return Err(AdapterError::Rejected);
            }
        } else if !result_ids.is_empty() {
            return Err(AdapterError::Rejected);
        }
        if call_ids.len() > 1 && call_provider_ids.iter().any(|id| id.is_none()) {
            return Err(AdapterError::Rejected);
        }
        if !call_ids.is_empty() {
            expected_results = Some(call_ids);
        }
        contents.push(Content {
            role: if message.role == MessageRole::Assistant {
                "model"
            } else {
                "user"
            },
            parts,
        });
    }
    if expected_results.is_some() {
        return Err(AdapterError::Rejected);
    }
    if contents.is_empty() {
        return Err(AdapterError::Rejected);
    }
    let parameters = &profile.parameters;
    let request = GenerateRequest {
        contents,
        system_instruction: (!system_chunks.is_empty()).then(|| Content {
            role: "user",
            parts: vec![ContentPart::Text {
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
        tools: tools.map(|request| {
            vec![GeminiTool {
                function_declarations: request
                    .definitions
                    .iter()
                    .map(|definition| FunctionDeclaration {
                        name: definition.name.clone(),
                        description: definition.description.clone(),
                        parameters: definition.parameters.clone(),
                    })
                    .collect(),
            }]
        }),
        tool_config: tools.map(|request| ToolConfig {
            function_calling_config: match &request.choice {
                ToolChoice::Auto => FunctionCallingConfig {
                    mode: "AUTO",
                    allowed_function_names: None,
                },
                ToolChoice::Required => FunctionCallingConfig {
                    mode: "ANY",
                    allowed_function_names: None,
                },
                ToolChoice::Named { name } => FunctionCallingConfig {
                    mode: "ANY",
                    allowed_function_names: Some(vec![name.clone()]),
                },
            },
        }),
        cached_content: None,
    };
    Ok(request)
}

fn gemini_tool_response(result: &lettuce_conversations::TranscriptToolResult) -> serde_json::Value {
    if result.output.is_error {
        serde_json::json!({ "error": result.output.value })
    } else if result.output.value.is_object() {
        result.output.value.clone()
    } else {
        serde_json::json!({ "result": result.output.value })
    }
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
        let parts = candidate
            .content
            .map(|content| content.parts)
            .unwrap_or_default();
        let has_function_calls = parts.iter().any(|part| part.function_call.is_some());
        if has_function_calls
            && parts.iter().any(|part| {
                part.thought_signature
                    .as_deref()
                    .is_some_and(|signature| !signature.is_empty())
            })
        {
            // Signed parts cannot be returned as executable calls until the
            // replay artifact bytes can be materialized for the next round.
            return Err(AdapterError::MalformedResponse);
        }
        let mut tool_calls = Vec::new();
        for part in parts {
            if (part.text.is_some() && part.function_call.is_some())
                || part.function_response.is_some()
                || part.server_tool_call.is_some()
                || part.server_tool_response.is_some()
            {
                return Err(AdapterError::MalformedResponse);
            }
            if part.thought.unwrap_or(false) {
                reasoning.push_str(part.text.as_deref().unwrap_or_default());
            } else if let Some(fragment) = part.text {
                text.push_str(&fragment);
            }
            if let Some(call) = part.function_call {
                if tool_calls.len() >= MAX_TOOL_CALLS_PER_RESPONSE {
                    return Err(AdapterError::MalformedResponse);
                }
                tool_calls.push(parse_function_call(call)?);
            }
        }
        if tool_calls.len() > 1
            && tool_calls
                .iter()
                .any(|call| call.provider_call_id.is_none())
        {
            return Err(AdapterError::MalformedResponse);
        }
        if !tool_calls.is_empty()
            && !matches!(
                candidate.finish_reason.as_deref(),
                None | Some("STOP") | Some("FINISH_REASON_UNSPECIFIED")
            )
        {
            return Err(AdapterError::MalformedResponse);
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
        has_content |=
            !text.trim().is_empty() || !reasoning.trim().is_empty() || !tool_calls.is_empty();
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
    if candidates.is_empty() {
        candidates.push(InferenceCandidate {
            ordinal: 0,
            parts: Vec::new(),
            tool_calls: Vec::new(),
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

fn parse_function_call(call: ResponseFunctionCall) -> Result<ProposedToolCall, AdapterError> {
    let name = call.name.ok_or(AdapterError::MalformedResponse)?;
    let arguments = call.args.unwrap_or_else(|| serde_json::json!({}));
    if !arguments.is_object() || call.id.as_deref().is_some_and(str::is_empty) || name.is_empty() {
        return Err(AdapterError::MalformedResponse);
    }
    Ok(ProposedToolCall {
        provider_call_id: call.id,
        name,
        arguments,
        raw_arguments: None,
        provider_replay: None,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tools: Option<Vec<GeminiTool>>,
    #[serde(rename = "toolConfig", skip_serializing_if = "Option::is_none")]
    pub(crate) tool_config: Option<ToolConfig>,
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
            tools: None,
            tool_config: None,
            cached_content: Some(name),
        })
    }
}

#[derive(Clone, Serialize)]
pub(crate) struct Content {
    role: &'static str,
    parts: Vec<ContentPart>,
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
enum ContentPart {
    Text {
        text: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: FunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: FunctionResponse,
    },
}

#[derive(Clone, Serialize)]
struct FunctionCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
    args: serde_json::Value,
}

#[derive(Clone, Serialize)]
struct FunctionResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
    response: serde_json::Value,
}

#[derive(Clone, Serialize)]
pub(crate) struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<FunctionDeclaration>,
}

#[derive(Clone, Serialize)]
struct FunctionDeclaration {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    parameters: serde_json::Value,
}

#[derive(Clone, Serialize)]
pub(crate) struct ToolConfig {
    #[serde(rename = "functionCallingConfig")]
    function_calling_config: FunctionCallingConfig,
}

#[derive(Clone, Serialize)]
struct FunctionCallingConfig {
    mode: &'static str,
    #[serde(
        rename = "allowedFunctionNames",
        skip_serializing_if = "Option::is_none"
    )]
    allowed_function_names: Option<Vec<String>>,
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
    #[serde(rename = "functionCall")]
    function_call: Option<ResponseFunctionCall>,
    #[serde(rename = "functionResponse")]
    function_response: Option<serde_json::Value>,
    #[serde(rename = "toolCall")]
    server_tool_call: Option<serde_json::Value>,
    #[serde(rename = "toolResponse")]
    server_tool_response: Option<serde_json::Value>,
    #[serde(rename = "thoughtSignature")]
    thought_signature: Option<String>,
}

#[derive(Deserialize)]
struct ResponseFunctionCall {
    id: Option<String>,
    name: Option<String>,
    args: Option<serde_json::Value>,
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
    use lettuce_conversations::{
        ContextAttributions, ContextBudgetReport, ProviderNeutralMessage, ToolDefinition,
        ToolOutput, TranscriptToolCall, TranscriptToolResult,
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

    fn reasoning_parameters() -> ResolvedChatParameters {
        ResolvedChatParameters {
            reasoning_mode: Some(ReasoningMode::Enabled),
            reasoning_effort: Some(ReasoningEffort::Medium),
            reasoning_budget_tokens: Some(8192),
            ..crate::integration_tests::parameters()
        }
    }

    fn test_profile() -> ResolvedChatProfile {
        ResolvedChatProfile {
            model_profile_id: lettuce_types::ModelProfileId::new(),
            model_revision: lettuce_types::Revision::INITIAL,
            provider_account_id: lettuce_types::ProviderAccountId::new(),
            provider_account_revision: lettuce_types::Revision::INITIAL,
            secret_owner_id: lettuce_settings::SecretOwnerId::new(),
            external_model_id: "gemini-2.0-flash".to_owned(),
            provider_kind: "gemini".to_owned(),
            provider_protocol: ProviderProtocol::Gemini,
            endpoint: Some("https://generativelanguage.googleapis.com/v1beta".to_owned()),
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
            definitions: vec![ToolDefinition {
                name: "lookup_weather".to_owned(),
                description: Some("Get current weather".to_owned()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {"city": {"type": "string"}},
                    "required": ["city"]
                }),
                version: 1,
            }],
            choice,
        }
    }

    #[test]
    fn encodes_native_tools_choices_and_complete_replay() {
        let execution_id = lettuce_types::ToolExecutionId::new();
        let context = ProviderNeutralContext {
            messages: vec![
                ProviderNeutralMessage {
                    role: MessageRole::User,
                    parts: vec![ProviderContextPart::Text {
                        text: "Weather?".to_owned(),
                    }],
                },
                ProviderNeutralMessage {
                    role: MessageRole::Assistant,
                    parts: vec![
                        ProviderContextPart::Text {
                            text: "Checking".to_owned(),
                        },
                        ProviderContextPart::ToolCall(TranscriptToolCall {
                            execution_id,
                            provider_call_id: Some("call-1".to_owned()),
                            name: "lookup_weather".to_owned(),
                            arguments: serde_json::json!({"city": "Istanbul"}),
                            raw_arguments: None,
                            provider_replay: None,
                        }),
                    ],
                },
                ProviderNeutralMessage {
                    role: MessageRole::User,
                    parts: vec![
                        ProviderContextPart::ToolResult(TranscriptToolResult {
                            execution_id,
                            provider_call_id: Some("call-1".to_owned()),
                            name: "lookup_weather".to_owned(),
                            output: ToolOutput {
                                value: serde_json::json!({"temperature": 18}),
                                is_error: false,
                            },
                        }),
                        ProviderContextPart::Text {
                            text: "Use Celsius".to_owned(),
                        },
                    ],
                },
            ],
            attributions: ContextAttributions::default(),
            budget: ContextBudgetReport::default(),
        };
        context.validate().expect("tool context");
        let tools = tool_request(ToolChoice::Named {
            name: "lookup_weather".to_owned(),
        });
        let body = serde_json::to_value(
            build_request(&test_profile(), &context, Some(&tools)).expect("request"),
        )
        .expect("json");
        assert_eq!(
            body["contents"],
            serde_json::json!([
                {"role":"user","parts":[{"text":"Weather?"}]},
                {"role":"model","parts":[
                    {"text":"Checking"},
                    {"functionCall":{"id":"call-1","name":"lookup_weather","args":{"city":"Istanbul"}}}
                ]},
                {"role":"user","parts":[
                    {"functionResponse":{"id":"call-1","name":"lookup_weather","response":{"temperature":18}}},
                    {"text":"Use Celsius"}
                ]}
            ])
        );
        assert_eq!(
            body["tools"],
            serde_json::json!([{"functionDeclarations":[{
                "name":"lookup_weather",
                "description":"Get current weather",
                "parameters":{
                    "type":"object",
                    "properties":{"city":{"type":"string"}},
                    "required":["city"]
                }
            }]}])
        );
        assert_eq!(
            body["toolConfig"],
            serde_json::json!({"functionCallingConfig":{
                "mode":"ANY",
                "allowedFunctionNames":["lookup_weather"]
            }})
        );
    }

    #[test]
    fn rejects_incomplete_replay_and_wraps_error_results() {
        let execution_id = lettuce_types::ToolExecutionId::new();
        let call = ProviderNeutralMessage {
            role: MessageRole::Assistant,
            parts: vec![ProviderContextPart::ToolCall(TranscriptToolCall {
                execution_id,
                provider_call_id: None,
                name: "lookup_weather".to_owned(),
                arguments: serde_json::json!({}),
                raw_arguments: None,
                provider_replay: None,
            })],
        };
        let incomplete = ProviderNeutralContext {
            messages: vec![call.clone()],
            attributions: ContextAttributions::default(),
            budget: ContextBudgetReport::default(),
        };
        assert!(build_request(&test_profile(), &incomplete, None).is_err());

        let result = ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::ToolResult(TranscriptToolResult {
                execution_id,
                provider_call_id: None,
                name: "lookup_weather".to_owned(),
                output: ToolOutput {
                    value: serde_json::json!("offline"),
                    is_error: true,
                },
            })],
        };
        let complete = ProviderNeutralContext {
            messages: vec![call, result],
            attributions: ContextAttributions::default(),
            budget: ContextBudgetReport::default(),
        };
        let body = serde_json::to_value(
            build_request(&test_profile(), &complete, None).expect("complete replay"),
        )
        .expect("json");
        assert_eq!(
            body["contents"][1]["parts"][0]["functionResponse"]["response"],
            serde_json::json!({"error":"offline"})
        );

        let second_execution_id = lettuce_types::ToolExecutionId::new();
        let ambiguous = ProviderNeutralContext {
            messages: vec![
                ProviderNeutralMessage {
                    role: MessageRole::Assistant,
                    parts: vec![
                        ProviderContextPart::ToolCall(TranscriptToolCall {
                            execution_id,
                            provider_call_id: None,
                            name: "lookup_weather".to_owned(),
                            arguments: serde_json::json!({"city":"Paris"}),
                            raw_arguments: None,
                            provider_replay: None,
                        }),
                        ProviderContextPart::ToolCall(TranscriptToolCall {
                            execution_id: second_execution_id,
                            provider_call_id: None,
                            name: "lookup_weather".to_owned(),
                            arguments: serde_json::json!({"city":"London"}),
                            raw_arguments: None,
                            provider_replay: None,
                        }),
                    ],
                },
                ProviderNeutralMessage {
                    role: MessageRole::User,
                    parts: vec![
                        ProviderContextPart::ToolResult(TranscriptToolResult {
                            execution_id,
                            provider_call_id: None,
                            name: "lookup_weather".to_owned(),
                            output: ToolOutput {
                                value: serde_json::json!({"temperature": 18}),
                                is_error: false,
                            },
                        }),
                        ProviderContextPart::ToolResult(TranscriptToolResult {
                            execution_id: second_execution_id,
                            provider_call_id: None,
                            name: "lookup_weather".to_owned(),
                            output: ToolOutput {
                                value: serde_json::json!({"temperature": 17}),
                                is_error: false,
                            },
                        }),
                    ],
                },
            ],
            attributions: ContextAttributions::default(),
            budget: ContextBudgetReport::default(),
        };
        ambiguous.validate().expect("domain-valid transcript");
        assert!(build_request(&test_profile(), &ambiguous, None).is_err());
    }

    #[test]
    fn parses_unsigned_native_calls_without_fabricating_ids() {
        let outcome = parse_response(response(
            r#"{"candidates":[{"content":{"parts":[{"text":"checking"},{"functionCall":{"name":"lookup_weather","args":{"city":"Istanbul"}}}],"role":"model"},"finishReason":"STOP"}]}"#,
        ))
        .expect("tool response");
        let call = &outcome.candidates[0].tool_calls[0];
        assert_eq!(call.provider_call_id, None);
        assert_eq!(call.name, "lookup_weather");
        assert_eq!(call.arguments, serde_json::json!({"city":"Istanbul"}));
        assert_eq!(
            outcome.candidates[0].parts,
            vec![MessagePart::Text {
                text: "checking".to_owned()
            }]
        );
    }

    #[test]
    fn rejects_signed_or_malformed_native_calls() {
        assert_eq!(
            parse_response(response(
                r#"{"candidates":[{"content":{"parts":[{"functionCall":{"id":"call-1","name":"lookup_weather","args":{}},"thoughtSignature":"opaque"}]},"finishReason":"STOP"}]}"#,
            )),
            Err(AdapterError::MalformedResponse)
        );
        assert_eq!(
            parse_response(response(
                r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"lookup_weather","args":"bad"}}]},"finishReason":"STOP"}]}"#,
            )),
            Err(AdapterError::MalformedResponse)
        );
        assert_eq!(
            parse_response(response(
                r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"lookup_weather","args":{}}}]},"finishReason":"MALFORMED_FUNCTION_CALL"}]}"#,
            )),
            Err(AdapterError::MalformedResponse)
        );
        assert_eq!(
            parse_response(response(
                r#"{"candidates":[{"content":{"parts":[{"functionCall":{"id":"call-1","name":"lookup_weather","args":{}}},{"functionCall":{"name":"lookup_weather","args":{}}}]},"finishReason":"STOP"}]}"#,
            )),
            Err(AdapterError::MalformedResponse)
        );
    }

    #[test]
    fn rejects_tool_modes_that_require_unavailable_signed_replay() {
        let tools = tool_request(ToolChoice::Auto);
        let mut profile = test_profile();
        profile.external_model_id = "gemini-3-flash-preview".to_owned();
        assert_eq!(
            validate_tool_features(&profile, Some(&tools)),
            Err(AdapterError::Rejected)
        );

        profile.external_model_id = "gemini-2.5-flash".to_owned();
        profile.parameters = reasoning_parameters();
        assert_eq!(
            validate_tool_features(&profile, Some(&tools)),
            Err(AdapterError::Rejected)
        );
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
