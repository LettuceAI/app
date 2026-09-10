use std::fmt;

use lettuce_settings::{SecretState, SecretStore};
use lettuce_transfer::{
    BackupEnvelopeError, ProviderBackupGraphError, ProviderBackupSecret, ProviderBackupSource,
    ProviderBackupSourceError, provider_backup_secret_requirements, provider_backup_sections,
    seal_backup,
};
use lettuce_types::TimestampMillis;

pub struct ProviderBackupCoordinator<'a, R: ?Sized, S: ?Sized> {
    source: &'a R,
    secrets: &'a S,
}

impl<R: ?Sized, S: ?Sized> fmt::Debug for ProviderBackupCoordinator<'_, R, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderBackupCoordinator")
            .finish_non_exhaustive()
    }
}

impl<'a, R, S> ProviderBackupCoordinator<'a, R, S>
where
    R: ProviderBackupSource + ?Sized,
    S: SecretStore + ?Sized,
{
    #[must_use]
    pub const fn new(source: &'a R, secrets: &'a S) -> Self {
        Self { source, secrets }
    }

    pub async fn export(
        &self,
        app_version: impl Into<String>,
        created_at: TimestampMillis,
        password: &str,
    ) -> Result<Vec<u8>, ProviderBackupError> {
        let graph = self.source.read_provider_backup_graph()?;
        let requirements = provider_backup_secret_requirements(&graph)?;
        let mut values = Vec::with_capacity(requirements.len());
        for (reference, purpose) in requirements {
            let before = self.secrets.status(&reference, &purpose).await?;
            if before.reference != reference
                || before.purpose != purpose
                || before.state != SecretState::Present
                || before.generation == 0
            {
                return Err(ProviderBackupError::SecretChanged);
            }
            let value = self.secrets.load(&reference, &purpose).await?;
            let after = self.secrets.status(&reference, &purpose).await?;
            if after != before {
                return Err(ProviderBackupError::SecretChanged);
            }
            values.push(ProviderBackupSecret {
                reference,
                purpose,
                generation: before.generation,
                value,
            });
        }
        let sections = provider_backup_sections(graph, values)?;
        seal_backup(app_version, created_at, password, sections).map_err(Into::into)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderBackupError {
    #[error("provider backup source failed: {0}")]
    Source(#[from] ProviderBackupSourceError),
    #[error("provider backup graph failed validation: {0}")]
    Graph(#[from] ProviderBackupGraphError),
    #[error("provider backup secret could not be read: {0}")]
    Secret(#[from] lettuce_settings::SecretStoreError),
    #[error("provider backup secret changed while it was being read")]
    SecretChanged,
    #[error("provider backup encryption failed: {0}")]
    Envelope(#[from] BackupEnvelopeError),
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use lettuce_characters::{
        Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
        CharacterProvenance, CharacterRepository, CreateCharacterPlan, CreateGroupPlan,
        GroupMember, GroupProfile, GroupRepository, Persona, PersonaRepository,
    };
    use lettuce_context::{
        DetectionPolicy, LorebookBehaviorVersion, LorebookMetadataDraft, LorebookRepository,
    };
    use lettuce_models::{
        ProviderAccount, ProviderAccountRepository, ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::{
        InMemorySecretStore, SecretBackendError, SecretOwnerId, SecretPurpose, SecretRecord,
        SecretRef, SecretStatus, SecretStore, SecretStoreError, SecretValue,
    };
    use lettuce_speech::{
        AudioProvider, AudioProviderConfig, TtsConfigurationRepository, UserVoice,
    };
    use lettuce_transfer::{BackupEnvelopeError, open_backup};
    use lettuce_types::{
        AudioProviderId, CharacterId, GroupId, OperationId, PersonaId, ProviderAccountId, Revision,
        VoiceProfileId,
    };

    use super::*;
    use crate::AppBackend;

    struct ChangingSecretStore {
        status_calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl SecretStore for ChangingSecretStore {
        async fn put(
            &self,
            _: SecretRecord,
            _: SecretValue,
            _: Option<u64>,
        ) -> Result<SecretStatus, SecretStoreError> {
            Err(SecretStoreError::Backend(SecretBackendError::Unavailable))
        }

        async fn load(
            &self,
            reference: &SecretRef,
            purpose: &SecretPurpose,
        ) -> Result<SecretValue, SecretStoreError> {
            let _ = (reference, purpose);
            SecretValue::new("changing-secret")
                .map_err(|_| SecretStoreError::Backend(SecretBackendError::Corrupt))
        }

        async fn status(
            &self,
            reference: &SecretRef,
            purpose: &SecretPurpose,
        ) -> Result<SecretStatus, SecretStoreError> {
            Ok(SecretStatus {
                reference: *reference,
                purpose: purpose.clone(),
                generation: if self.status_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    1
                } else {
                    2
                },
                state: SecretState::Present,
            })
        }

        async fn delete(
            &self,
            _: &SecretRef,
            _: &SecretPurpose,
            _: Option<u64>,
        ) -> Result<SecretStatus, SecretStoreError> {
            Err(SecretStoreError::Backend(SecretBackendError::Unavailable))
        }
    }

    #[tokio::test]
    async fn file_backed_provider_backup_reopens_and_seals_only_referenced_secrets() {
        let path =
            std::env::temp_dir().join(format!("provider-backup-{}.sqlite3", OperationId::new()));
        let secret_store = Arc::new(InMemorySecretStore::new());
        let reference = SecretRef::new();
        let owner = SecretOwnerId::new();
        let purpose = SecretPurpose::ProviderApiKey { owner };
        let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("open backend");
        ProviderAccountRepository::upsert(
            backend.database(),
            ProviderAccount {
                id: ProviderAccountId::new(),
                secret_owner_id: owner,
                provider_kind: "openai".into(),
                protocol: ProviderProtocol::OpenAiCompatible,
                label: "Primary".into(),
                endpoint: None,
                enabled: true,
                streaming_enabled: true,
                allow_invalid_tls: false,
                api_key_ref: Some(reference),
                secret_headers: Vec::new(),
                config: ProviderConfig::Standard,
                revision: Revision::new(1),
                created_at: TimestampMillis::new(2),
                updated_at: TimestampMillis::new(2),
            },
            None,
        )
        .expect("store provider");
        secret_store
            .put(
                SecretRecord::new(reference, purpose),
                SecretValue::new("provider-backup-canary").expect("secret"),
                None,
            )
            .await
            .expect("store secret");
        let audio_reference = SecretRef::new();
        let audio_owner = SecretOwnerId::new();
        let audio_provider_id = AudioProviderId::new();
        TtsConfigurationRepository::upsert_audio_provider(
            backend.database(),
            AudioProvider {
                id: audio_provider_id,
                secret_owner_id: audio_owner,
                label: "Speech provider".into(),
                api_key_ref: Some(audio_reference),
                config: AudioProviderConfig::OpenAiCompatible {
                    base_url: None,
                    request_path: None,
                },
                revision: Revision::new(1),
                created_at: TimestampMillis::new(2),
                updated_at: TimestampMillis::new(2),
            },
            None,
        )
        .expect("store audio provider");
        TtsConfigurationRepository::upsert_user_voice(
            backend.database(),
            UserVoice {
                id: VoiceProfileId::new(),
                provider_id: audio_provider_id,
                name: "Narrator".into(),
                model_id: "tts-model".into(),
                voice_id: "voice-one".into(),
                prompt: Some("Warm delivery".into()),
                revision: Revision::new(1),
                created_at: TimestampMillis::new(2),
                updated_at: TimestampMillis::new(2),
            },
            None,
        )
        .expect("store user voice");
        secret_store
            .put(
                SecretRecord::new(
                    audio_reference,
                    SecretPurpose::AudioApiKey { owner: audio_owner },
                ),
                SecretValue::new("audio-backup-canary").expect("secret"),
                None,
            )
            .await
            .expect("store audio secret");
        let persona = PersonaRepository::create(
            backend.database(),
            Persona::new(
                PersonaId::new(),
                "Backup persona".into(),
                "Preserved authored profile".into(),
                TimestampMillis::new(2),
            )
            .expect("persona"),
        )
        .expect("store persona");
        let default = PersonaRepository::get_default_snapshot(backend.database())
            .expect("default snapshot")
            .state;
        PersonaRepository::set_default(
            backend.database(),
            persona.id,
            default.revision,
            TimestampMillis::new(3),
        )
        .expect("set default persona");
        LorebookRepository::create(
            backend.database(),
            LorebookMetadataDraft {
                name: "Backup lorebook".into(),
                detection_policy: DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
            },
            Vec::new(),
            TimestampMillis::new(2),
        )
        .expect("store lorebook");
        let character_ids = ["Ada", "Bea"].map(|name| {
            let id = CharacterId::new();
            CharacterRepository::create(
                backend.database(),
                CreateCharacterPlan {
                    character: Character::new(
                        id,
                        CharacterProfile {
                            name: name.into(),
                            nickname: None,
                            description: Some("A member of the backup cast".into()),
                            definition: None,
                            design_description: None,
                        },
                        CharacterProvenance::default(),
                        CharacterDefaults::default(),
                        CharacterPresentationV1::default(),
                        None,
                        CharacterMedia::default(),
                        TimestampMillis::new(2),
                    )
                    .expect("character"),
                    scenes: Vec::new(),
                    variants: Vec::new(),
                    starters: Vec::new(),
                },
            )
            .expect("store character");
            id
        });
        GroupRepository::create(
            backend.database(),
            CreateGroupPlan {
                group: GroupProfile::new(
                    GroupId::new(),
                    "Backup cast".into(),
                    character_ids
                        .into_iter()
                        .enumerate()
                        .map(|(ordinal, character_id)| GroupMember {
                            character_id,
                            ordinal: u32::try_from(ordinal).expect("ordinal"),
                            muted: false,
                            model_profile_override: None,
                        })
                        .collect(),
                    TimestampMillis::new(2),
                )
                .expect("group"),
                starting_scene: None,
            },
        )
        .expect("store group");
        drop(backend);

        let reopened = AppBackend::open(&path, TimestampMillis::new(3)).expect("reopen backend");
        let envelope = ProviderBackupCoordinator::new(reopened.database(), secret_store.as_ref())
            .export("1.0.0", TimestampMillis::new(4), "backup password")
            .await
            .expect("export backup");
        assert_eq!(
            open_backup(&envelope, "wrong password"),
            Err(BackupEnvelopeError::Authentication)
        );
        let sections = open_backup(&envelope, "backup password").expect("open backup");
        assert_eq!(sections.len(), 2);
        assert!(
            sections[1]
                .bytes
                .windows("provider-backup-canary".len())
                .any(|window| window == b"provider-backup-canary")
        );
        assert!(
            sections[1]
                .bytes
                .windows("audio-backup-canary".len())
                .any(|window| window == b"audio-backup-canary")
        );
        assert!(
            !sections[0]
                .bytes
                .windows("provider-backup-canary".len())
                .any(|window| window == b"provider-backup-canary")
        );
        assert!(
            !sections[0]
                .bytes
                .windows("audio-backup-canary".len())
                .any(|window| window == b"audio-backup-canary")
        );
        let metadata: serde_json::Value =
            serde_json::from_slice(&sections[0].bytes).expect("metadata JSON");
        assert_eq!(metadata["settings"]["value"]["analytics_enabled"], true);
        assert_eq!(metadata["audio_providers"][0]["label"], "Speech provider");
        assert_eq!(metadata["user_voices"][0]["name"], "Narrator");
        assert_eq!(
            metadata["authored"]["personas"][0]["title"],
            "Backup persona"
        );
        assert_eq!(
            metadata["authored"]["persona_default"]["persona_id"],
            persona.id.to_string()
        );
        assert_eq!(
            metadata["authored"]["lorebooks"]
                .as_array()
                .expect("lorebooks array")
                .len(),
            1
        );
        assert_eq!(
            metadata["authored"]["characters"]
                .as_array()
                .expect("characters array")
                .len(),
            2
        );
        assert_eq!(
            metadata["authored"]["groups"][0]["group"]["name"],
            "Backup cast"
        );

        let changing = ChangingSecretStore {
            status_calls: AtomicUsize::new(0),
        };
        assert_eq!(
            reopened
                .provider_backup(&changing)
                .export("1.0.0", TimestampMillis::new(5), "backup password")
                .await,
            Err(ProviderBackupError::SecretChanged)
        );
    }
}
