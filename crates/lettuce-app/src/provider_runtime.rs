use std::sync::Arc;

use async_trait::async_trait;
use lettuce_contracts::{
    ApiKeyRequirementContract, KeyVerificationContract, PromptCacheRetentionContract,
    PromptCachingSupportContract, ProviderAccountRequest, ProviderCatalogContract,
    ProviderDescriptorContract, ProviderModelsContract, ProviderParameterSupportContract,
    ProviderProtocolContract, ReasoningSupportContract, RemoteModelContract,
};
use lettuce_conversations::{InferenceOutcome, InferencePort, InferenceRequest, PortError};
use lettuce_database::Database;
use lettuce_inference::{InferenceRuntime, InferenceRuntimePort};
use lettuce_models::{ModelRepositoryError, ProviderAccountRepository, ProviderProtocol};
use lettuce_network::{JsonClient, JsonClientError, TlsPolicy};
use lettuce_providers::{
    ApiKeyRequirement, PromptCachingSupport, ProviderRequestError, ReasoningSupport,
    RemoteProviders, provider_descriptors,
};
use lettuce_settings::SecretStore;

/// Application-facing provider operations. It owns one configured HTTP client
/// and loads provider accounts through the typed repository boundary.
pub struct ProviderRuntime<S: ?Sized> {
    database: Arc<Database>,
    inference_runtime: Arc<InferenceRuntime>,
    remote: RemoteProviders<S>,
}

impl<S: ?Sized> std::fmt::Debug for ProviderRuntime<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProviderRuntime")
    }
}

impl<S: SecretStore + ?Sized> ProviderRuntime<S> {
    pub fn new(
        database: Arc<Database>,
        secret_store: Arc<S>,
        tls_policy: &TlsPolicy,
    ) -> Result<Self, ProviderRuntimeInitializationError> {
        let network = Arc::new(
            JsonClient::with_tls(tls_policy)
                .map_err(ProviderRuntimeInitializationError::Network)?,
        );
        let inference_runtime = Arc::new(InferenceRuntime::default());
        let runtime_port: Arc<dyn InferenceRuntimePort> = inference_runtime.clone();
        Ok(Self {
            database,
            inference_runtime,
            remote: RemoteProviders::with_runtime(secret_store, network, runtime_port),
        })
    }

    /// Runtime registry used by generation flows to attach a bounded stream
    /// consumer or cancellation token before calling the inference port.
    #[must_use]
    pub fn inference_runtime(&self) -> Arc<InferenceRuntime> {
        Arc::clone(&self.inference_runtime)
    }

    #[must_use]
    pub fn catalog(&self) -> ProviderCatalogContract {
        ProviderCatalogContract {
            providers: provider_descriptors()
                .iter()
                .map(|descriptor| ProviderDescriptorContract {
                    kind: descriptor.kind.to_owned(),
                    display_name: descriptor.display_name.to_owned(),
                    protocol: protocol_contract(descriptor.protocol),
                    aliases: descriptor
                        .aliases
                        .iter()
                        .map(|alias| (*alias).to_owned())
                        .collect(),
                    default_endpoint: descriptor.default_endpoint.map(str::to_owned),
                    endpoint_editable: descriptor.endpoint_editable,
                    api_key: match descriptor.api_key {
                        ApiKeyRequirement::Required => ApiKeyRequirementContract::Required,
                        ApiKeyRequirement::Optional => ApiKeyRequirementContract::Optional,
                        ApiKeyRequirement::NotUsed => ApiKeyRequirementContract::NotUsed,
                    },
                    auth_header: descriptor.auth_header.to_owned(),
                    streaming: descriptor.streaming,
                    tools: descriptor.supports_tools(),
                    structured_output: descriptor.supports_structured_output(),
                    signed_tool_replay: descriptor.supports_signed_tool_replay(),
                    reasoning_with_tools: descriptor.supports_reasoning_with_tools(),
                    lists_models: descriptor.lists_models,
                    verifies_key: descriptor.verifies_key,
                    reasoning: match descriptor.reasoning {
                        ReasoningSupport::None => ReasoningSupportContract::None,
                        ReasoningSupport::Effort => ReasoningSupportContract::Effort,
                        ReasoningSupport::BudgetOnly => ReasoningSupportContract::BudgetOnly,
                        ReasoningSupport::Dynamic => ReasoningSupportContract::Dynamic,
                    },
                    prompt_caching: match descriptor.prompt_caching {
                        PromptCachingSupport::None => PromptCachingSupportContract::None,
                        PromptCachingSupport::Automatic => PromptCachingSupportContract::Automatic,
                        PromptCachingSupport::CacheControl
                        | PromptCachingSupport::ExplicitResource
                        | PromptCachingSupport::RequestRetention => {
                            PromptCachingSupportContract::Supported
                        }
                    },
                    prompt_cache_retentions: descriptor
                        .prompt_caching
                        .retentions()
                        .iter()
                        .map(|retention| match retention {
                            lettuce_models::PromptCacheRetention::InMemory => {
                                PromptCacheRetentionContract::InMemory
                            }
                            lettuce_models::PromptCacheRetention::FiveMinutes => {
                                PromptCacheRetentionContract::FiveMinutes
                            }
                            lettuce_models::PromptCacheRetention::OneHour => {
                                PromptCacheRetentionContract::OneHour
                            }
                            lettuce_models::PromptCacheRetention::TwentyFourHours => {
                                PromptCacheRetentionContract::TwentyFourHours
                            }
                        })
                        .collect(),
                    parameters: ProviderParameterSupportContract {
                        temperature: descriptor.parameters.temperature,
                        top_p: descriptor.parameters.top_p,
                        max_output_tokens: descriptor.parameters.max_output_tokens,
                        context_length: descriptor.parameters.context_length,
                        frequency_penalty: descriptor.parameters.frequency_penalty,
                        presence_penalty: descriptor.parameters.presence_penalty,
                        top_k: descriptor.parameters.top_k,
                        reasoning_budget: descriptor.parameters.reasoning_budget,
                    },
                    extra_body_keys: descriptor
                        .extra_body_keys
                        .iter()
                        .map(|key| (*key).to_owned())
                        .collect(),
                })
                .collect(),
        }
    }

    pub async fn list_models(
        &self,
        request: ProviderAccountRequest,
    ) -> Result<ProviderModelsContract, ProviderRuntimeError> {
        let account = self.account(request.provider_account_id)?;
        let models = self
            .remote
            .list_models(&account)
            .await
            .map_err(ProviderRuntimeError::Provider)?
            .into_iter()
            .map(|model| RemoteModelContract {
                id: model.id,
                display_name: model.display_name,
                description: model.description,
                context_length: model.context_length,
                input_modalities: model.input_modalities,
                output_modalities: model.output_modalities,
                supported_endpoints: model.supported_endpoints,
                input_price: model.input_price,
                output_price: model.output_price,
            })
            .collect();
        Ok(ProviderModelsContract {
            provider_account_id: request.provider_account_id,
            models,
        })
    }

    pub async fn verify_api_key(
        &self,
        request: ProviderAccountRequest,
    ) -> Result<KeyVerificationContract, ProviderRuntimeError> {
        let account = self.account(request.provider_account_id)?;
        let verification = self
            .remote
            .verify_api_key(&account)
            .await
            .map_err(ProviderRuntimeError::Provider)?;
        Ok(KeyVerificationContract {
            provider_account_id: request.provider_account_id,
            valid: verification.valid,
            status: verification.status,
        })
    }

    fn account(
        &self,
        id: lettuce_types::ProviderAccountId,
    ) -> Result<lettuce_models::ProviderAccount, ProviderRuntimeError> {
        ProviderAccountRepository::get(self.database.as_ref(), id)
            .map_err(ProviderRuntimeError::Storage)?
            .ok_or(ProviderRuntimeError::AccountNotFound)
    }
}

#[async_trait]
impl<S: SecretStore + ?Sized> InferencePort for ProviderRuntime<S> {
    async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError> {
        self.remote.run(request).await
    }
}

fn protocol_contract(protocol: ProviderProtocol) -> ProviderProtocolContract {
    match protocol {
        ProviderProtocol::OpenAiCompatible => ProviderProtocolContract::OpenAiCompatible,
        ProviderProtocol::Anthropic => ProviderProtocolContract::Anthropic,
        ProviderProtocol::Gemini => ProviderProtocolContract::Gemini,
        ProviderProtocol::Ollama => ProviderProtocolContract::Ollama,
        ProviderProtocol::LlamaCpp => ProviderProtocolContract::LlamaCpp,
        ProviderProtocol::StableDiffusion => ProviderProtocolContract::StableDiffusion,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderRuntimeInitializationError {
    #[error("provider HTTP client initialization failed")]
    Network(#[source] JsonClientError),
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderRuntimeError {
    #[error("provider account was not found")]
    AccountNotFound,
    #[error("provider account storage failed")]
    Storage(#[source] ModelRepositoryError),
    #[error("provider request failed")]
    Provider(#[source] ProviderRequestError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_models::{ProviderAccount, ProviderConfig};
    use lettuce_settings::{InMemorySecretStore, SecretOwnerId};
    use lettuce_types::{ProviderAccountId, Revision, TimestampMillis};

    fn runtime() -> ProviderRuntime<InMemorySecretStore> {
        ProviderRuntime::new(
            Arc::new(Database::open_in_memory().expect("database")),
            Arc::new(InMemorySecretStore::new()),
            &TlsPolicy::default(),
        )
        .expect("runtime")
    }

    #[test]
    fn catalog_is_complete_and_transport_owned() {
        let catalog = runtime().catalog();
        assert_eq!(catalog.providers.len(), 25);
        let openrouter = catalog
            .providers
            .iter()
            .find(|provider| provider.kind == "openrouter")
            .expect("openrouter");
        assert_eq!(
            openrouter.protocol,
            ProviderProtocolContract::OpenAiCompatible
        );
        assert!(openrouter.parameters.context_length);
        assert!(openrouter.tools);
        assert!(!openrouter.structured_output);
        assert!(!openrouter.signed_tool_replay);
        assert!(openrouter.reasoning_with_tools);
        assert!(openrouter.extra_body_keys.contains(&"provider".to_owned()));
        assert_eq!(
            openrouter.prompt_cache_retentions,
            vec![
                PromptCacheRetentionContract::FiveMinutes,
                PromptCacheRetentionContract::OneHour,
            ]
        );
        let gemini = catalog
            .providers
            .iter()
            .find(|provider| provider.kind == "gemini")
            .expect("gemini");
        assert_eq!(
            gemini.prompt_cache_retentions,
            vec![
                PromptCacheRetentionContract::FiveMinutes,
                PromptCacheRetentionContract::OneHour,
            ]
        );
        assert!(gemini.tools);
        assert!(!gemini.reasoning_with_tools);
        assert!(!gemini.signed_tool_replay);
        let anthropic = catalog
            .providers
            .iter()
            .find(|provider| provider.kind == "anthropic")
            .expect("anthropic");
        assert!(anthropic.tools);
        assert!(!anthropic.reasoning_with_tools);
        assert!(!anthropic.signed_tool_replay);
        let ollama = catalog
            .providers
            .iter()
            .find(|provider| provider.kind == "ollama")
            .expect("ollama");
        assert!(ollama.tools);
        assert!(!ollama.reasoning_with_tools);
        assert!(
            catalog
                .providers
                .iter()
                .all(|provider| !provider.structured_output && !provider.signed_tool_replay)
        );
        assert!(catalog.providers.iter().all(|provider| {
            !provider
                .extra_body_keys
                .contains(&"promptCachingTtl".to_owned())
        }));
        assert!(
            catalog
                .providers
                .iter()
                .all(|provider| provider.kind != "lettuce-engine")
        );
    }

    #[tokio::test]
    async fn account_operations_resolve_through_the_database() {
        let runtime = runtime();
        let missing = ProviderAccountRequest {
            provider_account_id: ProviderAccountId::new(),
        };
        assert!(matches!(
            runtime.verify_api_key(missing).await,
            Err(ProviderRuntimeError::AccountNotFound)
        ));

        let id = ProviderAccountId::new();
        ProviderAccountRepository::upsert(
            runtime.database.as_ref(),
            ProviderAccount {
                id,
                secret_owner_id: SecretOwnerId::new(),
                provider_kind: "intenserp".into(),
                protocol: ProviderProtocol::OpenAiCompatible,
                label: "IntenseRP".into(),
                endpoint: Some("http://127.0.0.1:7777/v1".into()),
                enabled: true,
                streaming_enabled: true,
                allow_invalid_tls: false,
                api_key_ref: None,
                secret_headers: Vec::new(),
                config: ProviderConfig::Standard,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            None,
        )
        .expect("account");

        let verification = runtime
            .verify_api_key(ProviderAccountRequest {
                provider_account_id: id,
            })
            .await
            .expect("verification");
        assert!(!verification.valid);
        assert_eq!(verification.status, None);
    }
}
