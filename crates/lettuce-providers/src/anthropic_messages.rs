use std::borrow::Cow;

use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceRequest, InferenceUsage,
    InferenceWarningCode, MessagePart, MessageRole, ProviderContextPart, ProviderNeutralContext,
};
use lettuce_models::{
    PromptCacheRetention, PromptCaching, ProviderAccount, ProviderConfig, ResolvedChatParameters,
    ResolvedChatProfile,
};
use lettuce_network::{JsonClient, JsonResponse, JsonStaticHeader, MAX_REQUEST_BYTES};
use lettuce_settings::SecretStore;
use serde::{Deserialize, Serialize};

use crate::common::{
    AdapterError, AuthPlan, Credentials, RemoteModel, decode_json, generation_policy, load_auth,
    load_secret_headers, max_output_tokens, reject_unsupported_features, validate_common_request,
    validate_prompt_caching,
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
        reject_unsupported_features(parameters)?;
        validate_prompt_caching(self.descriptor().prompt_caching, parameters)
    }
}

pub(crate) async fn run<S: SecretStore + ?Sized>(
    provider: &dyn AnthropicWireProvider,
    secret_store: &S,
    network: &JsonClient,
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
    let path = provider.chat_path(endpoint, config)?;
    let body = encode_request(
        profile,
        &request.context,
        provider.merges_same_role(config),
        provider.roles(config),
    )?;
    let credentials = Credentials::from(profile);
    let auth = load_auth(provider.auth(config)?, secret_store, &credentials).await?;
    let secret_headers = load_secret_headers(secret_store, &credentials).await?;
    let response = network
        .post_json(
            endpoint,
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
    text: String,
}

fn encode_request(
    profile: &ResolvedChatProfile,
    context: &ProviderNeutralContext,
    merge_same_role: bool,
    (user_role, assistant_role): (Cow<'static, str>, Cow<'static, str>),
) -> Result<Vec<u8>, AdapterError> {
    let mut system_parts: Vec<String> = Vec::new();
    let mut turns: Vec<Turn> = Vec::new();
    for message in &context.messages {
        let mut text = String::new();
        for part in &message.parts {
            match part {
                ProviderContextPart::Text { text: fragment } => text.push_str(fragment),
                ProviderContextPart::MediaAsset { .. } => return Err(AdapterError::Rejected),
            }
        }
        match message.role {
            MessageRole::System | MessageRole::Scene => {
                if !text.is_empty() {
                    system_parts.push(text);
                }
            }
            MessageRole::User | MessageRole::Assistant => {
                let assistant = message.role == MessageRole::Assistant;
                match turns.last_mut() {
                    Some(last) if merge_same_role && last.assistant == assistant => {
                        if !last.text.is_empty() {
                            last.text.push_str("\n\n");
                        }
                        last.text.push_str(&text);
                    }
                    _ => turns.push(Turn { assistant, text }),
                }
            }
        }
    }
    let mut messages = turns
        .into_iter()
        .filter(|turn| !turn.text.trim().is_empty())
        .map(|turn| WireMessage {
            role: if turn.assistant {
                assistant_role.clone()
            } else {
                user_role.clone()
            },
            content: vec![WireTextBlock {
                kind: "text",
                text: turn.text,
                cache_control: None,
            }],
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
        if let Some(last_text) = last_user.content.last_mut() {
            last_text.cache_control = Some(control);
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
        max_tokens: max_output_tokens(parameters),
        stream: false,
        temperature: parameters.temperature,
        top_p: parameters.top_p,
        top_k: parameters.top_k,
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
    let header_request_id = response.request_id.clone();
    let parsed: MessagesResponse =
        serde_json::from_slice(&response.body).map_err(|_| AdapterError::MalformedResponse)?;
    let mut text = String::new();
    let mut warnings = Vec::new();
    for block in parsed.content {
        match block.kind.as_str() {
            "text" => text.push_str(block.text.as_deref().unwrap_or_default()),
            "tool_use" | "server_tool_use" => {
                push(&mut warnings, InferenceWarningCode::ProviderDegraded)
            }
            _ => {}
        }
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
        Some("tool_use") | Some("pause_turn") => {
            push(&mut warnings, InferenceWarningCode::ProviderDegraded);
            FinishReason::Stop
        }
        _ => FinishReason::Stop,
    };
    if text.trim().is_empty()
        && !warnings.contains(&InferenceWarningCode::SafetyTransformed)
        && !warnings.contains(&InferenceWarningCode::ProviderDegraded)
    {
        return Err(AdapterError::EmptyResponse);
    }
    Ok(InferenceOutcome {
        candidates: vec![InferenceCandidate {
            ordinal: 0,
            parts: if text.is_empty() {
                Vec::new()
            } else {
                vec![MessagePart::Text { text }]
            },
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
    })
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
}

#[derive(Serialize)]
struct WireMessage {
    role: Cow<'static, str>,
    content: Vec<WireTextBlock>,
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
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_conversations::{ProviderFailure, ProviderFailureKind};

    fn response(body: &str) -> JsonResponse {
        JsonResponse {
            status: 200,
            body: body.as_bytes().to_vec(),
            request_id: None,
            retry_after: None,
        }
    }

    #[test]
    fn joins_text_blocks_and_maps_stop_reasons() {
        let outcome = parse_response(response(
            r#"{"content":[{"type":"text","text":"a"},{"type":"thinking","thinking":"x"},{"type":"text","text":"b"}],"stop_reason":"max_tokens","usage":{"input_tokens":3,"output_tokens":5}}"#,
        ))
        .expect("response");
        assert_eq!(
            outcome.candidates[0].parts,
            vec![MessagePart::Text {
                text: "ab".to_owned()
            }]
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
