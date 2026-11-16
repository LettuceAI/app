use lettuce_image_generation::LOCAL_DIFFUSION_PROVIDER_KIND;
use lettuce_models::{
    CapabilityEvidence, CapabilityEvidenceSource, CapabilityStatus, ModalityCapabilities,
    ModelCapabilities, ModelCatalog, ModelKind, ModelProfile, ModelProfileConfig,
    ModelProfileRepository, ModelRepositoryError, ParameterSupport, ProviderAccount,
    ProviderAccountRepository, ProviderConfig, ProviderProtocol,
};
use lettuce_settings::SecretOwnerId;
use lettuce_transfer::ModelTransferError;
use lettuce_types::{ModelProfileId, ProviderAccountId, Revision, TimestampMillis};

use crate::{
    LOCAL_DIFFUSION_PROVIDER_LABEL, LOCAL_LLAMA_PROVIDER_KIND, LOCAL_LLAMA_PROVIDER_LABEL,
};

#[derive(Debug, thiserror::Error)]
pub enum ModelFileError {
    #[error(transparent)]
    Format(#[from] ModelTransferError),
    #[error("model storage failed: {0}")]
    Repository(#[from] ModelRepositoryError),
    #[error("No provider account for {0}; add one before importing this model.")]
    NoProviderAccount(String),
    #[error("The model's settings are invalid.")]
    InvalidSettings,
    #[error("Model not found")]
    NotFound,
}

/// A model file written as a new model profile.
#[derive(Debug, Clone)]
pub struct ImportedModelFile {
    pub profile: ModelProfile,
    /// Settings the file held that were out of range or of the wrong type.
    pub lossy_fields: Vec<String>,
    /// Settings the file held that the app does not know.
    pub unknown_fields: Vec<String>,
}

/// Model files read into model profiles.
#[derive(Debug)]
pub struct ModelFileCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: ?Sized> ModelFileCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }
}

fn modalities(scopes: &[String]) -> ModalityCapabilities {
    let status = |scope: &str| {
        if scopes.iter().any(|listed| listed == scope) {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unsupported
        }
    };
    ModalityCapabilities {
        text: status("text"),
        image: status("image"),
        audio: status("audio"),
    }
}

impl<R> ModelFileCoordinator<'_, R>
where
    R: ModelCatalog
        + ModelProfileRepository
        + ProviderAccountRepository
        + lettuce_settings::GlobalSettingsStore
        + ?Sized,
{
    /// Creates a chat model from the file on the provider account of its
    /// provider and label, else the provider's only account.
    pub fn import(
        &self,
        json: &str,
        now: TimestampMillis,
    ) -> Result<ImportedModelFile, ModelFileError> {
        self.create(lettuce_transfer::parse_model_import(json)?, now)
    }

    /// Creates a chat model from a model in the old app's shape; it becomes
    /// the default model when none is set (the model is kept when that fails).
    pub fn create(
        &self,
        imported: lettuce_transfer::ImportedModel,
        now: TimestampMillis,
    ) -> Result<ImportedModelFile, ModelFileError> {
        let mut prepared = self.prepare(imported, now)?;
        prepared.profile = ModelProfileRepository::upsert(self.repository, prepared.profile, None)?;
        match self.repository.load() {
            Ok(stored) if stored.default_model_profile_id.is_none() => {
                if let Err(error) = self.repository.save(
                    stored.settings,
                    Some(prepared.profile.id),
                    stored.revision,
                ) {
                    tracing::warn!(%error, "the new model could not become the default model");
                }
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "the default model could not be checked");
            }
        }
        Ok(prepared)
    }

    pub fn prepare(
        &self,
        imported: lettuce_transfer::ImportedModel,
        now: TimestampMillis,
    ) -> Result<ImportedModelFile, ModelFileError> {
        let account =
            self.account_for_import(&imported.provider_id, &imported.provider_label, now, true)?;
        let parameters = lettuce_transfer::legacy_model_parameters(
            &imported.provider_id,
            &imported.advanced_model_settings,
        );
        let config = ModelProfileConfig {
            chat_parameters: parameters.chat_parameters,
            feature_parameters: parameters.feature_parameters,
            llama_cpp: parameters.llama_cpp,
            stable_diffusion: parameters.stable_diffusion,
            capabilities: ModelCapabilities {
                format_version: lettuce_models::MODEL_CAPABILITIES_FORMAT_VERSION,
                evidence: CapabilityEvidence {
                    source: CapabilityEvidenceSource::UserOverride,
                    source_version: 1,
                    observed_at: now,
                },
                input_modalities: modalities(&imported.input_scopes),
                output_modalities: modalities(&imported.output_scopes),
                streaming: if account.streaming_enabled {
                    CapabilityStatus::Supported
                } else {
                    CapabilityStatus::Unsupported
                },
                tools: CapabilityStatus::Unknown,
                structured_output: CapabilityStatus::Unknown,
                reasoning: CapabilityStatus::Unknown,
                prompt_cache: CapabilityStatus::Unknown,
                context_length: None,
                max_visible_output_tokens: None,
                max_total_completion_tokens: None,
                parameter_support: ParameterSupport::default(),
            },
        };
        config
            .validate_parameters()
            .map_err(|_| ModelFileError::InvalidSettings)?;
        let profile = ModelProfile {
            id: ModelProfileId::new(),
            provider_account_id: account.id,
            external_model_id: imported.name,
            display_name: imported.display_name,
            kind: ModelKind::Chat,
            config,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        Ok(ImportedModelFile {
            profile,
            lossy_fields: parameters.lossy_fields,
            unknown_fields: parameters.unknown_fields,
        })
    }

    /// The provider account a model file imports onto. A local model
    /// (llama.cpp, stable-diffusion.cpp) needs no provider set up first: it
    /// falls back to its managed account, created when `persist` and missing.
    pub(crate) fn account_for_import(
        &self,
        provider: &str,
        label: &str,
        now: TimestampMillis,
        persist: bool,
    ) -> Result<ProviderAccount, ModelFileError> {
        if let Some(account) = self.lookup_account(provider, label)? {
            return Ok(account);
        }
        let Some(protocol) = local_protocol(provider) else {
            return Err(ModelFileError::NoProviderAccount(provider.to_owned()));
        };
        let account = managed_local_account(protocol, now);
        if !persist {
            return Ok(account);
        }
        Ok(ProviderAccountRepository::upsert(self.repository, account, None)?)
    }

    fn lookup_account(
        &self,
        provider: &str,
        label: &str,
    ) -> Result<Option<ProviderAccount>, ModelFileError> {
        let local = provider.eq_ignore_ascii_case(LOCAL_LLAMA_PROVIDER_KIND);
        let candidates = self
            .repository
            .provider_accounts()?
            .into_iter()
            .filter(|account| {
                account.provider_kind == provider
                    || (local && account.provider_kind.eq_ignore_ascii_case(provider))
            })
            .collect::<Vec<_>>();
        if let Some(account) = candidates.iter().find(|account| account.label == label) {
            return Ok(Some(account.clone()));
        }
        Ok(match candidates.as_slice() {
            [only] => Some(only.clone()),
            _ => None,
        })
    }
}

/// The protocol of a local runtime provider kind, if it is one.
fn local_protocol(provider: &str) -> Option<ProviderProtocol> {
    if provider.eq_ignore_ascii_case(LOCAL_LLAMA_PROVIDER_KIND) {
        Some(ProviderProtocol::LlamaCpp)
    } else if provider.eq_ignore_ascii_case(LOCAL_DIFFUSION_PROVIDER_KIND) {
        Some(ProviderProtocol::StableDiffusion)
    } else {
        None
    }
}

/// The managed account for a local runtime, matching what the download and
/// install flows create so there is only ever one per runtime.
fn managed_local_account(protocol: ProviderProtocol, now: TimestampMillis) -> ProviderAccount {
    let (kind, label) = match protocol {
        ProviderProtocol::StableDiffusion => {
            (LOCAL_DIFFUSION_PROVIDER_KIND, LOCAL_DIFFUSION_PROVIDER_LABEL)
        }
        _ => (LOCAL_LLAMA_PROVIDER_KIND, LOCAL_LLAMA_PROVIDER_LABEL),
    };
    ProviderAccount {
        id: ProviderAccountId::new(),
        secret_owner_id: SecretOwnerId::new(),
        provider_kind: kind.to_owned(),
        protocol,
        label: label.to_owned(),
        endpoint: None,
        enabled: true,
        streaming_enabled: true,
        allow_invalid_tls: false,
        api_key_ref: None,
        secret_headers: Vec::new(),
        config: ProviderConfig::Standard,
        revision: Revision::INITIAL,
        created_at: now,
        updated_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_file_round_trips_onto_the_matching_provider_account() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let seeded = crate::launch::tests::seed_model(
            backend.database(),
            lettuce_models::ProviderProtocol::Anthropic,
            "anthropic",
        );
        let files = backend.model_files();
        let original = ModelProfileRepository::get(backend.database(), seeded)
            .expect("get")
            .expect("seeded");
        let file = serde_json::json!({
            "schema": {"name": "USC", "version": "1.0"},
            "kind": "model_profile",
            "payload": {
                "name": original.external_model_id,
                "providerId": "anthropic",
                "advancedModelSettings": {"temperature": 0.7, "maxOutputTokens": 1024, "contextLength": 8192}
            }
        });
        let imported = files
            .import(&file.to_string(), TimestampMillis::new(9))
            .expect("import");
        assert_ne!(imported.profile.id, seeded);
        assert_eq!(
            imported.profile.provider_account_id,
            original.provider_account_id
        );
        assert_eq!(imported.profile.external_model_id, "vendor/model");
        assert_eq!(
            imported.profile.config.chat_parameters,
            original.config.chat_parameters
        );
        assert!(imported.lossy_fields.is_empty());
        assert!(matches!(
            files.import(
                r#"{"name": "m", "providerId": "nobody"}"#,
                TimestampMillis::new(9)
            ),
            Err(ModelFileError::NoProviderAccount(provider)) if provider == "nobody"
        ));
    }

    #[test]
    fn a_local_model_file_imports_onto_a_freshly_created_managed_account() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let files = backend.model_files();
        assert!(
            ModelCatalog::provider_accounts(backend.database())
                .expect("accounts")
                .is_empty(),
            "no provider account should exist yet"
        );
        let file = serde_json::json!({
            "name": "/models/mira-8b.gguf",
            "providerId": LOCAL_LLAMA_PROVIDER_KIND,
            "providerLabel": LOCAL_LLAMA_PROVIDER_LABEL,
        });
        let imported = files
            .import(&file.to_string(), TimestampMillis::new(9))
            .expect("local import succeeds without a provider set up first");
        let account = ModelCatalog::provider_accounts(backend.database())
            .expect("accounts")
            .into_iter()
            .find(|account| account.id == imported.profile.provider_account_id)
            .expect("managed account created");
        assert_eq!(account.provider_kind, LOCAL_LLAMA_PROVIDER_KIND);
        assert_eq!(account.label, LOCAL_LLAMA_PROVIDER_LABEL);
        assert_eq!(account.protocol, ProviderProtocol::LlamaCpp);

        // A second local import reuses the managed account rather than making another.
        files
            .import(
                &serde_json::json!({
                    "name": "/models/other.gguf",
                    "providerId": LOCAL_LLAMA_PROVIDER_KIND,
                    "providerLabel": LOCAL_LLAMA_PROVIDER_LABEL,
                })
                .to_string(),
                TimestampMillis::new(10),
            )
            .expect("second local import");
        let llama_accounts = ModelCatalog::provider_accounts(backend.database())
            .expect("accounts")
            .into_iter()
            .filter(|account| account.provider_kind == LOCAL_LLAMA_PROVIDER_KIND)
            .count();
        assert_eq!(llama_accounts, 1, "only one managed llama.cpp account");
    }
}
