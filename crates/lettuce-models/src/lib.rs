//! Provider accounts and user-configured model profiles.

#![deny(unsafe_op_in_unsafe_fn)]

mod capabilities;
mod parameters;
mod resolution;

pub use capabilities::*;
pub use parameters::*;
pub use resolution::*;

use lettuce_settings::{HeaderName, SecretOwnerId, SecretRef};
use lettuce_types::{
    CharacterId, GroupId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis,
};
use serde::{Deserialize, Serialize};

const MAX_PROVIDER_KIND_BYTES: usize = 128;
const MAX_PROVIDER_ENDPOINT_BYTES: usize = 4096;
const MAX_PROVIDER_PATH_BYTES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProtocol {
    OpenAiCompatible,
    Anthropic,
    Gemini,
    Ollama,
    LlamaCpp,
    StableDiffusion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretHeader {
    pub name: HeaderName,
    pub secret_ref: SecretRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderConfig {
    Standard,
    Custom {
        chat_path: String,
        models_path: Option<String>,
        streaming: bool,
        auth: CustomAuth,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomAuth {
    Bearer,
    Header,
    Query,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAccount {
    pub id: ProviderAccountId,
    pub secret_owner_id: SecretOwnerId,
    pub provider_kind: String,
    pub protocol: ProviderProtocol,
    pub label: String,
    pub endpoint: Option<String>,
    pub enabled: bool,
    pub api_key_ref: Option<SecretRef>,
    pub secret_headers: Vec<SecretHeader>,
    pub config: ProviderConfig,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderConnectionValidationError {
    ProviderKind,
    Endpoint,
    Path,
}

/// Validates the non-secret connection metadata shared by storage and
/// resolution. Secret values are deliberately outside this contract.
pub fn validate_provider_connection(
    account: &ProviderAccount,
) -> Result<(), ProviderConnectionValidationError> {
    if account.provider_kind.trim().is_empty()
        || account.provider_kind.len() > MAX_PROVIDER_KIND_BYTES
    {
        return Err(ProviderConnectionValidationError::ProviderKind);
    }
    if let Some(endpoint) = account.endpoint.as_deref() {
        validate_endpoint(endpoint)?;
    }
    if let ProviderConfig::Custom {
        chat_path,
        models_path,
        ..
    } = &account.config
    {
        validate_path(chat_path)?;
        if let Some(models_path) = models_path {
            validate_path(models_path)?;
        }
    }
    Ok(())
}

fn validate_endpoint(endpoint: &str) -> Result<(), ProviderConnectionValidationError> {
    if endpoint.trim() != endpoint
        || endpoint.is_empty()
        || endpoint.len() > MAX_PROVIDER_ENDPOINT_BYTES
        || endpoint
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || endpoint.contains(['?', '#'])
    {
        return Err(ProviderConnectionValidationError::Endpoint);
    }
    let Some(scheme_end) = endpoint.find("://") else {
        return Err(ProviderConnectionValidationError::Endpoint);
    };
    if !matches!(
        &endpoint[..scheme_end],
        scheme if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    ) {
        return Err(ProviderConnectionValidationError::Endpoint);
    }
    let authority_start = scheme_end + 3;
    let authority_end = endpoint[authority_start..]
        .find('/')
        .map_or(endpoint.len(), |offset| authority_start + offset);
    let authority = &endpoint[authority_start..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return Err(ProviderConnectionValidationError::Endpoint);
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), ProviderConnectionValidationError> {
    if path.trim() != path
        || path.is_empty()
        || path.len() > MAX_PROVIDER_PATH_BYTES
        || !path.starts_with('/')
        || path.starts_with("//")
        || path.contains(['?', '#'])
        || path
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(ProviderConnectionValidationError::Path);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Chat,
    Image,
    Embedding,
    Speech,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    Text,
    Image,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelProfileConfig {
    #[serde(default)]
    pub chat_parameters: ChatParameterProfile,
    pub capabilities: ModelCapabilities,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelProfile {
    pub id: ModelProfileId,
    pub provider_account_id: ProviderAccountId,
    pub external_model_id: String,
    pub display_name: String,
    pub kind: ModelKind,
    pub config: ModelProfileConfig,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

/// A typed owner that prevents deleting a model profile still referenced by
/// authored aggregates. This vocabulary deliberately lives in the model
/// domain so storage adapters need not depend on the characters crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ModelDependencyReference {
    CharacterDefault {
        character_id: CharacterId,
    },
    GroupMemberOverride {
        group_id: GroupId,
        character_id: CharacterId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelRepositoryError {
    #[error("record was not found")]
    NotFound,
    #[error("update has a stale revision")]
    StaleRevision,
    #[error("a record with this identity already exists")]
    AlreadyExists,
    #[error("provider account is still used by model profiles")]
    AccountInUse(Vec<ModelProfileId>),
    #[error("model profile is still in use")]
    InUse(Vec<ModelDependencyReference>),
    #[error("provider account does not exist")]
    AccountMissing,
    #[error("stored model data are invalid")]
    InvalidData,
    #[error("model storage failed")]
    Storage,
}

pub trait ProviderAccountRepository: Send + Sync {
    fn upsert(
        &self,
        account: ProviderAccount,
        expected_revision: Option<Revision>,
    ) -> Result<ProviderAccount, ModelRepositoryError>;
    fn get(&self, id: ProviderAccountId) -> Result<Option<ProviderAccount>, ModelRepositoryError>;
    fn delete(&self, id: ProviderAccountId) -> Result<(), ModelRepositoryError>;
    /// Atomically clears the global default, removes dependent profiles, and
    /// removes the account. Secret cleanup is a separate workflow.
    fn delete_with_profiles(&self, id: ProviderAccountId) -> Result<(), ModelRepositoryError>;
}

pub trait ModelProfileRepository: Send + Sync {
    fn upsert(
        &self,
        profile: ModelProfile,
        expected_revision: Option<Revision>,
    ) -> Result<ModelProfile, ModelRepositoryError>;
    fn get(&self, id: ModelProfileId) -> Result<Option<ModelProfile>, ModelRepositoryError>;
    /// Deletes a profile and clears the global default in the same transaction.
    fn delete_and_clear_default(&self, id: ModelProfileId) -> Result<(), ModelRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::{ModelDependencyReference, ModelRepositoryError};
    use lettuce_types::CharacterId;

    #[test]
    fn model_dependencies_are_typed_and_closed() {
        let reference = ModelDependencyReference::CharacterDefault {
            character_id: CharacterId::new(),
        };
        let encoded = serde_json::to_string(&reference).expect("dependency serializes");
        assert_eq!(
            serde_json::from_str::<ModelDependencyReference>(&encoded).expect("dependency decodes"),
            reference
        );
        assert!(serde_json::from_str::<ModelDependencyReference>(
            r#"{"kind":"character_default","id":"00000000-0000-0000-0000-000000000000","extra":true}"#
        )
        .is_err());
        assert_eq!(
            ModelRepositoryError::InUse(vec![reference]).to_string(),
            "model profile is still in use"
        );

        let group_override = ModelDependencyReference::GroupMemberOverride {
            group_id: lettuce_types::GroupId::new(),
            character_id: CharacterId::new(),
        };
        let encoded = serde_json::to_string(&group_override).expect("dependency serializes");
        assert_eq!(
            serde_json::from_str::<ModelDependencyReference>(&encoded).expect("dependency decodes"),
            group_override
        );
        assert!(serde_json::from_str::<ModelDependencyReference>(
            r#"{"kind":"group_member_override","id":{"group_id":"00000000-0000-0000-0000-000000000000","character_id":"00000000-0000-0000-0000-000000000000","extra":true}}"#
        )
        .is_err());
    }
}
