use lettuce_models::{
    CapabilityEvidence, CapabilityEvidenceSource, CapabilityStatus, ModalityCapabilities,
    ModelCapabilities, ModelCatalog, ModelKind, ModelProfile, ModelProfileConfig,
    ModelProfileRepository, ModelRepositoryError, ParameterSupport, ProviderAccount,
    ProviderAccountRepository,
};
use lettuce_transfer::{ModelTransfer, ModelTransferError};
use lettuce_types::{ModelProfileId, Revision, TimestampMillis};

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

/// Which file a model profile is exported as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFileFormat {
    Json,
    Usc,
}

/// A model file ready to save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedModelFile {
    pub filename: String,
    pub content: String,
}

/// Model files read into model profiles and written from them.
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

fn scopes(modalities: &ModalityCapabilities) -> Vec<String> {
    [
        ("text", modalities.text),
        ("image", modalities.image),
        ("audio", modalities.audio),
    ]
    .into_iter()
    .filter(|(_, status)| *status == CapabilityStatus::Supported)
    .map(|(scope, _)| scope.to_owned())
    .collect()
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
        let account = self.account(&imported.provider_id, &imported.provider_label)?;
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
        let profile = ModelProfileRepository::upsert(
            self.repository,
            ModelProfile {
                id: ModelProfileId::new(),
                provider_account_id: account.id,
                external_model_id: imported.name,
                display_name: imported.display_name,
                kind: ModelKind::Chat,
                config,
                revision: Revision::INITIAL,
                created_at: now,
                updated_at: now,
            },
            None,
        )?;
        match self.repository.load() {
            Ok(stored) if stored.default_model_profile_id.is_none() => {
                if let Err(error) =
                    self.repository
                        .save(stored.settings, Some(profile.id), stored.revision)
                {
                    tracing::warn!(%error, "the new model could not become the default model");
                }
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "the default model could not be checked");
            }
        }
        Ok(ImportedModelFile {
            profile,
            lossy_fields: parameters.lossy_fields,
            unknown_fields: parameters.unknown_fields,
        })
    }

    fn account(&self, provider: &str, label: &str) -> Result<ProviderAccount, ModelFileError> {
        let local = provider.eq_ignore_ascii_case("llamacpp");
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
            return Ok(account.clone());
        }
        match candidates.as_slice() {
            [only] => Ok(only.clone()),
            _ => Err(ModelFileError::NoProviderAccount(provider.to_owned())),
        }
    }

    /// The model as a file of `format`, named after its display name like the
    /// old app named it.
    pub fn export(
        &self,
        id: ModelProfileId,
        format: ModelFileFormat,
        now: TimestampMillis,
    ) -> Result<ExportedModelFile, ModelFileError> {
        let profile =
            ModelProfileRepository::get(self.repository, id)?.ok_or(ModelFileError::NotFound)?;
        let account = ProviderAccountRepository::get(self.repository, profile.provider_account_id)?
            .ok_or(ModelFileError::NotFound)?;
        let config = &profile.config;
        let transfer = ModelTransfer {
            id: profile.id.to_string(),
            name: profile.external_model_id.clone(),
            display_name: profile.display_name.clone(),
            provider_id: account.provider_kind.clone(),
            provider_label: account.label.clone(),
            created_at: profile.created_at.get(),
            input_scopes: scopes(&config.capabilities.input_modalities),
            output_scopes: scopes(&config.capabilities.output_modalities),
            advanced_model_settings: lettuce_transfer::legacy_advanced_model_settings(
                &config.chat_parameters,
                &config.feature_parameters,
                &config.llama_cpp,
                &config.stable_diffusion,
            ),
        };
        let (content, extension) = match format {
            ModelFileFormat::Json => (lettuce_transfer::export_model_json(&transfer)?, "json"),
            ModelFileFormat::Usc => (lettuce_transfer::export_model_usc(&transfer)?, "usc"),
        };
        let name = if profile.display_name.is_empty() {
            &profile.external_model_id
        } else {
            &profile.display_name
        };
        Ok(ExportedModelFile {
            filename: format!(
                "model_{}_{}.{extension}",
                crate::prompt_files::export_name(name),
                crate::prompt_files::export_date(now)
            ),
            content,
        })
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
        let exported = files
            .export(seeded, ModelFileFormat::Usc, TimestampMillis::new(0))
            .expect("export");
        assert_eq!(exported.filename, "model_vendor_model_1970-01-01.usc");
        let imported = files
            .import(&exported.content, TimestampMillis::new(9))
            .expect("import");
        let original = ModelProfileRepository::get(backend.database(), seeded)
            .expect("get")
            .expect("seeded");
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
}
