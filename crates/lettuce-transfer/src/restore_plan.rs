use std::{
    collections::BTreeMap,
    io::{Read, Seek, SeekFrom},
    sync::Mutex,
};

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
    AuthoredProfileBackup, BackupConversationArtifact, BackupGlobalSettings, BackupReader,
    BackupSectionInfo, PROVIDER_BACKUP_GRAPH_VERSION, ProviderBackupGraph,
    ProviderBackupGraphError, ProviderBackupSecret, ProviderBackupSelections,
};

/// A seekable backup file or buffer.
pub trait BackupSource: Read + Seek + Send {}

impl<T: Read + Seek + Send> BackupSource for T {}

/// A ready media blob of the backup, read on demand through
/// [`ProviderBackupRestorePlan::read_media`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupMediaEntry {
    pub content_hash: lettuce_types::ContentHash,
    pub byte_size: u64,
    section: usize,
}

/// A decoded version-2 backup. Data sections, secrets and conversation
/// artifacts are in memory; media blobs stay in the authenticated source and
/// are read one at a time.
pub struct ProviderBackupRestorePlan {
    pub source_hash: lettuce_types::ContentHash,
    pub graph: ProviderBackupGraph,
    pub secrets: Vec<ProviderBackupSecret>,
    pub media: Vec<BackupMediaEntry>,
    pub artifacts: Vec<BackupConversationArtifact>,
    source: Mutex<BackupReader<Box<dyn BackupSource>>>,
}

impl std::fmt::Debug for ProviderBackupRestorePlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderBackupRestorePlan")
            .field("source_hash", &self.source_hash)
            .field("media", &self.media)
            .finish_non_exhaustive()
    }
}

impl ProviderBackupRestorePlan {
    /// Decrypts one media blob and checks it against the graph's size and
    /// content hash.
    pub fn read_media(
        &self,
        entry: &BackupMediaEntry,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, ProviderBackupRestorePlanError> {
        let bytes = self
            .source
            .lock()
            .map_err(|_| ProviderBackupRestorePlanError::InvalidInventory)?
            .read_section(entry.section)?;
        if u64::try_from(bytes.len()).ok() != Some(entry.byte_size)
            || content_hash(&bytes) != entry.content_hash
        {
            return Err(ProviderBackupRestorePlanError::InvalidInventory);
        }
        Ok(bytes)
    }
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
    #[error("backup source could not be read")]
    Io,
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
    #[serde(default)]
    creation: crate::CreationBackup,
    #[serde(default)]
    legacy_imports: crate::LegacyImportBackup,
    #[serde(default)]
    playground_history: crate::PlaygroundHistoryBackup,
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
    mut input: impl BackupSource + 'static,
    password: &str,
) -> Result<ProviderBackupRestorePlan, ProviderBackupRestorePlanError> {
    let mut prefix = [0u8; 16];
    let mut read = 0;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|_| ProviderBackupRestorePlanError::Io)?;
    while read < prefix.len() {
        match input.read(&mut prefix[read..]) {
            Ok(0) => break,
            Ok(count) => read += count,
            Err(_) => return Err(ProviderBackupRestorePlanError::Io),
        }
    }
    match crate::detect_backup_format(&prefix[..read])? {
        crate::BackupFormatVersion::LegacyV1 => {
            return Err(ProviderBackupRestorePlanError::LegacyRequiresCompatibility);
        }
        crate::BackupFormatVersion::CurrentV2 => {}
    }
    input
        .seek(SeekFrom::Start(0))
        .map_err(|_| ProviderBackupRestorePlanError::Io)?;
    let mut hasher = blake3::Hasher::new();
    hasher
        .update_reader(&mut input)
        .map_err(|_| ProviderBackupRestorePlanError::Io)?;
    let source_hash = lettuce_types::ContentHash::parse(hasher.finalize().to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash");
    let mut reader = BackupReader::open(Box::new(input) as Box<dyn BackupSource>, password)?;
    let mut sections = reader
        .sections()
        .into_iter()
        .enumerate()
        .map(|(index, section)| (section.name.clone(), (index, section)))
        .collect::<BTreeMap<_, _>>();
    let sections = &mut Sections {
        reader: &mut reader,
        sections: &mut sections,
    };
    let metadata: ProviderMetadata =
        take_json(sections, "data/provider-graph.json", "provider-graph.v2")?;
    let secret_document: SecretDocument = take_json(
        sections,
        "secrets/provider-secrets.json",
        "provider-secrets.v2",
    )?;
    let asr_learning = take_json(sections, "data/asr-learning.json", "asr-learning.v3")?;
    let conversation_history = take_json(
        sections,
        "data/conversation-history.json",
        "conversation-history.v1",
    )?;
    let conversation_runtime = take_json(
        sections,
        "data/conversation-runtime.json",
        "conversation-runtime.v1",
    )?;
    let job_backup = take_json(sections, "data/jobs.json", "jobs.v1")?;
    let conversation_usage = take_json(
        sections,
        "data/conversation-usage.json",
        "conversation-usage.v1",
    )?;
    let conversation_outbox = take_json(
        sections,
        "data/conversation-outbox.json",
        "conversation-outbox.v1",
    )?;
    let companion_state = take_json(sections, "data/companion-state.json", "companion-state.v1")?;
    let companion_effects = take_json(
        sections,
        "data/companion-effects.json",
        "companion-effects.v1",
    )?;
    let memory = take_json(sections, "data/memory.json", "memory.v1")?;
    let memory_projections = take_json(
        sections,
        "data/memory-projections.json",
        "memory-projections.v1",
    )?;
    let dynamic_memory = take_json(sections, "data/dynamic-memory.json", "dynamic-memory.v1")?;
    let mut graph = ProviderBackupGraph {
        creation: metadata.creation,
        legacy_imports: metadata.legacy_imports,
        playground_history: metadata.playground_history,
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
    let media = take_media(sections, &graph)?;
    let artifacts = take_artifacts(sections, &graph)?;
    if !sections.sections.is_empty() {
        return Err(ProviderBackupRestorePlanError::InvalidInventory);
    }
    Ok(ProviderBackupRestorePlan {
        source_hash,
        graph,
        secrets,
        media,
        artifacts,
        source: Mutex::new(reader),
    })
}

struct Sections<'a> {
    reader: &'a mut BackupReader<Box<dyn BackupSource>>,
    sections: &'a mut BTreeMap<String, (usize, BackupSectionInfo)>,
}

impl Sections<'_> {
    fn take(
        &mut self,
        name: &str,
        schema: &str,
    ) -> Result<(usize, BackupSectionInfo), ProviderBackupRestorePlanError> {
        let (index, section) = self
            .sections
            .remove(name)
            .ok_or(ProviderBackupRestorePlanError::InvalidInventory)?;
        if section.schema != schema {
            return Err(ProviderBackupRestorePlanError::InvalidInventory);
        }
        Ok((index, section))
    }

    fn read(
        &mut self,
        name: &str,
        schema: &str,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, ProviderBackupRestorePlanError> {
        let (index, _) = self.take(name, schema)?;
        Ok(self.reader.read_section(index)?)
    }
}

fn content_hash(bytes: &[u8]) -> lettuce_types::ContentHash {
    lettuce_types::ContentHash::parse(blake3::hash(bytes).to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash")
}

fn take_json<T: for<'de> Deserialize<'de>>(
    sections: &mut Sections<'_>,
    name: &str,
    schema: &str,
) -> Result<T, ProviderBackupRestorePlanError> {
    serde_json::from_slice(&sections.read(name, schema)?)
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
    sections: &mut Sections<'_>,
    graph: &ProviderBackupGraph,
) -> Result<Vec<BackupMediaEntry>, ProviderBackupRestorePlanError> {
    graph
        .authored
        .media_blobs
        .iter()
        .filter(|blob| blob.state == lettuce_media::BlobState::Ready)
        .map(|blob| {
            let (section, info) = sections.take(
                &crate::backup_media_section_name(&blob.content_hash),
                crate::BACKUP_MEDIA_SECTION_SCHEMA,
            )?;
            if info.plaintext_bytes != blob.byte_size || info.content_hash != blob.content_hash {
                return Err(ProviderBackupRestorePlanError::InvalidInventory);
            }
            Ok(BackupMediaEntry {
                content_hash: blob.content_hash.clone(),
                byte_size: blob.byte_size,
                section,
            })
        })
        .collect()
}

fn take_artifacts(
    sections: &mut Sections<'_>,
    graph: &ProviderBackupGraph,
) -> Result<Vec<BackupConversationArtifact>, ProviderBackupRestorePlanError> {
    crate::backup_graph::all_conversation_artifact_descriptors(graph)?
        .into_iter()
        .map(|descriptor| {
            let (name, schema, digest, byte_size) =
                crate::backup_graph::backup_artifact_section_identity(&descriptor);
            let bytes = sections.read(&name, &schema)?;
            if u64::try_from(bytes.len()).ok() != Some(byte_size) || content_hash(&bytes) != *digest
            {
                return Err(ProviderBackupRestorePlanError::InvalidInventory);
            }
            Ok(BackupConversationArtifact { descriptor, bytes })
        })
        .collect()
}
