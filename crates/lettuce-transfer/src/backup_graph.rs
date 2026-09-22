use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use lettuce_characters::{
    CharacterDetails, GroupDetails, Persona, PersonaDefaultState, Selection, VoicePreference,
};
use lettuce_context::{
    LorebookBinding, LorebookDetails, PromptDocument, PromptProvenance, validate_bindings,
};
use lettuce_conversations::TrustedArtifactDescriptor;
use lettuce_media::{BlobState, MAX_MEDIA_BLOB_BYTES, MediaAsset, MediaBlob};
use lettuce_models::{ModelProfile, ProviderAccount, validate_provider_connection};
use lettuce_settings::{GlobalSettings, SecretPurpose, SecretRef, SecretValue};
use lettuce_speech::{AudioProvider, UserVoice};
use lettuce_types::{
    CharacterId, GroupId, LorebookId, ModelProfileId, PersonaId, PromptDocumentId, Revision,
    TimestampMillis,
};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

use crate::BackupSection;

pub const PROVIDER_BACKUP_GRAPH_VERSION: u32 = 2;
pub const PROVIDER_BACKUP_FIXED_SECTIONS: usize = 13;
pub const MAX_BACKUP_PROVIDER_ACCOUNTS: usize = 128;
pub const MAX_BACKUP_MODEL_PROFILES: usize = 2_048;
pub const MAX_BACKUP_PROMPT_DOCUMENTS: usize = 2_048;
pub const MAX_BACKUP_AUDIO_PROVIDERS: usize = 128;
pub const MAX_BACKUP_USER_VOICES: usize = 4_096;
pub const MAX_BACKUP_AUTHORED_ROOTS: usize = 4_096;
pub const MAX_BACKUP_MEDIA_RECORDS: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderBackupSelections {
    pub default_model_profile_id: Option<ModelProfileId>,
    pub dynamic_memory_model_profile_id: Option<ModelProfileId>,
    pub group_speaker_model_profile_id: Option<ModelProfileId>,
    pub default_prompt_document_id: Option<PromptDocumentId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupGlobalSettings {
    pub value: GlobalSettings,
    #[serde(
        default,
        skip_serializing_if = "lettuce_models::ModelSettingsLayer::is_empty"
    )]
    pub model_settings: lettuce_models::ModelSettingsLayer,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupLorebookBindings<Owner> {
    pub owner_id: Owner,
    pub bindings: Vec<LorebookBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredProfileBackup {
    pub personas: Vec<Persona>,
    pub persona_default: PersonaDefaultState,
    pub lorebooks: Vec<LorebookDetails>,
    pub characters: Vec<CharacterDetails>,
    pub groups: Vec<GroupDetails>,
    pub character_lorebooks: Vec<BackupLorebookBindings<CharacterId>>,
    pub persona_lorebooks: Vec<BackupLorebookBindings<PersonaId>>,
    pub group_lorebooks: Vec<BackupLorebookBindings<GroupId>>,
    pub media_assets: Vec<MediaAsset>,
    pub media_blobs: Vec<MediaBlob>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderBackupGraph {
    pub version: u32,
    #[serde(default)]
    pub creation: CreationBackup,
    #[serde(default)]
    pub legacy_imports: crate::LegacyImportBackup,
    #[serde(default)]
    pub playground_history: crate::PlaygroundHistoryBackup,
    pub accounts: Vec<ProviderAccount>,
    pub profiles: Vec<ModelProfile>,
    pub prompts: Vec<PromptDocument>,
    pub selections: ProviderBackupSelections,
    pub settings: BackupGlobalSettings,
    pub audio_providers: Vec<AudioProvider>,
    pub user_voices: Vec<UserVoice>,
    pub authored: AuthoredProfileBackup,
    #[serde(skip)]
    pub asr_learning: crate::AsrLearningDocument,
    #[serde(skip)]
    pub conversation_history: crate::ConversationHistoryBackup,
    #[serde(skip)]
    pub conversation_runtime: crate::ConversationRuntimeBackup,
    #[serde(skip)]
    pub job_backup: crate::JobBackup,
    #[serde(skip)]
    pub conversation_usage: crate::ConversationUsageBackup,
    #[serde(skip)]
    pub conversation_outbox: crate::ConversationOutboxBackup,
    #[serde(skip)]
    pub companion_state: crate::CompanionStateBackup,
    #[serde(skip)]
    pub companion_effects: crate::CompanionEffectBackup,
    #[serde(skip)]
    pub memory: crate::MemoryBackup,
    #[serde(skip)]
    pub memory_projections: crate::MemoryProjectionBackup,
    #[serde(skip)]
    pub dynamic_memory: crate::DynamicMemoryBackup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderBackupSourceError {
    #[error("backup source contains invalid data")]
    InvalidData,
    #[error("backup source could not be read")]
    Storage,
}

/// Creation runs that are not tied to a creation workflow's proposal chain.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationBackup {
    #[serde(default)]
    pub lorebook_entry_runs: Vec<BackupLorebookEntryRun>,
    #[serde(default)]
    pub lorebook_keyword_runs: Vec<BackupLorebookKeywordRun>,
    #[serde(default)]
    pub staged_lorebooks: Vec<lettuce_creation::StagedLorebookPlanningRun>,
    #[serde(default)]
    pub staged_lorebook_writer_runs: Vec<lettuce_creation::StagedLorebookWriterRun>,
    #[serde(default)]
    pub workflows: Vec<BackupCreationWorkflow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupCreationWorkflow {
    pub workflow: lettuce_creation::CreationWorkflow,
    pub proposals: Vec<lettuce_creation::CreationProposal>,
    pub turns: Vec<lettuce_creation::CreationTurn>,
    pub attempts: Vec<BackupCreationAttempt>,
    pub persona_receipt: Option<lettuce_creation::CreationApplyReceipt>,
    pub character_receipt: Option<lettuce_creation::CreationCharacterApplyReceipt>,
    pub lorebook_receipt: Option<lettuce_creation::CreationLorebookApplyReceipt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupCreationAttempt {
    pub attempt: lettuce_creation::CreationInferenceAttempt,
    pub rounds: Vec<lettuce_creation::CreationInferenceRound>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupLorebookEntryRun {
    pub run: lettuce_creation::LorebookEntryGenerationRun,
    pub attempts: Vec<lettuce_creation::LorebookEntryAttemptCheckpoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupLorebookKeywordRun {
    pub run: lettuce_creation::LorebookKeywordGenerationRun,
    pub attempts: Vec<lettuce_creation::LorebookKeywordAttemptCheckpoint>,
}

pub trait ProviderBackupSource: Send + Sync {
    fn read_provider_backup_graph(&self) -> Result<ProviderBackupGraph, ProviderBackupSourceError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderBackupRestoreWriteError {
    #[error("backup restore target already holds data")]
    TargetNotEmpty,
    #[error("backup graph cannot be written")]
    InvalidData,
    #[error("backup restore target could not be written")]
    Storage,
}

/// Writes a decoded backup graph into an empty database exactly as it was
/// exported, keeping ids, revisions, timestamps and states.
pub trait ProviderBackupRestoreWriter: Send + Sync {
    fn restore_provider_backup_graph(
        &self,
        graph: &ProviderBackupGraph,
        artifacts: &[BackupConversationArtifact],
    ) -> Result<(), ProviderBackupRestoreWriteError>;
}

pub struct ProviderBackupSecret {
    pub reference: SecretRef,
    pub purpose: SecretPurpose,
    pub generation: u64,
    pub value: SecretValue,
}

pub struct BackupMediaObject {
    pub content_hash: lettuce_types::ContentHash,
    pub bytes: zeroize::Zeroizing<Vec<u8>>,
}

pub struct BackupConversationArtifact {
    pub descriptor: TrustedArtifactDescriptor,
    pub bytes: zeroize::Zeroizing<Vec<u8>>,
}

impl fmt::Debug for BackupMediaObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupMediaObject")
            .field("content_hash", &self.content_hash)
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

impl fmt::Debug for BackupConversationArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupConversationArtifact")
            .field("descriptor", &self.descriptor)
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

impl fmt::Debug for ProviderBackupSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderBackupSecret")
            .field("reference", &self.reference)
            .field("purpose", &self.purpose)
            .field("generation", &self.generation)
            .field("value", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderBackupGraphError {
    #[error("provider backup graph exceeds its limit")]
    LimitExceeded,
    #[error("provider backup graph is invalid")]
    InvalidGraph,
    #[error("provider backup secret inventory is incomplete or inconsistent")]
    InvalidSecrets,
    #[error("provider backup graph serialization failed")]
    Serialization,
}

/// A validated export: the fixed data and secret sections, then the media
/// blobs and conversation artifacts the writer appends one at a time after
/// [`verify_backup_media`] and [`verify_backup_artifact`].
#[derive(Debug)]
pub struct ProviderBackupExportPlan {
    pub data_sections: Vec<BackupSection>,
    pub media: Vec<(lettuce_types::ContentHash, u64)>,
    pub artifacts: Vec<TrustedArtifactDescriptor>,
}

pub fn provider_backup_sections(
    graph: ProviderBackupGraph,
    secrets: Vec<ProviderBackupSecret>,
    media: Vec<BackupMediaObject>,
    artifacts: Vec<BackupConversationArtifact>,
) -> Result<Vec<BackupSection>, ProviderBackupGraphError> {
    let plan = plan_provider_backup_export(graph, secrets)?;
    if plan.media.len() != media.len() || plan.artifacts.len() != artifacts.len() {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }
    let mut sections = plan.data_sections;
    for ((content_hash, byte_size), object) in plan.media.iter().zip(media) {
        if object.content_hash != *content_hash {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
        verify_backup_media(content_hash, *byte_size, &object.bytes)?;
        sections.push(BackupSection::new(
            backup_media_section_name(content_hash),
            BACKUP_MEDIA_SECTION_SCHEMA,
            object.bytes.to_vec(),
        ));
    }
    for (descriptor, artifact) in plan.artifacts.iter().zip(artifacts) {
        if artifact.descriptor != *descriptor {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
        let (name, schema) = verify_backup_artifact(descriptor, &artifact.bytes)?;
        sections.push(BackupSection::new(name, schema, artifact.bytes.to_vec()));
    }
    Ok(sections)
}

pub fn plan_provider_backup_export(
    mut graph: ProviderBackupGraph,
    secrets: Vec<ProviderBackupSecret>,
) -> Result<ProviderBackupExportPlan, ProviderBackupGraphError> {
    canonicalize_and_validate(&mut graph)?;
    let expected = expected_secrets(&graph)?;
    if secrets.len() != expected.len() {
        return Err(ProviderBackupGraphError::InvalidSecrets);
    }
    let mut supplied = BTreeMap::new();
    for secret in &secrets {
        if secret.generation == 0
            || supplied.insert(secret.reference, secret).is_some()
            || expected.get(&secret.reference) != Some(&secret.purpose)
        {
            return Err(ProviderBackupGraphError::InvalidSecrets);
        }
    }
    if supplied.len() != expected.len() {
        return Err(ProviderBackupGraphError::InvalidSecrets);
    }
    let media = graph
        .authored
        .media_blobs
        .iter()
        .filter(|blob| blob.state == BlobState::Ready)
        .map(|blob| {
            if blob.byte_size > MAX_MEDIA_BLOB_BYTES {
                Err(ProviderBackupGraphError::LimitExceeded)
            } else {
                Ok((blob.content_hash.clone(), blob.byte_size))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let artifacts = all_conversation_artifact_descriptors(&graph)?;
    let metadata =
        serde_json::to_vec(&graph).map_err(|_| ProviderBackupGraphError::Serialization)?;
    let asr_learning = serde_json::to_vec(&graph.asr_learning)
        .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let conversation_history = serde_json::to_vec(&graph.conversation_history)
        .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let conversation_runtime = serde_json::to_vec(&graph.conversation_runtime)
        .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let jobs = serde_json::to_vec(&graph.job_backup)
        .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let conversation_usage = serde_json::to_vec(&graph.conversation_usage)
        .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let conversation_outbox = serde_json::to_vec(&graph.conversation_outbox)
        .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let companion_state = serde_json::to_vec(&graph.companion_state)
        .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let companion_effects = serde_json::to_vec(&graph.companion_effects)
        .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let memory =
        serde_json::to_vec(&graph.memory).map_err(|_| ProviderBackupGraphError::Serialization)?;
    let memory_projections = serde_json::to_vec(&graph.memory_projections)
        .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let dynamic_memory = serde_json::to_vec(&graph.dynamic_memory)
        .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let ordered_secrets = expected
        .keys()
        .map(|reference| supplied[reference])
        .collect::<Vec<_>>();
    let secret_bytes = serde_json::to_vec(&SecretDocument {
        version: PROVIDER_BACKUP_GRAPH_VERSION,
        secrets: ordered_secrets,
    })
    .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let mut data_sections = Vec::with_capacity(PROVIDER_BACKUP_FIXED_SECTIONS);
    data_sections.extend([
        BackupSection::new("data/provider-graph.json", "provider-graph.v2", metadata),
        BackupSection::new(
            "secrets/provider-secrets.json",
            "provider-secrets.v2",
            secret_bytes,
        ),
        BackupSection::new("data/asr-learning.json", "asr-learning.v3", asr_learning),
        BackupSection::new(
            "data/conversation-history.json",
            "conversation-history.v1",
            conversation_history,
        ),
        BackupSection::new(
            "data/conversation-runtime.json",
            "conversation-runtime.v1",
            conversation_runtime,
        ),
        BackupSection::new("data/jobs.json", "jobs.v1", jobs),
        BackupSection::new(
            "data/conversation-usage.json",
            "conversation-usage.v1",
            conversation_usage,
        ),
        BackupSection::new(
            "data/conversation-outbox.json",
            "conversation-outbox.v1",
            conversation_outbox,
        ),
        BackupSection::new(
            "data/companion-state.json",
            "companion-state.v1",
            companion_state,
        ),
        BackupSection::new(
            "data/companion-effects.json",
            "companion-effects.v1",
            companion_effects,
        ),
        BackupSection::new("data/memory.json", "memory.v1", memory),
        BackupSection::new(
            "data/memory-projections.json",
            "memory-projections.v1",
            memory_projections,
        ),
        BackupSection::new(
            "data/dynamic-memory.json",
            "dynamic-memory.v1",
            dynamic_memory,
        ),
    ]);
    Ok(ProviderBackupExportPlan {
        data_sections,
        media,
        artifacts,
    })
}

pub fn provider_backup_secret_requirements(
    graph: &ProviderBackupGraph,
) -> Result<Vec<(SecretRef, SecretPurpose)>, ProviderBackupGraphError> {
    let mut graph = graph.clone();
    canonicalize_and_validate(&mut graph)?;
    Ok(expected_secrets(&graph)?.into_iter().collect())
}

pub fn provider_backup_artifact_requirements(
    graph: &ProviderBackupGraph,
) -> Result<Vec<TrustedArtifactDescriptor>, ProviderBackupGraphError> {
    let mut graph = graph.clone();
    canonicalize_and_validate(&mut graph)?;
    all_conversation_artifact_descriptors(&graph)
}

pub(crate) fn all_conversation_artifact_descriptors(
    graph: &ProviderBackupGraph,
) -> Result<Vec<TrustedArtifactDescriptor>, ProviderBackupGraphError> {
    let mut snapshots = BTreeMap::new();
    let mut replays = BTreeMap::new();
    let memory_replays = graph
        .dynamic_memory
        .runs
        .iter()
        .flat_map(|run| &run.attempts)
        .flat_map(|attempt| &attempt.rounds)
        .flat_map(|entry| {
            entry.round.provider_replay.iter().cloned().chain(
                entry
                    .round
                    .calls
                    .iter()
                    .filter_map(|call| call.call.provider_replay.clone()),
            )
        })
        .chain(
            graph
                .creation
                .workflows
                .iter()
                .flat_map(|entry| &entry.attempts)
                .flat_map(|value| &value.rounds)
                .flat_map(|round| {
                    round.provider_replay.iter().cloned().chain(
                        round
                            .calls
                            .iter()
                            .filter_map(|call| call.call.provider_replay.clone()),
                    )
                }),
        )
        .map(TrustedArtifactDescriptor::Replay)
        .collect::<Vec<_>>();
    let descriptors = graph
        .conversation_history
        .artifact_descriptors()
        .map_err(|_| ProviderBackupGraphError::InvalidGraph)?
        .into_iter()
        .chain(
            graph
                .conversation_runtime
                .artifact_descriptors()
                .map_err(|_| ProviderBackupGraphError::InvalidGraph)?,
        )
        .chain(memory_replays);
    for descriptor in descriptors {
        match descriptor {
            TrustedArtifactDescriptor::Snapshot(reference) => {
                if snapshots
                    .insert(reference.artifact_id, reference.clone())
                    .is_some_and(|existing| existing != reference)
                {
                    return Err(ProviderBackupGraphError::InvalidGraph);
                }
            }
            TrustedArtifactDescriptor::Replay(reference) => {
                if replays
                    .insert(reference.artifact_id, reference.clone())
                    .is_some_and(|existing| existing != reference)
                {
                    return Err(ProviderBackupGraphError::InvalidGraph);
                }
            }
        }
    }
    Ok(snapshots
        .into_values()
        .map(TrustedArtifactDescriptor::Snapshot)
        .chain(replays.into_values().map(TrustedArtifactDescriptor::Replay))
        .collect())
}

/// Checks one conversation artifact against its descriptor's size and
/// digest and returns its section name and schema.
pub fn verify_backup_artifact(
    descriptor: &TrustedArtifactDescriptor,
    bytes: &[u8],
) -> Result<(String, String), ProviderBackupGraphError> {
    let (name, schema, digest, byte_size) = backup_artifact_section_identity(descriptor);
    if u64::try_from(bytes.len()).ok() != Some(byte_size) || &content_hash_of(bytes) != digest {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }
    Ok((name, schema))
}

pub(crate) fn backup_artifact_section_identity(
    descriptor: &TrustedArtifactDescriptor,
) -> (String, String, &lettuce_types::ContentHash, u64) {
    match descriptor {
        TrustedArtifactDescriptor::Snapshot(reference) => (
            format!("conversation/snapshots/{}", reference.artifact_id),
            format!("conversation-snapshot.v{}", reference.schema_version),
            &reference.digest,
            reference.byte_size,
        ),
        TrustedArtifactDescriptor::Replay(reference) => (
            format!("conversation/replays/{}", reference.artifact_id),
            format!("conversation-replay.v{}", reference.schema_version),
            &reference.digest,
            reference.byte_size,
        ),
    }
}

pub const BACKUP_MEDIA_SECTION_SCHEMA: &str = "media-blob.v2";

#[must_use]
pub fn backup_media_section_name(content_hash: &lettuce_types::ContentHash) -> String {
    format!("media/blobs/{content_hash}")
}

/// Checks one ready media blob against its size and content hash.
pub fn verify_backup_media(
    content_hash: &lettuce_types::ContentHash,
    byte_size: u64,
    bytes: &[u8],
) -> Result<(), ProviderBackupGraphError> {
    if u64::try_from(bytes.len()).ok() != Some(byte_size) || &content_hash_of(bytes) != content_hash
    {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }
    Ok(())
}

fn content_hash_of(bytes: &[u8]) -> lettuce_types::ContentHash {
    lettuce_types::ContentHash::parse(blake3::hash(bytes).to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash")
}

pub fn canonicalize_and_validate(
    graph: &mut ProviderBackupGraph,
) -> Result<(), ProviderBackupGraphError> {
    if graph.version != PROVIDER_BACKUP_GRAPH_VERSION {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }
    if graph.accounts.len() > MAX_BACKUP_PROVIDER_ACCOUNTS
        || graph.profiles.len() > MAX_BACKUP_MODEL_PROFILES
        || graph.prompts.len() > MAX_BACKUP_PROMPT_DOCUMENTS
        || graph.audio_providers.len() > MAX_BACKUP_AUDIO_PROVIDERS
        || graph.user_voices.len() > MAX_BACKUP_USER_VOICES
        || graph.authored.personas.len() > MAX_BACKUP_AUTHORED_ROOTS
        || graph.authored.lorebooks.len() > MAX_BACKUP_AUTHORED_ROOTS
        || graph.authored.characters.len() > MAX_BACKUP_AUTHORED_ROOTS
        || graph.authored.groups.len() > MAX_BACKUP_AUTHORED_ROOTS
        || graph.authored.media_assets.len() > MAX_BACKUP_MEDIA_RECORDS
        || graph.authored.media_blobs.len() > MAX_BACKUP_MEDIA_RECORDS
    {
        return Err(ProviderBackupGraphError::LimitExceeded);
    }
    graph.accounts.sort_by_key(|account| account.id.to_string());
    graph.profiles.sort_by_key(|profile| profile.id.to_string());
    graph.prompts.sort_by_key(|prompt| prompt.id.to_string());
    graph
        .audio_providers
        .sort_by_key(|provider| provider.id.to_string());
    graph.user_voices.sort_by_key(|voice| voice.id.to_string());
    canonicalize_authored(&mut graph.authored);
    let creation = &mut graph.creation;
    creation
        .lorebook_entry_runs
        .sort_by_key(|entry| entry.run.request_id);
    creation
        .lorebook_keyword_runs
        .sort_by_key(|entry| entry.run.request_id);
    creation.staged_lorebooks.sort_by_key(|run| run.request_id);
    creation
        .staged_lorebook_writer_runs
        .sort_by_key(|run| run.request_id);
    creation.workflows.sort_by_key(|entry| entry.workflow.id);
    for entry in &mut creation.workflows {
        entry.proposals.sort_by_key(|proposal| proposal.ordinal);
        entry.turns.sort_by_key(|turn| turn.ordinal);
        let turn_ordinals = entry
            .turns
            .iter()
            .map(|turn| (turn.id, turn.ordinal))
            .collect::<BTreeMap<_, _>>();
        entry.attempts.sort_by_key(|value| {
            (
                turn_ordinals.get(&value.attempt.turn_id).copied(),
                value.attempt.ordinal,
            )
        });
        if entry
            .proposals
            .first()
            .is_none_or(|first| first.ordinal != 0)
            || entry
                .proposals
                .iter()
                .any(|proposal| proposal.validate().is_err())
            || entry
                .turns
                .iter()
                .any(|turn| turn.workflow_id != entry.workflow.id)
            || entry.attempts.iter().any(|value| {
                value.attempt.workflow_id != entry.workflow.id
                    || value.attempt.validate().is_err()
                    || !turn_ordinals.contains_key(&value.attempt.turn_id)
                    || value.rounds.iter().any(|round| {
                        round.validate().is_err()
                            || round.attempt_id != value.attempt.id
                            || round
                                .calls
                                .iter()
                                .any(|call| call.attempt_id != value.attempt.id)
                    })
            })
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    if creation.lorebook_entry_runs.iter().any(|entry| {
        entry.run.validate().is_err()
            || lettuce_creation::validate_lorebook_entry_attempts(&entry.attempts).is_err()
    }) || creation.lorebook_keyword_runs.iter().any(|entry| {
        entry.run.validate().is_err()
            || lettuce_creation::validate_lorebook_keyword_attempts(&entry.attempts).is_err()
    }) || creation
        .staged_lorebooks
        .iter()
        .any(|run| run.validate().is_err())
        || creation
            .staged_lorebook_writer_runs
            .iter()
            .any(|run| run.validate().is_err())
    {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }
    graph
        .legacy_imports
        .canonicalize_and_validate()
        .map_err(|error| match error {
            crate::LegacyImportBackupError::LimitExceeded => {
                ProviderBackupGraphError::LimitExceeded
            }
            crate::LegacyImportBackupError::InvalidData => ProviderBackupGraphError::InvalidGraph,
        })?;
    graph
        .playground_history
        .canonicalize_and_validate()
        .map_err(|error| match error {
            crate::PlaygroundHistoryBackupError::LimitExceeded => {
                ProviderBackupGraphError::LimitExceeded
            }
            crate::PlaygroundHistoryBackupError::InvalidData => {
                ProviderBackupGraphError::InvalidGraph
            }
        })?;
    let asset_ids = graph
        .authored
        .media_assets
        .iter()
        .map(|asset| asset.id.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    if graph.playground_history.images.iter().any(|image| {
        crate::backup_sql_text(image, "asset_id").is_some_and(|asset| !asset_ids.contains(asset))
    }) {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }
    graph.asr_learning.vocabulary.sort_by_key(|term| term.id);
    graph.asr_learning.corrections.sort_by_key(|rule| rule.id);
    graph
        .asr_learning
        .ignored_suggestions
        .sort_by_key(|ignored| ignored.id);
    graph
        .asr_learning
        .voice_examples
        .sort_by_key(|example| example.id);
    graph
        .asr_learning
        .audio_assets
        .sort_by_key(|asset| asset.asset_id);

    if graph.settings.revision.get() == 0
        || graph.settings.created_at > graph.settings.updated_at
        || graph.settings.model_settings.validate().is_err()
        || !graph.settings.value.ui_preferences.within_bounds()
    {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }

    let mut account_ids = BTreeMap::new();
    let mut owner_ids = BTreeMap::new();
    for account in &graph.accounts {
        if validate_provider_connection(account).is_err()
            || account.label.trim().is_empty()
            || account.revision.get() == 0
            || account.created_at > account.updated_at
            || account_ids
                .insert(account.id, account.secret_owner_id)
                .is_some()
            || owner_ids.insert(account.secret_owner_id, ()).is_some()
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    let mut profile_ids = BTreeMap::new();
    for profile in &graph.profiles {
        if !account_ids.contains_key(&profile.provider_account_id)
            || profile.external_model_id.trim().is_empty()
            || profile.display_name.trim().is_empty()
            || profile.revision.get() == 0
            || profile.created_at > profile.updated_at
            || profile.config.validate_parameters().is_err()
            || profile.config.capabilities.validate().is_err()
            || profile_ids.insert(profile.id, ()).is_some()
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    let mut prompt_ids = BTreeMap::new();
    for prompt in &graph.prompts {
        if prompt.validate().is_err() || prompt_ids.insert(prompt.id, ()).is_some() {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    for prompt in &graph.prompts {
        if matches!(
            prompt.provenance,
            PromptProvenance::Derived { source } if !prompt_ids.contains_key(&source)
        ) {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    for selected in [
        graph.selections.default_model_profile_id,
        graph.selections.dynamic_memory_model_profile_id,
        graph.selections.group_speaker_model_profile_id,
    ]
    .into_iter()
    .flatten()
    {
        if !profile_ids.contains_key(&selected) {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    if graph
        .selections
        .default_prompt_document_id
        .is_some_and(|id| !prompt_ids.contains_key(&id))
    {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }
    let generator = &graph.settings.value.lorebook_generator.selection;
    let memory_prompts = &graph.settings.value.dynamic_memory_prompts;
    let help_me_reply = &graph.settings.value.help_me_reply;
    if graph
        .settings
        .value
        .selected_model_profiles()
        .into_iter()
        .flatten()
        .any(|id| !profile_ids.contains_key(&id))
        || [
            generator.planner_prompt_id,
            generator.writer_prompt_id,
            generator.refine_prompt_id,
            generator.coherence_prompt_id,
            memory_prompts.summarizer_prompt_id,
            memory_prompts.manager_prompt_id,
            help_me_reply.roleplay_prompt_id,
            help_me_reply.conversational_prompt_id,
            graph.settings.value.lorebook_entry_generator.entry_prompt_id,
            graph.settings.value.lorebook_entry_generator.keyword_prompt_id,
            graph.settings.value.companion_soul_writer.prompt_id,
        ]
        .into_iter()
        .flatten()
        .any(|id| !prompt_ids.contains_key(&id))
    {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }

    let mut audio_provider_ids = BTreeMap::new();
    for provider in &graph.audio_providers {
        if provider.validate().is_err()
            || provider.revision.get() == 0
            || audio_provider_ids.insert(provider.id, ()).is_some()
            || owner_ids.insert(provider.secret_owner_id, ()).is_some()
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    let mut voice_ids = BTreeMap::new();
    for voice in &graph.user_voices {
        if voice.validate().is_err()
            || voice.revision.get() == 0
            || !audio_provider_ids.contains_key(&voice.provider_id)
            || voice_ids.insert(voice.id, ()).is_some()
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    validate_authored(graph, &profile_ids, &prompt_ids, &voice_ids)?;
    validate_asr_learning(graph)?;
    let media_asset_ids = graph
        .authored
        .media_assets
        .iter()
        .map(|asset| asset.id)
        .collect();
    graph
        .conversation_history
        .canonicalize_and_validate(&media_asset_ids)
        .map_err(|error| match error {
            crate::ConversationHistoryBackupError::LimitExceeded => {
                ProviderBackupGraphError::LimitExceeded
            }
            crate::ConversationHistoryBackupError::InvalidData => {
                ProviderBackupGraphError::InvalidGraph
            }
        })?;
    graph
        .conversation_runtime
        .canonicalize_and_validate(&graph.conversation_history)
        .map_err(|error| match error {
            crate::ConversationRuntimeBackupError::LimitExceeded => {
                ProviderBackupGraphError::LimitExceeded
            }
            crate::ConversationRuntimeBackupError::InvalidData => {
                ProviderBackupGraphError::InvalidGraph
            }
        })?;
    graph
        .job_backup
        .canonicalize_and_validate()
        .map_err(|error| match error {
            crate::JobBackupError::LimitExceeded => ProviderBackupGraphError::LimitExceeded,
            crate::JobBackupError::InvalidData => ProviderBackupGraphError::InvalidGraph,
        })?;
    validate_job_links(graph)?;
    graph
        .conversation_usage
        .canonicalize_and_validate(&graph.conversation_runtime, &graph.job_backup)
        .map_err(|error| match error {
            crate::ConversationUsageBackupError::LimitExceeded => {
                ProviderBackupGraphError::LimitExceeded
            }
            crate::ConversationUsageBackupError::InvalidData => {
                ProviderBackupGraphError::InvalidGraph
            }
        })?;
    graph
        .conversation_outbox
        .canonicalize_and_validate(
            &graph.conversation_history,
            &graph.conversation_runtime,
            &graph.conversation_usage,
        )
        .map_err(|error| match error {
            crate::ConversationOutboxBackupError::LimitExceeded => {
                ProviderBackupGraphError::LimitExceeded
            }
            crate::ConversationOutboxBackupError::InvalidData => {
                ProviderBackupGraphError::InvalidGraph
            }
        })?;
    graph
        .companion_state
        .canonicalize_and_validate(&graph.authored, &graph.conversation_history)
        .map_err(|_| ProviderBackupGraphError::InvalidGraph)?;
    graph
        .companion_effects
        .canonicalize_and_validate(&graph.conversation_history, &graph.conversation_runtime)
        .map_err(|_| ProviderBackupGraphError::InvalidGraph)?;
    graph
        .memory
        .canonicalize_and_validate(
            &graph.conversation_history,
            &graph.conversation_runtime,
            &graph.companion_effects,
        )
        .map_err(|_| ProviderBackupGraphError::InvalidGraph)?;
    graph
        .memory_projections
        .canonicalize_and_validate(&graph.memory)
        .map_err(|_| ProviderBackupGraphError::InvalidGraph)?;
    graph
        .dynamic_memory
        .canonicalize_and_validate(
            &graph.conversation_history,
            &graph.job_backup,
            &graph.memory,
        )
        .map_err(|_| ProviderBackupGraphError::InvalidGraph)
}

fn validate_job_links(graph: &ProviderBackupGraph) -> Result<(), ProviderBackupGraphError> {
    let inference_owners = graph.job_backup.inference_owners();
    for conversation in &graph.conversation_runtime.conversations {
        for turn in &conversation.turns {
            for attempt in &turn.attempts {
                for dispatch in [
                    attempt
                        .speaker_inference
                        .as_ref()
                        .map(|record| (record.usage_event_id, record.binding.job_id)),
                    attempt
                        .initial_inference
                        .as_ref()
                        .map(|record| (record.usage_event_id, record.binding.job_id)),
                ]
                .into_iter()
                .flatten()
                {
                    if inference_owners.get(&dispatch.0) != Some(&dispatch.1) {
                        return Err(ProviderBackupGraphError::InvalidGraph);
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_asr_learning(graph: &ProviderBackupGraph) -> Result<(), ProviderBackupGraphError> {
    graph
        .asr_learning
        .validate()
        .map_err(|_| ProviderBackupGraphError::InvalidGraph)?;
    let assets = graph
        .authored
        .media_assets
        .iter()
        .map(|asset| (asset.id, asset))
        .collect::<BTreeMap<_, _>>();
    let blobs = graph
        .authored
        .media_blobs
        .iter()
        .map(|blob| (blob.id, blob))
        .collect::<BTreeMap<_, _>>();
    for expected in &graph.asr_learning.audio_assets {
        let asset = assets
            .get(&expected.asset_id)
            .ok_or(ProviderBackupGraphError::InvalidGraph)?;
        let blob = blobs
            .get(&asset.blob_id)
            .ok_or(ProviderBackupGraphError::InvalidGraph)?;
        if asset.kind != expected.kind
            || asset.origin != expected.origin
            || asset.provenance != expected.provenance
            || blob.state != BlobState::Ready
            || blob.kind != lettuce_media::MediaKind::Audio
            || blob.content_hash != expected.content_hash
            || blob.byte_size != expected.byte_size
            || blob.mime_type != expected.mime_type
            || blob.duration_ms != expected.duration_ms
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    Ok(())
}

fn canonicalize_authored(authored: &mut AuthoredProfileBackup) {
    authored.personas.sort_by_key(|value| value.id.to_string());
    authored
        .lorebooks
        .sort_by_key(|value| value.book.id.to_string());
    authored
        .characters
        .sort_by_key(|value| value.character.id.to_string());
    authored
        .groups
        .sort_by_key(|value| value.group.id.to_string());
    authored
        .character_lorebooks
        .sort_by_key(|value| value.owner_id.to_string());
    authored
        .persona_lorebooks
        .sort_by_key(|value| value.owner_id.to_string());
    authored
        .group_lorebooks
        .sort_by_key(|value| value.owner_id.to_string());
    authored
        .media_assets
        .sort_by_key(|value| value.id.to_string());
    authored
        .media_blobs
        .sort_by_key(|value| value.id.to_string());
}

fn validate_authored(
    graph: &ProviderBackupGraph,
    profile_ids: &BTreeMap<ModelProfileId, ()>,
    prompt_ids: &BTreeMap<PromptDocumentId, ()>,
    voice_ids: &BTreeMap<lettuce_types::VoiceProfileId, ()>,
) -> Result<(), ProviderBackupGraphError> {
    let authored = &graph.authored;
    let mut persona_ids = BTreeSet::new();
    for persona in &authored.personas {
        if persona.validate().is_err() || !persona_ids.insert(persona.id) {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    if authored.persona_default.validate().is_err()
        || authored
            .persona_default
            .persona_id
            .is_some_and(|id| !persona_ids.contains(&id))
    {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }
    let mut lorebook_ids = BTreeSet::new();
    for details in &authored.lorebooks {
        if details.validate().is_err() || !lorebook_ids.insert(details.book.id) {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    let mut character_ids = BTreeSet::new();
    for details in &authored.characters {
        let character = &details.character;
        if details.validate().is_err()
            || !character_ids.insert(character.id)
            || character
                .defaults
                .model_profile_id
                .is_some_and(|id| !profile_ids.contains_key(&id))
            || [
                character.defaults.direct_prompt_id,
                character.defaults.group_conversation_prompt_id,
                character.defaults.group_roleplay_prompt_id,
            ]
            .into_iter()
            .flatten()
            .any(|id| !prompt_ids.contains_key(&id))
            || matches!(
                character.defaults.voice,
                Some(VoicePreference::VoiceProfile(id)) if !voice_ids.contains_key(&id)
            )
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
        for starter in &details.starters {
            if starter
                .prompt_id
                .is_some_and(|id| !prompt_ids.contains_key(&id))
                || matches!(
                    &starter.lorebooks,
                    Selection::Explicit(ids) if ids.iter().any(|id| !lorebook_ids.contains(id))
                )
            {
                return Err(ProviderBackupGraphError::InvalidGraph);
            }
        }
    }
    let mut group_ids = BTreeSet::new();
    for details in &authored.groups {
        let group = &details.group;
        if details.validate().is_err()
            || !group_ids.insert(group.id)
            || group.members.iter().any(|member| {
                !character_ids.contains(&member.character_id)
                    || member
                        .model_profile_override
                        .is_some_and(|id| !profile_ids.contains_key(&id))
            })
            || matches!(group.persona, Selection::Explicit(id) if !persona_ids.contains(&id))
            || [
                group.group_conversation_prompt_id,
                group.group_roleplay_prompt_id,
            ]
            .into_iter()
            .flatten()
            .any(|id| !prompt_ids.contains_key(&id))
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    validate_owner_bindings(&authored.character_lorebooks, &character_ids, &lorebook_ids)?;
    validate_owner_bindings(&authored.persona_lorebooks, &persona_ids, &lorebook_ids)?;
    validate_owner_bindings(&authored.group_lorebooks, &group_ids, &lorebook_ids)?;

    let mut blob_kinds = BTreeMap::new();
    let mut blob_hashes = BTreeSet::new();
    for blob in &authored.media_blobs {
        if blob.validate().is_err()
            || blob_kinds.insert(blob.id, blob.kind).is_some()
            || !blob_hashes.insert(blob.content_hash.clone())
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    let mut asset_ids = BTreeSet::new();
    for asset in &authored.media_assets {
        if asset
            .validate_for_blob_kind(
                *blob_kinds
                    .get(&asset.blob_id)
                    .ok_or(ProviderBackupGraphError::InvalidGraph)?,
            )
            .is_err()
            || !asset_ids.insert(asset.id)
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    let referenced_assets = authored
        .personas
        .iter()
        .flat_map(|value| value.media.links.iter().map(|link| link.asset_id))
        .chain(
            authored
                .lorebooks
                .iter()
                .filter_map(|value| value.book.icon_asset_id),
        )
        .chain(authored.characters.iter().flat_map(|details| {
            details
                .character
                .media
                .links
                .iter()
                .map(|link| link.asset_id)
                .chain(details.character.presentation.referenced_asset_ids())
                .chain(
                    details
                        .scenes
                        .iter()
                        .flat_map(|scene| scene.assets.iter().map(|link| link.asset_id)),
                )
        }))
        .chain(authored.groups.iter().flat_map(|details| {
            details
                .group
                .background_asset_id
                .into_iter()
                .chain(details.group.presentation.referenced_asset_ids())
                .chain(
                    details
                        .starting_scene
                        .iter()
                        .flat_map(|scene| scene.scene.assets.iter().map(|link| link.asset_id)),
                )
        }));
    if referenced_assets
        .into_iter()
        .any(|id| !asset_ids.contains(&id))
    {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }
    Ok(())
}

fn validate_owner_bindings<Owner: Ord>(
    owners: &[BackupLorebookBindings<Owner>],
    owner_ids: &BTreeSet<Owner>,
    lorebook_ids: &BTreeSet<LorebookId>,
) -> Result<(), ProviderBackupGraphError> {
    let mut seen = BTreeSet::new();
    for owner in owners {
        if !owner_ids.contains(&owner.owner_id)
            || !seen.insert(&owner.owner_id)
            || validate_bindings(&owner.bindings).is_err()
            || owner
                .bindings
                .iter()
                .any(|binding| !lorebook_ids.contains(&binding.lorebook_id))
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
    }
    if seen.len() != owner_ids.len() {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }
    Ok(())
}

/// Moves every restored secret to a fresh reference, so a restore never
/// overwrites a secret that the previous database file still points at.
pub fn rebind_provider_backup_secrets(
    graph: &mut ProviderBackupGraph,
    secrets: Vec<ProviderBackupSecret>,
) -> Result<Vec<ProviderBackupSecret>, ProviderBackupGraphError> {
    let mut fresh = BTreeMap::new();
    let mut rebind = |reference: &mut SecretRef| {
        *reference = *fresh.entry(*reference).or_insert_with(SecretRef::new);
    };
    for account in &mut graph.accounts {
        if let Some(reference) = account.api_key_ref.as_mut() {
            rebind(reference);
        }
        for header in &mut account.secret_headers {
            rebind(&mut header.secret_ref);
        }
    }
    for provider in &mut graph.audio_providers {
        if let Some(reference) = provider.api_key_ref.as_mut() {
            rebind(reference);
        }
    }
    let rebound = fresh
        .iter()
        .map(|(old, new)| (old.to_string(), new.to_string()))
        .collect::<BTreeMap<_, _>>();
    for entry in &mut graph.legacy_imports.runs {
        let rows = entry
            .assignments
            .iter_mut()
            .filter(|row| {
                matches!(
                    crate::backup_sql_text(row, "source_kind"),
                    Some("provider_api_key" | "provider_secret_header")
                )
            })
            .map(|row| (row, "destination_id"))
            .chain(
                entry
                    .secret_completions
                    .iter_mut()
                    .map(|row| (row, "destination_ref")),
            );
        for (row, column) in rows {
            if let Some(crate::BackupSqlValue::Text(value)) = row.get_mut(column)
                && let Some(new) = rebound.get(value.as_str())
            {
                value.clone_from(new);
            }
        }
    }
    if fresh.len() != secrets.len() {
        return Err(ProviderBackupGraphError::InvalidSecrets);
    }
    secrets
        .into_iter()
        .map(|mut secret| {
            secret.reference = *fresh
                .get(&secret.reference)
                .ok_or(ProviderBackupGraphError::InvalidSecrets)?;
            Ok(secret)
        })
        .collect()
}

/// Settles generation work that was in flight when the backup was taken as
/// interrupted (user decision 2026-09-14), the way a restore writes it: a turn
/// that never started an attempt is left out, unfinished attempts are
/// interrupted with an interrupted usage event, unfinished turns become
/// interrupted, running tools are interrupted and requested or validated tools
/// are cancelled.
pub fn settle_in_flight_generation(graph: &mut ProviderBackupGraph) {
    use lettuce_conversations::{
        GenerationAttemptStatus, GenerationTurnStatus, ToolExecutionStatus, UsageCounters,
        UsageOutcome, UsageRecord, UsageUnavailableReason,
    };

    let mut dispatched = BTreeMap::<_, Vec<_>>::new();
    for entry in &graph.job_backup.inference {
        dispatched
            .entry(entry.evidence.logical_attempt_id)
            .or_default()
            .push(entry.evidence.id);
    }
    let terminal_turn = |status: GenerationTurnStatus| {
        matches!(
            status,
            GenerationTurnStatus::Succeeded
                | GenerationTurnStatus::Failed
                | GenerationTurnStatus::Cancelled
                | GenerationTurnStatus::Interrupted
        )
    };
    let mut events = Vec::new();
    for conversation in &mut graph.conversation_runtime.conversations {
        conversation
            .turns
            .retain(|entry| terminal_turn(entry.turn.status) || !entry.turn.attempts.is_empty());
        for entry in &mut conversation.turns {
            let turn = &mut entry.turn;
            let updated_at = turn.updated_at;
            for attempt in &mut turn.attempts {
                if matches!(
                    attempt.status,
                    GenerationAttemptStatus::Succeeded
                        | GenerationAttemptStatus::Failed
                        | GenerationAttemptStatus::Cancelled
                        | GenerationAttemptStatus::Interrupted
                ) {
                    continue;
                }
                let started_at = attempt.started_at.unwrap_or(updated_at);
                attempt.status = GenerationAttemptStatus::Interrupted;
                attempt.failure = None;
                attempt.started_at = Some(started_at);
                attempt.finished_at = Some(updated_at.max(started_at));
                if attempt.usage_event_id.is_none() {
                    let id = lettuce_types::UsageEventId::from_uuid(uuid::Uuid::new_v5(
                        &attempt.id.as_uuid(),
                        b"restore-interrupted",
                    ));
                    attempt.usage_event_id = Some(id);
                    let mut overlapping = dispatched.get(&attempt.id).cloned().unwrap_or_default();
                    overlapping.sort();
                    let model = turn.resolved_model.as_ref();
                    events.push(crate::BackupConversationUsage {
                        event: lettuce_usage::UsageEvent {
                            id,
                            record: UsageRecord {
                                turn_id: turn.id,
                                attempt_id: attempt.id,
                                outcome: UsageOutcome::Interrupted,
                                usage: UsageCounters::Unavailable(if overlapping.is_empty() {
                                    UsageUnavailableReason::NotAdmitted
                                } else {
                                    UsageUnavailableReason::TransportFailed
                                }),
                                model_profile_id: model.map(|model| model.source_id),
                                model_revision: model.map(|model| model.source_revision),
                                provider_account_id: model.map(|model| model.provider_account_id),
                                provider_account_revision: model
                                    .map(|model| model.provider_account_revision),
                                recorded_at: started_at,
                            },
                        },
                        cost_basis: None,
                        overlapping_job_inference_ids: overlapping,
                    });
                }
            }
            if !terminal_turn(turn.status) {
                turn.status = GenerationTurnStatus::Interrupted;
                turn.failure = None;
                turn.selected_candidate_id = None;
            }
            for tool in entry
                .attempts
                .iter_mut()
                .flat_map(|runtime| runtime.tools.iter_mut())
            {
                match tool.status {
                    ToolExecutionStatus::Running => {
                        tool.status = ToolExecutionStatus::Interrupted;
                        tool.finished_at = Some(tool.updated_at);
                        tool.revision = lettuce_types::Revision::new(4);
                    }
                    ToolExecutionStatus::Requested | ToolExecutionStatus::Validated => {
                        tool.status = ToolExecutionStatus::Cancelled;
                        tool.finished_at = Some(tool.updated_at);
                        tool.revision = lettuce_types::Revision::new(tool.revision.get() + 1);
                    }
                    _ => {}
                }
            }
        }
    }
    graph.conversation_usage.events.extend(events);
}

pub(crate) fn expected_secrets(
    graph: &ProviderBackupGraph,
) -> Result<BTreeMap<SecretRef, SecretPurpose>, ProviderBackupGraphError> {
    let mut expected = BTreeMap::new();
    for account in &graph.accounts {
        if let Some(reference) = account.api_key_ref {
            insert_secret(
                &mut expected,
                reference,
                SecretPurpose::ProviderApiKey {
                    owner: account.secret_owner_id,
                },
            )?;
        }
        for header in &account.secret_headers {
            insert_secret(
                &mut expected,
                header.secret_ref,
                SecretPurpose::ProviderSecretHeader {
                    owner: account.secret_owner_id,
                    name: header.name.clone(),
                },
            )?;
        }
    }
    for provider in &graph.audio_providers {
        if let Some(reference) = provider.api_key_ref {
            insert_secret(
                &mut expected,
                reference,
                SecretPurpose::AudioApiKey {
                    owner: provider.secret_owner_id,
                },
            )?;
        }
    }
    Ok(expected)
}

fn insert_secret(
    expected: &mut BTreeMap<SecretRef, SecretPurpose>,
    reference: SecretRef,
    purpose: SecretPurpose,
) -> Result<(), ProviderBackupGraphError> {
    if expected.insert(reference, purpose).is_some() {
        Err(ProviderBackupGraphError::InvalidGraph)
    } else {
        Ok(())
    }
}

#[derive(Serialize)]
struct SecretDocument<'a> {
    version: u32,
    secrets: Vec<&'a ProviderBackupSecret>,
}

impl Serialize for ProviderBackupSecret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ProviderBackupSecret", 4)?;
        state.serialize_field("reference", &self.reference)?;
        state.serialize_field("purpose", &self.purpose)?;
        state.serialize_field("generation", &self.generation)?;
        self.value
            .with(|value| state.serialize_field("value", value))?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_models::{ProviderConfig, ProviderProtocol};
    use lettuce_settings::SecretOwnerId;
    use lettuce_types::{
        ContentHash, MediaBlobId, OperationId, ProviderAccountId, Revision, TimestampMillis,
    };

    fn graph(reference: SecretRef) -> ProviderBackupGraph {
        ProviderBackupGraph {
            creation: CreationBackup::default(),
            legacy_imports: crate::LegacyImportBackup::default(),
            playground_history: crate::PlaygroundHistoryBackup::default(),
            version: PROVIDER_BACKUP_GRAPH_VERSION,
            accounts: vec![ProviderAccount {
                id: ProviderAccountId::new(),
                secret_owner_id: SecretOwnerId::new(),
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
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            }],
            profiles: Vec::new(),
            prompts: Vec::new(),
            selections: ProviderBackupSelections {
                default_model_profile_id: None,
                dynamic_memory_model_profile_id: None,
                group_speaker_model_profile_id: None,
                default_prompt_document_id: None,
            },
            settings: BackupGlobalSettings {
                value: GlobalSettings::default(),
                model_settings: Default::default(),
                revision: Revision::new(1),
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            audio_providers: Vec::new(),
            user_voices: Vec::new(),
            authored: AuthoredProfileBackup {
                personas: Vec::new(),
                persona_default: PersonaDefaultState {
                    persona_id: None,
                    revision: Revision::new(1),
                    created_at: TimestampMillis::new(1),
                    updated_at: TimestampMillis::new(1),
                },
                lorebooks: Vec::new(),
                characters: Vec::new(),
                groups: Vec::new(),
                character_lorebooks: Vec::new(),
                persona_lorebooks: Vec::new(),
                group_lorebooks: Vec::new(),
                media_assets: Vec::new(),
                media_blobs: Vec::new(),
            },
            asr_learning: crate::AsrLearningDocument {
                version: crate::ASR_LEARNING_DOCUMENT_VERSION,
                vocabulary: Vec::new(),
                corrections: Vec::new(),
                ignored_suggestions: Vec::new(),
                voice_examples: Vec::new(),
                audio_assets: Vec::new(),
            },
            conversation_history: crate::ConversationHistoryBackup {
                version: crate::CONVERSATION_HISTORY_BACKUP_VERSION,
                conversations: Vec::new(),
            },
            conversation_runtime: crate::ConversationRuntimeBackup {
                version: crate::CONVERSATION_RUNTIME_BACKUP_VERSION,
                conversations: Vec::new(),
            },
            job_backup: crate::JobBackup {
                version: crate::JOB_BACKUP_VERSION,
                jobs: Vec::new(),
                inference: Vec::new(),
                speech_transcriptions: Vec::new(),
                speech_syntheses: Vec::new(),
                image_generations: Vec::new(),
            },
            conversation_usage: crate::ConversationUsageBackup {
                version: crate::CONVERSATION_USAGE_BACKUP_VERSION,
                events: Vec::new(),
            },
            conversation_outbox: crate::ConversationOutboxBackup {
                version: crate::CONVERSATION_OUTBOX_BACKUP_VERSION,
                conversations: Vec::new(),
            },
            companion_state: crate::CompanionStateBackup {
                souls: Vec::new(),
                scheduled_notes: Vec::new(),
                growth_runs: Vec::new(),
                consolidation_runs: Vec::new(),
                soul_writer_runs: Vec::new(),
                version: crate::COMPANION_STATE_BACKUP_VERSION,
                relationships: Vec::new(),
                sessions: Vec::new(),
                episodes: Vec::new(),
                receipts: Vec::new(),
            },
            companion_effects: crate::CompanionEffectBackup {
                version: crate::COMPANION_EFFECT_BACKUP_VERSION,
                effects: Vec::new(),
                rewinds: Vec::new(),
            },
            memory: crate::MemoryBackup {
                pools: Vec::new(),
                version: crate::MEMORY_BACKUP_VERSION,
                spaces: Vec::new(),
                retrieval_accesses: Vec::new(),
            },
            memory_projections: crate::MemoryProjectionBackup {
                version: crate::MEMORY_PROJECTION_BACKUP_VERSION,
                projections: Vec::new(),
            },
            dynamic_memory: crate::DynamicMemoryBackup {
                version: crate::DYNAMIC_MEMORY_BACKUP_VERSION,
                pending_approvals: Vec::new(),
                runs: Vec::new(),
            },
        }
    }

    #[test]
    fn provider_sections_require_the_exact_secret_inventory_and_redact_debug() {
        let reference = SecretRef::new();
        let source_graph = graph(reference);
        let purpose = SecretPurpose::ProviderApiKey {
            owner: source_graph.accounts[0].secret_owner_id,
        };
        assert_eq!(
            provider_backup_sections(source_graph.clone(), Vec::new(), Vec::new(), Vec::new()),
            Err(ProviderBackupGraphError::InvalidSecrets)
        );
        let secret = ProviderBackupSecret {
            reference,
            purpose,
            generation: 1,
            value: SecretValue::new("backup-secret-canary").expect("secret"),
        };
        assert!(!format!("{secret:?}").contains("backup-secret-canary"));
        let sections = provider_backup_sections(source_graph, vec![secret], Vec::new(), Vec::new())
            .expect("sections");
        assert_eq!(sections.len(), 13);
        assert!(
            sections[1]
                .bytes
                .windows("backup-secret-canary".len())
                .any(|window| window == b"backup-secret-canary")
        );
        assert!(
            !sections[0]
                .bytes
                .windows("backup-secret-canary".len())
                .any(|window| window == b"backup-secret-canary")
        );

        let orphan = ProviderBackupSecret {
            reference: SecretRef::new(),
            purpose: SecretPurpose::ProviderApiKey {
                owner: SecretOwnerId::new(),
            },
            generation: 1,
            value: SecretValue::new("orphan-secret").expect("secret"),
        };
        assert_eq!(
            provider_backup_sections(graph(reference), vec![orphan], Vec::new(), Vec::new()),
            Err(ProviderBackupGraphError::InvalidSecrets)
        );

        let mut duplicate_graph = graph(reference);
        let mut duplicate_account = duplicate_graph.accounts[0].clone();
        duplicate_account.id = ProviderAccountId::new();
        duplicate_account.secret_owner_id = SecretOwnerId::new();
        duplicate_graph.accounts.push(duplicate_account);
        assert_eq!(
            provider_backup_secret_requirements(&duplicate_graph),
            Err(ProviderBackupGraphError::InvalidGraph)
        );
    }

    #[test]
    fn restore_plan_decodes_the_complete_validated_inventory() {
        let reference = SecretRef::new();
        let source_graph = graph(reference);
        let purpose = SecretPurpose::ProviderApiKey {
            owner: source_graph.accounts[0].secret_owner_id,
        };
        let sections = provider_backup_sections(
            source_graph.clone(),
            vec![ProviderBackupSecret {
                reference,
                purpose: purpose.clone(),
                generation: 7,
                value: SecretValue::new("restore-secret-canary").expect("secret"),
            }],
            Vec::new(),
            Vec::new(),
        )
        .expect("sections");
        let envelope = crate::seal_backup(
            "restore-test",
            TimestampMillis::new(10),
            "backup password",
            sections,
        )
        .expect("sealed backup");

        let plan = crate::decode_provider_backup_restore_plan(
            std::io::Cursor::new(envelope.clone()),
            "backup password",
        )
        .expect("restore plan");
        assert_eq!(plan.graph, source_graph);
        assert_eq!(plan.secrets.len(), 1);
        assert_eq!(plan.secrets[0].reference, reference);
        assert_eq!(plan.secrets[0].purpose, purpose);
        assert_eq!(plan.secrets[0].generation, 7);
        assert!(
            plan.secrets[0]
                .value
                .with(|value| value == "restore-secret-canary")
        );
        assert!(plan.media.is_empty());
        assert!(plan.artifacts.is_empty());
    }

    #[test]
    fn restore_plan_rejects_missing_and_unknown_sections() {
        let reference = SecretRef::new();
        let source_graph = graph(reference);
        let purpose = SecretPurpose::ProviderApiKey {
            owner: source_graph.accounts[0].secret_owner_id,
        };
        let make_sections = || {
            provider_backup_sections(
                source_graph.clone(),
                vec![ProviderBackupSecret {
                    reference,
                    purpose: purpose.clone(),
                    generation: 1,
                    value: SecretValue::new("secret").expect("secret"),
                }],
                Vec::new(),
                Vec::new(),
            )
            .expect("sections")
        };
        let mut missing = make_sections();
        missing.retain(|section| section.name != "data/memory.json");
        let missing_envelope = crate::seal_backup(
            "restore-test",
            TimestampMillis::new(10),
            "backup password",
            missing,
        )
        .expect("sealed backup");
        assert!(matches!(
            crate::decode_provider_backup_restore_plan(
                std::io::Cursor::new(missing_envelope.clone()),
                "backup password"
            ),
            Err(crate::ProviderBackupRestorePlanError::InvalidInventory)
        ));

        let mut unknown = make_sections();
        unknown.push(BackupSection::new(
            "data/unknown.json",
            "unknown.v1",
            b"{}".to_vec(),
        ));
        let unknown_envelope = crate::seal_backup(
            "restore-test",
            TimestampMillis::new(10),
            "backup password",
            unknown,
        )
        .expect("sealed backup");
        assert!(matches!(
            crate::decode_provider_backup_restore_plan(
                std::io::Cursor::new(unknown_envelope.clone()),
                "backup password"
            ),
            Err(crate::ProviderBackupRestorePlanError::InvalidInventory)
        ));
    }

    #[test]
    fn restore_plan_routes_unversioned_zip_backups_to_legacy_compatibility() {
        assert!(matches!(
            crate::decode_provider_backup_restore_plan(
                std::io::Cursor::new(b"PK\x03\x04legacy".to_vec()),
                "backup password"
            ),
            Err(crate::ProviderBackupRestorePlanError::LegacyRequiresCompatibility)
        ));
    }

    #[test]
    fn authored_roots_require_complete_bindings_and_valid_defaults() {
        let reference = SecretRef::new();
        let mut source_graph = graph(reference);
        let persona = Persona::new(
            PersonaId::new(),
            "Traveler".into(),
            "A complete authored persona".into(),
            TimestampMillis::new(2),
        )
        .expect("persona");
        source_graph.authored.personas.push(persona.clone());
        source_graph.authored.persona_default.persona_id = Some(persona.id);
        assert_eq!(
            provider_backup_secret_requirements(&source_graph),
            Err(ProviderBackupGraphError::InvalidGraph)
        );

        source_graph
            .authored
            .persona_lorebooks
            .push(BackupLorebookBindings {
                owner_id: persona.id,
                bindings: Vec::new(),
            });
        assert_eq!(
            provider_backup_secret_requirements(&source_graph),
            Ok(vec![(
                reference,
                SecretPurpose::ProviderApiKey {
                    owner: source_graph.accounts[0].secret_owner_id,
                },
            )])
        );

        source_graph.authored.persona_default.persona_id = Some(PersonaId::new());
        assert_eq!(
            provider_backup_secret_requirements(&source_graph),
            Err(ProviderBackupGraphError::InvalidGraph)
        );
    }

    #[test]
    fn ready_media_is_stored_once_and_must_match_its_snapshot() {
        let reference = SecretRef::new();
        let mut source_graph = graph(reference);
        let bytes = b"verified image bytes".to_vec();
        let content_hash =
            ContentHash::parse(blake3::hash(&bytes).to_hex().to_string()).expect("content hash");
        source_graph.authored.media_blobs.push(MediaBlob {
            id: MediaBlobId::new(),
            content_hash: content_hash.clone(),
            kind: lettuce_media::MediaKind::Image,
            mime_type: "image/png".into(),
            byte_size: u64::try_from(bytes.len()).expect("byte size"),
            width: Some(1),
            height: Some(1),
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Ready,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        });
        let purpose = SecretPurpose::ProviderApiKey {
            owner: source_graph.accounts[0].secret_owner_id,
        };
        assert_eq!(
            provider_backup_sections(
                source_graph.clone(),
                vec![ProviderBackupSecret {
                    reference,
                    purpose: purpose.clone(),
                    generation: 1,
                    value: SecretValue::new("secret").expect("secret"),
                }],
                vec![BackupMediaObject {
                    content_hash: content_hash.clone(),
                    bytes: zeroize::Zeroizing::new(b"changed bytes".to_vec()),
                }],
                Vec::new(),
            ),
            Err(ProviderBackupGraphError::InvalidGraph)
        );
        let sections = provider_backup_sections(
            source_graph,
            vec![ProviderBackupSecret {
                reference,
                purpose,
                generation: 1,
                value: SecretValue::new("secret").expect("secret"),
            }],
            vec![BackupMediaObject {
                content_hash: content_hash.clone(),
                bytes: zeroize::Zeroizing::new(bytes.clone()),
            }],
            Vec::new(),
        )
        .expect("media sections");
        assert_eq!(sections.len(), 14);
        assert_eq!(&*sections[13].bytes, &bytes);

        let envelope = crate::seal_backup(
            "restore-test",
            TimestampMillis::new(10),
            "backup password",
            sections,
        )
        .expect("sealed backup");
        let plan = crate::decode_provider_backup_restore_plan(
            std::io::Cursor::new(envelope.clone()),
            "backup password",
        )
        .expect("restore plan");
        let root =
            std::env::temp_dir().join(format!("lettuce-restore-workspace-{}", OperationId::new()));
        let partial = root
            .join("partial")
            .join("media")
            .join(format!("{content_hash}.partial"));
        std::fs::create_dir_all(partial.parent().expect("partial parent"))
            .expect("partial directory");
        std::fs::write(&partial, &bytes[..5]).expect("partial bytes");
        let workspace = crate::BackupRestoreWorkspace::open(&root).expect("restore workspace");
        let receipt = workspace.stage(&plan).expect("stage restore plan");
        assert_eq!(receipt.source_hash, plan.source_hash);
        assert_eq!(receipt.media, vec![content_hash.clone()]);
        assert!(receipt.artifacts.is_empty());
        assert_eq!(
            std::fs::read(root.join("media").join("blobs").join(content_hash.as_str()))
                .expect("staged media"),
            bytes
        );
        assert_eq!(workspace.stage(&plan).expect("replay staging"), receipt);
        assert_eq!(
            &*workspace
                .read_staged_media(&content_hash)
                .expect("staged media"),
            &bytes
        );

        let manifest_len = u32::from_be_bytes(
            envelope[envelope.len() - 12..envelope.len() - 8]
                .try_into()
                .expect("footer length"),
        ) as usize;
        let mut tampered = envelope.clone();
        let media_end = tampered.len() - 36 - manifest_len;
        tampered[media_end - 1] ^= 1;
        let tampered_plan = crate::decode_provider_backup_restore_plan(
            std::io::Cursor::new(tampered),
            "backup password",
        )
        .expect("data sections still decode");
        assert!(matches!(
            tampered_plan.read_media(&tampered_plan.media[0]),
            Err(crate::ProviderBackupRestorePlanError::Envelope(
                crate::BackupEnvelopeError::Authentication
            ))
        ));
        let tampered_root = std::env::temp_dir().join(format!(
            "lettuce-restore-workspace-tampered-{}",
            OperationId::new()
        ));
        assert_eq!(
            crate::BackupRestoreWorkspace::open(&tampered_root)
                .expect("tampered workspace")
                .stage(&tampered_plan),
            Err(crate::BackupRestoreWorkspaceError::Source)
        );
        std::fs::remove_dir_all(tampered_root).expect("remove tampered workspace");

        std::fs::write(
            root.join("media").join("blobs").join(content_hash.as_str()),
            b"conflicting bytes",
        )
        .expect("corrupt staged media");
        assert_eq!(
            workspace.stage(&plan),
            Err(crate::BackupRestoreWorkspaceError::Conflict)
        );
        std::fs::remove_dir_all(root).expect("remove restore workspace");

        let conflict_root = std::env::temp_dir().join(format!(
            "lettuce-restore-workspace-conflict-{}",
            OperationId::new()
        ));
        let conflict_partial = conflict_root
            .join("partial")
            .join("media")
            .join(format!("{content_hash}.partial"));
        std::fs::create_dir_all(conflict_partial.parent().expect("partial parent"))
            .expect("partial directory");
        std::fs::write(&conflict_partial, b"wrong").expect("conflicting partial bytes");
        let conflict_workspace =
            crate::BackupRestoreWorkspace::open(&conflict_root).expect("conflict workspace");
        assert_eq!(
            conflict_workspace.stage(&plan),
            Err(crate::BackupRestoreWorkspaceError::Conflict)
        );
        std::fs::remove_dir_all(conflict_root).expect("remove conflict workspace");
    }
}
