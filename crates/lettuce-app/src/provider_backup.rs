use std::fmt;

use lettuce_conversations::{
    ArtifactTransferError, ConversationArtifactTransferPort, TrustedArtifactDescriptor,
    TrustedArtifactSink,
};
use lettuce_settings::{SecretState, SecretStore};
use lettuce_transfer::{
    BackupConversationArtifact, BackupEnvelopeError, BackupMediaObject, MAX_BACKUP_ENTRIES,
    MAX_BACKUP_TOTAL_BYTES, ProviderBackupGraphError, ProviderBackupSecret, ProviderBackupSource,
    ProviderBackupSourceError, provider_backup_artifact_requirements,
    provider_backup_media_requirements, provider_backup_secret_requirements,
    provider_backup_sections, seal_backup,
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
            .and_then(|count| count.checked_add(4))
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
    };

    use lettuce_characters::{
        Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
        CharacterProvenance, CharacterRepository, ConversationStarter, CreateCharacterPlan,
        CreateGroupPlan, GroupMember, GroupProfile, GroupRepository, Persona, PersonaRepository,
        SpeakerSelection, StarterMessage, StarterRole,
    };
    use lettuce_context::{
        DetectionPolicy, LorebookBehaviorVersion, LorebookMetadataDraft, LorebookRepository,
    };
    use lettuce_conversations::{ConversationKind, MessagePart};
    use lettuce_database::Database;
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
        LocalSyncMediaStore, RetentionClass,
    };
    use lettuce_models::{
        ProviderAccount, ProviderAccountRepository, ProviderConfig, ProviderProtocol,
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
        ConversationHistoryBackupError, open_backup,
    };
    use lettuce_types::{
        AudioProviderId, CharacterId, GroupId, MessageId, OperationId, PersonaId,
        ProviderAccountId, Revision, StarterMessageId, VoiceProfileId,
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
        let mut direct_starter_id = None;
        let character_ids = ["Ada", "Bea"].map(|name| {
            let id = CharacterId::new();
            let starters = if name == "Ada" {
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
                    character_id: character_ids[0],
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
        assert_eq!(sections.len(), 13);
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
        assert_eq!(direct.messages.len(), 2);
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
            2
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
