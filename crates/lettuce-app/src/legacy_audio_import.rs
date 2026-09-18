use lettuce_settings::{
    SecretOwnerId, SecretPurpose, SecretRecord, SecretRef, SecretState, SecretStore,
    SecretStoreError, SecretValue,
};
use lettuce_speech::{AudioProvider, UserVoice};
use lettuce_transfer::{
    LegacyAudioMaterializationRequest, LegacyImportAdmission, LegacyImportPlan,
    LegacyImportRepository, LegacyImportRepositoryError, LegacyImportStageReceipt,
    ProviderBackupSecret,
};
use lettuce_types::TimestampMillis;

#[derive(Debug)]
pub struct LegacyAudioImportCoordinator<'a, R: ?Sized, S: ?Sized> {
    repository: &'a R,
    secret_store: &'a S,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyAudioImportError {
    InvalidAdmission,
    Repository(LegacyImportRepositoryError),
    SecretStore(SecretStoreError),
    SecretChanged,
}

impl<'a, R, S> LegacyAudioImportCoordinator<'a, R, S>
where
    R: LegacyImportRepository + ?Sized,
    S: SecretStore + ?Sized,
{
    #[must_use]
    pub const fn new(repository: &'a R, secret_store: &'a S) -> Self {
        Self {
            repository,
            secret_store,
        }
    }

    /// Stores every legacy audio provider API key under its run-scoped
    /// reference, then writes the planned audio providers and user voices.
    /// A retry loads and compares an existing key instead of rewriting it.
    pub async fn execute(
        &self,
        admission: &LegacyImportAdmission,
        plan: &LegacyImportPlan,
        audio_providers: &[AudioProvider],
        user_voices: &[UserVoice],
        secrets: &[ProviderBackupSecret],
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, LegacyAudioImportError> {
        let plan_fingerprint = crate::legacy_import::plan_fingerprint(plan);
        if plan_fingerprint != admission.plan_fingerprint {
            return Err(LegacyAudioImportError::InvalidAdmission);
        }
        let source_fingerprint = plan
            .source_fingerprint
            .clone()
            .ok_or(LegacyAudioImportError::InvalidAdmission)?;
        let scope = lettuce_transfer::LegacyIdScope::new(&source_fingerprint);
        let scoped_providers = audio_providers
            .iter()
            .map(|provider| AudioProvider {
                id: lettuce_types::AudioProviderId::from_uuid(scope.uuid(provider.id.as_uuid())),
                secret_owner_id: SecretOwnerId::from_uuid(
                    scope.uuid(provider.secret_owner_id.as_uuid()),
                ),
                api_key_ref: provider
                    .api_key_ref
                    .map(|reference| legacy_audio_secret_ref(admission.run_id, scope, reference)),
                ..provider.clone()
            })
            .collect::<Vec<_>>();
        let scoped_voices = user_voices
            .iter()
            .map(|voice| UserVoice {
                id: lettuce_types::VoiceProfileId::from_uuid(scope.uuid(voice.id.as_uuid())),
                provider_id: lettuce_types::AudioProviderId::from_uuid(
                    scope.uuid(voice.provider_id.as_uuid()),
                ),
                ..voice.clone()
            })
            .collect::<Vec<_>>();
        for (legacy, provider) in audio_providers.iter().zip(&scoped_providers) {
            let (Some(legacy_reference), Some(reference)) =
                (legacy.api_key_ref, provider.api_key_ref)
            else {
                continue;
            };
            let legacy_purpose = SecretPurpose::AudioApiKey {
                owner: legacy.secret_owner_id,
            };
            let purpose = SecretPurpose::AudioApiKey {
                owner: provider.secret_owner_id,
            };
            let secret = secrets
                .iter()
                .find(|secret| {
                    secret.reference == legacy_reference && secret.purpose == legacy_purpose
                })
                .ok_or(LegacyAudioImportError::InvalidAdmission)?;
            let status = self
                .secret_store
                .status(&reference, &purpose)
                .await
                .map_err(LegacyAudioImportError::SecretStore)?;
            match status.state {
                SecretState::Present => {
                    let stored = self
                        .secret_store
                        .load(&reference, &purpose)
                        .await
                        .map_err(LegacyAudioImportError::SecretStore)?;
                    if !stored.with(|stored| secret.value.with(|source| stored == source)) {
                        return Err(LegacyAudioImportError::SecretChanged);
                    }
                }
                SecretState::Missing => {
                    let value = secret
                        .value
                        .with(|value| SecretValue::new(value))
                        .map_err(|_| LegacyAudioImportError::InvalidAdmission)?;
                    self.secret_store
                        .put(SecretRecord::new(reference, purpose.clone()), value, None)
                        .await
                        .map_err(LegacyAudioImportError::SecretStore)?;
                }
                SecretState::Unavailable { reason } => {
                    return Err(LegacyAudioImportError::SecretStore(
                        SecretStoreError::Unavailable(reason),
                    ));
                }
            }
        }
        self.repository
            .materialize_audio(LegacyAudioMaterializationRequest {
                run_id: admission.run_id,
                plan_fingerprint,
                source_fingerprint,
                audio_providers: scoped_providers,
                user_voices: scoped_voices,
                completed_at,
            })
            .map_err(LegacyAudioImportError::Repository)
    }

    pub async fn execute_database_import(
        &self,
        admission: &LegacyImportAdmission,
        import: &crate::LegacyDatabaseImportPlan,
        completed_at: TimestampMillis,
    ) -> Result<LegacyImportStageReceipt, LegacyAudioImportError> {
        let configuration = &import.compatibility.authored_plan().configuration;
        self.execute(
            admission,
            &import.plan,
            &configuration.audio_providers,
            &configuration.user_voices,
            &configuration.secrets,
            completed_at,
        )
        .await
    }
}

/// The store reference of an imported legacy audio API key. It is scoped by
/// the legacy source and the import run, so importing the same legacy source
/// into a replacement database never shares a secret record with the database
/// it replaces.
pub(crate) fn legacy_audio_secret_ref(
    run_id: lettuce_types::LegacyImportRunId,
    scope: lettuce_transfer::LegacyIdScope,
    reference: SecretRef,
) -> SecretRef {
    SecretRef::from_uuid(uuid::Uuid::new_v5(
        &run_id.as_uuid(),
        scope.uuid(reference.as_uuid()).as_bytes(),
    ))
}

/// The scoped store reference and purpose of a legacy audio provider's API key
/// for one import run.
pub(crate) fn legacy_audio_secret(
    run_id: lettuce_types::LegacyImportRunId,
    scope: lettuce_transfer::LegacyIdScope,
    provider: &AudioProvider,
) -> Option<(SecretRef, SecretPurpose)> {
    provider.api_key_ref.map(|reference| {
        (
            legacy_audio_secret_ref(run_id, scope, reference),
            SecretPurpose::AudioApiKey {
                owner: SecretOwnerId::from_uuid(scope.uuid(provider.secret_owner_id.as_uuid())),
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lettuce_settings::InMemorySecretStore;
    use lettuce_speech::{AudioProviderConfig, TtsConfigurationRepository};
    use lettuce_transfer::{
        LegacyAsrPlan, LegacyDatabaseInventory, LegacyLorebookPlan, LegacyMediaPlan,
        LegacyPersonaPlan, LegacyPromptPlan, LegacyProviderModelPlan,
    };
    use lettuce_types::{
        AudioProviderId, ContentHash, LegacyImportRunId, Revision, VoiceProfileId,
    };

    use super::*;
    use crate::AppBackend;

    #[tokio::test]
    async fn legacy_audio_providers_voices_and_keys_import_once_and_replay() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-app-legacy-audio-{}.sqlite3",
            LegacyImportRunId::new()
        ));
        let plan = LegacyImportPlan {
            provider_models: LegacyProviderModelPlan {
                skipped: Vec::new(),
                provider_accounts: Vec::new(),
                model_profiles: Vec::new(),
                default_provider_account_id: None,
                default_model_profile_id: None,
            },
            prompts: LegacyPromptPlan {
                skipped: Vec::new(),
                prompts: Vec::new(),
                default_prompt_source_id: None,
                deprecated_system_prompt: None,
            },
            personas: LegacyPersonaPlan {
                skipped: Vec::new(),
                personas: Vec::new(),
                default_persona_id: None,
            },
            lorebooks: LegacyLorebookPlan {
                skipped: Vec::new(),
                lorebooks: Vec::new(),
            },
            asr: LegacyAsrPlan {
                vocabulary: Vec::new(),
                corrections: Vec::new(),
                ignored_suggestions: Vec::new(),
                voice_examples: Vec::new(),
            },
            media: LegacyMediaPlan {
                media: Vec::new(),
                total_bytes: 0,
                skipped: Vec::new(),
            },
            source_fingerprint: Some(ContentHash::parse("66".repeat(32)).expect("source hash")),
            later_skips: Vec::new(),
        };
        let inventory = LegacyDatabaseInventory {
            schema_version: 92,
            provider_accounts: 0,
            models: 0,
            prompts: 0,
            personas: 0,
            characters: 0,
            lorebooks: 0,
            chat_templates: 0,
            direct_conversations: 0,
            group_profiles: 0,
            group_conversations: 0,
        };
        let reference = SecretRef::new();
        let owner = SecretOwnerId::new();
        let provider = AudioProvider {
            id: AudioProviderId::new(),
            secret_owner_id: owner,
            label: "ElevenLabs".to_owned(),
            api_key_ref: Some(reference),
            config: AudioProviderConfig::Elevenlabs,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(3),
            updated_at: TimestampMillis::new(3),
        };
        let voice = UserVoice {
            id: VoiceProfileId::new(),
            provider_id: provider.id,
            name: "Narrator".to_owned(),
            model_id: "eleven_multilingual_v2".to_owned(),
            voice_id: "voice-1".to_owned(),
            prompt: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(4),
            updated_at: TimestampMillis::new(4),
        };
        let secrets = vec![ProviderBackupSecret {
            reference,
            purpose: SecretPurpose::AudioApiKey { owner },
            generation: 1,
            value: SecretValue::new("xi-legacy-key").expect("secret value"),
        }];
        let run_id = LegacyImportRunId::new();
        let secret_store = InMemorySecretStore::new();
        let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("open backend");
        let admission = backend
            .legacy_import_admission()
            .admit(run_id, &inventory, &plan, TimestampMillis::new(10))
            .expect("admit import");
        backend
            .legacy_import_executor()
            .execute(&admission, &plan, TimestampMillis::new(20))
            .expect("materialize authored graph");
        backend
            .legacy_provider_model_importer(&secret_store)
            .execute(&admission, &plan, TimestampMillis::new(30))
            .await
            .expect("materialize providers");

        let receipt = backend
            .legacy_audio_importer(&secret_store)
            .execute(
                &admission,
                &plan,
                std::slice::from_ref(&provider),
                std::slice::from_ref(&voice),
                &secrets,
                TimestampMillis::new(40),
            )
            .await
            .expect("materialize audio");

        assert_eq!(receipt.record_count, 2);
        assert!(!receipt.replayed);
        let scope = lettuce_transfer::LegacyIdScope::new(
            plan.source_fingerprint
                .as_ref()
                .expect("source fingerprint"),
        );
        let scoped_reference = super::legacy_audio_secret_ref(run_id, scope, reference);
        let scoped_owner = SecretOwnerId::from_uuid(scope.uuid(owner.as_uuid()));
        assert_eq!(
            TtsConfigurationRepository::get_audio_provider(
                backend.database(),
                AudioProviderId::from_uuid(scope.uuid(provider.id.as_uuid())),
            )
            .expect("read provider")
            .map(|stored| stored.api_key_ref),
            Some(Some(scoped_reference))
        );
        assert!(
            TtsConfigurationRepository::get_user_voice(
                backend.database(),
                VoiceProfileId::from_uuid(scope.uuid(voice.id.as_uuid())),
            )
            .expect("read voice")
            .is_some()
        );
        let stored = secret_store
            .load(
                &scoped_reference,
                &SecretPurpose::AudioApiKey {
                    owner: scoped_owner,
                },
            )
            .await
            .expect("stored key");
        assert!(stored.with(|value| value == "xi-legacy-key"));

        let replay = backend
            .legacy_audio_importer(&secret_store)
            .execute(
                &admission,
                &plan,
                std::slice::from_ref(&provider),
                std::slice::from_ref(&voice),
                &secrets,
                TimestampMillis::new(50),
            )
            .await
            .expect("replay audio");
        assert!(replay.replayed);
        assert_eq!(replay.completed_at, TimestampMillis::new(40));
        drop(backend);
        fs::remove_file(path).expect("remove database");
    }
}
