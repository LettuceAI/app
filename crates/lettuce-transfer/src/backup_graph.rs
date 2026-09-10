use std::{collections::BTreeMap, fmt};

use lettuce_context::{PromptDocument, PromptProvenance};
use lettuce_models::{ModelProfile, ProviderAccount, validate_provider_connection};
use lettuce_settings::{GlobalSettings, SecretPurpose, SecretRef, SecretValue};
use lettuce_speech::{AudioProvider, UserVoice};
use lettuce_types::{ModelProfileId, PromptDocumentId, Revision, TimestampMillis};
use serde::{Serialize, Serializer, ser::SerializeStruct};

use crate::BackupSection;

pub const PROVIDER_BACKUP_GRAPH_VERSION: u32 = 2;
pub const MAX_BACKUP_PROVIDER_ACCOUNTS: usize = 128;
pub const MAX_BACKUP_MODEL_PROFILES: usize = 2_048;
pub const MAX_BACKUP_PROMPT_DOCUMENTS: usize = 2_048;
pub const MAX_BACKUP_AUDIO_PROVIDERS: usize = 128;
pub const MAX_BACKUP_USER_VOICES: usize = 4_096;

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
    Ok(vec![
        BackupSection::new("data/provider-graph.json", "provider-graph.v2", metadata),
        BackupSection::new(
            "secrets/provider-secrets.json",
            "provider-secrets.v2",
            secret_bytes,
        ),
    ])
}

pub fn provider_backup_secret_requirements(
    graph: &ProviderBackupGraph,
) -> Result<Vec<(SecretRef, SecretPurpose)>, ProviderBackupGraphError> {
    let mut graph = graph.clone();
    canonicalize_and_validate(&mut graph)?;
    Ok(expected_secrets(&graph)?.into_iter().collect())
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
    use lettuce_types::{ProviderAccountId, Revision, TimestampMillis};

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
            provider_backup_sections(source_graph.clone(), Vec::new()),
            Err(ProviderBackupGraphError::InvalidSecrets)
        );
        let secret = ProviderBackupSecret {
            reference,
            purpose,
            generation: 1,
            value: SecretValue::new("backup-secret-canary").expect("secret"),
        };
        assert!(!format!("{secret:?}").contains("backup-secret-canary"));
        let sections = provider_backup_sections(source_graph, vec![secret]).expect("sections");
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
            provider_backup_sections(graph(reference), vec![orphan]),
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
}
