use std::collections::BTreeMap;

use lettuce_context::PromptDocument;
use lettuce_models::{
    ModelKind, ModelProfile, ModelProfileConfig, ProviderAccount, ProviderConfig, ProviderProtocol,
    SecretHeader,
};
use lettuce_settings::{SecretOwnerId, SecretPurpose, SecretRef, SecretValue};
use lettuce_speech::{AudioProvider, UserVoice};
use lettuce_types::{ModelProfileId, ProviderAccountId, Revision, TimestampMillis};
use serde::Deserialize;

use crate::{
    AuthoredProfileBackup, BackupConversationArtifact, BackupGlobalSettings, BackupMediaObject,
    BackupSection, PROVIDER_BACKUP_GRAPH_VERSION, ProviderBackupGraph, ProviderBackupGraphError,
    ProviderBackupSecret, ProviderBackupSelections,
};

#[derive(Debug)]
pub struct ProviderBackupRestorePlan {
    pub source_hash: lettuce_types::ContentHash,
    pub graph: ProviderBackupGraph,
    pub secrets: Vec<ProviderBackupSecret>,
    pub media: Vec<BackupMediaObject>,
    pub artifacts: Vec<BackupConversationArtifact>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderBackupRestorePlanError {
    #[error("backup envelope could not be opened: {0}")]
    Envelope(#[from] crate::BackupEnvelopeError),
    #[error("legacy backup requires the version-1 compatibility decoder")]
    LegacyRequiresCompatibility,
    #[error("backup restore inventory is invalid")]
    InvalidInventory,
    #[error("backup restore graph is invalid: {0}")]
    InvalidGraph(#[from] ProviderBackupGraphError),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderMetadata {
    version: u32,
    accounts: Vec<ProviderAccountDocument>,
    profiles: Vec<ModelProfileDocument>,
    prompts: Vec<PromptDocument>,
    selections: ProviderBackupSelections,
    settings: BackupGlobalSettings,
    audio_providers: Vec<AudioProvider>,
    user_voices: Vec<UserVoice>,
    authored: AuthoredProfileBackup,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderAccountDocument {
    id: ProviderAccountId,
    secret_owner_id: SecretOwnerId,
    provider_kind: String,
    protocol: ProviderProtocol,
    label: String,
    endpoint: Option<String>,
    enabled: bool,
    streaming_enabled: bool,
    allow_invalid_tls: bool,
    api_key_ref: Option<SecretRef>,
    secret_headers: Vec<SecretHeader>,
    config: ProviderConfig,
    revision: Revision,
    created_at: TimestampMillis,
    updated_at: TimestampMillis,
}

impl From<ProviderAccountDocument> for ProviderAccount {
    fn from(value: ProviderAccountDocument) -> Self {
        Self {
            id: value.id,
            secret_owner_id: value.secret_owner_id,
            provider_kind: value.provider_kind,
            protocol: value.protocol,
            label: value.label,
            endpoint: value.endpoint,
            enabled: value.enabled,
            streaming_enabled: value.streaming_enabled,
            allow_invalid_tls: value.allow_invalid_tls,
            api_key_ref: value.api_key_ref,
            secret_headers: value.secret_headers,
            config: value.config,
            revision: value.revision,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelProfileDocument {
    id: ModelProfileId,
    provider_account_id: ProviderAccountId,
    external_model_id: String,
    display_name: String,
    kind: ModelKind,
    config: ModelProfileConfig,
    revision: Revision,
    created_at: TimestampMillis,
    updated_at: TimestampMillis,
}

impl From<ModelProfileDocument> for ModelProfile {
    fn from(value: ModelProfileDocument) -> Self {
        Self {
            id: value.id,
            provider_account_id: value.provider_account_id,
            external_model_id: value.external_model_id,
            display_name: value.display_name,
            kind: value.kind,
            config: value.config,
            revision: value.revision,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretDocument {
    version: u32,
    secrets: Vec<DecodedSecret>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecodedSecret {
    reference: SecretRef,
    purpose: SecretPurpose,
    generation: u64,
    value: String,
}

pub fn decode_provider_backup_restore_plan(
    bytes: &[u8],
    password: &str,
) -> Result<ProviderBackupRestorePlan, ProviderBackupRestorePlanError> {
    match crate::detect_backup_format(bytes)? {
        crate::BackupFormatVersion::LegacyV1 => {
            return Err(ProviderBackupRestorePlanError::LegacyRequiresCompatibility);
        }
        crate::BackupFormatVersion::CurrentV2 => {}
    }
    let mut sections = crate::open_backup(bytes, password)?
        .into_iter()
        .map(|section| (section.name.clone(), section))
        .collect::<BTreeMap<_, _>>();
    let metadata: ProviderMetadata = take_json(
        &mut sections,
        "data/provider-graph.json",
        "provider-graph.v2",
    )?;
    let secret_document: SecretDocument = take_json(
        &mut sections,
        "secrets/provider-secrets.json",
        "provider-secrets.v2",
    )?;
    let asr_learning = take_json(&mut sections, "data/asr-learning.json", "asr-learning.v3")?;
    let conversation_history = take_json(
        &mut sections,
        "data/conversation-history.json",
        "conversation-history.v1",
    )?;
    let conversation_runtime = take_json(
        &mut sections,
        "data/conversation-runtime.json",
        "conversation-runtime.v1",
    )?;
    let job_backup = take_json(&mut sections, "data/jobs.json", "jobs.v1")?;
    let conversation_usage = take_json(
        &mut sections,
        "data/conversation-usage.json",
        "conversation-usage.v1",
    )?;
    let conversation_outbox = take_json(
        &mut sections,
        "data/conversation-outbox.json",
        "conversation-outbox.v1",
    )?;
    let companion_state = take_json(
        &mut sections,
        "data/companion-state.json",
        "companion-state.v1",
    )?;
    let companion_effects = take_json(
        &mut sections,
        "data/companion-effects.json",
        "companion-effects.v1",
    )?;
    let memory = take_json(&mut sections, "data/memory.json", "memory.v1")?;
    let memory_projections = take_json(
        &mut sections,
        "data/memory-projections.json",
        "memory-projections.v1",
    )?;
    let dynamic_memory = take_json(
        &mut sections,
        "data/dynamic-memory.json",
        "dynamic-memory.v1",
    )?;
    let mut graph = ProviderBackupGraph {
        version: metadata.version,
        accounts: metadata.accounts.into_iter().map(Into::into).collect(),
        profiles: metadata.profiles.into_iter().map(Into::into).collect(),
        prompts: metadata.prompts,
        selections: metadata.selections,
        settings: metadata.settings,
        audio_providers: metadata.audio_providers,
        user_voices: metadata.user_voices,
        authored: metadata.authored,
        asr_learning,
        conversation_history,
        conversation_runtime,
        job_backup,
        conversation_usage,
        conversation_outbox,
        companion_state,
        companion_effects,
        memory,
        memory_projections,
        dynamic_memory,
    };
    crate::backup_graph::canonicalize_and_validate(&mut graph)?;
    let secrets = decode_secrets(secret_document, &graph)?;
    let media = take_media(&mut sections, &graph)?;
    let artifacts = take_artifacts(&mut sections, &graph)?;
    if !sections.is_empty() {
        return Err(ProviderBackupRestorePlanError::InvalidInventory);
    }
    Ok(ProviderBackupRestorePlan {
        source_hash: content_hash(bytes),
        graph,
        secrets,
        media,
        artifacts,
    })
}

fn content_hash(bytes: &[u8]) -> lettuce_types::ContentHash {
    lettuce_types::ContentHash::parse(blake3::hash(bytes).to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash")
}

fn take_json<T: for<'de> Deserialize<'de>>(
    sections: &mut BTreeMap<String, BackupSection>,
    name: &str,
    schema: &str,
) -> Result<T, ProviderBackupRestorePlanError> {
    let section = sections
        .remove(name)
        .ok_or(ProviderBackupRestorePlanError::InvalidInventory)?;
    if section.schema != schema {
        return Err(ProviderBackupRestorePlanError::InvalidInventory);
    }
    serde_json::from_slice(&section.bytes)
        .map_err(|_| ProviderBackupRestorePlanError::InvalidInventory)
}

fn decode_secrets(
    document: SecretDocument,
    graph: &ProviderBackupGraph,
) -> Result<Vec<ProviderBackupSecret>, ProviderBackupRestorePlanError> {
    if document.version != PROVIDER_BACKUP_GRAPH_VERSION {
        return Err(ProviderBackupRestorePlanError::InvalidInventory);
    }
    let expected = crate::backup_graph::expected_secrets(graph)?;
    if document.secrets.len() != expected.len() {
        return Err(ProviderBackupRestorePlanError::InvalidInventory);
    }
    let mut seen = BTreeMap::new();
    let mut secrets = Vec::with_capacity(document.secrets.len());
    for secret in document.secrets {
        if secret.generation == 0
            || expected.get(&secret.reference) != Some(&secret.purpose)
            || seen.insert(secret.reference, ()).is_some()
        {
            return Err(ProviderBackupRestorePlanError::InvalidInventory);
        }
        secrets.push(ProviderBackupSecret {
            reference: secret.reference,
            purpose: secret.purpose,
            generation: secret.generation,
            value: SecretValue::new(secret.value)
                .map_err(|_| ProviderBackupRestorePlanError::InvalidInventory)?,
        });
    }
    secrets.sort_by_key(|secret| secret.reference);
    Ok(secrets)
}

fn take_media(
    sections: &mut BTreeMap<String, BackupSection>,
    graph: &ProviderBackupGraph,
) -> Result<Vec<BackupMediaObject>, ProviderBackupRestorePlanError> {
    graph
        .authored
        .media_blobs
        .iter()
        .filter(|blob| blob.state == lettuce_media::BlobState::Ready)
        .map(|blob| {
            let name = format!("media/blobs/{}", blob.content_hash);
            let section = sections
                .remove(&name)
                .ok_or(ProviderBackupRestorePlanError::InvalidInventory)?;
            if section.schema != "media-blob.v2"
                || section.bytes.len()
                    != usize::try_from(blob.byte_size)
                        .map_err(|_| ProviderBackupRestorePlanError::InvalidInventory)?
                || lettuce_types::ContentHash::parse(
                    blake3::hash(&section.bytes).to_hex().to_string(),
                )
                .as_ref()
                    != Ok(&blob.content_hash)
            {
                return Err(ProviderBackupRestorePlanError::InvalidInventory);
            }
            Ok(BackupMediaObject {
                content_hash: blob.content_hash.clone(),
                bytes: section.bytes,
            })
        })
        .collect()
}

fn take_artifacts(
    sections: &mut BTreeMap<String, BackupSection>,
    graph: &ProviderBackupGraph,
) -> Result<Vec<BackupConversationArtifact>, ProviderBackupRestorePlanError> {
    crate::backup_graph::all_conversation_artifact_descriptors(graph)?
        .into_iter()
        .map(|descriptor| {
            let (name, schema, digest, byte_size) = match &descriptor {
                lettuce_conversations::TrustedArtifactDescriptor::Snapshot(reference) => (
                    format!("conversation/snapshots/{}", reference.artifact_id),
                    format!("conversation-snapshot.v{}", reference.schema_version),
                    &reference.digest,
                    reference.byte_size,
                ),
                lettuce_conversations::TrustedArtifactDescriptor::Replay(reference) => (
                    format!("conversation/replays/{}", reference.artifact_id),
                    format!("conversation-replay.v{}", reference.schema_version),
                    &reference.digest,
                    reference.byte_size,
                ),
            };
            let section = sections
                .remove(&name)
                .ok_or(ProviderBackupRestorePlanError::InvalidInventory)?;
            if section.schema != schema
                || section.bytes.len()
                    != usize::try_from(byte_size)
                        .map_err(|_| ProviderBackupRestorePlanError::InvalidInventory)?
                || lettuce_types::ContentHash::parse(
                    blake3::hash(&section.bytes).to_hex().to_string(),
                )
                .as_ref()
                    != Ok(digest)
            {
                return Err(ProviderBackupRestorePlanError::InvalidInventory);
            }
            Ok(BackupConversationArtifact {
                descriptor,
                bytes: section.bytes,
            })
        })
        .collect()
}
