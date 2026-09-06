use std::borrow::Cow;

use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceRequest, InferenceUsage,
    InferenceWarningCode, MessagePart, MessageRole, ProposedToolCall, ProviderContextPart,
    ProviderNeutralContext, ToolChoice, ToolRequest,
};
use lettuce_inference::InferenceRuntimePort;
use lettuce_models::{
    CapabilityStatus, ProviderAccount, ProviderConfig, ReasoningEffort, ReasoningMode,
    ResolvedChatProfile,
};
use lettuce_network::{JsonClient, JsonResponse, MAX_REQUEST_BYTES};
use lettuce_settings::SecretStore;
use serde::{Deserialize, Serialize};

use crate::common::{
    AdapterError, AuthPlan, Credentials, RemoteModel, STANDARD_HEADERS, decode_json,
    generation_policy, load_auth, load_secret_headers, max_output_tokens,
    reject_unsupported_features, validate_common_request_with_tools, validate_prompt_caching,
    validate_supported_reasoning,
};
use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};

pub(crate) const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:11434";

pub(crate) fn api_base(endpoint: &str) -> Cow<'_, str> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Cow::Borrowed(DEFAULT_ENDPOINT);
    }
    let trimmed = trimmed.trim_end_matches('/');
    let stripped = trimmed
        .strip_suffix("/api/chat")
        .or_else(|| trimmed.strip_suffix("/api/tags"))
        .or_else(|| trimmed.strip_suffix("/v1"))
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    Cow::Borrowed(stripped)
}

pub(crate) async fn run<S: SecretStore + ?Sized>(
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
    validate_supported_reasoning(&profile.parameters)?;
    validate_tool_features(profile, request.tools.as_ref())?;
    if profile.parameters.reasoning_mode != Some(ReasoningMode::Enabled) {
        reject_unsupported_features(&profile.parameters)?;
    }
    validate_prompt_caching(DESCRIPTOR.prompt_caching, &profile.parameters)?;
    let base = api_base(profile.endpoint.as_deref().unwrap_or(DEFAULT_ENDPOINT));
    let streaming = request.stream_sink.is_some();
    if streaming && (!profile.streaming_enabled || !DESCRIPTOR.streaming) {
        return Err(AdapterError::Rejected);
    }
    let body = encode_request(profile, &request.context, request.tools.as_ref(), streaming)?;
    let credentials = Credentials::from(profile);
    let auth = load_auth(AuthPlan::OptionalBearer, secret_store, &credentials).await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    if streaming {
        let response = crate::streaming::await_cancelable(runtime, request.cancellation, async {
            network
                .post_json_stream(
                    &base,
                    "/api/chat",
                    body,
                    &STANDARD_HEADERS,
                    auth,
                    secret_headers,
                    generation_policy(&credentials),
                )
                .await
                .map_err(Into::into)
        })
        .await?;
        let outcome = crate::streaming::consume_stream(
            response,
            crate::stream_framing::StreamFormat::Ndjson,
            crate::stream_normalize::StreamProtocol::Ollama,
            runtime,
            &request,
        )
        .await?;
        validate_tool_outcome(&request, outcome)
    } else {
        let response = crate::streaming::await_cancelable(runtime, request.cancellation, async {
            network
                .post_json(
                    &base,
                    "/api/chat",
                    body,
                    &STANDARD_HEADERS,
                    auth,
                    secret_headers,
                    generation_policy(&credentials),
                )
                .await
                .map_err(Into::into)
        })
        .await?;
        validate_tool_outcome(&request, parse_response(response)?)
    }
}

fn validate_tool_features(
    profile: &ResolvedChatProfile,
    tools: Option<&ToolRequest>,
) -> Result<(), AdapterError> {
    let Some(tools) = tools else {
        return Ok(());
    };
    if profile.capabilities.tools == CapabilityStatus::Unsupported
        || !matches!(tools.choice, ToolChoice::Auto)
        || profile.parameters.reasoning_mode == Some(ReasoningMode::Enabled)
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
    if !calls.is_empty()
        && outcome.candidates.iter().any(|candidate| {
            candidate
                .parts
                .iter()
                .any(|part| matches!(part, MessagePart::ReasoningSummary { .. }))
        })
    {
        return Err(AdapterError::MalformedResponse);
    }
    let Some(tools) = &request.tools else {
        return if calls.is_empty() {
            Ok(outcome)
        } else {
            Err(AdapterError::MalformedResponse)
        };
    };
    if calls.iter().any(|call| {
        !tools
            .definitions
            .iter()
            .any(|definition| definition.name == call.name)
    }) {
        return Err(AdapterError::MalformedResponse);
    }
    Ok(outcome)
}

pub(crate) async fn list_models<S: SecretStore + ?Sized>(
    secret_store: &S,
    network: &JsonClient,
    account: &ProviderAccount,
) -> Result<Vec<RemoteModel>, AdapterError> {
    if !matches!(account.config, ProviderConfig::Standard) {
        return Err(AdapterError::Rejected);
    }
    let base = api_base(account.endpoint.as_deref().unwrap_or(DEFAULT_ENDPOINT));
    let credentials = Credentials::from(account);
    let auth = load_auth(AuthPlan::OptionalBearer, secret_store, &credentials).await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    let response = network
        .get_json(
            &base,
            "/api/tags",
            &STANDARD_HEADERS,
            auth,
            secret_headers,
            generation_policy(&credentials),
        )
        .await?;
    Ok(parse_models(&decode_json(&response)?))
}

pub(crate) fn parse_models(payload: &serde_json::Value) -> Vec<RemoteModel> {
    payload
        .get("models")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item.get("name")?.as_str()?;
                    Some(RemoteModel {
                        id: name.to_owned(),
                        display_name: Some(name.to_owned()),
                        description: item
                            .get("details")
                            .and_then(|details| details.get("parameter_size"))
                            .and_then(serde_json::Value::as_str)
                            .map(|size| format!("{size} parameters"))
                            .or_else(|| {
                                item.get("size")
                                    .and_then(serde_json::Value::as_u64)
                                    .map(|size| format!("{size} bytes"))
                            }),
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

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "ollama",
    display_name: "Ollama (Local)",
    protocol: lettuce_models::ProviderProtocol::Ollama,
    aliases: &[],
    default_endpoint: Some(DEFAULT_ENDPOINT),
    endpoint_editable: true,
    api_key: ApiKeyRequirement::Optional,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: false,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags {
        repetition_penalty: true,
        ..ParameterFlags::PENALTIES_TOP_K_BUDGET
    },
    extra_body_keys: &["options"],
};

#[derive(Clone, Serialize, PartialEq, Debug)]
struct WireMessage {
    role: &'static str,
    content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<WireToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<String>,
}

fn wire_messages(context: &ProviderNeutralContext) -> Result<Vec<WireMessage>, AdapterError> {
    let mut out = Vec::new();
    let mut expected_results: Option<Vec<&lettuce_conversations::TranscriptToolCall>> = None;
    for message in &context.messages {
        let mut content = String::new();
        let mut calls = Vec::new();
        let mut results = Vec::new();
        for part in &message.parts {
            match part {
                ProviderContextPart::Text { text } => content.push_str(text),
                ProviderContextPart::ToolCall(call) => calls.push(call),
                ProviderContextPart::ToolResult(result) => results.push(result),
                ProviderContextPart::MediaAsset { .. } => return Err(AdapterError::Rejected),
            }
        }
        if let Some(expected) = expected_results.take() {
            if message.role != MessageRole::User || results.len() != expected.len() {
                return Err(AdapterError::Rejected);
            }
            for (result, call) in results.iter().zip(expected) {
                if result.execution_id != call.execution_id
                    || result.name != call.name
                    || result.provider_call_id != call.provider_call_id
                {
                    return Err(AdapterError::Rejected);
                }
                out.push(WireMessage {
                    role: "tool",
                    content: serde_json::to_string(&result.output.value)
                        .map_err(|_| AdapterError::Rejected)?,
                    tool_calls: Vec::new(),
                    tool_name: Some(result.name.clone()),
                });
            }
            if !calls.is_empty() {
                return Err(AdapterError::Rejected);
            }
            if !content.is_empty() {
                out.push(text_wire_message("user", content));
            }
            continue;
        }
        if !results.is_empty() {
            return Err(AdapterError::Rejected);
        }
        if !calls.is_empty() {
            if message.role != MessageRole::Assistant
                || calls.iter().any(|call| call.provider_replay.is_some())
            {
                return Err(AdapterError::Rejected);
            }
            expected_results = Some(calls.clone());
            out.push(WireMessage {
                role: "assistant",
                content,
                tool_calls: calls
                    .iter()
                    .enumerate()
                    .map(|(index, call)| WireToolCall {
                        id: call.provider_call_id.clone(),
                        kind: "function".to_owned(),
                        function: WireFunctionCall {
                            index,
                            name: call.name.clone(),
                            arguments: call.arguments.clone(),
                        },
                    })
                    .collect(),
                tool_name: None,
            });
            continue;
        }
        out.push(text_wire_message(
            match message.role {
                MessageRole::System | MessageRole::Scene => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
            },
            content,
        ));
    }
    if expected_results.is_some() {
        return Err(AdapterError::Rejected);
    }
    Ok(out)
}

fn text_wire_message(role: &'static str, content: String) -> WireMessage {
    WireMessage {
        role,
        content,
        tool_calls: Vec::new(),
        tool_name: None,
    }
}

fn normalize_system_messages(messages: Vec<WireMessage>) -> Vec<WireMessage> {
    let leading = messages
        .iter()
        .take_while(|message| message.role == "system")
        .count();
    let mut staged: Vec<(WireMessage, bool)> = Vec::with_capacity(messages.len());
    if leading == 1 {
        staged.push((messages[0].clone(), false));
    } else if leading > 1 {
        let merged = messages[..leading]
            .iter()
            .filter(|message| !message.content.is_empty())
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        staged.push((
            WireMessage {
                role: "system",
                content: merged,
                tool_calls: Vec::new(),
                tool_name: None,
            },
            false,
        ));
    }
    for message in messages.into_iter().skip(leading) {
        if message.role == "system" {
            staged.push((
                WireMessage {
                    role: "user",
                    content: message.content,
                    tool_calls: Vec::new(),
                    tool_name: None,
                },
                true,
            ));
        } else {
            staged.push((message, false));
        }
    }
    let mut out: Vec<WireMessage> = Vec::with_capacity(staged.len());
    let mut previous_was_demoted = false;
    for (message, was_demoted) in staged {
        let should_merge = message.role == "user"
            && out.last().is_some_and(|last| last.role == "user")
            && (was_demoted || previous_was_demoted);
        if should_merge {
            if let Some(previous) = out.last_mut() {
                if !previous.content.is_empty() && !message.content.is_empty() {
                    previous.content.push_str("\n\n");
                }
                previous.content.push_str(&message.content);
            }
            previous_was_demoted |= was_demoted;
        } else {
            out.push(message);
            previous_was_demoted = was_demoted;
        }
    }
    out
}

fn encode_request(
    profile: &ResolvedChatProfile,
    context: &ProviderNeutralContext,
    tools: Option<&ToolRequest>,
    streaming: bool,
) -> Result<Vec<u8>, AdapterError> {
    let parameters = &profile.parameters;
    let request = ChatRequest {
        model: profile.external_model_id.clone(),
        messages: normalize_system_messages(wire_messages(context)?),
        stream: streaming,
        think: ollama_think(parameters),
        options: options(parameters),
        tools: tools.map(|request| {
            request
                .definitions
                .iter()
                .map(|definition| WireToolDefinition {
                    kind: "function",
                    function: WireTool {
                        name: definition.name.clone(),
                        description: definition.description.clone(),
                        parameters: definition.parameters.clone(),
                    },
                })
                .collect()
        }),
    };
    let body = serde_json::to_vec(&request).map_err(|_| AdapterError::Rejected)?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err(AdapterError::Rejected);
    }
    Ok(body)
}

fn options(parameters: &lettuce_models::ResolvedChatParameters) -> Options {
    Options {
        temperature: parameters.temperature,
        top_p: parameters.top_p,
        top_k: parameters.top_k,
        frequency_penalty: parameters.frequency_penalty,
        presence_penalty: parameters.presence_penalty,
        num_ctx: parameters.context_length,
        num_predict: max_output_tokens(parameters),
        num_keep: parameters.ollama.num_keep,
        num_batch: parameters.ollama.num_batch,
        num_gpu: parameters.ollama.num_gpu,
        num_thread: parameters.ollama.num_thread,
        tfs_z: parameters.ollama.tfs_z,
        typical_p: parameters.ollama.typical_p,
        min_p: parameters.ollama.min_p,
        mirostat: parameters.ollama.mirostat,
        mirostat_tau: parameters.ollama.mirostat_tau,
        mirostat_eta: parameters.ollama.mirostat_eta,
        repeat_penalty: parameters.repetition_penalty,
        seed: parameters.ollama.seed,
        stop: parameters.ollama.stop.clone(),
    }
}

fn parse_response(response: JsonResponse) -> Result<InferenceOutcome, AdapterError> {
    if let Some(error) = AdapterError::from_response(&response) {
        return Err(error);
    }
    let provider_request_id = response.request_id.clone();
    let parsed: ChatResponse =
        serde_json::from_slice(&response.body).map_err(|_| AdapterError::MalformedResponse)?;
    let message = parsed.message.unwrap_or_default();
    let (text, tagged_reasoning) = crate::stream_normalize::split_complete_thinking(
        message.content.as_deref().unwrap_or_default(),
    );
    let explicit_reasoning = [
        message.thinking.as_deref(),
        message.reasoning.as_deref(),
        message.reasoning_content.as_deref(),
    ]
    .into_iter()
    .flatten();
    let reasoning =
        crate::stream_normalize::merge_complete_reasoning(tagged_reasoning, explicit_reasoning);
    let mut warnings = Vec::new();
    let finish_reason = match parsed.done_reason.as_deref() {
        Some("length") => {
            warnings.push(InferenceWarningCode::Truncated);
            FinishReason::Length
        }
        Some("stop") | None => FinishReason::Stop,
        Some(_) => {
            warnings.push(InferenceWarningCode::ProviderDegraded);
            FinishReason::Stop
        }
    };
    if parsed.done != Some(true) {
        return Err(AdapterError::MalformedResponse);
    }
    let tool_calls = parse_tool_calls(message.tool_calls)?;
    if text.trim().is_empty() && reasoning.trim().is_empty() && tool_calls.is_empty() {
        return Err(AdapterError::EmptyResponse);
    }
    let mut parts = Vec::new();
    if !reasoning.is_empty() {
        parts.push(MessagePart::ReasoningSummary { text: reasoning });
    }
    if !text.is_empty() {
        parts.push(MessagePart::Text { text });
    }
    Ok(InferenceOutcome {
        candidates: vec![InferenceCandidate {
            ordinal: 0,
            parts,
            tool_calls,
            provider_replay: None,
        }],
        usage: match (parsed.prompt_eval_count, parsed.eval_count) {
            (Some(input_tokens), Some(output_tokens)) => Some(InferenceUsage {
                provider_reported_cost: None,
                cache_write_tokens: None,
                web_search_requests: None,
                cached_input_tokens: None,
                reasoning_tokens: None,
                input_tokens,
                output_tokens,
            }),
            _ => None,
        },
        finish_reason,
        provider_finish_reason: parsed.done_reason,
        provider_request_id,
        warning_codes: warnings,
    })
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<WireMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    think: Option<OllamaThink>,
    options: Options,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<WireToolDefinition>>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
struct WireToolCall {
    #[serde(skip_serializing)]
    id: Option<String>,
    #[serde(rename = "type", default = "function_kind")]
    kind: String,
    function: WireFunctionCall,
}

fn function_kind() -> String {
    "function".to_owned()
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
struct WireFunctionCall {
    #[serde(default)]
    index: usize,
    name: String,
    arguments: serde_json::Value,
}

#[derive(Serialize)]
struct WireToolDefinition {
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireTool,
}

#[derive(Serialize)]
struct WireTool {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
#[serde(untagged)]
enum OllamaThink {
    Enabled(bool),
    Effort(&'static str),
}

fn ollama_think(parameters: &lettuce_models::ResolvedChatParameters) -> Option<OllamaThink> {
    if parameters.reasoning_mode != Some(ReasoningMode::Enabled) {
        return None;
    }
    Some(match parameters.reasoning_effort {
        Some(effort) => OllamaThink::Effort(match effort {
            ReasoningEffort::Low => "low",
            ReasoningEffort::Medium => "medium",
            ReasoningEffort::High => "high",
        }),
        None => OllamaThink::Enabled(true),
    })
}

#[derive(Serialize)]
struct Options {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_ctx: Option<u32>,
    num_predict: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_keep: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_batch: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_gpu: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_thread: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tfs_z: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    typical_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mirostat: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mirostat_tau: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mirostat_eta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeat_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: Option<ResponseMessage>,
    done: Option<bool>,
    done_reason: Option<String>,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
}

#[derive(Default, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    thinking: Option<String>,
    reasoning: Option<String>,
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<WireToolCall>,
}

fn parse_tool_calls(calls: Vec<WireToolCall>) -> Result<Vec<ProposedToolCall>, AdapterError> {
    if calls.len() > lettuce_conversations::MAX_TOOL_CALLS_PER_RESPONSE {
        return Err(AdapterError::MalformedResponse);
    }
    calls
        .into_iter()
        .map(|call| {
            if call.kind != "function" {
                return Err(AdapterError::MalformedResponse);
            }
            let proposal = ProposedToolCall {
                provider_call_id: call.id,
                name: call.function.name,
                arguments: call.function.arguments,
                raw_arguments: None,
                provider_replay: None,
            };
            proposal
                .validate()
                .map_err(|_| AdapterError::MalformedResponse)?;
            Ok(proposal)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_conversations::{
        ContextAttributions, ContextBudgetReport, ProviderNeutralMessage, ToolOutput,
        TranscriptToolCall, TranscriptToolResult,
    };

    fn message(role: &'static str, content: &str) -> WireMessage {
        WireMessage {
            role,
            content: content.to_owned(),
            tool_calls: Vec::new(),
            tool_name: None,
        }
    }

    #[test]
    fn normalizes_system_placement_like_legacy() {
        let out = normalize_system_messages(vec![
            message("system", "main"),
            message("user", "hi"),
            message("system", "author note"),
            message("assistant", "ok"),
        ]);
        assert_eq!(
            out,
            vec![
                message("system", "main"),
                message("user", "hi\n\nauthor note"),
                message("assistant", "ok"),
            ]
        );
        let out = normalize_system_messages(vec![
            message("user", "hi"),
            message("system", "be nice"),
            message("user", "hello again"),
        ]);
        assert_eq!(out, vec![message("user", "hi\n\nbe nice\n\nhello again")]);
        let out = normalize_system_messages(vec![
            message("assistant", "first"),
            message("system", "be nice"),
            message("assistant", "second"),
        ]);
        assert_eq!(out[1], message("user", "be nice"));
        let out = normalize_system_messages(vec![
            message("system", "first"),
            message("system", "second"),
            message("user", "hi"),
        ]);
        assert_eq!(
            out,
            vec![message("system", "first\n\nsecond"), message("user", "hi")]
        );
    }

    #[test]
    fn strips_known_suffixes_from_the_endpoint() {
        assert_eq!(
            api_base("http://127.0.0.1:11434/v1/"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(api_base("http://host:11434/api/chat"), "http://host:11434");
        assert_eq!(api_base("  "), DEFAULT_ENDPOINT);
    }

    #[test]
    fn parses_native_response_and_usage() {
        let outcome = parse_response(JsonResponse {
            status: 200,
            body: br#"{"model":"m","message":{"role":"assistant","content":"hey"},"done":true,"done_reason":"length","prompt_eval_count":11,"eval_count":2}"#.to_vec(),
            request_id: None,
            retry_after: None,
        })
        .expect("response");
        assert_eq!(outcome.finish_reason, FinishReason::Length);
        assert_eq!(
            outcome.usage,
            Some(InferenceUsage {
                provider_reported_cost: None,
                cache_write_tokens: None,
                web_search_requests: None,
                cached_input_tokens: None,
                reasoning_tokens: None,
                input_tokens: 11,
                output_tokens: 2
            })
        );
        assert_eq!(
            parse_response(JsonResponse {
                status: 200,
                body: br#"{"message":{"content":""},"done":true}"#.to_vec(),
                request_id: None,
                retry_after: None,
            }),
            Err(AdapterError::EmptyResponse)
        );
    }

    #[test]
    fn encodes_ollama_think_without_using_the_reasoning_budget() {
        let mut parameters = crate::integration_tests::parameters();
        parameters.reasoning_mode = Some(ReasoningMode::Enabled);
        parameters.reasoning_budget_tokens = Some(8192);
        assert_eq!(
            serde_json::to_value(ollama_think(&parameters)).expect("think json"),
            serde_json::json!(true)
        );
        assert_eq!(options(&parameters).num_predict, 4096);

        parameters.reasoning_effort = Some(ReasoningEffort::High);
        assert_eq!(
            serde_json::to_value(ollama_think(&parameters)).expect("think json"),
            serde_json::json!("high")
        );
        parameters.reasoning_mode = Some(ReasoningMode::Disabled);
        assert!(ollama_think(&parameters).is_none());
    }

    #[test]
    fn buffered_ollama_preserves_native_and_tagged_reasoning() {
        let outcome = parse_response(JsonResponse {
            status: 200,
            body: br#"{"message":{"content":"<think>tagged</think>visible","thinking":"native"},"done":true}"#.to_vec(),
            request_id: None,
            retry_after: None,
        })
        .expect("response");
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

        let outcome = parse_response(JsonResponse {
            status: 200,
            body: br#"{"message":{"content":"<think>same</think>visible","thinking":"same","reasoning":"second"},"done":true}"#.to_vec(),
            request_id: None,
            retry_after: None,
        })
        .expect("response");
        assert_eq!(
            outcome.candidates[0].parts[0],
            MessagePart::ReasoningSummary {
                text: "same\n\nsecond".to_owned()
            }
        );
    }

    #[test]
    fn encodes_all_legacy_ollama_options_with_specific_fallbacks() {
        let mut parameters = crate::integration_tests::parameters();
        parameters.repetition_penalty = Some(1.1);
        parameters.ollama = lettuce_models::OllamaOptions {
            num_keep: Some(32),
            num_batch: Some(128),
            num_gpu: Some(2),
            num_thread: Some(8),
            tfs_z: Some(0.9),
            typical_p: Some(0.8),
            min_p: Some(0.05),
            mirostat: Some(2),
            mirostat_tau: Some(5.0),
            mirostat_eta: Some(0.1),
            seed: Some(42),
            stop: Some(vec!["END".to_owned()]),
        };
        let options = serde_json::to_value(options(&parameters)).expect("options json");
        assert_eq!(options["num_ctx"], serde_json::json!(4096));
        assert_eq!(options["num_predict"], serde_json::json!(4096));
        assert_eq!(options["repeat_penalty"], serde_json::json!(1.1));
        assert_eq!(options["stop"], serde_json::json!(["END"]));
        assert_eq!(options.as_object().expect("options").len(), 16);
    }

    #[test]
    fn encodes_native_tool_replay_in_exact_call_order() {
        let first = lettuce_types::ToolExecutionId::new();
        let second = lettuce_types::ToolExecutionId::new();
        let context = ProviderNeutralContext {
            messages: vec![
                ProviderNeutralMessage {
                    role: MessageRole::Assistant,
                    parts: vec![
                        ProviderContextPart::Text {
                            text: "checking".to_owned(),
                        },
                        ProviderContextPart::ToolCall(TranscriptToolCall {
                            execution_id: first,
                            provider_call_id: None,
                            name: "lookup_weather".to_owned(),
                            arguments: serde_json::json!({"city":"Paris"}),
                            raw_arguments: None,
                            provider_replay: None,
                        }),
                        ProviderContextPart::ToolCall(TranscriptToolCall {
                            execution_id: second,
                            provider_call_id: Some("call-2".to_owned()),
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
                            execution_id: first,
                            provider_call_id: None,
                            name: "lookup_weather".to_owned(),
                            output: ToolOutput {
                                value: serde_json::json!({"temperature":18}),
                                is_error: false,
                            },
                        }),
                        ProviderContextPart::ToolResult(TranscriptToolResult {
                            execution_id: second,
                            provider_call_id: Some("call-2".to_owned()),
                            name: "lookup_weather".to_owned(),
                            output: ToolOutput {
                                value: serde_json::json!("offline"),
                                is_error: true,
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

        let messages = serde_json::to_value(wire_messages(&context).expect("wire messages"))
            .expect("message json");
        assert_eq!(
            messages,
            serde_json::json!([
                {"role":"assistant","content":"checking","tool_calls":[
                    {"type":"function","function":{"index":0,"name":"lookup_weather","arguments":{"city":"Paris"}}},
                    {"type":"function","function":{"index":1,"name":"lookup_weather","arguments":{"city":"London"}}}
                ]},
                {"role":"tool","content":"{\"temperature\":18}","tool_name":"lookup_weather"},
                {"role":"tool","content":"\"offline\"","tool_name":"lookup_weather"},
                {"role":"user","content":"Use Celsius"}
            ])
        );

        let incomplete = ProviderNeutralContext {
            messages: context.messages[..1].to_vec(),
            attributions: ContextAttributions::default(),
            budget: ContextBudgetReport::default(),
        };
        assert_eq!(wire_messages(&incomplete), Err(AdapterError::Rejected));
    }

    #[test]
    fn parses_native_tool_calls_without_fabricating_ids() {
        let outcome = parse_response(JsonResponse {
            status: 200,
            body: br#"{"message":{"content":"checking","tool_calls":[{"type":"function","function":{"index":0,"name":"lookup_weather","arguments":{"city":"Paris"}}},{"id":"call-2","type":"function","function":{"index":1,"name":"lookup_weather","arguments":{"city":"London"}}}]},"done":true,"done_reason":"stop"}"#.to_vec(),
            request_id: None,
            retry_after: None,
        })
        .expect("tool response");
        assert_eq!(outcome.candidates[0].tool_calls.len(), 2);
        assert_eq!(outcome.candidates[0].tool_calls[0].provider_call_id, None);
        assert_eq!(
            outcome.candidates[0].tool_calls[1]
                .provider_call_id
                .as_deref(),
            Some("call-2")
        );

        for body in [
            r#"{"message":{"tool_calls":[{"function":{"name":"lookup","arguments":[]}}]},"done":true}"#,
            r#"{"message":{"tool_calls":[{"function":{"name":"lookup","arguments":{}}}]},"done":false}"#,
        ] {
            assert_eq!(
                parse_response(JsonResponse {
                    status: 200,
                    body: body.as_bytes().to_vec(),
                    request_id: None,
                    retry_after: None,
                }),
                Err(AdapterError::MalformedResponse)
            );
        }
    }
}
