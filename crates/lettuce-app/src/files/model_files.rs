use lettuce_models::{
    CapabilityEvidence, CapabilityEvidenceSource, CapabilityStatus, ModalityCapabilities,
    ModelCapabilities, ModelCatalog, ModelKind, ModelProfile, ModelProfileConfig,
    ModelImportRepository, ModelRepositoryError, ParameterSupport, ProviderAccount,
};
use lettuce_transfer::ModelTransferError;
use lettuce_types::{ModelProfileId, Revision, TimestampMillis};

use crate::models::local_provider_accounts::LocalModelProvider;

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
    pub(crate) account: ProviderAccount,
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
    R: ModelCatalog + ?Sized,
{
    /// Creates a chat model from the file on the provider account of its
    /// provider and label, else the provider's only account.
    pub fn import(
        &self,
        json: &str,
        now: TimestampMillis,
    ) -> Result<ImportedModelFile, ModelFileError>
    where
        R: ModelImportRepository,
    {
        self.create(lettuce_transfer::parse_model_import(json)?, now)
    }

    /// Creates a chat model from a model in the old app's shape; it becomes
    /// the default model when none is set. Account and model creation are atomic.
    pub fn create(
        &self,
        imported: lettuce_transfer::ImportedModel,
        now: TimestampMillis,
    ) -> Result<ImportedModelFile, ModelFileError>
    where
        R: ModelImportRepository,
    {
        let mut prepared = self.prepare(imported, now)?;
        prepared.profile = self.repository.import_model_profile(
            prepared.profile,
            prepared.account.clone(),
        )?;
        Ok(prepared)
    }

    pub fn prepare(
        &self,
        imported: lettuce_transfer::ImportedModel,
        now: TimestampMillis,
    ) -> Result<ImportedModelFile, ModelFileError> {
        let account =
            self.account_for_import(&imported.provider_id, &imported.provider_label, now)?;
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
            account,
            lossy_fields: parameters.lossy_fields,
            unknown_fields: parameters.unknown_fields,
        })
    }

    /// The provider account a model file imports onto. A local model
    /// (llama.cpp, stable-diffusion.cpp) needs no provider set up first: it
    /// falls back to its managed account. A new account is only a draft here;
    /// the import transaction persists it together with the model.
    pub(crate) fn account_for_import(
        &self,
        provider: &str,
        label: &str,
        now: TimestampMillis,
    ) -> Result<ProviderAccount, ModelFileError> {
        if let Some(account) = self.lookup_account(provider, label)? {
            return Ok(account);
        }
        let Some(local) = LocalModelProvider::from_kind(provider) else {
            return Err(ModelFileError::NoProviderAccount(provider.to_owned()));
        };
        Ok(local.new_account(now))
    }

    fn lookup_account(
        &self,
        provider: &str,
        label: &str,
    ) -> Result<Option<ProviderAccount>, ModelFileError> {
        let local = LocalModelProvider::from_kind(provider);
        let candidates = self
            .repository
            .provider_accounts()?
            .into_iter()
            .filter(|account| {
                match local {
                    Some(local) => account.provider_kind.eq_ignore_ascii_case(provider)
                        && account.protocol == local.protocol(),
                    None => account.provider_kind == provider,
                }
            })
            .collect::<Vec<_>>();
        if let Some(account) = candidates.iter().find(|account| account.label == label) {
            return Ok(Some(account.clone()));
        }
        if let Some(local) = local
            && let Some(account) = local.managed_labels().iter().find_map(|label| {
                candidates.iter().find(|account| account.label == *label)
            })
        {
            return Ok(Some(account.clone()));
        }
        Ok(match candidates.as_slice() {
            [only] => Some(only.clone()),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_models::{ModelProfileRepository, ProviderAccountRepository, ProviderProtocol};
    use crate::{LOCAL_LLAMA_PROVIDER_KIND, LOCAL_LLAMA_PROVIDER_LABEL};

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

    #[test]
    fn preparing_local_model_imports_does_not_create_accounts() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        for local in [LocalModelProvider::LlamaCpp, LocalModelProvider::StableDiffusion] {
            let account = local.new_account(TimestampMillis::new(2));
            let model = lettuce_transfer::parse_model_import(&serde_json::json!({
                "name": "/models/model.gguf", "providerId": account.provider_kind,
            }).to_string()).expect("model");
            backend.model_files().prepare(model, TimestampMillis::new(2)).expect("prepare");
        }
        assert!(ModelCatalog::provider_accounts(backend.database()).expect("accounts").is_empty());
        assert!(ModelCatalog::model_profiles(backend.database()).expect("models").is_empty());
    }

    #[test]
    fn local_model_imports_reuse_managed_accounts_when_labels_differ() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let database = backend.database();
        for local in [LocalModelProvider::LlamaCpp, LocalModelProvider::StableDiffusion] {
            let managed = local.new_account(TimestampMillis::new(2));
            let mut custom = local.new_account(TimestampMillis::new(3));
            custom.label = "Custom account".into();
            ProviderAccountRepository::upsert(database, managed.clone(), None).expect("managed");
            ProviderAccountRepository::upsert(database, custom.clone(), None).expect("custom");
            assert_eq!(backend.model_files().account_for_import(
                &managed.provider_kind, &custom.label, TimestampMillis::new(4),
            ).expect("explicit label").id, custom.id);
            for _ in 0..2 {
                let imported = backend.model_files().import(&serde_json::json!({
                    "name": "/models/model.gguf", "providerId": managed.provider_kind.to_uppercase(),
                    "providerLabel": "Different label on the source device",
                }).to_string(), TimestampMillis::new(4)).expect("import");
                assert_eq!(imported.profile.provider_account_id, managed.id);
            }
        }
        assert_eq!(ModelCatalog::provider_accounts(database).expect("accounts").len(), 4);
    }

    #[test]
    fn local_model_imports_and_installers_reuse_the_old_diffusion_account_label() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let database = backend.database();
        let local = LocalModelProvider::StableDiffusion;
        let mut old = local.new_account(TimestampMillis::new(2));
        old.label = lettuce_models::LEGACY_LOCAL_DIFFUSION_PROVIDER_LABEL.into();
        let mut custom = local.new_account(TimestampMillis::new(3));
        custom.label = "Custom account".into();
        ProviderAccountRepository::upsert(database, old.clone(), None).expect("old account");
        ProviderAccountRepository::upsert(database, custom, None).expect("custom account");
        assert_eq!(local.ensure_account(database, TimestampMillis::new(4)).expect("installer").id, old.id);
        let imported = backend.model_files().import(&serde_json::json!({
            "name": "/models/model.gguf", "providerId": old.provider_kind,
            "providerLabel": "A label from another device",
        }).to_string(), TimestampMillis::new(4)).expect("import");
        assert_eq!(imported.profile.provider_account_id, old.id);
        assert_eq!(ModelCatalog::provider_accounts(database).expect("accounts").len(), 2);
    }

    #[test]
    fn local_model_imports_resolve_managed_accounts_again_at_commit() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let model = lettuce_transfer::parse_model_import(&serde_json::json!({
            "name": "/models/model.gguf", "providerId": LOCAL_LLAMA_PROVIDER_KIND,
        }).to_string()).expect("model");
        let first = backend.model_files().prepare(model.clone(), TimestampMillis::new(2)).expect("first");
        let second = backend.model_files().prepare(model, TimestampMillis::new(3)).expect("second");
        assert_ne!(first.account.id, second.account.id);
        let first = ModelImportRepository::import_model_profile(
            backend.database(), first.profile, first.account,
        ).expect("first commit");
        let second = ModelImportRepository::import_model_profile(
            backend.database(), second.profile, second.account,
        ).expect("second commit");
        assert_eq!(first.provider_account_id, second.provider_account_id);
        assert_eq!(ModelCatalog::provider_accounts(backend.database()).expect("accounts").len(), 1);
    }

    #[test]
    fn failed_local_model_imports_roll_back_accounts_models_and_defaults() {
        use lettuce_settings::GlobalSettingsStore;
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let database = backend.database();
        let model = lettuce_transfer::parse_model_import(&serde_json::json!({
            "name": "/models/model.gguf", "providerId": LOCAL_LLAMA_PROVIDER_KIND,
        }).to_string()).expect("model");
        let mut prepared = backend.model_files().prepare(model, TimestampMillis::new(2)).expect("prepare");
        prepared.profile.display_name.clear();
        assert!(ModelImportRepository::import_model_profile(
            database, prepared.profile, prepared.account,
        ).is_err());
        assert!(ModelCatalog::provider_accounts(database).expect("accounts").is_empty());
        assert!(ModelCatalog::model_profiles(database).expect("models").is_empty());
        assert!(database.load().expect("settings").default_model_profile_id.is_none());
    }
}
