use std::fmt;

use lettuce_conversations::{
    ArtifactTransferError, ConversationArtifactTransferPort, TrustedArtifactDescriptor,
    TrustedArtifactSink,
};
use lettuce_settings::{SecretState, SecretStore};
use lettuce_transfer::{
    BackupConversationArtifact, BackupEnvelopeError, BackupMediaObject, MAX_BACKUP_ENTRIES,
    MAX_BACKUP_TOTAL_BYTES, PROVIDER_BACKUP_FIXED_SECTIONS, ProviderBackupGraphError,
    ProviderBackupSecret, ProviderBackupSource, ProviderBackupSourceError,
    provider_backup_artifact_requirements, provider_backup_media_requirements,
    provider_backup_secret_requirements, provider_backup_sections, seal_backup,
};
use lettuce_types::{ContentHash, TimestampMillis};
use zeroize::Zeroizing;

pub trait BackupMediaReader: Send + Sync {
    fn read_backup_chunk(
        &self,
        hash: &ContentHash,
        offset: u64,
        max_bytes: usize,
    ) -> Result<Vec<u8>, lettuce_media::MediaStoreError>;
}

impl<BR, AR> BackupMediaReader for lettuce_media::LocalSyncMediaStore<BR, AR>
where
    BR: lettuce_media::MediaBlobRepository,
    AR: lettuce_media::MediaAssetRepository,
{
    fn read_backup_chunk(
        &self,
        hash: &ContentHash,
        offset: u64,
        max_bytes: usize,
    ) -> Result<Vec<u8>, lettuce_media::MediaStoreError> {
        self.read_sync_chunk(hash, offset, max_bytes)
    }
}

pub struct ProviderBackupCoordinator<'a, R: ?Sized, S: ?Sized, M: ?Sized, A: ?Sized> {
    source: &'a R,
    secrets: &'a S,
    media: &'a M,
    artifacts: &'a A,
}

impl<R: ?Sized, S: ?Sized, M: ?Sized, A: ?Sized> fmt::Debug
    for ProviderBackupCoordinator<'_, R, S, M, A>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderBackupCoordinator")
            .finish_non_exhaustive()
    }
}

impl<'a, R, S, M, A> ProviderBackupCoordinator<'a, R, S, M, A>
where
    R: ProviderBackupSource + ?Sized,
    S: SecretStore + ?Sized,
    M: BackupMediaReader + ?Sized,
    A: ConversationArtifactTransferPort + ?Sized,
{
    #[must_use]
    pub const fn new(source: &'a R, secrets: &'a S, media: &'a M, artifacts: &'a A) -> Self {
        Self {
            source,
            secrets,
            media,
            artifacts,
        }
    }

    pub async fn export(
        &self,
        app_version: impl Into<String>,
        created_at: TimestampMillis,
        password: &str,
    ) -> Result<Vec<u8>, ProviderBackupError> {
        let graph = self.source.read_provider_backup_graph()?;
        let requirements = provider_backup_secret_requirements(&graph)?;
        let media_requirements = provider_backup_media_requirements(&graph)?;
        let artifact_requirements = provider_backup_artifact_requirements(&graph)?;
        if media_requirements
            .len()
            .checked_add(artifact_requirements.len())
            .and_then(|count| count.checked_add(PROVIDER_BACKUP_FIXED_SECTIONS))
            .is_none_or(|count| count > MAX_BACKUP_ENTRIES)
        {
            return Err(ProviderBackupGraphError::LimitExceeded.into());
        }
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
        let mut media = Vec::with_capacity(media_requirements.len());
        let mut binary_bytes = 0_usize;
        for (content_hash, byte_size) in media_requirements {
            let capacity = usize::try_from(byte_size)
                .map_err(|_| ProviderBackupError::Graph(ProviderBackupGraphError::LimitExceeded))?;
            binary_bytes = binary_bytes
                .checked_add(capacity)
                .filter(|total| *total <= MAX_BACKUP_TOTAL_BYTES)
                .ok_or(ProviderBackupGraphError::LimitExceeded)?;
            let mut bytes = Zeroizing::new(Vec::with_capacity(capacity));
            while bytes.len() < capacity {
                let chunk = self.media.read_backup_chunk(
                    &content_hash,
                    u64::try_from(bytes.len()).map_err(|_| {
                        ProviderBackupError::Graph(ProviderBackupGraphError::LimitExceeded)
                    })?,
                    lettuce_media::MAX_SYNC_MEDIA_CHUNK_BYTES.min(capacity - bytes.len()),
                )?;
                if chunk.is_empty() {
                    return Err(ProviderBackupError::MediaIncomplete);
                }
                bytes.extend_from_slice(&chunk);
            }
            media.push(BackupMediaObject {
                content_hash,
                bytes,
            });
        }
        let mut artifacts = Vec::with_capacity(artifact_requirements.len());
        for descriptor in artifact_requirements {
            let byte_size = match &descriptor {
                TrustedArtifactDescriptor::Snapshot(reference) => reference.byte_size,
                TrustedArtifactDescriptor::Replay(reference) => reference.byte_size,
            };
            binary_bytes = binary_bytes
                .checked_add(
                    usize::try_from(byte_size)
                        .map_err(|_| ProviderBackupGraphError::LimitExceeded)?,
                )
                .filter(|total| *total <= MAX_BACKUP_TOTAL_BYTES)
                .ok_or(ProviderBackupGraphError::LimitExceeded)?;
            let mut sink = BackupArtifactSink::new(descriptor.clone());
            match descriptor {
                TrustedArtifactDescriptor::Snapshot(reference) => self
                    .artifacts
                    .export_snapshot(reference.artifact_id, &mut sink)?,
                TrustedArtifactDescriptor::Replay(reference) => self
                    .artifacts
                    .export_replay(reference.artifact_id, &mut sink)?,
            }
            artifacts.push(sink.complete()?);
        }
        let sections = provider_backup_sections(graph, values, media, artifacts)?;
        seal_backup(app_version, created_at, password, sections).map_err(Into::into)
    }
}

struct BackupArtifactSink {
    expected: TrustedArtifactDescriptor,
    began: bool,
    finished: bool,
    bytes: Zeroizing<Vec<u8>>,
}

impl BackupArtifactSink {
    fn new(expected: TrustedArtifactDescriptor) -> Self {
        Self {
            expected,
            began: false,
            finished: false,
            bytes: Zeroizing::new(Vec::new()),
        }
    }

    fn complete(self) -> Result<BackupConversationArtifact, ArtifactTransferError> {
        if !self.finished {
            return Err(ArtifactTransferError::SinkRejected);
        }
        Ok(BackupConversationArtifact {
            descriptor: self.expected,
            bytes: self.bytes,
        })
    }

    fn expected_size(&self) -> Result<usize, ArtifactTransferError> {
        let size = match &self.expected {
            TrustedArtifactDescriptor::Snapshot(reference) => reference.byte_size,
            TrustedArtifactDescriptor::Replay(reference) => reference.byte_size,
        };
        usize::try_from(size).map_err(|_| ArtifactTransferError::SinkRejected)
    }
}

impl TrustedArtifactSink for BackupArtifactSink {
    fn begin(
        &mut self,
        descriptor: &TrustedArtifactDescriptor,
    ) -> Result<(), ArtifactTransferError> {
        if self.began || descriptor != &self.expected {
            return Err(ArtifactTransferError::SinkRejected);
        }
        self.began = true;
        Ok(())
    }

    fn chunk(&mut self, bytes: &[u8]) -> Result<(), ArtifactTransferError> {
        let expected_size = self.expected_size()?;
        if !self.began
            || self.finished
            || self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|size| size > expected_size)
        {
            return Err(ArtifactTransferError::SinkRejected);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn finish(&mut self) -> Result<(), ArtifactTransferError> {
        let expected_digest = match &self.expected {
            TrustedArtifactDescriptor::Snapshot(reference) => &reference.digest,
            TrustedArtifactDescriptor::Replay(reference) => &reference.digest,
        };
        if !self.began
            || self.finished
            || self.bytes.len() != self.expected_size()?
            || ContentHash::parse(blake3::hash(&self.bytes).to_hex().to_string()).as_ref()
                != Ok(expected_digest)
        {
            return Err(ArtifactTransferError::SinkRejected);
        }
        self.finished = true;
        Ok(())
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
    #[error("backup media could not be read: {0}")]
    Media(#[from] lettuce_media::MediaStoreError),
    #[error("backup media ended before its declared size")]
    MediaIncomplete,
    #[error("protected conversation artifact export failed: {0}")]
    Artifact(#[from] ArtifactTransferError),
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use lettuce_characters::{
        Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
        CharacterProvenance, CharacterRepository, ConversationStarter, CreateCharacterPlan,
        CreateGroupPlan, GroupMember, GroupProfile, GroupRepository, InteractionMode, Persona,
        PersonaRepository, SpeakerSelection, StarterMessage, StarterRole,
    };
    use lettuce_companions::{
        CompanionConversationSender, CompanionEffectSourceWindow, CompanionMemoryChanges,
        CompanionStateOwner, CompanionStateReplacement, CompanionStateRepository,
        CompanionTurnEffectOutcome, CompanionTurnEffectRepository, CompanionTurnEffectSeed,
        CompanionTurnEffectStatus, PreparedCompanionSend,
    };
    use lettuce_context::{
        DetectionPolicy, LorebookBehaviorVersion, LorebookMetadataDraft, LorebookRepository,
    };
    use lettuce_conversations::{
        ConversationKind, ConversationOutboxEvent, ConversationReader, ConversationRepository,
        FinalizationDraft, GenerationCheckpointEnvelope, GenerationCheckpointEvent,
        GenerationFailureCode, GenerationInput, GenerationTurnStatus, MessageDraft, MessagePart,
        MessageRole, MessageVisibility, OperationToken, SendConversation, SnapshotSelection,
        ToolExecution, ToolExecutionRepository, ToolExecutionStatus, ToolOutput, UsageCounters,
        UsageOutcome, UsageRecord, UsageUnavailableReason,
    };
    use lettuce_database::Database;
    use lettuce_jobs::{
        JobKind, JobMutation, JobSpec, JobStore, JobSubject, OutcomeRef, ProgressSnapshot,
        ResourceAvailability, ResourceClass, SubjectKind, UnitsProgress, WorkerId,
    };
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
        LocalSyncMediaStore, RetentionClass,
    };
    use lettuce_memory::{
        DynamicMemorySuffixRewind, DynamicMemorySuffixRewindRepository, MemoryCategory,
        MemoryChangeSet, MemoryItem, MemoryRepository, MemoryRetrievalAccess,
        MemoryRetrievalRepository, MemorySummary, MemorySummaryChange, MemorySummaryRepository,
        Score,
    };
    use lettuce_models::{
        ModelKind, ModelProfile, ModelProfileConfig, ModelProfileRepository, ProviderAccount,
        ProviderAccountRepository, ProviderConfig, ProviderProtocol,
    };
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    use lettuce_settings::{
        InMemorySecretStore, SecretBackendError, SecretOwnerId, SecretPurpose, SecretRecord,
        SecretRef, SecretStatus, SecretStore, SecretStoreError, SecretValue,
    };
    use lettuce_speech::{
        AsrCorrectionRule, AsrIgnoredSuggestion, AsrLearningRepository, AsrVocabularyTerm,
        AsrVoiceExample, AudioProvider, AudioProviderConfig, TtsConfigurationRepository, UserVoice,
    };
    use lettuce_transfer::{
        AsrLearningDocument, BackupEnvelopeError, ConversationHistoryBackup,
        ConversationHistoryBackupError, JobBackup, open_backup,
    };
    use lettuce_types::{
        AudioProviderId, CharacterId, GroupId, MemoryId, MessageId, ModelProfileId, OperationId,
        PersonaId, ProviderAccountId, Revision, StarterMessageId, ToolExecutionId, UsageEventId,
        VoiceProfileId,
    };
    use lettuce_usage::{
        JobInferenceUsage, JobInferenceUsageResult, JobUsageLedger, ModelPricing,
        OpenRouterCostInput, UsageCostBasis, UsageCostLedger, UsageLedger,
    };

    use super::*;
    use crate::{
        AppBackend, DIRECT_LAUNCH_REQUEST_FORMAT_V1, DirectConversationLaunchRequest,
        DirectUserParticipant, GROUP_LAUNCH_REQUEST_FORMAT_V1, GroupConversationLaunchRequest,
        LaunchSelection,
    };

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
    async fn file_backed_full_profile_backup_seals_secrets_and_shared_media() {
        let root = std::env::temp_dir().join(format!("provider-backup-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("backup fixture root");
        let path = root.join("state.sqlite3");
        let secret_store = Arc::new(InMemorySecretStore::new());
        let reference = SecretRef::new();
        let owner = SecretOwnerId::new();
        let purpose = SecretPurpose::ProviderApiKey { owner };
        let provider_account_id = ProviderAccountId::new();
        let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("open backend");
        ProviderAccountRepository::upsert(
            backend.database(),
            ProviderAccount {
                id: provider_account_id,
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
        let backup_model = ModelProfileRepository::upsert(
            backend.database(),
            ModelProfile {
                id: ModelProfileId::new(),
                provider_account_id,
                external_model_id: "backup/chat-model".into(),
                display_name: "Backup chat model".into(),
                kind: ModelKind::Chat,
                config: ModelProfileConfig {
                    chat_parameters: Default::default(),
                    lorebook_generator_parameters: Default::default(),
                    capabilities: lettuce_models::ModelCapabilities {
                        input_modalities: lettuce_models::ModalityCapabilities {
                            text: lettuce_models::CapabilityStatus::Supported,
                            ..Default::default()
                        },
                        output_modalities: lettuce_models::ModalityCapabilities {
                            text: lettuce_models::CapabilityStatus::Supported,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                },
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(2),
                updated_at: TimestampMillis::new(2),
            },
            None,
        )
        .expect("store backup model");
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
        let mut direct_starter_id = None;
        let character_ids = ["Ada", "Bea", "Cora"].map(|name| {
            let id = CharacterId::new();
            let starters = if name == "Cora" {
                let starter_id = lettuce_types::ConversationStarterId::new();
                direct_starter_id = Some(starter_id);
                vec![
                    ConversationStarter::new(
                        starter_id,
                        id,
                        "Backup opening".into(),
                        0,
                        vec![
                            StarterMessage {
                                id: StarterMessageId::new(),
                                role: StarterRole::User,
                                content: String::new(),
                            },
                            StarterMessage {
                                id: StarterMessageId::new(),
                                role: StarterRole::Assistant,
                                content: "Welcome back.".into(),
                            },
                        ],
                        TimestampMillis::new(2),
                    )
                    .expect("conversation starter"),
                ]
            } else {
                Vec::new()
            };
            let mut defaults = CharacterDefaults::default();
            if name == "Cora" {
                defaults.interaction_mode = InteractionMode::Companion;
                defaults.memory_policy = lettuce_characters::MemoryPolicy::Dynamic;
                defaults.model_profile_id = Some(backup_model.id);
                defaults.companion_soul = Some(Default::default());
            }
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
                        defaults,
                        CharacterPresentationV1::default(),
                        None,
                        CharacterMedia::default(),
                        TimestampMillis::new(2),
                    )
                    .expect("character"),
                    scenes: Vec::new(),
                    variants: Vec::new(),
                    starters,
                },
            )
            .expect("store character");
            id
        });
        let group_id = GroupId::new();
        let mut group = GroupProfile::new(
            group_id,
            "Backup cast".into(),
            character_ids
                .iter()
                .copied()
                .take(2)
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
        .expect("group");
        group.speaker_selection = SpeakerSelection::Director;
        GroupRepository::create(
            backend.database(),
            CreateGroupPlan {
                group,
                starting_scene: None,
            },
        )
        .expect("store group");
        let direct_conversation = backend
            .launch_direct_conversation(
                &DirectConversationLaunchRequest {
                    format_version: DIRECT_LAUNCH_REQUEST_FORMAT_V1,
                    title: "Backup direct chat".into(),
                    user: DirectUserParticipant {
                        display_name: "User".into(),
                        authored_description: None,
                    },
                    character_id: character_ids[2],
                    scene: LaunchSelection::Disabled,
                    starter: LaunchSelection::Explicit(
                        direct_starter_id.expect("direct starter id"),
                    ),
                    persona: LaunchSelection::Explicit(persona.id),
                    operation_key: lettuce_jobs::IdempotencyKey::new("backup-direct-chat")
                        .expect("operation key"),
                },
                TimestampMillis::new(3),
            )
            .expect("launch direct conversation")
            .value
            .conversation;
        let companion_owner = CompanionStateOwner {
            conversation_id: direct_conversation.id,
            character_id: character_ids[2],
            persona_id: Some(persona.id),
        };
        let companion_state = CompanionStateRepository::get(backend.database(), companion_owner)
            .expect("read companion state")
            .expect("companion state");
        let mut replacement_state = companion_state.state;
        replacement_state.active_signals = vec!["backup continuity".into()];
        replacement_state.relationship_state.interaction_count = 1;
        replacement_state.relationship_state.last_interaction_at = TimestampMillis::new(4);
        replacement_state.updated_at = TimestampMillis::new(4);
        let companion_receipt = CompanionStateRepository::replace(
            backend.database(),
            companion_owner,
            lettuce_types::OperationRecordId::new(),
            CompanionStateReplacement {
                expected_session_revision: companion_state.session_revision,
                expected_relationship_revision: companion_state.relationship_revision,
                state: replacement_state,
                applied_at: TimestampMillis::new(4),
            },
        )
        .expect("replace companion state");
        let user_participant_id = direct_conversation
            .participants
            .iter()
            .find(|participant| participant.role == lettuce_conversations::ParticipantRole::User)
            .expect("user participant")
            .id;
        let pending_send = ConversationRepository::begin_send(
            backend.database(),
            &SendConversation {
                conversation_id: direct_conversation.id,
                branch_id: direct_conversation.active_branch_id,
                expected_revision: direct_conversation.revision,
                operation: OperationToken {
                    key: lettuce_jobs::IdempotencyKey::new("backup-pending-send")
                        .expect("operation key"),
                    request_digest: ContentHash::parse(
                        blake3::hash(b"backup pending send").to_hex().to_string(),
                    )
                    .expect("request digest"),
                },
                message: MessageDraft {
                    role: MessageRole::User,
                    author_participant_id: Some(user_participant_id),
                    parts: vec![MessagePart::Text {
                        text: "Persist this pending turn.".into(),
                    }],
                    visibility: MessageVisibility::Visible,
                    pinned: false,
                    scene_edited: false,
                },
                swap_roles: false,
            },
            TimestampMillis::new(4),
        )
        .expect("begin pending generation")
        .value;
        let attempt_id = pending_send.turn.attempts[0].id;
        let mut turn_revision = pending_send.turn.revision;
        for (sequence, status) in [
            GenerationTurnStatus::Preparing,
            GenerationTurnStatus::ContextPrepared,
            GenerationTurnStatus::Running,
        ]
        .into_iter()
        .enumerate()
        {
            let sequence = u64::try_from(sequence + 1).expect("checkpoint sequence");
            turn_revision = ConversationRepository::append_event(
                backend.database(),
                pending_send.turn.id,
                turn_revision,
                &OperationToken {
                    key: lettuce_jobs::IdempotencyKey::new(format!("backup-stage-{sequence}"))
                        .expect("stage key"),
                    request_digest: ContentHash::parse(
                        blake3::hash(format!("backup stage {sequence}").as_bytes())
                            .to_hex()
                            .to_string(),
                    )
                    .expect("stage digest"),
                },
                GenerationCheckpointEnvelope {
                    turn_id: pending_send.turn.id,
                    attempt_id,
                    job_id: None,
                    correlation_id: None,
                    sequence,
                    event: GenerationCheckpointEvent::Stage { status },
                },
                TimestampMillis::new(4 + i64::try_from(sequence).expect("checkpoint time")),
            )
            .expect("append generation stage")
            .value
            .revision;
        }
        let tool = ToolExecution {
            id: ToolExecutionId::new(),
            conversation_id: direct_conversation.id,
            turn_id: pending_send.turn.id,
            attempt_id,
            ordinal: 0,
            definition_name: "create_memory".into(),
            definition_version: 1,
            provider_call_id: Some("backup-call".into()),
            arguments: serde_json::json!({"content": "remember this"}),
            raw_arguments: Some(r#"{"content":"remember this"}"#.into()),
            provider_replay: None,
            status: ToolExecutionStatus::Requested,
            output: None,
            failure: None,
            revision: Revision::INITIAL,
            requested_at: TimestampMillis::new(8),
            started_at: None,
            finished_at: None,
            updated_at: TimestampMillis::new(8),
        };
        let tool = ToolExecutionRepository::append_tool_executions(
            backend.database(),
            0,
            std::slice::from_ref(&tool),
        )
        .expect("append tool")[0]
            .clone();
        let tool = ToolExecutionRepository::transition_tool_execution(
            backend.database(),
            tool.id,
            tool.revision,
            ToolExecutionStatus::Validated,
            None,
            None,
            TimestampMillis::new(9),
        )
        .expect("validate tool");
        let tool = ToolExecutionRepository::transition_tool_execution(
            backend.database(),
            tool.id,
            tool.revision,
            ToolExecutionStatus::Running,
            None,
            None,
            TimestampMillis::new(10),
        )
        .expect("run tool");
        ToolExecutionRepository::transition_tool_execution(
            backend.database(),
            tool.id,
            tool.revision,
            ToolExecutionStatus::Succeeded,
            Some(ToolOutput {
                value: serde_json::json!({"memory_id": "backup-memory"}),
                is_error: false,
            }),
            None,
            TimestampMillis::new(11),
        )
        .expect("settle tool");
        let conversation_revision =
            ConversationReader::get(backend.database(), direct_conversation.id)
                .expect("conversation after send")
                .conversation
                .revision;
        let usage_model_id = ModelProfileId::new();
        let known_usage = UsageLedger::record(
            backend.database(),
            UsageRecord {
                turn_id: pending_send.turn.id,
                attempt_id,
                outcome: UsageOutcome::Failed,
                usage: UsageCounters::Known(lettuce_conversations::InferenceUsage {
                    provider_reported_cost: None,
                    cache_write_tokens: Some(3),
                    web_search_requests: Some(0),
                    cached_input_tokens: Some(5),
                    reasoning_tokens: Some(2),
                    input_tokens: 21,
                    output_tokens: 8,
                }),
                model_profile_id: Some(usage_model_id),
                model_revision: Some(Revision::INITIAL),
                provider_account_id: Some(provider_account_id),
                provider_account_revision: Some(Revision::INITIAL),
                recorded_at: TimestampMillis::new(12),
            },
        )
        .expect("record known conversation usage");
        UsageCostLedger::record_cost(
            backend.database(),
            known_usage.id,
            UsageCostBasis {
                model_profile_id: usage_model_id,
                provider_account_id,
                source: "Backup fixture pricing".into(),
                captured_at: TimestampMillis::new(12),
                pricing: ModelPricing {
                    prompt: "0.001".into(),
                    completion: "0.002".into(),
                    request: "0".into(),
                    image: "0".into(),
                    image_output: "0".into(),
                    web_search: "0".into(),
                    internal_reasoning: "0".into(),
                    input_cache_read: "0.0005".into(),
                    input_cache_write: "0.0015".into(),
                },
                input: OpenRouterCostInput {
                    prompt_tokens: 21,
                    completion_tokens: 8,
                    cached_prompt_tokens: 5,
                    cache_write_tokens: 3,
                    reasoning_tokens: 2,
                    web_search_requests: 0,
                    authoritative_total_cost: None,
                },
                openrouter: None,
            },
        )
        .expect("record conversation cost");
        ConversationRepository::fail_generation(
            backend.database(),
            pending_send.turn.id,
            attempt_id,
            conversation_revision,
            turn_revision,
            &OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("backup-fail-generation")
                    .expect("failure key"),
                request_digest: ContentHash::parse(
                    blake3::hash(b"backup fail generation").to_hex().to_string(),
                )
                .expect("failure digest"),
            },
            GenerationFailureCode::Internal,
            known_usage.id,
            TimestampMillis::new(13),
        )
        .expect("fail generation after settled tool");
        let direct_after_failure =
            ConversationReader::get(backend.database(), direct_conversation.id)
                .expect("conversation after first failure")
                .conversation;
        let unavailable_send = ConversationRepository::begin_send(
            backend.database(),
            &SendConversation {
                conversation_id: direct_after_failure.id,
                branch_id: direct_after_failure.active_branch_id,
                expected_revision: direct_after_failure.revision,
                operation: OperationToken {
                    key: lettuce_jobs::IdempotencyKey::new("backup-unavailable-send")
                        .expect("operation key"),
                    request_digest: ContentHash::parse(
                        blake3::hash(b"backup unavailable send")
                            .to_hex()
                            .to_string(),
                    )
                    .expect("request digest"),
                },
                message: MessageDraft {
                    role: MessageRole::User,
                    author_participant_id: Some(user_participant_id),
                    parts: vec![MessagePart::Text {
                        text: "Preserve unavailable usage too.".into(),
                    }],
                    visibility: MessageVisibility::Visible,
                    pinned: false,
                    scene_edited: false,
                },
                swap_roles: false,
            },
            TimestampMillis::new(14),
        )
        .expect("begin unavailable generation")
        .value;
        let mut unavailable_turn_revision = unavailable_send.turn.revision;
        for (sequence, status) in [
            GenerationTurnStatus::Preparing,
            GenerationTurnStatus::ContextPrepared,
            GenerationTurnStatus::Running,
        ]
        .into_iter()
        .enumerate()
        {
            let sequence = u64::try_from(sequence + 1).expect("checkpoint sequence");
            unavailable_turn_revision = ConversationRepository::append_event(
                backend.database(),
                unavailable_send.turn.id,
                unavailable_turn_revision,
                &OperationToken {
                    key: lettuce_jobs::IdempotencyKey::new(format!(
                        "backup-unavailable-stage-{sequence}"
                    ))
                    .expect("stage key"),
                    request_digest: ContentHash::parse(
                        blake3::hash(format!("backup unavailable stage {sequence}").as_bytes())
                            .to_hex()
                            .to_string(),
                    )
                    .expect("stage digest"),
                },
                GenerationCheckpointEnvelope {
                    turn_id: unavailable_send.turn.id,
                    attempt_id: unavailable_send.attempt.id,
                    job_id: None,
                    correlation_id: None,
                    sequence,
                    event: GenerationCheckpointEvent::Stage { status },
                },
                TimestampMillis::new(14 + i64::try_from(sequence).expect("checkpoint time")),
            )
            .expect("append unavailable stage")
            .value
            .revision;
        }
        let unavailable_usage = UsageLedger::record(
            backend.database(),
            UsageRecord {
                turn_id: unavailable_send.turn.id,
                attempt_id: unavailable_send.attempt.id,
                outcome: UsageOutcome::Failed,
                usage: UsageCounters::Unavailable(UsageUnavailableReason::TransportFailed),
                model_profile_id: None,
                model_revision: None,
                provider_account_id: None,
                provider_account_revision: None,
                recorded_at: TimestampMillis::new(18),
            },
        )
        .expect("record unavailable conversation usage");
        let unavailable_conversation_revision =
            ConversationReader::get(backend.database(), direct_conversation.id)
                .expect("conversation with unavailable turn")
                .conversation
                .revision;
        ConversationRepository::fail_generation(
            backend.database(),
            unavailable_send.turn.id,
            unavailable_send.attempt.id,
            unavailable_conversation_revision,
            unavailable_turn_revision,
            &OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("backup-unavailable-failure")
                    .expect("failure key"),
                request_digest: ContentHash::parse(
                    blake3::hash(b"backup unavailable failure")
                        .to_hex()
                        .to_string(),
                )
                .expect("failure digest"),
            },
            GenerationFailureCode::ProviderUnavailable,
            unavailable_usage.id,
            TimestampMillis::new(19),
        )
        .expect("fail unavailable generation");
        let direct_after_failures =
            ConversationReader::get(backend.database(), direct_conversation.id)
                .expect("conversation after failures")
                .conversation;
        let current_companion = CompanionStateRepository::get(backend.database(), companion_owner)
            .expect("current companion state")
            .expect("companion state");
        let mut next_companion_state = current_companion.state;
        next_companion_state.updated_at = TimestampMillis::new(20);
        let effect_send = CompanionConversationSender::begin_companion_send(
            backend.database(),
            PreparedCompanionSend::new(
                SendConversation {
                    conversation_id: direct_after_failures.id,
                    branch_id: direct_after_failures.active_branch_id,
                    expected_revision: direct_after_failures.revision,
                    operation: OperationToken {
                        key: lettuce_jobs::IdempotencyKey::new("backup-effect-send")
                            .expect("effect send key"),
                        request_digest: ContentHash::parse(
                            blake3::hash(b"backup effect send").to_hex().to_string(),
                        )
                        .expect("effect send digest"),
                    },
                    message: MessageDraft {
                        role: MessageRole::User,
                        author_participant_id: Some(user_participant_id),
                        parts: vec![MessagePart::Text {
                            text: "Preserve this companion effect.".into(),
                        }],
                        visibility: MessageVisibility::Visible,
                        pinned: false,
                        scene_edited: false,
                    },
                    swap_roles: false,
                },
                companion_owner,
                CompanionStateReplacement {
                    expected_session_revision: current_companion.session_revision,
                    expected_relationship_revision: current_companion.relationship_revision,
                    state: next_companion_state,
                    applied_at: TimestampMillis::new(20),
                },
                Some(CompanionTurnEffectSeed::default()),
            )
            .expect("prepare effect send"),
            TimestampMillis::new(20),
        )
        .expect("begin effect send")
        .value;
        let effect_attempt_id = effect_send.attempt.id;
        let effect_user_message_id = match effect_send.turn.input {
            GenerationInput::UserMessage { message_id } => message_id,
            _ => panic!("expected effect user message"),
        };
        let mut effect_turn_revision = effect_send.turn.revision;
        for (sequence, status) in [
            GenerationTurnStatus::Preparing,
            GenerationTurnStatus::ContextPrepared,
            GenerationTurnStatus::Running,
        ]
        .into_iter()
        .enumerate()
        {
            let sequence = u64::try_from(sequence + 1).expect("effect checkpoint sequence");
            effect_turn_revision = ConversationRepository::append_event(
                backend.database(),
                effect_send.turn.id,
                effect_turn_revision,
                &OperationToken {
                    key: lettuce_jobs::IdempotencyKey::new(format!(
                        "backup-effect-stage-{sequence}"
                    ))
                    .expect("effect stage key"),
                    request_digest: ContentHash::parse(
                        blake3::hash(format!("backup effect stage {sequence}").as_bytes())
                            .to_hex()
                            .to_string(),
                    )
                    .expect("effect stage digest"),
                },
                GenerationCheckpointEnvelope {
                    turn_id: effect_send.turn.id,
                    attempt_id: effect_attempt_id,
                    job_id: None,
                    correlation_id: None,
                    sequence,
                    event: GenerationCheckpointEvent::Stage { status },
                },
                TimestampMillis::new(20 + i64::try_from(sequence).expect("effect stage time")),
            )
            .expect("append effect stage")
            .value
            .revision;
        }
        let effect_usage = UsageLedger::record(
            backend.database(),
            UsageRecord {
                turn_id: effect_send.turn.id,
                attempt_id: effect_attempt_id,
                outcome: UsageOutcome::Succeeded,
                usage: UsageCounters::Known(lettuce_conversations::InferenceUsage {
                    provider_reported_cost: None,
                    cache_write_tokens: None,
                    web_search_requests: None,
                    cached_input_tokens: None,
                    reasoning_tokens: None,
                    input_tokens: 7,
                    output_tokens: 4,
                }),
                model_profile_id: Some(backup_model.id),
                model_revision: Some(backup_model.revision),
                provider_account_id: Some(provider_account_id),
                provider_account_revision: Some(Revision::INITIAL),
                recorded_at: TimestampMillis::new(24),
            },
        )
        .expect("record effect usage");
        let direct_before_effect =
            ConversationReader::get(backend.database(), direct_conversation.id)
                .expect("conversation before effect finalization")
                .conversation;
        let ConversationKind::Direct(details) = &direct_before_effect.kind else {
            panic!("expected direct conversation");
        };
        let effect_model = match &details.model {
            SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => {
                model.clone()
            }
            SnapshotSelection::Disabled => panic!("expected effect model"),
        };
        let effect_finalized = ConversationRepository::finalize_generation(
            backend.database(),
            effect_send.turn.id,
            effect_attempt_id,
            direct_before_effect.revision,
            effect_turn_revision,
            &OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("backup-effect-finalize")
                    .expect("effect finalize key"),
                request_digest: ContentHash::parse(
                    blake3::hash(b"backup effect finalize").to_hex().to_string(),
                )
                .expect("effect finalize digest"),
            },
            FinalizationDraft {
                parts: vec![MessagePart::Text {
                    text: "This effect will survive the backup.".into(),
                }],
                ordinal: 0,
                model: effect_model,
                replay: None,
                outcome: GenerationCheckpointEvent::Completed,
            },
            effect_usage.id,
            TimestampMillis::new(25),
        )
        .expect("finalize effect message")
        .value;
        let effect = CompanionTurnEffectRepository::get_for_message(
            backend.database(),
            direct_conversation.id,
            effect_finalized.assistant_message.id,
        )
        .expect("load processing effect")
        .expect("processing effect");
        let effect = CompanionTurnEffectRepository::settle(
            backend.database(),
            effect.id,
            CompanionTurnEffectOutcome::Ready {
                summary: Some("Backup effect summary".into()),
                memory_changes: CompanionMemoryChanges::default(),
                source_window: CompanionEffectSourceWindow {
                    message_ids: vec![effect_user_message_id, effect.assistant_message_id],
                    enqueued_at: TimestampMillis::new(25),
                },
            },
            TimestampMillis::new(26),
        )
        .expect("settle companion effect");
        let memory_space =
            MemoryRepository::get_for_conversation(backend.database(), direct_conversation.id)
                .expect("read conversation memory")
                .expect("dynamic memory space");
        let rewind = DynamicMemorySuffixRewindRepository::rewind_dynamic_memory_suffix(
            backend.database(),
            DynamicMemorySuffixRewind {
                operation_id: OperationId::new(),
                conversation_id: direct_conversation.id,
                invalid_run_id: None,
                expected_memory_revision: memory_space.revision,
                invalidated_effect_ids: vec![effect.id],
                at: TimestampMillis::new(27),
            },
        )
        .expect("rewind companion effect");
        assert_eq!(rewind.invalidated_effect_ids, vec![effect.id]);
        let memory_id = MemoryId::new();
        let memory_after_item = MemoryRepository::compare_and_apply(
            backend.database(),
            MemoryChangeSet {
                space_id: memory_space.id,
                expected_revision: rewind.memory.revision,
                items: vec![MemoryItem {
                    id: memory_id,
                    text: "The user asked to preserve full backup state.".into(),
                    category: MemoryCategory::Preference,
                    source_message_id: Some(effect_user_message_id),
                    source_role: Some(MessageRole::User),
                    observed_at: Some(TimestampMillis::new(20)),
                    observed_time_precision: Some("turn".into()),
                    superseded_by: None,
                    superseded_at: None,
                    supersedes: Vec::new(),
                    token_count: 9,
                    is_cold: true,
                    is_pinned: false,
                    importance: Score::from_basis_points(4_000).expect("importance"),
                    persistence_importance: Score::from_basis_points(8_000)
                        .expect("persistence importance"),
                    prompt_importance: Score::from_basis_points(6_000).expect("prompt importance"),
                    volatility: Score::LEGACY_VOLATILITY,
                    access_count: 0,
                    created_at: TimestampMillis::new(28),
                    last_accessed_at: TimestampMillis::new(28),
                }],
            },
        )
        .expect("store backup memory");
        let memory_summary = MemorySummaryRepository::compare_and_apply_summary(
            backend.database(),
            MemorySummaryChange {
                expected_revision: memory_after_item.revision,
                summary: MemorySummary {
                    space_id: memory_space.id,
                    text: "The conversation established a backup preference.".into(),
                    token_count: 7,
                    window_start: 0,
                    window_end: 2,
                    source_message_ids: vec![effect_user_message_id, effect.assistant_message_id],
                    updated_at: TimestampMillis::new(29),
                },
            },
        )
        .expect("store backup memory summary");
        let memory_access = MemoryRetrievalRepository::apply_retrieval_access(
            backend.database(),
            MemoryRetrievalAccess {
                conversation_id: direct_conversation.id,
                turn_id: effect_send.turn.id,
                attempt_id: effect_attempt_id,
                space_id: memory_space.id,
                expected_revision: memory_summary.memory.revision,
                selected_memory_ids: vec![memory_id],
                accessed_at: TimestampMillis::new(30),
            },
        )
        .expect("record backup memory access");
        let group_conversation = backend
            .launch_group_conversation(
                &GroupConversationLaunchRequest {
                    format_version: GROUP_LAUNCH_REQUEST_FORMAT_V1,
                    title: "Backup group chat".into(),
                    user: DirectUserParticipant {
                        display_name: "User".into(),
                        authored_description: None,
                    },
                    group_id,
                    persona: LaunchSelection::Disabled,
                    operation_key: lettuce_jobs::IdempotencyKey::new("backup-group-chat")
                        .expect("operation key"),
                },
                TimestampMillis::new(3),
            )
            .expect("launch group conversation")
            .value
            .conversation;
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
            .expect("filesystem authority");
        let ingest = LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read capability"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write capability"),
            Database::open(&path).expect("blob database"),
            Database::open(&path).expect("asset database"),
        );
        let mut image_bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        image_bytes.extend_from_slice(&13_u32.to_be_bytes());
        image_bytes.extend_from_slice(b"IHDR");
        image_bytes.extend_from_slice(&2_u32.to_be_bytes());
        image_bytes.extend_from_slice(&3_u32.to_be_bytes());
        image_bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        image_bytes.extend_from_slice(b"backup image bytes");
        let avatar = ingest
            .ingest(
                image_bytes.as_slice(),
                IngestRequest::new(
                    AssetKind::AvatarOriginal,
                    AssetOrigin::Upload,
                    RetentionClass::Persistent,
                    AssetProvenanceV1::default(),
                ),
            )
            .expect("ingest avatar");
        let illustration = ingest
            .ingest(
                image_bytes.as_slice(),
                IngestRequest::new(
                    AssetKind::Illustration,
                    AssetOrigin::Upload,
                    RetentionClass::Persistent,
                    AssetProvenanceV1::default(),
                ),
            )
            .expect("ingest shared illustration");
        assert_eq!(avatar.blob.id, illustration.blob.id);
        let samples = [0_i16, 1, -1, 0];
        let data_size = u32::try_from(samples.len() * 2).expect("WAV data size");
        let mut voice_bytes = Vec::new();
        voice_bytes.extend_from_slice(b"RIFF");
        voice_bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        voice_bytes.extend_from_slice(b"WAVEfmt ");
        voice_bytes.extend_from_slice(&16_u32.to_le_bytes());
        voice_bytes.extend_from_slice(&1_u16.to_le_bytes());
        voice_bytes.extend_from_slice(&1_u16.to_le_bytes());
        voice_bytes.extend_from_slice(&16_000_u32.to_le_bytes());
        voice_bytes.extend_from_slice(&32_000_u32.to_le_bytes());
        voice_bytes.extend_from_slice(&2_u16.to_le_bytes());
        voice_bytes.extend_from_slice(&16_u16.to_le_bytes());
        voice_bytes.extend_from_slice(b"data");
        voice_bytes.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            voice_bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let voice_audio = ingest
            .ingest(
                voice_bytes.as_slice(),
                IngestRequest::new(
                    AssetKind::OtherAudio,
                    AssetOrigin::Upload,
                    RetentionClass::Library,
                    AssetProvenanceV1::default(),
                ),
            )
            .expect("ingest voice example");
        let term = backend
            .database()
            .save_vocabulary(
                AsrVocabularyTerm::new(
                    "Lettuce AI",
                    Some("en"),
                    Some("product"),
                    Some("workspace"),
                    80,
                    TimestampMillis::new(2),
                )
                .expect("vocabulary term"),
            )
            .expect("store vocabulary");
        let correction = backend
            .database()
            .save_correction(
                AsrCorrectionRule::new(
                    "lettuce a eye",
                    "Lettuce AI",
                    Some("en"),
                    Some("group-chat"),
                    true,
                    TimestampMillis::new(2),
                )
                .expect("correction"),
            )
            .expect("store correction");
        backend
            .database()
            .save_ignored_suggestion(AsrIgnoredSuggestion {
                id: lettuce_types::AsrIgnoredSuggestionId::new(),
                wrong: "green salad".into(),
                normalized_wrong: "green salad".into(),
                correct: "green solid".into(),
                normalized_correct: "green solid".into(),
                language: Some("en".into()),
                scope: "direct-chat".into(),
                ignored_count: 2,
                last_ignored_at: TimestampMillis::new(2),
                created_at: TimestampMillis::new(2),
                updated_at: TimestampMillis::new(2),
            })
            .expect("store ignored suggestion");
        let mut voice_example = AsrVoiceExample::new(
            voice_audio.asset.id,
            "Lettuce AI",
            Some("lettuce a eye".into()),
            Some("en"),
            Some("voice-profile"),
            TimestampMillis::new(2),
        )
        .expect("voice example");
        voice_example.vocabulary_term_id = Some(term.id);
        voice_example.correction_id = Some(correction.id);
        backend
            .database()
            .save_voice_example(voice_example.clone())
            .expect("store voice example");
        let backup_job = JobStore::create_or_get(
            backend.database(),
            JobSpec::new(
                JobKind::ArtifactInstall,
                JobSubject::new(SubjectKind::ArtifactInstall, avatar.asset.id.to_string())
                    .expect("job subject"),
                OutcomeRef::ArtifactInstallation(avatar.asset.id),
            )
            .with_resources(vec![ResourceClass::Network, ResourceClass::DiskWrite]),
        )
        .expect("create backup job")
        .job;
        let claimed_at = TimestampMillis::new(backup_job.updated_at.get() + 1);
        let claim = JobStore::claim(
            backend.database(),
            backup_job.id,
            WorkerId::new(),
            claimed_at,
            Duration::from_secs(10),
            &ResourceAvailability::all(),
        )
        .expect("claim backup job")
        .expect("eligible backup job");
        JobStore::append_and_transition(
            backend.database(),
            JobMutation::Start {
                claim: claim.claim.clone(),
                at: TimestampMillis::new(claimed_at.get() + 1),
            },
        )
        .expect("start backup job");
        JobStore::append_and_transition(
            backend.database(),
            JobMutation::Progress {
                claim: claim.claim.clone(),
                progress: ProgressSnapshot {
                    units: Some(UnitsProgress::new(2, Some(4)).expect("job progress")),
                    ..ProgressSnapshot::default()
                },
                at: TimestampMillis::new(claimed_at.get() + 2),
            },
        )
        .expect("progress backup job");
        JobStore::append_and_transition(
            backend.database(),
            JobMutation::RetryScheduled {
                claim: claim.claim,
                at: TimestampMillis::new(claimed_at.get() + 3),
            },
        )
        .expect("retry backup job");
        let usage_id = UsageEventId::new();
        backend
            .database()
            .admit_job_usage(JobInferenceUsage {
                id: usage_id,
                job_id: backup_job.id,
                logical_attempt_id: attempt_id,
                model_profile_id: ModelProfileId::new(),
                model_revision: Revision::INITIAL,
                provider_account_id,
                provider_account_revision: Revision::INITIAL,
                admitted_at: TimestampMillis::new(claimed_at.get() + 4),
                result: None,
            })
            .expect("admit job inference evidence");
        backend
            .database()
            .settle_job_usage(
                usage_id,
                JobInferenceUsageResult::Response {
                    usage: Some(lettuce_conversations::InferenceUsage {
                        provider_reported_cost: None,
                        cache_write_tokens: Some(3),
                        web_search_requests: Some(0),
                        cached_input_tokens: Some(5),
                        reasoning_tokens: Some(2),
                        input_tokens: 21,
                        output_tokens: 8,
                    }),
                    provider_response_id: Some("backup-provider-response".into()),
                },
            )
            .expect("settle job inference evidence");
        drop(backend);

        let reopened = AppBackend::open(&path, TimestampMillis::new(3)).expect("reopen backend");
        let media = LocalSyncMediaStore::open(
            root.join("platform-v2/media-blobs"),
            Database::open(&path).expect("blob database"),
            Database::open(&path).expect("asset database"),
        )
        .expect("backup media reader");
        let envelope = ProviderBackupCoordinator::new(
            reopened.database(),
            secret_store.as_ref(),
            &media,
            reopened.database(),
        )
        .export("1.0.0", TimestampMillis::new(4), "backup password")
        .await
        .expect("export backup");
        assert_eq!(
            open_backup(&envelope, "wrong password"),
            Err(BackupEnvelopeError::Authentication)
        );
        let sections = open_backup(&envelope, "backup password").expect("open backup");
        assert_eq!(sections.len(), 22);
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
        let learning: AsrLearningDocument =
            serde_json::from_slice(&sections[2].bytes).expect("ASR learning JSON");
        assert_eq!(learning.vocabulary, vec![term]);
        assert_eq!(learning.corrections, vec![correction]);
        assert_eq!(learning.ignored_suggestions.len(), 1);
        assert_eq!(learning.voice_examples, vec![voice_example]);
        assert_eq!(learning.audio_assets.len(), 1);
        assert_eq!(learning.audio_assets[0].asset_id, voice_audio.asset.id);
        assert_eq!(
            learning.audio_assets[0].content_hash,
            voice_audio.blob.content_hash
        );
        let history: ConversationHistoryBackup =
            serde_json::from_slice(&sections[3].bytes).expect("conversation history JSON");
        assert_eq!(history.conversations.len(), 2);
        let direct = history
            .conversations
            .iter()
            .find(|backup| backup.aggregate.conversation.id == direct_conversation.id)
            .expect("direct conversation backup");
        assert!(matches!(
            &direct.aggregate.conversation.kind,
            ConversationKind::Direct(_)
        ));
        assert_eq!(direct.messages.len(), 6);
        assert_eq!(
            direct.messages[0].revisions[0].parts,
            vec![MessagePart::Text {
                text: String::new()
            }]
        );
        assert_eq!(direct.messages[1].timeline_ordinal, 2);
        let starter_snapshot = match direct.messages[0]
            .initial_origin
            .as_ref()
            .expect("starter origin")
        {
            lettuce_conversations::InitialMessageOrigin::StarterMessage {
                snapshot_ref, ..
            } => snapshot_ref,
            lettuce_conversations::InitialMessageOrigin::SelectedScene { .. } => {
                panic!("starter origin")
            }
        };
        let starter_section = sections
            .iter()
            .find(|section| {
                section.name == format!("conversation/snapshots/{}", starter_snapshot.artifact_id)
            })
            .expect("starter snapshot section");
        assert_eq!(
            starter_section.bytes.len(),
            usize::try_from(starter_snapshot.byte_size).expect("snapshot size")
        );
        assert_eq!(
            ContentHash::parse(blake3::hash(&starter_section.bytes).to_hex().to_string())
                .expect("snapshot hash"),
            starter_snapshot.digest
        );
        assert!(
            starter_section
                .bytes
                .windows("Welcome back.".len())
                .any(|window| window == b"Welcome back.")
        );
        let group = history
            .conversations
            .iter()
            .find(|backup| backup.aggregate.conversation.id == group_conversation.id)
            .expect("group conversation backup");
        assert!(matches!(
            &group.aggregate.conversation.kind,
            ConversationKind::Group(_)
        ));
        assert!(group.messages.is_empty());
        let runtime: lettuce_transfer::ConversationRuntimeBackup =
            serde_json::from_slice(&sections[4].bytes).expect("conversation runtime JSON");
        let jobs: JobBackup = serde_json::from_slice(&sections[5].bytes).expect("jobs JSON");
        let usage: lettuce_transfer::ConversationUsageBackup =
            serde_json::from_slice(&sections[6].bytes).expect("conversation usage JSON");
        let outbox: lettuce_transfer::ConversationOutboxBackup =
            serde_json::from_slice(&sections[7].bytes).expect("conversation outbox JSON");
        let companion: lettuce_transfer::CompanionStateBackup =
            serde_json::from_slice(&sections[8].bytes).expect("companion state JSON");
        let companion_effects: lettuce_transfer::CompanionEffectBackup =
            serde_json::from_slice(&sections[9].bytes).expect("companion effects JSON");
        let memory: lettuce_transfer::MemoryBackup =
            serde_json::from_slice(&sections[10].bytes).expect("memory JSON");
        assert_eq!(usage.events.len(), 3);
        let known = usage
            .events
            .iter()
            .find(|entry| entry.event.id == known_usage.id)
            .expect("known usage");
        assert!(known.cost_basis.is_some());
        assert_eq!(known.overlapping_job_inference_ids, vec![usage_id]);
        let unavailable = usage
            .events
            .iter()
            .find(|entry| entry.event.id == unavailable_usage.id)
            .expect("unavailable usage");
        assert!(matches!(
            unavailable.event.record.usage,
            UsageCounters::Unavailable(UsageUnavailableReason::TransportFailed)
        ));
        assert!(unavailable.cost_basis.is_none());
        assert!(unavailable.overlapping_job_inference_ids.is_empty());
        assert_eq!(outbox.conversations.len(), 2);
        let direct_outbox = outbox
            .conversations
            .iter()
            .find(|journal| journal.conversation_id == direct_conversation.id)
            .expect("direct conversation outbox");
        assert!(direct_outbox.operations.len() > direct_outbox.events.len());
        assert_eq!(
            direct_outbox
                .events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            (1..=u64::try_from(direct_outbox.events.len()).expect("outbox length"))
                .collect::<Vec<_>>()
        );
        assert!(direct_outbox.events.iter().any(|event| {
            matches!(
                event.event,
                ConversationOutboxEvent::TurnFailed { usage_event_id, .. }
                    if usage_event_id == unavailable_usage.id
            )
        }));
        let mut corrupt_outbox = outbox.clone();
        corrupt_outbox
            .conversations
            .iter_mut()
            .find(|journal| journal.conversation_id == direct_conversation.id)
            .expect("direct conversation outbox")
            .events[1]
            .sequence = 1;
        assert_eq!(
            corrupt_outbox.canonicalize_and_validate(&history, &runtime, &usage),
            Err(lettuce_transfer::ConversationOutboxBackupError::InvalidData)
        );
        assert_eq!(companion.relationships.len(), 1);
        assert_eq!(companion.sessions.len(), 1);
        assert_eq!(companion.episodes.len(), 1);
        assert_eq!(companion.receipts.len(), 1);
        assert_eq!(companion.sessions[0].owner, companion_owner);
        assert_eq!(
            companion.sessions[0].active_signals,
            vec!["backup continuity"]
        );
        assert_eq!(companion.episodes[0].episode_index, 1);
        assert_eq!(companion.receipts[0].receipt, companion_receipt);
        let current_graph =
            lettuce_transfer::ProviderBackupSource::read_provider_backup_graph(reopened.database())
                .expect("current backup graph");
        let mut corrupt_companion = companion.clone();
        corrupt_companion.episodes[0].episode_index = 0;
        assert_eq!(
            corrupt_companion.canonicalize_and_validate(&current_graph.authored, &history),
            Err(lettuce_transfer::CompanionStateBackupError::InvalidData)
        );
        assert_eq!(companion_effects.effects.len(), 1);
        assert_eq!(companion_effects.rewinds.len(), 1);
        assert_eq!(companion_effects.effects[0].id, effect.id);
        assert_eq!(
            companion_effects.effects[0].status,
            CompanionTurnEffectStatus::Invalidated
        );
        assert_eq!(
            companion_effects.effects[0].summary.as_deref(),
            Some("Backup effect summary")
        );
        assert_eq!(
            companion_effects.rewinds[0].invalidated_effect_ids,
            vec![effect.id]
        );
        assert_eq!(companion_effects.rewinds[0].resulting_memory, rewind.memory);
        let mut corrupt_effects = companion_effects.clone();
        corrupt_effects.rewinds[0].invalidated_effect_ids.clear();
        assert_eq!(
            corrupt_effects.canonicalize_and_validate(&history, &runtime),
            Err(lettuce_transfer::CompanionEffectBackupError::InvalidData)
        );
        assert_eq!(memory.spaces.len(), 2);
        let direct_memory = memory
            .spaces
            .iter()
            .find(|space| space.conversation_id == direct_conversation.id)
            .expect("direct memory");
        assert_eq!(
            direct_memory.snapshot.revision,
            memory_access.resulting_revision
        );
        assert_eq!(direct_memory.snapshot.items.len(), 1);
        assert_eq!(direct_memory.snapshot.items[0].id, memory_id);
        assert_eq!(direct_memory.snapshot.items[0].access_count, 2);
        assert!(!direct_memory.snapshot.items[0].is_cold);
        assert_eq!(
            direct_memory.summary.as_ref().expect("memory summary"),
            &memory_summary.summary
        );
        assert_eq!(memory.retrieval_accesses, vec![memory_access.clone()]);
        let mut corrupt_memory = memory.clone();
        corrupt_memory.retrieval_accesses[0].access.space_id = lettuce_types::MemorySpaceId::new();
        assert_eq!(
            corrupt_memory.canonicalize_and_validate(&history, &runtime, &companion_effects),
            Err(lettuce_transfer::MemoryBackupError::InvalidData)
        );
        let backed_up_job = jobs
            .jobs
            .iter()
            .find(|job| job.snapshot.id == backup_job.id)
            .expect("backed up job");
        assert_eq!(backed_up_job.snapshot.state, lettuce_jobs::JobState::Queued);
        assert_eq!(backed_up_job.snapshot.attempt.get(), 1);
        assert_eq!(backed_up_job.events.len(), 6);
        assert_eq!(jobs.inference.len(), 1);
        assert_eq!(jobs.inference[0].evidence.id, usage_id);
        assert!(matches!(
            &jobs.inference[0].evidence.result,
            Some(JobInferenceUsageResult::Response {
                provider_response_id: Some(id),
                ..
            }) if id == "backup-provider-response"
        ));
        let mut corrupt_jobs = jobs.clone();
        corrupt_jobs
            .jobs
            .iter_mut()
            .find(|job| job.snapshot.id == backup_job.id)
            .expect("backed up job")
            .events[0]
            .seq = lettuce_jobs::EventSeq::new(2);
        assert_eq!(
            corrupt_jobs.canonicalize_and_validate(),
            Err(lettuce_transfer::JobBackupError::InvalidData)
        );
        let direct_runtime = runtime
            .conversations
            .iter()
            .find(|value| value.conversation_id == direct_conversation.id)
            .expect("direct runtime");
        assert_eq!(direct_runtime.turns.len(), 3);
        assert_eq!(direct_runtime.turns[0].turn.id, pending_send.turn.id);
        assert_eq!(direct_runtime.turns[0].turn.attempts.len(), 1);
        assert_eq!(direct_runtime.turns[0].attempts.len(), 1);
        assert_eq!(direct_runtime.turns[0].attempts[0].checkpoints.len(), 3);
        assert_eq!(direct_runtime.turns[0].attempts[0].tools.len(), 1);
        assert_eq!(
            direct_runtime.turns[0].attempts[0].tools[0].status,
            ToolExecutionStatus::Succeeded
        );
        let mut corrupt_runtime = runtime.clone();
        corrupt_runtime
            .conversations
            .iter_mut()
            .find(|value| value.conversation_id == direct_conversation.id)
            .expect("direct runtime")
            .turns[0]
            .attempts[0]
            .attempt_id = lettuce_types::GenerationAttemptId::new();
        assert_eq!(
            corrupt_runtime.canonicalize_and_validate(&history),
            Err(lettuce_transfer::ConversationRuntimeBackupError::InvalidData)
        );
        let mut corrupt_history = history.clone();
        corrupt_history
            .conversations
            .iter_mut()
            .find(|backup| backup.aggregate.conversation.id == direct_conversation.id)
            .expect("direct conversation backup")
            .messages[1]
            .message
            .parent_message_id = Some(MessageId::new());
        assert_eq!(
            corrupt_history.canonicalize_and_validate(&BTreeSet::new()),
            Err(ConversationHistoryBackupError::InvalidData)
        );
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
            3
        );
        assert_eq!(
            metadata["authored"]["groups"][0]["group"]["name"],
            "Backup cast"
        );
        let image_section = sections
            .iter()
            .find(|section| section.name == format!("media/blobs/{}", avatar.blob.content_hash))
            .expect("image section");
        assert_eq!(&*image_section.bytes, &image_bytes);
        let voice_section = sections
            .iter()
            .find(|section| {
                section.name == format!("media/blobs/{}", voice_audio.blob.content_hash)
            })
            .expect("voice section");
        assert_eq!(&*voice_section.bytes, &voice_bytes);
        assert_eq!(
            metadata["authored"]["media_assets"]
                .as_array()
                .expect("assets")
                .len(),
            3
        );
        assert_eq!(
            metadata["authored"]["media_blobs"]
                .as_array()
                .expect("blobs")
                .len(),
            2
        );

        let hash = avatar.blob.content_hash.as_str();
        std::fs::remove_file(
            root.join("platform-v2/media-blobs/objects")
                .join(&hash[..2])
                .join(&hash[2..4])
                .join(hash),
        )
        .expect("remove media fixture");
        assert!(matches!(
            reopened
                .provider_backup(secret_store.as_ref(), &media)
                .export("1.0.0", TimestampMillis::new(5), "backup password")
                .await,
            Err(ProviderBackupError::Media(
                lettuce_media::MediaStoreError::ObjectMissing
            ))
        ));

        let changing = ChangingSecretStore {
            status_calls: AtomicUsize::new(0),
        };
        assert_eq!(
            reopened
                .provider_backup(&changing, &media)
                .export("1.0.0", TimestampMillis::new(6), "backup password")
                .await,
            Err(ProviderBackupError::SecretChanged)
        );
    }
}
