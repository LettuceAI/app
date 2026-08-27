use std::borrow::Cow;

use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceRequest, InferenceUsage,
    InferenceWarningCode, MessagePart, MessageRole, ProviderContextPart, ProviderNeutralContext,
};
use lettuce_models::{ProviderAccount, ProviderConfig, ResolvedChatProfile};
use lettuce_network::{JsonClient, JsonResponse, MAX_REQUEST_BYTES};
use lettuce_settings::SecretStore;
use serde::{Deserialize, Serialize};

use crate::common::{
    AdapterError, AuthPlan, Credentials, RemoteModel, STANDARD_HEADERS, decode_json,
    generation_policy, load_auth, load_secret_headers, max_output_tokens,
    reject_unsupported_features, validate_common_request, validate_prompt_caching,
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
    request: InferenceRequest,
) -> Result<InferenceOutcome, AdapterError> {
    validate_common_request(&request)?;
    let profile = &request.profile.chat_profile;
    if !matches!(profile.provider_config, ProviderConfig::Standard) {
        return Err(AdapterError::Rejected);
    }
    reject_unsupported_features(&profile.parameters)?;
    validate_prompt_caching(DESCRIPTOR.prompt_caching, &profile.parameters)?;
    let base = api_base(profile.endpoint.as_deref().unwrap_or(DEFAULT_ENDPOINT));
    let body = encode_request(profile, &request.context)?;
    let credentials = Credentials::from(profile);
    let auth = load_auth(AuthPlan::OptionalBearer, secret_store, &credentials).await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    let response = network
        .post_json(
            &base,
            "/api/chat",
            body,
            &STANDARD_HEADERS,
            auth,
            secret_headers,
            generation_policy(&credentials),
        )
        .await?;
    parse_response(response)
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
}

fn wire_messages(context: &ProviderNeutralContext) -> Result<Vec<WireMessage>, AdapterError> {
    context
        .messages
        .iter()
        .map(|message| {
            let mut content = String::new();
            for part in &message.parts {
                match part {
                    ProviderContextPart::Text { text } => content.push_str(text),
                    ProviderContextPart::MediaAsset { .. } => return Err(AdapterError::Rejected),
                }
            }
            Ok(WireMessage {
                role: match message.role {
                    MessageRole::System | MessageRole::Scene => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                },
                content,
            })
        })
        .collect()
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
) -> Result<Vec<u8>, AdapterError> {
    let parameters = &profile.parameters;
    let request = ChatRequest {
        model: profile.external_model_id.clone(),
        messages: normalize_system_messages(wire_messages(context)?),
        stream: false,
        options: options(parameters),
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
    let text = parsed
        .message
        .and_then(|message| message.content)
        .unwrap_or_default();
    let mut warnings = Vec::new();
    let finish_reason = match parsed.done_reason.as_deref() {
        Some("length") => {
            warnings.push(InferenceWarningCode::Truncated);
            FinishReason::Length
        }
        _ => FinishReason::Stop,
    };
    if text.trim().is_empty() {
        return Err(AdapterError::EmptyResponse);
    }
    Ok(InferenceOutcome {
        candidates: vec![InferenceCandidate {
            ordinal: 0,
            parts: vec![MessagePart::Text { text }],
            provider_replay: None,
        }],
        usage: match (parsed.prompt_eval_count, parsed.eval_count) {
            (Some(input_tokens), Some(output_tokens)) => Some(InferenceUsage {
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
    options: Options,
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
    done_reason: Option<String>,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: &'static str, content: &str) -> WireMessage {
        WireMessage {
            role,
            content: content.to_owned(),
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
}
