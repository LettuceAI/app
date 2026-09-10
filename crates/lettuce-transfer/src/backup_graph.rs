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
use lettuce_media::{BlobState, MAX_MEDIA_BLOB_BYTES, MediaAsset, MediaBlob};
use lettuce_models::{ModelProfile, ProviderAccount, validate_provider_connection};
use lettuce_settings::{GlobalSettings, SecretPurpose, SecretRef, SecretValue};
use lettuce_speech::{AudioProvider, UserVoice};
use lettuce_types::{
    CharacterId, GroupId, LorebookId, ModelProfileId, PersonaId, PromptDocumentId, Revision,
    TimestampMillis,
};
use serde::{Serialize, Serializer, ser::SerializeStruct};

use crate::BackupSection;

pub const PROVIDER_BACKUP_GRAPH_VERSION: u32 = 2;
pub const MAX_BACKUP_PROVIDER_ACCOUNTS: usize = 128;
pub const MAX_BACKUP_MODEL_PROFILES: usize = 2_048;
pub const MAX_BACKUP_PROMPT_DOCUMENTS: usize = 2_048;
pub const MAX_BACKUP_AUDIO_PROVIDERS: usize = 128;
pub const MAX_BACKUP_USER_VOICES: usize = 4_096;
pub const MAX_BACKUP_AUTHORED_ROOTS: usize = 4_096;
pub const MAX_BACKUP_MEDIA_RECORDS: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderBackupSelections {
    pub default_model_profile_id: Option<ModelProfileId>,
    pub dynamic_memory_model_profile_id: Option<ModelProfileId>,
    pub group_speaker_model_profile_id: Option<ModelProfileId>,
    pub default_prompt_document_id: Option<PromptDocumentId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BackupGlobalSettings {
    pub value: GlobalSettings,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BackupLorebookBindings<Owner> {
    pub owner_id: Owner,
    pub bindings: Vec<LorebookBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
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
    pub accounts: Vec<ProviderAccount>,
    pub profiles: Vec<ModelProfile>,
    pub prompts: Vec<PromptDocument>,
    pub selections: ProviderBackupSelections,
    pub settings: BackupGlobalSettings,
    pub audio_providers: Vec<AudioProvider>,
    pub user_voices: Vec<UserVoice>,
    pub authored: AuthoredProfileBackup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderBackupSourceError {
    #[error("backup source contains invalid data")]
    InvalidData,
    #[error("backup source could not be read")]
    Storage,
}

pub trait ProviderBackupSource: Send + Sync {
    fn read_provider_backup_graph(&self) -> Result<ProviderBackupGraph, ProviderBackupSourceError>;
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

impl fmt::Debug for BackupMediaObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupMediaObject")
            .field("content_hash", &self.content_hash)
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

pub fn provider_backup_sections(
    mut graph: ProviderBackupGraph,
    secrets: Vec<ProviderBackupSecret>,
    media: Vec<BackupMediaObject>,
) -> Result<Vec<BackupSection>, ProviderBackupGraphError> {
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
    let metadata =
        serde_json::to_vec(&graph).map_err(|_| ProviderBackupGraphError::Serialization)?;
    let ordered_secrets = expected
        .keys()
        .map(|reference| supplied[reference])
        .collect::<Vec<_>>();
    let secret_bytes = serde_json::to_vec(&SecretDocument {
        version: PROVIDER_BACKUP_GRAPH_VERSION,
        secrets: ordered_secrets,
    })
    .map_err(|_| ProviderBackupGraphError::Serialization)?;
    let media_sections = media_sections(&graph, media)?;
    let mut sections = vec![
        BackupSection::new("data/provider-graph.json", "provider-graph.v2", metadata),
        BackupSection::new(
            "secrets/provider-secrets.json",
            "provider-secrets.v2",
            secret_bytes,
        ),
    ];
    sections.extend(media_sections);
    Ok(sections)
}

pub fn provider_backup_secret_requirements(
    graph: &ProviderBackupGraph,
) -> Result<Vec<(SecretRef, SecretPurpose)>, ProviderBackupGraphError> {
    let mut graph = graph.clone();
    canonicalize_and_validate(&mut graph)?;
    Ok(expected_secrets(&graph)?.into_iter().collect())
}

pub fn provider_backup_media_requirements(
    graph: &ProviderBackupGraph,
) -> Result<Vec<(lettuce_types::ContentHash, u64)>, ProviderBackupGraphError> {
    let mut graph = graph.clone();
    canonicalize_and_validate(&mut graph)?;
    Ok(graph
        .authored
        .media_blobs
        .into_iter()
        .filter(|blob| blob.state == BlobState::Ready)
        .map(|blob| (blob.content_hash, blob.byte_size))
        .collect())
}

fn media_sections(
    graph: &ProviderBackupGraph,
    media: Vec<BackupMediaObject>,
) -> Result<Vec<BackupSection>, ProviderBackupGraphError> {
    let expected = graph
        .authored
        .media_blobs
        .iter()
        .filter(|blob| blob.state == BlobState::Ready)
        .map(|blob| (&blob.content_hash, blob.byte_size))
        .collect::<Vec<_>>();
    if expected.len() != media.len() {
        return Err(ProviderBackupGraphError::InvalidGraph);
    }
    let total = expected.iter().try_fold(0_usize, |total, (_, size)| {
        if *size > MAX_MEDIA_BLOB_BYTES {
            return Err(ProviderBackupGraphError::LimitExceeded);
        }
        let size = usize::try_from(*size).map_err(|_| ProviderBackupGraphError::LimitExceeded)?;
        total
            .checked_add(size)
            .ok_or(ProviderBackupGraphError::LimitExceeded)
    })?;
    if total > crate::MAX_BACKUP_TOTAL_BYTES {
        return Err(ProviderBackupGraphError::LimitExceeded);
    }
    let mut sections = Vec::with_capacity(expected.len());
    for ((expected_hash, expected_size), object) in expected.into_iter().zip(media) {
        if &object.content_hash != expected_hash
            || object.bytes.len()
                != usize::try_from(expected_size)
                    .map_err(|_| ProviderBackupGraphError::LimitExceeded)?
            || lettuce_types::ContentHash::parse(blake3::hash(&object.bytes).to_hex().to_string())
                .as_ref()
                != Ok(expected_hash)
        {
            return Err(ProviderBackupGraphError::InvalidGraph);
        }
        sections.push(BackupSection::new(
            format!("media/blobs/{expected_hash}"),
            "media-blob.v2",
            object.bytes.to_vec(),
        ));
    }
    Ok(sections)
}

fn canonicalize_and_validate(
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

    if graph.settings.revision.get() == 0 || graph.settings.created_at > graph.settings.updated_at {
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
            || profile.config.chat_parameters.validate().is_err()
            || profile
                .config
                .lorebook_generator_parameters
                .validate()
                .is_err()
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
    if generator
        .model_profile_id
        .is_some_and(|id| !profile_ids.contains_key(&id))
        || [
            generator.planner_prompt_id,
            generator.writer_prompt_id,
            generator.refine_prompt_id,
            generator.coherence_prompt_id,
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

fn expected_secrets(
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
    use lettuce_types::{ContentHash, MediaBlobId, ProviderAccountId, Revision, TimestampMillis};

    fn graph(reference: SecretRef) -> ProviderBackupGraph {
        ProviderBackupGraph {
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
            provider_backup_sections(source_graph.clone(), Vec::new(), Vec::new()),
            Err(ProviderBackupGraphError::InvalidSecrets)
        );
        let secret = ProviderBackupSecret {
            reference,
            purpose,
            generation: 1,
            value: SecretValue::new("backup-secret-canary").expect("secret"),
        };
        assert!(!format!("{secret:?}").contains("backup-secret-canary"));
        let sections =
            provider_backup_sections(source_graph, vec![secret], Vec::new()).expect("sections");
        assert_eq!(sections.len(), 2);
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
            provider_backup_sections(graph(reference), vec![orphan], Vec::new()),
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
                content_hash,
                bytes: zeroize::Zeroizing::new(bytes.clone()),
            }],
        )
        .expect("media sections");
        assert_eq!(sections.len(), 3);
        assert_eq!(&*sections[2].bytes, &bytes);
    }
}
