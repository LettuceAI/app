use std::borrow::Cow;

use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceRequest, InferenceUsage,
    InferenceWarningCode, MessagePart, MessageRole, ProviderContextPart, ProviderNeutralContext,
};
use lettuce_models::{
    ProviderAccount, ProviderConfig, ResolvedChatParameters, ResolvedChatProfile,
};
use lettuce_network::{JsonClient, JsonResponse, JsonStaticHeader, MAX_REQUEST_BYTES};
use lettuce_settings::{HeaderName, SecretStore};
use serde::{Deserialize, Serialize};

use crate::common::{
    AdapterError, AuthPlan, Credentials, RemoteModel, decode_json, generation_policy, load_auth,
    load_secret_headers, max_output_tokens, reject_unsupported_features, validate_common_request,
};
use crate::descriptor::ProviderDescriptor;

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
        reject_unsupported_features(parameters)
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
    secret_store: &S,
    network: &JsonClient,
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
    let body = encode_request(profile, &request.context)?;
    let credentials = Credentials::from(profile);
    let auth = load_auth(
        provider.auth(&profile.provider_config)?,
        secret_store,
        &credentials,
    )
    .await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    let response = network
        .post_json(
            &base,
            &path,
            body,
            provider.static_headers(),
            auth,
            secret_headers,
            generation_policy(&credentials),
        )
        .await?;
    parse_response(response)
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

fn encode_request(
    profile: &ResolvedChatProfile,
    context: &ProviderNeutralContext,
) -> Result<Vec<u8>, AdapterError> {
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
        },
    };
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
        for part in candidate
            .content
            .map(|content| content.parts)
            .unwrap_or_default()
        {
            if part.thought.unwrap_or(false) {
                continue;
            }
            if let Some(fragment) = part.text {
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
        has_content |= !text.trim().is_empty();
        candidates.push(InferenceCandidate {
            ordinal,
            parts: if text.is_empty() {
                Vec::new()
            } else {
                vec![MessagePart::Text { text }]
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

#[derive(Serialize)]
struct GenerateRequest {
    contents: Vec<Content>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct Content {
    role: &'static str,
    parts: Vec<TextPart>,
}

#[derive(Serialize)]
struct TextPart {
    text: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(rename = "topP", skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
    #[serde(rename = "topK", skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
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

    #[test]
    fn skips_thought_parts_and_reads_native_usage() {
        let outcome = parse_response(response(
            r#"{"candidates":[{"content":{"parts":[{"text":"hidden","thought":true},{"text":"vis"},{"text":"ible"}],"role":"model"},"finishReason":"MAX_TOKENS"}],"usageMetadata":{"promptTokenCount":9,"candidatesTokenCount":4,"totalTokenCount":13}}"#,
        ))
        .expect("response");
        assert_eq!(
            outcome.candidates[0].parts,
            vec![MessagePart::Text {
                text: "visible".to_owned()
            }]
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
