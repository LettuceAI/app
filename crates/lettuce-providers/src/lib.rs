//! Remote provider adapters.

#![deny(unsafe_op_in_unsafe_fn)]

mod anannas;
mod anthropic;
mod anthropic_messages;
mod catalog;
mod cerebras;
mod chutes;
mod common;
mod custom;
mod custom_anthropic;
mod deepseek;
mod descriptor;
mod featherless;
mod gemini;
mod gemini_cache;
mod gemini_express;
mod gemini_generate;
mod groq;
mod intenserp;
mod literouter;
mod lmstudio;
mod mistral;
mod moonshot;
mod nanogpt;
mod nvidia;
mod ollama;
mod openai;
mod openai_compatible;
mod openrouter;
mod pollinations;
mod qwen;
mod stream_framing;
mod stream_normalize;
mod streaming;
mod verify;
mod xai;
mod zai;

pub use catalog::{provider_descriptor, provider_descriptors};
pub use descriptor::{
    ApiKeyRequirement, KeyVerification, ParameterFlags, PromptCachingSupport, ProviderDescriptor,
    ProviderRequestError, ReasoningSupport, RemoteModel,
};

use std::{fmt, sync::Arc};

use anthropic_messages::AnthropicWireProvider;
use async_trait::async_trait;
use gemini_generate::GeminiWireProvider;
use lettuce_conversations::{InferenceOutcome, InferencePort, InferenceRequest, PortError};
use lettuce_inference::{InferenceRuntime, InferenceRuntimePort};
use lettuce_models::{ProviderAccount, ProviderProtocol};
use lettuce_network::JsonClient;
use lettuce_settings::SecretStore;
use openai_compatible::OpenAiWireProvider;

/// Explicit dispatch over every remote chat provider the legacy app shipped.
/// Each provider is one module implementing its family's wire trait plus one
/// arm here; unknown kinds are rejected, never defaulted to OpenAI.
pub struct RemoteProviders<S: ?Sized> {
    secret_store: Arc<S>,
    network: Arc<JsonClient>,
    runtime: Arc<dyn InferenceRuntimePort>,
    gemini_cache: gemini_cache::GeminiCache,
}

impl<S: SecretStore + ?Sized> RemoteProviders<S> {
    /// Fetches the provider's model catalog for one stored account
    /// (legacy `get_remote_models`).
    pub async fn list_models(
        &self,
        account: &ProviderAccount,
    ) -> Result<Vec<RemoteModel>, ProviderRequestError> {
        let kind = account.provider_kind.as_str();
        let store = &*self.secret_store;
        let result = match account.protocol {
            ProviderProtocol::OpenAiCompatible => {
                let provider = provider_for(kind).ok_or(ProviderRequestError::Unsupported)?;
                openai_compatible::list_models(provider, store, &self.network, account).await
            }
            ProviderProtocol::Anthropic => {
                let provider =
                    anthropic_provider_for(kind).ok_or(ProviderRequestError::Unsupported)?;
                anthropic_messages::list_models(provider, store, &self.network, account).await
            }
            ProviderProtocol::Gemini => {
                let provider =
                    gemini_provider_for(kind).ok_or(ProviderRequestError::Unsupported)?;
                gemini_generate::list_models(provider, store, &self.network, account).await
            }
            ProviderProtocol::Ollama if kind.eq_ignore_ascii_case("ollama") => {
                ollama::list_models(store, &self.network, account).await
            }
            _ => return Err(ProviderRequestError::Unsupported),
        };
        result.map_err(Into::into)
    }

    /// Probes the account's credential the way the legacy settings page did
    /// on save (legacy `verify_provider_api_key`).
    pub async fn verify_api_key(
        &self,
        account: &ProviderAccount,
    ) -> Result<KeyVerification, ProviderRequestError> {
        verify::verify_api_key(&*self.secret_store, &self.network, account)
            .await
            .map_err(Into::into)
    }
}

fn anthropic_provider_for(kind: &str) -> Option<&'static dyn AnthropicWireProvider> {
    match kind.to_ascii_lowercase().as_str() {
        "anthropic" => Some(&anthropic::Anthropic),
        "custom-anthropic" => Some(&custom_anthropic::CustomAnthropic),
        _ => None,
    }
}

fn gemini_provider_for(kind: &str) -> Option<&'static dyn GeminiWireProvider> {
    match kind.to_ascii_lowercase().as_str() {
        "gemini" | "google" | "google-gemini" => Some(&gemini::Gemini),
        "gemini-agent-platform-express" => Some(&gemini_express::GeminiExpress),
        _ => None,
    }
}

fn provider_for(kind: &str) -> Option<&'static dyn OpenAiWireProvider> {
    match kind.to_ascii_lowercase().as_str() {
        "openai" => Some(&openai::OpenAi),
        "openrouter" => Some(&openrouter::OpenRouter),
        "custom" => Some(&custom::Custom),
        "cerebras" | "cerebras.ai" => Some(&cerebras::Cerebras),
        "deepseek" => Some(&deepseek::DeepSeek),
        "groq" => Some(&groq::Groq),
        "xai" => Some(&xai::Xai),
        "mistral" => Some(&mistral::Mistral),
        "qwen" => Some(&qwen::Qwen),
        "featherless" => Some(&featherless::Featherless),
        "chutes" | "chutes.ai" => Some(&chutes::Chutes),
        "anannas" => Some(&anannas::Anannas),
        "nanogpt" => Some(&nanogpt::NanoGpt),
        "nvidia" | "nvidia-nim" => Some(&nvidia::Nvidia),
        "moonshot" | "moonshot-ai" => Some(&moonshot::Moonshot),
        "literouter" => Some(&literouter::LiteRouter),
        "intenserp" => Some(&intenserp::IntenseRp),
        "pollinations" => Some(&pollinations::Pollinations),
        "zai" | "z.ai" => Some(&zai::Zai),
        "lmstudio" => Some(&lmstudio::LmStudio),
        _ => None,
    }
}

impl<S: ?Sized> fmt::Debug for RemoteProviders<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RemoteProviders")
    }
}

impl<S: SecretStore + ?Sized> RemoteProviders<S> {
    pub fn new(secret_store: Arc<S>, network: Arc<JsonClient>) -> Self {
        Self::with_runtime(secret_store, network, Arc::new(InferenceRuntime::default()))
    }

    pub fn with_runtime(
        secret_store: Arc<S>,
        network: Arc<JsonClient>,
        runtime: Arc<dyn InferenceRuntimePort>,
    ) -> Self {
        Self {
            secret_store,
            network,
            runtime,
            gemini_cache: gemini_cache::GeminiCache::default(),
        }
    }
}

#[async_trait]
impl<S: SecretStore + ?Sized> InferencePort for RemoteProviders<S> {
    async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        let profile = &request.profile.chat_profile;
        let kind = profile.provider_kind.as_str();
        let result = match profile.provider_protocol {
            ProviderProtocol::OpenAiCompatible => {
                let provider = provider_for(kind).ok_or(PortError::Rejected)?;
                openai_compatible::run(
                    provider,
                    &*self.secret_store,
                    &self.network,
                    &*self.runtime,
                    request,
                )
                .await
            }
            ProviderProtocol::Anthropic => {
                let provider = anthropic_provider_for(kind).ok_or(PortError::Rejected)?;
                anthropic_messages::run(
                    provider,
                    &*self.secret_store,
                    &self.network,
                    &*self.runtime,
                    request,
                )
                .await
            }
            ProviderProtocol::Gemini => {
                let provider = gemini_provider_for(kind).ok_or(PortError::Rejected)?;
                gemini_generate::run(
                    provider,
                    &self.gemini_cache,
                    &*self.secret_store,
                    &self.network,
                    &*self.runtime,
                    request,
                )
                .await
            }
            ProviderProtocol::Ollama if kind.eq_ignore_ascii_case("ollama") => {
                ollama::run(&*self.secret_store, &self.network, &*self.runtime, request).await
            }
            ProviderProtocol::Ollama
            | ProviderProtocol::LlamaCpp
            | ProviderProtocol::StableDiffusion => return Err(PortError::Rejected),
        };
        result.map_err(Into::into)
    }
}

#[cfg(test)]
mod integration_tests {
    use std::sync::{Arc, Mutex};

    use crate::{KeyVerification, ProviderRequestError, RemoteProviders};
    use async_trait::async_trait;
    use lettuce_conversations::{
        ContextAttributions, ContextBudgetReport, GenerationOperation, GenerationStreamEvent,
        InferenceOutcome, InferencePort, InferenceRequest, MessagePart, MessageRole, OutputPolicy,
        ProviderContextPart, ProviderNeutralContext, ProviderNeutralMessage,
        ResolvedInferenceProfile, SafetyContext, ToolPolicy,
    };
    use lettuce_inference::{InferenceRuntime, InferenceRuntimePort};
    use lettuce_jobs::handle::JobHandle;
    use lettuce_models::{
        ChatProfileWarning, CustomAuth, CustomProviderConfig, ModelCapabilities, ProviderConfig,
        ProviderProtocol, ResolvedChatParameters, ResolvedChatProfile,
    };
    use lettuce_network::JsonClient;
    use lettuce_settings::{
        HeaderName, InMemorySecretStore, SecretPurpose, SecretRecord, SecretRef, SecretStatus,
        SecretStore, SecretStoreError, SecretValue,
    };
    use lettuce_types::{
        GenerationAttemptId, GenerationTurnId, JobId, ModelProfileId, ProviderAccountId, RequestId,
        Revision,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::oneshot,
    };

    #[derive(Clone, Default)]
    struct RecordingSecretStore {
        inner: InMemorySecretStore,
        loads: Arc<Mutex<Vec<SecretPurpose>>>,
    }

    impl RecordingSecretStore {
        fn take_loads(&self) -> Vec<SecretPurpose> {
            std::mem::take(&mut *self.loads.lock().expect("load lock"))
        }
    }

    #[async_trait]
    impl SecretStore for RecordingSecretStore {
        async fn put(
            &self,
            record: SecretRecord,
            value: SecretValue,
            expected_generation: Option<u64>,
        ) -> Result<SecretStatus, SecretStoreError> {
            self.inner.put(record, value, expected_generation).await
        }

        async fn load(
            &self,
            reference: &SecretRef,
            purpose: &SecretPurpose,
        ) -> Result<SecretValue, SecretStoreError> {
            self.loads.lock().expect("load lock").push(purpose.clone());
            self.inner.load(reference, purpose).await
        }

        async fn status(
            &self,
            reference: &SecretRef,
            purpose: &SecretPurpose,
        ) -> Result<SecretStatus, SecretStoreError> {
            self.inner.status(reference, purpose).await
        }

        async fn delete(
            &self,
            reference: &SecretRef,
            purpose: &SecretPurpose,
            expected_generation: Option<u64>,
        ) -> Result<SecretStatus, SecretStoreError> {
            self.inner
                .delete(reference, purpose, expected_generation)
                .await
        }
    }

    async fn read_test_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let body_start = loop {
            let read = stream.read(&mut buffer).await.expect("read request");
            if read == 0 {
                break None;
            }
            request.extend_from_slice(&buffer[..read]);
            if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                break Some(position + 4);
            }
        };
        if let Some(body_start) = body_start {
            let headers = String::from_utf8_lossy(&request[..body_start]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("Content-Length:")
                        .or_else(|| line.strip_prefix("content-length:"))
                })
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            while request.len() < body_start + content_length {
                let read = stream.read(&mut buffer).await.expect("read body");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
            }
        }
        request
    }

    async fn test_server(response: String) -> (String, oneshot::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind server");
        let address = listener.local_addr().expect("server address");
        let (sender, receiver) = oneshot::channel();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept request");
            let request = read_test_request(&mut stream).await;
            let _ = sender.send(request);
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        });
        (format!("http://{address}"), receiver)
    }

    async fn test_server_many(responses: Vec<String>) -> (String, oneshot::Receiver<Vec<Vec<u8>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind server");
        let address = listener.local_addr().expect("server address");
        let (sender, receiver) = oneshot::channel();
        tokio::spawn(async move {
            let mut requests = Vec::with_capacity(responses.len());
            for response in responses {
                let (mut stream, _) = listener.accept().await.expect("accept request");
                requests.push(read_test_request(&mut stream).await);
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("write response");
            }
            let _ = sender.send(requests);
        });
        (format!("http://{address}"), receiver)
    }

    async fn fragmented_test_server(
        response_parts: Vec<Vec<u8>>,
    ) -> (String, oneshot::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind server");
        let address = listener.local_addr().expect("server address");
        let (sender, receiver) = oneshot::channel();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept request");
            let request = read_test_request(&mut stream).await;
            let _ = sender.send(request);
            for part in response_parts {
                stream.write_all(&part).await.expect("write response part");
                tokio::task::yield_now().await;
            }
        });
        (format!("http://{address}"), receiver)
    }

    async fn stalled_test_server() -> (String, oneshot::Receiver<Vec<u8>>, oneshot::Sender<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind server");
        let address = listener.local_addr().expect("server address");
        let (request_sender, request_receiver) = oneshot::channel();
        let (release_sender, release_receiver) = oneshot::channel();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept request");
            let request = read_test_request(&mut stream).await;
            let _ = request_sender.send(request);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 1024\r\n\r\n",
                )
                .await
                .expect("write headers");
            let _ = release_receiver.await;
        });
        (
            format!("http://{address}"),
            request_receiver,
            release_sender,
        )
    }

    async fn run_stream_fixture(
        kind: &str,
        protocol: ProviderProtocol,
        endpoint_suffix: &str,
        payload: &str,
    ) -> (String, InferenceOutcome, Vec<GenerationStreamEvent>) {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{payload}",
            payload.len()
        );
        let (endpoint, captured) = test_server(response).await;
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("key-canary").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        let runtime = Arc::new(InferenceRuntime::default());
        let runtime_port: Arc<dyn InferenceRuntimePort> = runtime.clone();
        let adapter = RemoteProviders::with_runtime(
            store,
            Arc::new(JsonClient::new().expect("client")),
            runtime_port,
        );
        let sink_id = RequestId::new();
        let mut receiver = runtime.register_stream(sink_id).expect("register stream");
        let mut inference = request(profile(
            kind,
            format!("{endpoint}{endpoint_suffix}"),
            ProviderConfig::Standard,
            Some(key_ref),
            owner,
        ));
        inference.profile.chat_profile.provider_protocol = protocol;
        inference.stream_sink = Some(sink_id);
        let outcome = adapter.run(inference).await.expect("stream outcome");
        runtime
            .unregister_stream(sink_id)
            .expect("unregister stream");
        let mut events = Vec::new();
        while let Some(event) = receiver.recv().await {
            events.push(event.event);
        }
        let request =
            String::from_utf8(captured.await.expect("captured request")).expect("UTF-8 request");
        (request, outcome, events)
    }

    pub(crate) fn parameters() -> ResolvedChatParameters {
        ResolvedChatParameters {
            temperature: None,
            top_p: None,
            top_k: Some(40),
            visible_max_output_tokens: None,
            context_length: Some(4096),
            frequency_penalty: None,
            presence_penalty: None,
            repetition_penalty: None,
            reasoning_mode: None,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            prompt_caching: None,
            total_completion_allowance: None,
            ollama: Default::default(),
            openrouter: Default::default(),
        }
    }

    fn context() -> ProviderNeutralContext {
        ProviderNeutralContext {
            messages: vec![
                ProviderNeutralMessage {
                    role: MessageRole::System,
                    parts: vec![ProviderContextPart::Text {
                        text: "system text".to_owned(),
                    }],
                },
                ProviderNeutralMessage {
                    role: MessageRole::Scene,
                    parts: vec![ProviderContextPart::Text {
                        text: "scene text".to_owned(),
                    }],
                },
                ProviderNeutralMessage {
                    role: MessageRole::User,
                    parts: vec![ProviderContextPart::Text {
                        text: "user text".to_owned(),
                    }],
                },
            ],
            attributions: ContextAttributions::default(),
            budget: ContextBudgetReport::default(),
        }
    }

    fn cache_context() -> ProviderNeutralContext {
        ProviderNeutralContext {
            messages: vec![
                ProviderNeutralMessage {
                    role: MessageRole::System,
                    parts: vec![ProviderContextPart::Text {
                        text: "system text".to_owned(),
                    }],
                },
                ProviderNeutralMessage {
                    role: MessageRole::Assistant,
                    parts: vec![ProviderContextPart::Text {
                        text: "prefix text".to_owned(),
                    }],
                },
                ProviderNeutralMessage {
                    role: MessageRole::User,
                    parts: vec![ProviderContextPart::Text {
                        text: "live text".to_owned(),
                    }],
                },
            ],
            attributions: ContextAttributions::default(),
            budget: ContextBudgetReport::default(),
        }
    }

    fn profile(
        kind: &str,
        endpoint: String,
        config: ProviderConfig,
        api_key_ref: Option<SecretRef>,
        owner: lettuce_settings::SecretOwnerId,
    ) -> ResolvedInferenceProfile {
        ResolvedInferenceProfile {
            chat_profile: ResolvedChatProfile {
                model_profile_id: ModelProfileId::new(),
                model_revision: Revision::INITIAL,
                provider_account_id: ProviderAccountId::new(),
                provider_account_revision: Revision::INITIAL,
                secret_owner_id: owner,
                external_model_id: "test-model".to_owned(),
                provider_kind: kind.to_owned(),
                provider_protocol: ProviderProtocol::OpenAiCompatible,
                endpoint: Some(endpoint),
                provider_config: config,
                streaming_enabled: true,
                allow_invalid_tls: false,
                capabilities: ModelCapabilities::default(),
                parameters: parameters(),
                api_key_ref,
                secret_headers: Vec::new(),
                warnings: Vec::<ChatProfileWarning>::new(),
            },
            tool_policy: ToolPolicy::Disabled,
            output_policy: OutputPolicy::Plain,
            safety_policy: SafetyContext::Standard,
            correlation_id: None,
        }
    }

    fn request(profile: ResolvedInferenceProfile) -> InferenceRequest {
        InferenceRequest {
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
            operation: GenerationOperation::Send,
            profile,
            context: context(),
            cancellation: None,
            stream_sink: None,
            media_grants: Vec::new(),
        }
    }

    fn request_with_context(
        profile: ResolvedInferenceProfile,
        context: ProviderNeutralContext,
    ) -> InferenceRequest {
        let mut request = request(profile);
        request.context = context;
        request
    }

    fn response_body() -> String {
        let body = r#"{"id":"response-id","choices":[{"index":7,"message":{"content":"ok"},"finish_reason":"stop"}]}"#;
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
    }

    #[tokio::test]
    async fn preflight_rejects_before_loading_secrets() {
        let store = Arc::new(RecordingSecretStore::default());
        let adapter = RemoteProviders::new(
            Arc::clone(&store),
            Arc::new(JsonClient::new().expect("client")),
        );
        let owner = lettuce_settings::SecretOwnerId::new();
        let request_id = RequestId::new();
        let mut inference = request(profile(
            "custom",
            "https://example.invalid".to_owned(),
            ProviderConfig::Custom(CustomProviderConfig {
                chat_path: "/chat".to_owned(),
                models_path: None,
                streaming: false,
                auth: CustomAuth::Bearer,
                ..Default::default()
            }),
            Some(SecretRef::new()),
            owner,
        ));
        inference.stream_sink = Some(request_id);
        assert_eq!(
            adapter.run(inference).await,
            Err(lettuce_conversations::PortError::Rejected)
        );
        assert!(store.take_loads().is_empty());

        for (kind, protocol, retention) in [
            (
                "openai",
                ProviderProtocol::OpenAiCompatible,
                lettuce_models::PromptCacheRetention::OneHour,
            ),
            (
                "groq",
                ProviderProtocol::OpenAiCompatible,
                lettuce_models::PromptCacheRetention::FiveMinutes,
            ),
            (
                "gemini",
                ProviderProtocol::Gemini,
                lettuce_models::PromptCacheRetention::InMemory,
            ),
            (
                "gemini-agent-platform-express",
                ProviderProtocol::Gemini,
                lettuce_models::PromptCacheRetention::FiveMinutes,
            ),
        ] {
            let mut inference = request(profile(
                kind,
                "https://example.invalid".to_owned(),
                ProviderConfig::Standard,
                Some(SecretRef::new()),
                owner,
            ));
            inference.profile.chat_profile.provider_protocol = protocol;
            inference.profile.chat_profile.parameters.prompt_caching =
                Some(lettuce_models::PromptCaching::Enabled { retention });
            assert_eq!(
                adapter.run(inference).await,
                Err(lettuce_conversations::PortError::Rejected),
                "{kind} must reject unsupported cache policy before secret loading"
            );
            assert!(store.take_loads().is_empty());
        }
    }

    #[tokio::test]
    async fn openai_stream_routes_bounded_deltas_and_returns_the_complete_outcome() {
        let payload = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hi<th\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ink>why</think>!\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nX-Request-Id: stream-request\r\n\r\n",
            payload.len()
        );
        let split = payload.find("<th").expect("split marker") + 2;
        let (endpoint, captured) = fragmented_test_server(vec![
            header.into_bytes(),
            payload.as_bytes()[..split].to_vec(),
            payload.as_bytes()[split..].to_vec(),
        ])
        .await;

        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("key-canary").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        let runtime = Arc::new(InferenceRuntime::default());
        let runtime_port: Arc<dyn InferenceRuntimePort> = runtime.clone();
        let adapter = RemoteProviders::with_runtime(
            Arc::clone(&store),
            Arc::new(JsonClient::new().expect("client")),
            runtime_port,
        );
        let sink_id = RequestId::new();
        let mut receiver = runtime.register_stream(sink_id).expect("register stream");
        let mut inference = request(profile(
            "nanogpt",
            endpoint,
            ProviderConfig::Standard,
            Some(key_ref),
            owner,
        ));
        inference.stream_sink = Some(sink_id);

        let outcome = adapter.run(inference).await.expect("stream outcome");
        let request =
            String::from_utf8(captured.await.expect("captured request")).expect("UTF-8 request");
        assert!(request.contains(r#""stream":true"#));
        assert!(request.contains(r#""stream_options":{"include_usage":true}"#));
        assert_eq!(
            outcome.provider_request_id.as_deref(),
            Some("stream-request")
        );
        assert!(matches!(
            &outcome.candidates[0].parts[..],
            [MessagePart::ReasoningSummary { text: reasoning }, MessagePart::Text { text }]
                if reasoning == "why" && text == "Hi!"
        ));

        let first = receiver.recv().await.expect("first delta");
        let second = receiver.recv().await.expect("second delta");
        let third = receiver.recv().await.expect("third delta");
        assert_eq!((first.sequence, second.sequence, third.sequence), (1, 2, 3));
        assert_eq!(
            (first.event, second.event, third.event),
            (
                GenerationStreamEvent::TextDelta {
                    text: "Hi".to_owned()
                },
                GenerationStreamEvent::TextDelta {
                    text: "!".to_owned()
                },
                GenerationStreamEvent::ReasoningDelta {
                    text: "why".to_owned()
                },
            )
        );
    }

    #[tokio::test]
    async fn anthropic_stream_uses_sse_headers_and_message_stop() {
        let payload = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":2,\"output_tokens\":0}}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"answer\"}}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        let (request, outcome, events) =
            run_stream_fixture("anthropic", ProviderProtocol::Anthropic, "", payload).await;
        assert!(
            request.starts_with("POST /v1/messages HTTP/1.1\r\n"),
            "unexpected request: {request:?}"
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("accept: text/event-stream\r\n")
        );
        assert_eq!(
            events,
            vec![GenerationStreamEvent::TextDelta {
                text: "answer".to_owned()
            }]
        );
        assert_eq!(outcome.usage.expect("usage").input_tokens, 2);
    }

    #[tokio::test]
    async fn gemini_stream_uses_sse_query_and_preserves_thought_parts() {
        let payload = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"why\",\"thought\":true},{\"text\":\"answer\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":2,\"candidatesTokenCount\":1}}\n\n";
        let (request, outcome, events) =
            run_stream_fixture("gemini", ProviderProtocol::Gemini, "/v1", payload).await;
        assert!(request.starts_with(
            "POST /v1beta/models/test-model:streamGenerateContent?alt=sse HTTP/1.1\r\n"
        ));
        assert_eq!(
            events,
            vec![
                GenerationStreamEvent::ReasoningDelta {
                    text: "why".to_owned()
                },
                GenerationStreamEvent::TextDelta {
                    text: "answer".to_owned()
                }
            ]
        );
        assert_eq!(outcome.usage.expect("usage").output_tokens, 1);
    }

    #[tokio::test]
    async fn ollama_stream_uses_ndjson_and_native_reasoning() {
        let payload = concat!(
            "{\"message\":{\"content\":\"answer\",\"thinking\":\"why\"},\"done\":false}\n",
            "{\"message\":{},\"done\":true,\"done_reason\":\"stop\",\"prompt_eval_count\":2,\"eval_count\":1}\n",
        );
        let (request, outcome, events) =
            run_stream_fixture("ollama", ProviderProtocol::Ollama, "", payload).await;
        assert!(
            request.starts_with("POST /api/chat HTTP/1.1\r\n"),
            "unexpected request: {request:?}"
        );
        assert_eq!(
            events,
            vec![
                GenerationStreamEvent::TextDelta {
                    text: "answer".to_owned()
                },
                GenerationStreamEvent::ReasoningDelta {
                    text: "why".to_owned()
                }
            ]
        );
        assert_eq!(outcome.usage.expect("usage").output_tokens, 1);
    }

    #[tokio::test]
    async fn cancellation_interrupts_a_stalled_buffered_response_without_changing_wire_mode() {
        let (endpoint, captured, release) = stalled_test_server().await;
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let runtime = Arc::new(InferenceRuntime::default());
        let runtime_port: Arc<dyn InferenceRuntimePort> = runtime.clone();
        let adapter = RemoteProviders::with_runtime(
            store,
            Arc::new(JsonClient::new().expect("client")),
            runtime_port,
        );
        let handle = JobHandle::new(JobId::new());
        runtime
            .register_cancellation(handle.id(), handle.cancellation_token())
            .expect("register cancellation");
        let mut inference = request(profile(
            "lmstudio",
            endpoint,
            ProviderConfig::Standard,
            None,
            owner,
        ));
        inference.cancellation = Some(handle.id());
        let task = tokio::spawn(async move { adapter.run(inference).await });
        let captured = String::from_utf8(captured.await.expect("request reached server"))
            .expect("UTF-8 request");
        assert!(captured.contains(r#""stream":false"#));
        handle.request_cancel();
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .expect("cancellation deadline")
            .expect("provider task");
        assert_eq!(result, Err(lettuce_conversations::PortError::Cancelled));
        let _ = release.send(());
    }

    #[tokio::test]
    async fn cancellation_interrupts_a_backpressured_stream_sink() {
        let mut payload = String::new();
        for _ in 0..65 {
            payload.push_str("data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\n");
        }
        payload.push_str(
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n",
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{payload}",
            payload.len()
        );
        let (endpoint, captured) = test_server(response).await;
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let runtime = Arc::new(InferenceRuntime::default());
        let runtime_port: Arc<dyn InferenceRuntimePort> = runtime.clone();
        let adapter = RemoteProviders::with_runtime(
            store,
            Arc::new(JsonClient::new().expect("client")),
            runtime_port,
        );
        let sink_id = RequestId::new();
        let _receiver = runtime.register_stream(sink_id).expect("register stream");
        let handle = JobHandle::new(JobId::new());
        runtime
            .register_cancellation(handle.id(), handle.cancellation_token())
            .expect("register cancellation");
        let mut inference = request(profile(
            "lmstudio",
            endpoint,
            ProviderConfig::Standard,
            None,
            owner,
        ));
        inference.stream_sink = Some(sink_id);
        inference.cancellation = Some(handle.id());
        let task = tokio::spawn(async move { adapter.run(inference).await });
        captured.await.expect("request reached server");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        handle.request_cancel();
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .expect("cancellation deadline")
            .expect("provider task");
        assert_eq!(result, Err(lettuce_conversations::PortError::Cancelled));
    }

    #[tokio::test]
    async fn custom_auth_modes_use_one_credential_and_preserve_system_scene_roles() {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("api+canary&").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        let network = Arc::new(JsonClient::new().expect("client"));
        let cases = [
            (CustomAuth::Bearer, "authorization: bearer api+canary&"),
            (
                CustomAuth::Header {
                    name: HeaderName::new("x-api-key").expect("header"),
                },
                "x-api-key: api+canary&",
            ),
            (
                CustomAuth::Query {
                    name: lettuce_models::QueryParameterName::new("api-key").expect("query"),
                },
                "?api-key=api%2bcanary%26",
            ),
        ];
        for (auth, expected_auth) in cases {
            let (endpoint, request_receiver) = test_server(response_body()).await;
            let adapter = RemoteProviders::new(Arc::clone(&store), Arc::clone(&network));
            let outcome = adapter
                .run(request(profile(
                    "custom",
                    endpoint,
                    ProviderConfig::Custom(CustomProviderConfig {
                        chat_path: "/chat".to_owned(),
                        models_path: None,
                        streaming: false,
                        auth,
                        ..Default::default()
                    }),
                    Some(key_ref),
                    owner,
                )))
                .await
                .expect("custom response");
            assert_eq!(outcome.candidates[0].ordinal, 7);
            assert_eq!(store.take_loads().len(), 1);
            let raw = String::from_utf8(request_receiver.await.expect("request")).expect("HTTP");
            let lower = raw.to_ascii_lowercase();
            assert!(
                lower.contains(expected_auth),
                "missing auth {expected_auth} in request: {raw}"
            );
            assert!(lower.contains("accept: application/json\r\n"));
            let body_start = raw.find("\r\n\r\n").expect("body boundary") + 4;
            let body: serde_json::Value = serde_json::from_str(&raw[body_start..]).expect("JSON");
            let messages: Vec<(&str, &str)> = body["messages"]
                .as_array()
                .expect("messages")
                .iter()
                .map(|message| {
                    (
                        message["role"].as_str().expect("role"),
                        message["content"].as_str().expect("content"),
                    )
                })
                .collect();
            assert_eq!(
                messages,
                [
                    ("system", "system text\n\nscene text"),
                    ("user", "user text")
                ]
            );
            assert_eq!(body["context_length"], serde_json::json!(4096));
            assert!(body.get("top_k").is_none());
        }
    }

    #[tokio::test]
    async fn native_openai_maps_system_and_scene_to_developer() {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("native-canary").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        let (endpoint, request_receiver) = test_server(response_body()).await;
        let adapter = RemoteProviders::new(
            Arc::clone(&store),
            Arc::new(JsonClient::new().expect("client")),
        );
        let outcome = adapter
            .run(request(profile(
                "openai",
                endpoint,
                ProviderConfig::Standard,
                Some(key_ref),
                owner,
            )))
            .await
            .expect("native response");
        assert_eq!(outcome.candidates[0].ordinal, 7);
        let raw = String::from_utf8(request_receiver.await.expect("request")).expect("HTTP");
        assert!(
            raw.to_ascii_lowercase()
                .contains("authorization: bearer native-canary\r\n")
        );
        let body_start = raw.find("\r\n\r\n").expect("body boundary") + 4;
        let body: serde_json::Value = serde_json::from_str(&raw[body_start..]).expect("JSON");
        let roles: Vec<&str> = body["messages"]
            .as_array()
            .expect("messages")
            .iter()
            .map(|message| message["role"].as_str().expect("role"))
            .collect();
        assert_eq!(roles, ["developer", "developer", "user"]);
        assert!(body.get("context_length").is_none());
        assert!(body.get("top_k").is_none());
        assert_eq!(body["max_tokens"], serde_json::json!(4096));
        assert!(raw.starts_with("POST /v1/chat/completions HTTP/1.1"));
        assert!(raw.to_ascii_lowercase().contains("user-agent: lettuceai/"));
    }

    struct Captured {
        request_line: String,
        headers: String,
        body: serde_json::Value,
    }

    fn captured_request(raw: Vec<u8>) -> Captured {
        let raw = String::from_utf8(raw).expect("HTTP");
        let body_start = raw.find("\r\n\r\n").expect("body boundary") + 4;
        Captured {
            request_line: raw.lines().next().expect("request line").to_owned(),
            headers: raw[..body_start].to_ascii_lowercase(),
            body: serde_json::from_str(&raw[body_start..]).expect("JSON body"),
        }
    }

    async fn capture(kind: &str, endpoint_suffix: &str, with_key: bool) -> Captured {
        capture_with(
            kind,
            ProviderProtocol::OpenAiCompatible,
            ProviderConfig::Standard,
            endpoint_suffix,
            with_key,
            response_body(),
        )
        .await
    }

    async fn capture_with(
        kind: &str,
        protocol: ProviderProtocol,
        config: ProviderConfig,
        endpoint_suffix: &str,
        with_key: bool,
        response: String,
    ) -> Captured {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("key-canary").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        let (endpoint, request_receiver) = test_server(response).await;
        let adapter = RemoteProviders::new(
            Arc::clone(&store),
            Arc::new(JsonClient::new().expect("client")),
        );
        let mut profile = profile(
            kind,
            format!("{endpoint}{endpoint_suffix}"),
            config,
            with_key.then_some(key_ref),
            owner,
        );
        profile.chat_profile.provider_protocol = protocol;
        profile.chat_profile.parameters.frequency_penalty = Some(0.25);
        profile.chat_profile.parameters.presence_penalty = Some(0.5);
        adapter
            .run(request(profile))
            .await
            .unwrap_or_else(|error| panic!("{kind}: {error:?}"));
        captured_request(request_receiver.await.expect("request"))
    }

    async fn capture_reasoning(
        kind: &str,
        protocol: ProviderProtocol,
        model: &str,
        response: String,
    ) -> Captured {
        capture_reasoning_with_config(kind, protocol, model, ProviderConfig::Standard, response)
            .await
    }

    async fn capture_reasoning_with_config(
        kind: &str,
        protocol: ProviderProtocol,
        model: &str,
        config: ProviderConfig,
        response: String,
    ) -> Captured {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("key-canary").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        let (endpoint, request_receiver) = test_server(response).await;
        let adapter = RemoteProviders::new(store, Arc::new(JsonClient::new().expect("client")));
        let mut profile = profile(kind, endpoint, config, Some(key_ref), owner);
        profile.chat_profile.provider_protocol = protocol;
        profile.chat_profile.external_model_id = model.to_owned();
        profile.chat_profile.parameters.visible_max_output_tokens = Some(100);
        profile.chat_profile.parameters.reasoning_mode =
            Some(lettuce_models::ReasoningMode::Enabled);
        profile.chat_profile.parameters.reasoning_effort =
            Some(lettuce_models::ReasoningEffort::Medium);
        profile.chat_profile.parameters.reasoning_budget_tokens = Some(20);
        profile.chat_profile.parameters.total_completion_allowance = Some(120);
        adapter
            .run(request(profile))
            .await
            .unwrap_or_else(|error| panic!("{kind}: {error:?}"));
        captured_request(request_receiver.await.expect("request"))
    }

    #[tokio::test]
    async fn reasoning_requests_use_each_protocols_native_wire_shape() {
        let openai = capture_reasoning(
            "openai",
            ProviderProtocol::OpenAiCompatible,
            "o3",
            response_body(),
        )
        .await;
        assert_eq!(openai.body["max_completion_tokens"], serde_json::json!(120));
        assert_eq!(openai.body["reasoning_effort"], serde_json::json!("medium"));
        assert!(openai.body.get("max_tokens").is_none());

        let anthropic = capture_reasoning(
            "anthropic",
            ProviderProtocol::Anthropic,
            "claude-sonnet",
            http_json(r#"{"content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn"}"#),
        )
        .await;
        assert_eq!(anthropic.body["max_tokens"], serde_json::json!(120));
        assert_eq!(anthropic.body["temperature"], serde_json::json!(1.0));
        assert_eq!(
            anthropic.body["thinking"],
            serde_json::json!({ "type": "enabled", "budget_tokens": 20 })
        );

        let custom_anthropic = capture_reasoning_with_config(
            "custom-anthropic",
            ProviderProtocol::Anthropic,
            "claude-proxy",
            ProviderConfig::Custom(CustomProviderConfig {
                chat_path: "/proxy/messages".to_owned(),
                models_path: None,
                streaming: false,
                auth: CustomAuth::Bearer,
                ..Default::default()
            }),
            http_json(r#"{"content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn"}"#),
        )
        .await;
        assert_eq!(
            custom_anthropic.request_line,
            "POST /proxy/messages HTTP/1.1"
        );
        assert_eq!(
            custom_anthropic.body["thinking"],
            serde_json::json!({ "type": "enabled", "budget_tokens": 20 })
        );

        let gemini = capture_reasoning(
            "gemini",
            ProviderProtocol::Gemini,
            "gemini-2.5-pro",
            http_json(
                r#"{"candidates":[{"content":{"parts":[{"text":"ok"}]},"finishReason":"STOP"}]}"#,
            ),
        )
        .await;
        assert_eq!(
            gemini.body["generationConfig"]["maxOutputTokens"],
            serde_json::json!(100)
        );
        assert_eq!(
            gemini.body["generationConfig"]["thinkingConfig"],
            serde_json::json!({ "includeThoughts": true, "thinkingBudget": 20 })
        );

        let express = capture_reasoning(
            "gemini-agent-platform-express",
            ProviderProtocol::Gemini,
            "publishers/google/models/gemini-3-pro",
            http_json(
                r#"{"candidates":[{"content":{"parts":[{"text":"ok"}]},"finishReason":"STOP"}]}"#,
            ),
        )
        .await;
        assert_eq!(
            express.request_line,
            "POST /v1beta1/publishers/google/models/gemini-3-pro:generateContent HTTP/1.1"
        );
        assert_eq!(
            express.body["generationConfig"]["thinkingConfig"],
            serde_json::json!({ "includeThoughts": true, "thinkingLevel": "MEDIUM" })
        );

        let ollama = capture_reasoning(
            "ollama",
            ProviderProtocol::Ollama,
            "qwen3",
            http_json(r#"{"message":{"content":"ok"},"done":true}"#),
        )
        .await;
        assert_eq!(ollama.body["think"], serde_json::json!("medium"));
        assert_eq!(
            ollama.body["options"]["num_predict"],
            serde_json::json!(100)
        );
        assert!(ollama.body.get("chat_template_kwargs").is_none());
    }

    async fn capture_cached(
        kind: &str,
        protocol: ProviderProtocol,
        retention: lettuce_models::PromptCacheRetention,
        response: String,
    ) -> Captured {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("key-canary").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        let (endpoint, request_receiver) = test_server(response).await;
        let adapter = RemoteProviders::new(
            Arc::clone(&store),
            Arc::new(JsonClient::new().expect("client")),
        );
        let config = if kind == "custom-anthropic" {
            ProviderConfig::Custom(CustomProviderConfig {
                chat_path: "/v1/messages".to_owned(),
                models_path: None,
                streaming: false,
                auth: CustomAuth::Bearer,
                ..Default::default()
            })
        } else {
            ProviderConfig::Standard
        };
        let mut resolved = profile(kind, endpoint, config, Some(key_ref), owner);
        resolved.chat_profile.provider_protocol = protocol;
        resolved.chat_profile.parameters.prompt_caching =
            Some(lettuce_models::PromptCaching::Enabled { retention });
        if kind == "openrouter" {
            resolved.chat_profile.parameters.openrouter.pinned_provider =
                Some("provider/tag".to_owned());
        }
        adapter
            .run(request(resolved))
            .await
            .unwrap_or_else(|error| panic!("{kind}: {error:?}"));
        captured_request(request_receiver.await.expect("request"))
    }

    fn roles(body: &serde_json::Value) -> Vec<&str> {
        body["messages"]
            .as_array()
            .expect("messages")
            .iter()
            .map(|message| message["role"].as_str().expect("role"))
            .collect()
    }

    #[tokio::test]
    async fn bearer_providers_follow_their_legacy_paths_and_bodies() {
        for (kind, suffix, path, context_length) in [
            ("cerebras", "", "/v1/chat/completions", false),
            ("cerebras.ai", "/v1", "/v1/chat/completions", false),
            ("nvidia-nim", "", "/v1/chat/completions", false),
            ("literouter", "/v1/", "/v1/chat/completions", false),
            ("groq", "", "/openai/v1/chat/completions", false),
            ("groq", "/openai", "/openai/v1/chat/completions", false),
            ("deepseek", "", "/v1/chat/completions", true),
            ("xai", "/v1", "/v1/chat/completions", true),
            ("anannas", "", "/v1/chat/completions", true),
            ("moonshot-ai", "", "/v1/chat/completions", true),
            ("chutes.ai", "", "/v1/chat/completions", true),
            ("nanogpt", "", "/v1/chat/completions", true),
            ("featherless", "", "/v1/chat/completions", true),
            ("qwen", "", "/v1/chat/completions", true),
            ("openrouter", "", "/v1/chat/completions", true),
        ] {
            let captured = capture(kind, suffix, true).await;
            assert_eq!(
                captured.request_line,
                format!("POST {path} HTTP/1.1"),
                "{kind}"
            );
            assert!(
                captured
                    .headers
                    .contains("authorization: bearer key-canary\r\n"),
                "{kind}"
            );
            assert_eq!(
                roles(&captured.body),
                ["system", "system", "user"],
                "{kind}"
            );
            assert_eq!(
                captured.body.get("context_length").is_some(),
                context_length,
                "{kind}"
            );
            assert_eq!(
                captured.body["frequency_penalty"],
                serde_json::json!(0.25),
                "{kind}"
            );
            assert_eq!(
                captured.body["max_tokens"],
                serde_json::json!(4096),
                "{kind}"
            );
            assert_eq!(captured.body["stream"], serde_json::json!(false), "{kind}");
            assert!(captured.body.get("top_k").is_none(), "{kind}");
        }
    }

    #[tokio::test]
    async fn explicit_prompt_caching_uses_each_provider_native_shape() {
        let uncached_openai = capture("openai", "", true).await;
        assert!(uncached_openai.body.get("prompt_cache_retention").is_none());

        let openai = capture_cached(
            "openai",
            ProviderProtocol::OpenAiCompatible,
            lettuce_models::PromptCacheRetention::InMemory,
            response_body(),
        )
        .await;
        assert_eq!(
            openai.body["prompt_cache_retention"],
            serde_json::json!("in_memory")
        );

        let openai = capture_cached(
            "openai",
            ProviderProtocol::OpenAiCompatible,
            lettuce_models::PromptCacheRetention::TwentyFourHours,
            response_body(),
        )
        .await;
        assert_eq!(
            openai.body["prompt_cache_retention"],
            serde_json::json!("24h")
        );
        assert!(openai.body.to_string().find("promptCachingTtl").is_none());

        let openrouter = capture_cached(
            "openrouter",
            ProviderProtocol::OpenAiCompatible,
            lettuce_models::PromptCacheRetention::OneHour,
            response_body(),
        )
        .await;
        let messages = openrouter.body["messages"].as_array().expect("messages");
        assert_eq!(
            messages[0]["content"][0]["cache_control"],
            serde_json::json!({"type":"ephemeral","ttl":"1h"})
        );
        assert!(messages[1]["content"].is_string());
        assert_eq!(
            messages[2]["content"][0]["cache_control"],
            serde_json::json!({"type":"ephemeral","ttl":"1h"})
        );
        assert_eq!(
            openrouter.body["provider"],
            serde_json::json!({"order":["provider/tag"],"allow_fallbacks":false})
        );

        let anthropic_response =
            http_json(r#"{"content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn"}"#);
        for kind in ["anthropic", "custom-anthropic"] {
            let captured = capture_cached(
                kind,
                ProviderProtocol::Anthropic,
                lettuce_models::PromptCacheRetention::FiveMinutes,
                anthropic_response.clone(),
            )
            .await;
            assert_eq!(
                captured.body["system"][0]["cache_control"],
                serde_json::json!({"type":"ephemeral"}),
                "{kind}"
            );
            assert_eq!(
                captured.body["messages"][0]["content"][0]["cache_control"],
                serde_json::json!({"type":"ephemeral"}),
                "{kind}"
            );
        }
    }

    #[tokio::test]
    async fn provider_specific_headers_roles_and_bodies_match_legacy() {
        let captured = capture("mistral", "", true).await;
        assert!(captured.headers.contains("x-api-key: key-canary\r\n"));
        assert!(!captured.headers.contains("authorization:"));
        assert!(captured.body.get("frequency_penalty").is_none());
        assert!(captured.body.get("presence_penalty").is_none());
        assert!(captured.body.get("context_length").is_some());

        let captured = capture("pollinations", "", true).await;
        assert_eq!(roles(&captured.body), ["developer", "developer", "user"]);
        assert!(captured.headers.contains("user-agent: lettuceai/"));

        let captured = capture("qwen", "", true).await;
        assert!(captured.headers.contains("accept: application/json\r\n"));
        assert!(!captured.headers.contains("user-agent:"));

        let captured = capture("openrouter", "", true).await;
        assert!(
            captured
                .headers
                .contains("x-openrouter-title: lettuceai\r\n")
        );
        assert!(
            captured
                .headers
                .contains("x-openrouter-categories: roleplay\r\n")
        );
        assert!(
            captured
                .headers
                .contains("http-referer: https://github.com/lettuceai/\r\n")
        );

        let captured = capture("z.ai", "/chat/completions", true).await;
        assert_eq!(captured.request_line, "POST /chat/completions HTTP/1.1");
        assert_eq!(
            captured.body["thinking"],
            serde_json::json!({ "type": "disabled" })
        );
        assert!(captured.body.get("context_length").is_none());
        assert!(captured.body.get("frequency_penalty").is_none());
        assert!(!captured.headers.contains("accept: application/json"));
        let captured = capture("zai", "/api/paas/v4", true).await;
        assert_eq!(
            captured.request_line,
            "POST /api/paas/v4/chat/completions HTTP/1.1"
        );

        let captured = capture("intenserp", "/v1", false).await;
        assert_eq!(captured.request_line, "POST /v1/chat/completions HTTP/1.1");
        assert!(!captured.headers.contains("authorization:"));

        let captured = capture("lmstudio", "/v1", false).await;
        assert!(!captured.headers.contains("authorization:"));
        let captured = capture("lmstudio", "", true).await;
        assert_eq!(captured.request_line, "POST /v1/chat/completions HTTP/1.1");
        assert!(
            captured
                .headers
                .contains("authorization: bearer key-canary\r\n")
        );
    }

    fn http_json(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
    }

    async fn cached_gemini_fixture(
        responses: Vec<String>,
        retention: lettuce_models::PromptCacheRetention,
    ) -> (
        RemoteProviders<RecordingSecretStore>,
        ResolvedInferenceProfile,
        oneshot::Receiver<Vec<Vec<u8>>>,
        Arc<RecordingSecretStore>,
        SecretRef,
    ) {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("key-canary").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        let (endpoint, requests) = test_server_many(responses).await;
        let adapter = RemoteProviders::new(
            Arc::clone(&store),
            Arc::new(JsonClient::new().expect("client")),
        );
        let mut resolved = profile(
            "gemini",
            format!("{endpoint}/v1"),
            ProviderConfig::Standard,
            Some(key_ref),
            owner,
        );
        resolved.chat_profile.provider_protocol = ProviderProtocol::Gemini;
        resolved.chat_profile.parameters.prompt_caching =
            Some(lettuce_models::PromptCaching::Enabled { retention });
        (adapter, resolved, requests, store, key_ref)
    }

    #[tokio::test]
    async fn anthropic_family_sends_messages_requests_like_legacy() {
        let anthropic_reply = http_json(
            r#"{"content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}"#,
        );
        let captured = capture_with(
            "anthropic",
            ProviderProtocol::Anthropic,
            ProviderConfig::Standard,
            "",
            true,
            anthropic_reply.clone(),
        )
        .await;
        assert_eq!(captured.request_line, "POST /v1/messages HTTP/1.1");
        assert!(captured.headers.contains("x-api-key: key-canary\r\n"));
        assert!(
            captured
                .headers
                .contains("anthropic-version: 2023-06-01\r\n")
        );
        assert!(!captured.headers.contains("authorization:"));
        assert_eq!(
            captured.body["system"],
            serde_json::json!("system text\n\nscene text")
        );
        assert_eq!(captured.body["max_tokens"], serde_json::json!(4096));
        assert_eq!(captured.body["stream"], serde_json::json!(false));
        assert_eq!(captured.body["top_k"], serde_json::json!(40));
        assert!(captured.body.get("frequency_penalty").is_none());
        assert_eq!(
            captured.body["messages"],
            serde_json::json!([{"role":"user","content":[{"type":"text","text":"user text"}]}])
        );

        let captured = capture_with(
            "anthropic",
            ProviderProtocol::Anthropic,
            ProviderConfig::Standard,
            "/v1",
            true,
            anthropic_reply.clone(),
        )
        .await;
        assert_eq!(captured.request_line, "POST /v1/messages HTTP/1.1");

        let captured = capture_with(
            "custom-anthropic",
            ProviderProtocol::Anthropic,
            ProviderConfig::Custom(CustomProviderConfig {
                chat_path: "/proxy/messages".to_owned(),
                models_path: None,
                streaming: false,
                auth: CustomAuth::Bearer,
                ..Default::default()
            }),
            "",
            true,
            anthropic_reply,
        )
        .await;
        assert_eq!(captured.request_line, "POST /proxy/messages HTTP/1.1");
        assert!(
            captured
                .headers
                .contains("authorization: bearer key-canary\r\n")
        );
        assert!(
            captured
                .headers
                .contains("anthropic-version: 2023-06-01\r\n")
        );
    }

    #[tokio::test]
    async fn gemini_family_targets_generate_content_with_header_auth() {
        let gemini_reply = http_json(
            r#"{"candidates":[{"content":{"parts":[{"text":"ok"}],"role":"model"},"finishReason":"STOP"}]}"#,
        );
        for (kind, suffix, path) in [
            ("gemini", "/v1", "/v1beta/models/test-model:generateContent"),
            (
                "google-gemini",
                "/v1beta",
                "/v1beta/models/test-model:generateContent",
            ),
            (
                "gemini-agent-platform-express",
                "/v1",
                "/v1beta1/publishers/google/models/test-model:generateContent",
            ),
        ] {
            let captured = capture_with(
                kind,
                ProviderProtocol::Gemini,
                ProviderConfig::Standard,
                suffix,
                true,
                gemini_reply.clone(),
            )
            .await;
            assert_eq!(
                captured.request_line,
                format!("POST {path} HTTP/1.1"),
                "{kind}"
            );
            assert!(
                captured.headers.contains("x-goog-api-key: key-canary\r\n"),
                "{kind}"
            );
            assert!(!captured.request_line.contains("key="), "{kind}");
            assert_eq!(
                captured.body["systemInstruction"]["parts"][0]["text"],
                serde_json::json!("system text\n\nscene text"),
                "{kind}"
            );
            assert_eq!(
                captured.body["contents"],
                serde_json::json!([{"role":"user","parts":[{"text":"user text"}]}]),
                "{kind}"
            );
            assert_eq!(
                captured.body["generationConfig"]["maxOutputTokens"],
                serde_json::json!(4096),
                "{kind}"
            );
            assert_eq!(
                captured.body["generationConfig"]["topK"],
                serde_json::json!(40),
                "{kind}"
            );
            assert!(captured.body.get("model").is_none(), "{kind}");
        }
    }

    #[tokio::test]
    async fn gemini_explicit_cache_creates_once_and_reuses_the_resource() {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("key-canary").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        let generate_response = http_json(
            r#"{"candidates":[{"content":{"parts":[{"text":"ok"}],"role":"model"},"finishReason":"STOP"}]}"#,
        );
        let (endpoint, request_receiver) = test_server_many(vec![
            http_json(r#"{"name":"cachedContents/test-resource"}"#),
            generate_response.clone(),
            generate_response,
        ])
        .await;
        let adapter = RemoteProviders::new(
            Arc::clone(&store),
            Arc::new(JsonClient::new().expect("client")),
        );
        let mut resolved = profile(
            "gemini",
            format!("{endpoint}/v1"),
            ProviderConfig::Standard,
            Some(key_ref),
            owner,
        );
        resolved.chat_profile.provider_protocol = ProviderProtocol::Gemini;
        resolved.chat_profile.parameters.prompt_caching =
            Some(lettuce_models::PromptCaching::Enabled {
                retention: lettuce_models::PromptCacheRetention::FiveMinutes,
            });
        let first = request_with_context(resolved.clone(), cache_context());
        let second = request_with_context(resolved, cache_context());
        let (first, second) = tokio::join!(adapter.run(first), adapter.run(second));
        first.expect("first Gemini response");
        second.expect("reused Gemini response");

        let requests = request_receiver.await.expect("requests");
        assert_eq!(requests.len(), 3);
        let create = captured_request(requests[0].clone());
        assert_eq!(create.request_line, "POST /v1beta/cachedContents HTTP/1.1");
        assert!(!create.request_line.contains('?'));
        assert!(create.headers.contains("x-goog-api-key: key-canary\r\n"));
        assert_eq!(
            create.body,
            serde_json::json!({
                "model": "models/test-model",
                "contents": [{"role":"model","parts":[{"text":"prefix text"}]}],
                "systemInstruction": {"role":"user","parts":[{"text":"system text"}]},
                "ttl": "300s"
            })
        );

        for raw in &requests[1..] {
            let generation = captured_request(raw.clone());
            assert_eq!(
                generation.request_line,
                "POST /v1beta/models/test-model:generateContent HTTP/1.1"
            );
            assert!(
                generation
                    .headers
                    .contains("x-goog-api-key: key-canary\r\n")
            );
            assert_eq!(
                generation.body,
                serde_json::json!({
                    "contents": [{"role":"user","parts":[{"text":"live text"}]}],
                    "generationConfig": {"maxOutputTokens":4096,"topK":40},
                    "cachedContent": "cachedContents/test-resource"
                })
            );
            assert!(generation.body.get("systemInstruction").is_none());
        }
    }

    #[tokio::test]
    async fn gemini_cache_skips_short_context_and_falls_back_cleanly() {
        let generate_response = http_json(
            r#"{"candidates":[{"content":{"parts":[{"text":"ok"}],"role":"model"},"finishReason":"STOP"}]}"#,
        );
        let (adapter, resolved, requests, _, _) = cached_gemini_fixture(
            vec![generate_response.clone()],
            lettuce_models::PromptCacheRetention::OneHour,
        )
        .await;
        adapter
            .run(request(resolved))
            .await
            .expect("short context remains usable");
        let requests = requests.await.expect("short request");
        assert_eq!(requests.len(), 1);
        let generation = captured_request(requests[0].clone());
        assert!(generation.request_line.contains(":generateContent"));
        assert!(generation.body.get("cachedContent").is_none());
        assert!(
            generation
                .body
                .get("_lettucePromptCachingEnabled")
                .is_none()
        );
        assert!(generation.body.get("_lettucePromptCachingTtl").is_none());

        let rejected = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".to_owned();
        let (adapter, resolved, requests, _, _) = cached_gemini_fixture(
            vec![rejected, generate_response],
            lettuce_models::PromptCacheRetention::OneHour,
        )
        .await;
        adapter
            .run(request_with_context(resolved, cache_context()))
            .await
            .expect("cache create failure falls back");
        let requests = requests.await.expect("fallback requests");
        assert_eq!(requests.len(), 2);
        assert!(
            captured_request(requests[0].clone())
                .request_line
                .contains("/cachedContents")
        );
        let generation = captured_request(requests[1].clone());
        assert_eq!(
            generation.body["contents"],
            serde_json::json!([
                {"role":"model","parts":[{"text":"prefix text"}]},
                {"role":"user","parts":[{"text":"live text"}]}
            ])
        );
        assert_eq!(
            generation.body["systemInstruction"]["parts"][0]["text"],
            serde_json::json!("system text")
        );
        assert!(generation.body.get("cachedContent").is_none());
        assert!(
            generation
                .body
                .get("_lettucePromptCachingEnabled")
                .is_none()
        );
        assert!(generation.body.get("_lettucePromptCachingTtl").is_none());
    }

    #[tokio::test]
    async fn missing_gemini_cache_retries_the_clean_request_once() {
        let not_found = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_owned();
        let generate_response = http_json(
            r#"{"candidates":[{"content":{"parts":[{"text":"ok"}],"role":"model"},"finishReason":"STOP"}]}"#,
        );
        let (adapter, resolved, requests, _, _) = cached_gemini_fixture(
            vec![
                http_json(r#"{"name":"cachedContents/stale"}"#),
                not_found,
                generate_response,
            ],
            lettuce_models::PromptCacheRetention::FiveMinutes,
        )
        .await;
        adapter
            .run(request_with_context(resolved, cache_context()))
            .await
            .expect("missing cache retries uncached");
        let requests = requests.await.expect("retry requests");
        assert_eq!(requests.len(), 3);
        let cached = captured_request(requests[1].clone());
        assert_eq!(
            cached.body["cachedContent"],
            serde_json::json!("cachedContents/stale")
        );
        let retry = captured_request(requests[2].clone());
        assert!(retry.body.get("cachedContent").is_none());
        assert_eq!(retry.body["contents"].as_array().map(Vec::len), Some(2));
        assert!(retry.body.get("systemInstruction").is_some());
    }

    #[tokio::test]
    async fn missing_gemini_cache_retries_the_clean_stream_once() {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("key-canary").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        let not_found = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_owned();
        let stream_payload = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]},\"finishReason\":\"STOP\"}]}\n\n";
        let stream_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{stream_payload}",
            stream_payload.len()
        );
        let (endpoint, requests) = test_server_many(vec![
            http_json(r#"{"name":"cachedContents/stale"}"#),
            not_found,
            stream_response,
        ])
        .await;
        let runtime = Arc::new(InferenceRuntime::default());
        let runtime_port: Arc<dyn InferenceRuntimePort> = runtime.clone();
        let adapter = RemoteProviders::with_runtime(
            store,
            Arc::new(JsonClient::new().expect("client")),
            runtime_port,
        );
        let mut resolved = profile(
            "gemini",
            format!("{endpoint}/v1"),
            ProviderConfig::Standard,
            Some(key_ref),
            owner,
        );
        resolved.chat_profile.provider_protocol = ProviderProtocol::Gemini;
        resolved.chat_profile.parameters.prompt_caching =
            Some(lettuce_models::PromptCaching::Enabled {
                retention: lettuce_models::PromptCacheRetention::FiveMinutes,
            });
        let sink_id = RequestId::new();
        let _receiver = runtime.register_stream(sink_id).expect("register stream");
        let mut inference = request_with_context(resolved, cache_context());
        inference.stream_sink = Some(sink_id);
        adapter
            .run(inference)
            .await
            .expect("missing cache retries uncached stream");

        let requests = requests.await.expect("stream retry requests");
        assert_eq!(requests.len(), 3);
        let cached = captured_request(requests[1].clone());
        let retry = captured_request(requests[2].clone());
        assert_eq!(
            cached.request_line,
            "POST /v1beta/models/test-model:streamGenerateContent?alt=sse HTTP/1.1"
        );
        assert_eq!(retry.request_line, cached.request_line);
        assert_eq!(
            cached.body["cachedContent"],
            serde_json::json!("cachedContents/stale")
        );
        assert!(retry.body.get("cachedContent").is_none());
        assert_eq!(retry.body["contents"].as_array().map(Vec::len), Some(2));
        assert!(retry.body.get("systemInstruction").is_some());
    }

    #[tokio::test]
    async fn rotating_the_gemini_key_creates_a_new_cache_resource() {
        let generate_response = http_json(
            r#"{"candidates":[{"content":{"parts":[{"text":"ok"}],"role":"model"},"finishReason":"STOP"}]}"#,
        );
        let (adapter, resolved, requests, store, key_ref) = cached_gemini_fixture(
            vec![
                http_json(r#"{"name":"cachedContents/first"}"#),
                generate_response.clone(),
                http_json(r#"{"name":"cachedContents/second"}"#),
                generate_response,
            ],
            lettuce_models::PromptCacheRetention::FiveMinutes,
        )
        .await;
        adapter
            .run(request_with_context(resolved.clone(), cache_context()))
            .await
            .expect("first cached generation");
        let purpose = SecretPurpose::ProviderApiKey {
            owner: resolved.chat_profile.secret_owner_id,
        };
        let generation = store
            .status(&key_ref, &purpose)
            .await
            .expect("key status")
            .generation;
        store
            .put(
                SecretRecord::new(key_ref, purpose),
                SecretValue::new("rotated-key").expect("rotated secret"),
                Some(generation),
            )
            .await
            .expect("rotate key");
        adapter
            .run(request_with_context(resolved, cache_context()))
            .await
            .expect("generation after rotation");

        let requests = requests.await.expect("rotation requests");
        assert_eq!(requests.len(), 4);
        assert!(
            captured_request(requests[0].clone())
                .request_line
                .contains("/cachedContents")
        );
        assert!(
            captured_request(requests[2].clone())
                .request_line
                .contains("/cachedContents")
        );
        assert_eq!(
            captured_request(requests[1].clone()).body["cachedContent"],
            serde_json::json!("cachedContents/first")
        );
        let rotated = captured_request(requests[3].clone());
        assert!(rotated.headers.contains("x-goog-api-key: rotated-key\r\n"));
        assert_eq!(
            rotated.body["cachedContent"],
            serde_json::json!("cachedContents/second")
        );
    }

    #[tokio::test]
    async fn ollama_uses_native_chat_with_options_and_optional_bearer() {
        let ollama_reply = http_json(
            r#"{"model":"test-model","message":{"role":"assistant","content":"ok"},"done":true,"done_reason":"stop","prompt_eval_count":3,"eval_count":1}"#,
        );
        let captured = capture_with(
            "ollama",
            ProviderProtocol::Ollama,
            ProviderConfig::Standard,
            "/v1",
            false,
            ollama_reply.clone(),
        )
        .await;
        assert_eq!(captured.request_line, "POST /api/chat HTTP/1.1");
        assert!(!captured.headers.contains("authorization:"));
        assert_eq!(
            captured.body["messages"],
            serde_json::json!([
                {"role":"system","content":"system text\n\nscene text"},
                {"role":"user","content":"user text"}
            ])
        );
        assert_eq!(captured.body["stream"], serde_json::json!(false));
        assert_eq!(
            captured.body["options"]["num_predict"],
            serde_json::json!(4096)
        );
        assert_eq!(captured.body["options"]["num_ctx"], serde_json::json!(4096));
        assert_eq!(captured.body["options"]["top_k"], serde_json::json!(40));
        assert_eq!(
            captured.body["options"]["frequency_penalty"],
            serde_json::json!(0.25)
        );
        assert!(captured.body.get("think").is_none());

        let captured = capture_with(
            "ollama",
            ProviderProtocol::Ollama,
            ProviderConfig::Standard,
            "/api/chat",
            true,
            ollama_reply,
        )
        .await;
        assert_eq!(captured.request_line, "POST /api/chat HTTP/1.1");
        assert!(
            captured
                .headers
                .contains("authorization: bearer key-canary\r\n")
        );
    }

    fn account(
        kind: &str,
        protocol: ProviderProtocol,
        endpoint: Option<String>,
        config: ProviderConfig,
        key_ref: Option<SecretRef>,
        owner: lettuce_settings::SecretOwnerId,
    ) -> lettuce_models::ProviderAccount {
        lettuce_models::ProviderAccount {
            id: ProviderAccountId::new(),
            secret_owner_id: owner,
            provider_kind: kind.to_owned(),
            protocol,
            label: "Account".to_owned(),
            endpoint,
            enabled: true,
            streaming_enabled: true,
            allow_invalid_tls: false,
            api_key_ref: key_ref,
            secret_headers: Vec::new(),
            config,
            revision: Revision::INITIAL,
            created_at: lettuce_types::TimestampMillis::new(1),
            updated_at: lettuce_types::TimestampMillis::new(1),
        }
    }

    async fn keyed_store() -> (
        Arc<RecordingSecretStore>,
        lettuce_settings::SecretOwnerId,
        SecretRef,
    ) {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("key-canary").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        (store, owner, key_ref)
    }

    #[tokio::test]
    async fn model_listing_follows_each_family_shape() {
        let (store, owner, key_ref) = keyed_store().await;
        let network = Arc::new(JsonClient::new().expect("client"));
        let providers = RemoteProviders::new(Arc::clone(&store), Arc::clone(&network));
        let cases: Vec<(&str, ProviderProtocol, ProviderConfig, &str, &str, &str)> = vec![
            (
                "openrouter",
                ProviderProtocol::OpenAiCompatible,
                ProviderConfig::Standard,
                "/v1",
                "GET /v1/models HTTP/1.1",
                r#"{"data":[{"id":"a/b","name":"A B","context_length":8000,"pricing":{"prompt":"0.1","completion":0.2},"architecture":{"input_modalities":["text","image"]}}]}"#,
            ),
            (
                "groq",
                ProviderProtocol::OpenAiCompatible,
                ProviderConfig::Standard,
                "",
                "GET /openai/v1/models HTTP/1.1",
                r#"{"data":[{"id":"a/b"}]}"#,
            ),
            (
                "anthropic",
                ProviderProtocol::Anthropic,
                ProviderConfig::Standard,
                "",
                "GET /v1/models HTTP/1.1",
                r#"{"data":[{"id":"a/b","display_name":"A B"}]}"#,
            ),
            (
                "gemini",
                ProviderProtocol::Gemini,
                ProviderConfig::Standard,
                "/v1",
                "GET /v1beta/models HTTP/1.1",
                r#"{"models":[{"name":"models/a/b","displayName":"A B","inputTokenLimit":8000}]}"#,
            ),
            (
                "ollama",
                ProviderProtocol::Ollama,
                ProviderConfig::Standard,
                "/v1",
                "GET /api/tags HTTP/1.1",
                r#"{"models":[{"name":"a/b","details":{"parameter_size":"7B"}}]}"#,
            ),
            (
                "custom",
                ProviderProtocol::OpenAiCompatible,
                ProviderConfig::Custom(CustomProviderConfig {
                    chat_path: "/chat".to_owned(),
                    models_path: Some("/list".to_owned()),
                    model_list: lettuce_models::CustomModelList {
                        list_path: lettuce_models::JsonPath::new("result.items").expect("path"),
                        id_path: lettuce_models::JsonPath::new("slug").expect("path"),
                        display_name_path: Some(
                            lettuce_models::JsonPath::new("title").expect("path"),
                        ),
                        description_path: None,
                        context_length_path: Some(
                            lettuce_models::JsonPath::new("limits.ctx").expect("path"),
                        ),
                    },
                    auth: CustomAuth::Bearer,
                    ..Default::default()
                }),
                "",
                "GET /list HTTP/1.1",
                r#"{"result":{"items":[{"slug":"a/b","title":"A B","limits":{"ctx":"8000"}}]}}"#,
            ),
        ];
        for (kind, protocol, config, suffix, request_line, body) in cases {
            let (endpoint, request_receiver) = test_server(http_json(body)).await;
            let models = providers
                .list_models(&account(
                    kind,
                    protocol,
                    Some(format!("{endpoint}{suffix}")),
                    config,
                    Some(key_ref),
                    owner,
                ))
                .await
                .unwrap_or_else(|error| panic!("{kind}: {error:?}"));
            let raw = String::from_utf8(request_receiver.await.expect("request")).expect("HTTP");
            assert!(raw.starts_with(request_line), "{kind}: {raw}");
            assert_eq!(models.len(), 1, "{kind}");
            assert_eq!(models[0].id, "a/b", "{kind}");
            if !matches!(kind, "groq" | "ollama") {
                assert_eq!(models[0].display_name.as_deref(), Some("A B"), "{kind}");
            }
            if matches!(kind, "openrouter" | "gemini" | "custom") {
                assert_eq!(models[0].context_length, Some(8000), "{kind}");
            }
            if kind == "openrouter" {
                assert_eq!(models[0].input_price, Some(0.1));
                assert_eq!(models[0].output_price, Some(0.2));
                assert_eq!(
                    models[0].input_modalities.as_deref(),
                    Some(&["text".to_owned(), "image".to_owned()][..])
                );
            }
        }
        for (kind, protocol) in [
            ("zai", ProviderProtocol::OpenAiCompatible),
            ("intenserp", ProviderProtocol::OpenAiCompatible),
            ("gemini-agent-platform-express", ProviderProtocol::Gemini),
        ] {
            assert!(
                providers
                    .list_models(&account(
                        kind,
                        protocol,
                        Some("http://127.0.0.1:1".to_owned()),
                        ProviderConfig::Standard,
                        Some(key_ref),
                        owner,
                    ))
                    .await
                    .is_err(),
                "{kind}"
            );
        }
        let custom_without_listing = account(
            "custom",
            ProviderProtocol::OpenAiCompatible,
            Some("http://127.0.0.1:1".to_owned()),
            ProviderConfig::Custom(CustomProviderConfig {
                chat_path: "/chat".to_owned(),
                models_path: None,
                auth: CustomAuth::None,
                ..Default::default()
            }),
            None,
            owner,
        );
        assert_eq!(
            providers.list_models(&custom_without_listing).await,
            Err(ProviderRequestError::Rejected)
        );
    }

    #[tokio::test]
    async fn key_verification_probes_like_legacy() {
        let (store, owner, key_ref) = keyed_store().await;
        let providers = RemoteProviders::new(
            Arc::clone(&store),
            Arc::new(JsonClient::new().expect("client")),
        );
        let chutes = account(
            "chutes",
            ProviderProtocol::OpenAiCompatible,
            None,
            ProviderConfig::Standard,
            Some(key_ref),
            owner,
        );
        assert_eq!(
            providers.verify_api_key(&chutes).await,
            Ok(KeyVerification {
                valid: true,
                status: None
            })
        );
        assert!(store.take_loads().is_empty());
        let keyless = account(
            "openai",
            ProviderProtocol::OpenAiCompatible,
            None,
            ProviderConfig::Standard,
            None,
            owner,
        );
        assert_eq!(
            providers.verify_api_key(&keyless).await,
            Ok(KeyVerification {
                valid: false,
                status: None
            })
        );
        type VerifyCase<'a> = (
            &'a str,
            ProviderProtocol,
            &'a str,
            &'a str,
            &'a str,
            &'a str,
            u16,
            bool,
        );
        let cases: Vec<VerifyCase<'_>> = vec![
            (
                "openrouter",
                ProviderProtocol::OpenAiCompatible,
                "",
                "GET /v1/key HTTP/1.1",
                "authorization: bearer key-canary",
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
                200,
                true,
            ),
            (
                "groq",
                ProviderProtocol::OpenAiCompatible,
                "",
                "GET /openai/v1/models HTTP/1.1",
                "authorization: bearer key-canary",
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: 2\r\n\r\n{}",
                401,
                false,
            ),
            (
                "gemini",
                ProviderProtocol::Gemini,
                "/v1",
                "GET /v1/models HTTP/1.1",
                "x-goog-api-key: key-canary",
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
                200,
                true,
            ),
            (
                "z.ai",
                ProviderProtocol::OpenAiCompatible,
                "",
                "POST /chat/completions HTTP/1.1",
                "authorization: bearer key-canary",
                "HTTP/1.1 400 Bad Request\r\nContent-Length: 2\r\n\r\n{}",
                400,
                true,
            ),
            (
                "mistral",
                ProviderProtocol::OpenAiCompatible,
                "/v1",
                "GET /v1/models HTTP/1.1",
                "x-api-key: key-canary",
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
                200,
                true,
            ),
            (
                "anthropic",
                ProviderProtocol::Anthropic,
                "",
                "GET /v1/models HTTP/1.1",
                "anthropic-version: 2023-06-01",
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
                200,
                true,
            ),
            (
                "cerebras",
                ProviderProtocol::OpenAiCompatible,
                "/v1",
                "GET /v1/models HTTP/1.1",
                "authorization: bearer key-canary",
                "HTTP/1.1 429 Too Many\r\nRetry-After: 0\r\nContent-Length: 19\r\n\r\n{\"data\":[{\"id\":1}]}",
                429,
                true,
            ),
        ];
        for (kind, protocol, suffix, request_line, header, reply, status, valid) in cases {
            let (endpoint, request_receiver) = test_server(reply.to_owned()).await;
            let verification = providers
                .verify_api_key(&account(
                    kind,
                    protocol,
                    Some(format!("{endpoint}{suffix}")),
                    ProviderConfig::Standard,
                    Some(key_ref),
                    owner,
                ))
                .await
                .unwrap_or_else(|error| panic!("{kind}: {error:?}"));
            let raw = String::from_utf8(request_receiver.await.expect("request")).expect("HTTP");
            assert!(raw.starts_with(request_line), "{kind}: {raw}");
            assert!(raw.to_ascii_lowercase().contains(header), "{kind}: {raw}");
            assert_eq!(
                verification,
                KeyVerification {
                    valid,
                    status: Some(status)
                },
                "{kind}"
            );
        }
        let (endpoint, request_receiver) =
            test_server("HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\n\r\n{}".to_owned()).await;
        let custom = account(
            "custom-anthropic",
            ProviderProtocol::Anthropic,
            Some(endpoint),
            ProviderConfig::Custom(CustomProviderConfig {
                chat_path: "/v1/messages".to_owned(),
                auth: CustomAuth::Query {
                    name: lettuce_models::QueryParameterName::new("key").expect("query"),
                },
                ..Default::default()
            }),
            Some(key_ref),
            owner,
        );
        assert_eq!(
            providers.verify_api_key(&custom).await,
            Ok(KeyVerification {
                valid: true,
                status: Some(404)
            })
        );
        let raw = String::from_utf8(request_receiver.await.expect("request")).expect("HTTP");
        assert!(
            raw.starts_with("POST /v1/messages?key=key-canary HTTP/1.1"),
            "{raw}"
        );
        assert!(raw.ends_with("null"), "{raw}");
    }

    #[tokio::test]
    async fn protocol_and_kind_must_agree() {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let adapter = RemoteProviders::new(
            Arc::clone(&store),
            Arc::new(JsonClient::new().expect("client")),
        );
        for (kind, protocol) in [
            ("openai", ProviderProtocol::Anthropic),
            ("anthropic", ProviderProtocol::Gemini),
            ("gemini", ProviderProtocol::Ollama),
            ("ollama", ProviderProtocol::OpenAiCompatible),
            ("openai", ProviderProtocol::LlamaCpp),
            ("stability", ProviderProtocol::StableDiffusion),
        ] {
            let mut inference = request(profile(
                kind,
                "http://127.0.0.1:1".to_owned(),
                ProviderConfig::Standard,
                Some(SecretRef::new()),
                owner,
            ));
            inference.profile.chat_profile.provider_protocol = protocol;
            assert_eq!(
                adapter.run(inference).await,
                Err(lettuce_conversations::PortError::Rejected),
                "{kind}"
            );
        }
        assert!(store.take_loads().is_empty());
    }

    #[tokio::test]
    async fn unknown_kinds_and_missing_required_endpoints_are_rejected() {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let adapter = RemoteProviders::new(
            Arc::clone(&store),
            Arc::new(JsonClient::new().expect("client")),
        );
        for kind in [
            "lettuce-host",
            "lettuce-engine",
            "ollama",
            "anthropic",
            "gemini",
            "llamacpp",
            "stability",
            "",
        ] {
            let mut inference = request(profile(
                kind,
                "https://example.invalid".to_owned(),
                ProviderConfig::Standard,
                Some(SecretRef::new()),
                owner,
            ));
            inference.profile.chat_profile.endpoint = None;
            assert_eq!(
                adapter.run(inference).await,
                Err(lettuce_conversations::PortError::Rejected),
                "{kind}"
            );
        }
        let mut inference = request(profile(
            "lmstudio",
            String::new(),
            ProviderConfig::Standard,
            None,
            owner,
        ));
        inference.profile.chat_profile.endpoint = None;
        assert_eq!(
            adapter.run(inference).await,
            Err(lettuce_conversations::PortError::Rejected)
        );
        assert!(store.take_loads().is_empty());
    }

    #[tokio::test]
    async fn native_openai_does_not_double_a_versioned_endpoint() {
        let store = Arc::new(RecordingSecretStore::default());
        let owner = lettuce_settings::SecretOwnerId::new();
        let key_ref = SecretRef::new();
        store
            .put(
                SecretRecord::new(key_ref, SecretPurpose::ProviderApiKey { owner }),
                SecretValue::new("native-canary").expect("secret"),
                None,
            )
            .await
            .expect("store key");
        let (endpoint, request_receiver) = test_server(response_body()).await;
        let adapter = RemoteProviders::new(
            Arc::clone(&store),
            Arc::new(JsonClient::new().expect("client")),
        );
        adapter
            .run(request(profile(
                "openai",
                format!("{endpoint}/v1/"),
                ProviderConfig::Standard,
                Some(key_ref),
                owner,
            )))
            .await
            .expect("native response");
        let raw = String::from_utf8(request_receiver.await.expect("request")).expect("HTTP");
        assert!(
            raw.starts_with("POST /v1/chat/completions HTTP/1.1"),
            "{raw}"
        );
    }
}
