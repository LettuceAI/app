//! Provider accounts and user-configured model profiles.

#![deny(unsafe_op_in_unsafe_fn)]

use lettuce_settings::{HeaderName, SecretRef};
use lettuce_types::{CharacterId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis};
use serde::{Deserialize, Serialize};

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
    pub input_modalities: Vec<Modality>,
    pub output_modalities: Vec<Modality>,
    pub temperature: Option<f64>,
    pub context_length: Option<u32>,
    pub max_output_tokens: Option<u32>,
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
    CharacterDefault { character_id: CharacterId },
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
    }
}
