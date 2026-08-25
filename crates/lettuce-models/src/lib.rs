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
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

const MAX_PROVIDER_KIND_BYTES: usize = 128;
const MAX_PROVIDER_ENDPOINT_BYTES: usize = 4096;
const MAX_PROVIDER_PATH_BYTES: usize = 1024;
const MAX_PROVIDER_QUERY_PARAMETER_BYTES: usize = 128;
const MAX_PROVIDER_SECRET_HEADERS: usize = 16;

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
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CustomAuth {
    Bearer,
    Header { name: HeaderName },
    Query { name: QueryParameterName },
    None,
}

/// A bounded query parameter name for custom provider authentication.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct QueryParameterName(String);

impl QueryParameterName {
    pub fn new(value: impl Into<String>) -> Result<Self, QueryParameterNameError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_PROVIDER_QUERY_PARAMETER_BYTES {
            return Err(QueryParameterNameError::InvalidLength);
        }
        if !value.bytes().all(is_query_parameter_byte) {
            return Err(QueryParameterNameError::InvalidCharacter);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for QueryParameterName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

fn is_query_parameter_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!$%'*+-.^_`|~".contains(&byte)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum QueryParameterNameError {
    #[error("query parameter name has an invalid length")]
    InvalidLength,
    #[error("query parameter name contains an invalid character")]
    InvalidCharacter,
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
    Authentication,
    SecretHeaders,
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
        auth,
        ..
    } = &account.config
    {
        if account.endpoint.is_none() {
            return Err(ProviderConnectionValidationError::Endpoint);
        }
        validate_path(chat_path)?;
        if let Some(models_path) = models_path {
            validate_path(models_path)?;
        }
        match auth {
            CustomAuth::Bearer => {
                if account.api_key_ref.is_none() {
                    return Err(ProviderConnectionValidationError::Authentication);
                }
            }
            CustomAuth::Header { name } => {
                if account.api_key_ref.is_none() || is_dangerous_header_name(name) {
                    return Err(ProviderConnectionValidationError::Authentication);
                }
            }
            CustomAuth::Query { .. } => {
                if account.api_key_ref.is_none() {
                    return Err(ProviderConnectionValidationError::Authentication);
                }
            }
            CustomAuth::None => {}
        }
    }
    validate_secret_headers(account)?;
    Ok(())
}

fn validate_secret_headers(
    account: &ProviderAccount,
) -> Result<(), ProviderConnectionValidationError> {
    if account.secret_headers.len() > MAX_PROVIDER_SECRET_HEADERS {
        return Err(ProviderConnectionValidationError::SecretHeaders);
    }
    let mut names = HashSet::with_capacity(account.secret_headers.len());
    let mut refs = HashSet::with_capacity(account.secret_headers.len());
    let auth_header_name = match &account.config {
        ProviderConfig::Custom {
            auth: CustomAuth::Header { name },
            ..
        } => Some(name.as_str()),
        _ => None,
    };
    let standard_or_bearer = matches!(
        &account.config,
        ProviderConfig::Standard
            | ProviderConfig::Custom {
                auth: CustomAuth::Bearer,
                ..
            }
    );
    for header in &account.secret_headers {
        let normalized_name = header.name.as_str().to_ascii_lowercase();
        if !names.insert(normalized_name)
            || !refs.insert(header.secret_ref)
            || is_dangerous_header_name(&header.name)
            || account
                .api_key_ref
                .is_some_and(|api_key_ref| api_key_ref == header.secret_ref)
            || auth_header_name.is_some_and(|name| name.eq_ignore_ascii_case(header.name.as_str()))
            || (standard_or_bearer && header.name.as_str().eq_ignore_ascii_case("authorization"))
        {
            return Err(ProviderConnectionValidationError::SecretHeaders);
        }
    }
    Ok(())
}

fn is_dangerous_header_name(name: &HeaderName) -> bool {
    matches_ignore_ascii_case(
        name.as_str(),
        [
            "host",
            "content-length",
            "content-type",
            "transfer-encoding",
            "connection",
            "upgrade",
            "keep-alive",
            "te",
            "trailer",
            "proxy-authorization",
            "proxy-connection",
        ],
    )
}

fn matches_ignore_ascii_case<const N: usize>(value: &str, candidates: [&str; N]) -> bool {
    candidates
        .into_iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
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
    use super::{
        CustomAuth, ModelDependencyReference, ModelRepositoryError, ProviderAccount,
        ProviderConfig, ProviderConnectionValidationError, ProviderProtocol, QueryParameterName,
        SecretHeader, validate_provider_connection,
    };
    use lettuce_settings::{HeaderName, SecretOwnerId, SecretRef};
    use lettuce_types::CharacterId;
    use lettuce_types::{ProviderAccountId, Revision, TimestampMillis};

    fn account() -> ProviderAccount {
        ProviderAccount {
            id: ProviderAccountId::new(),
            secret_owner_id: SecretOwnerId::new(),
            provider_kind: "test".into(),
            protocol: ProviderProtocol::OpenAiCompatible,
            label: "Test".into(),
            endpoint: Some("https://example.invalid".into()),
            enabled: true,
            api_key_ref: Some(SecretRef::new()),
            secret_headers: Vec::new(),
            config: ProviderConfig::Standard,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        }
    }

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

    #[test]
    fn custom_auth_metadata_is_typed_and_exactly_serialized() {
        let header = CustomAuth::Header {
            name: HeaderName::new("X-API-Key").expect("header name"),
        };
        assert_eq!(
            serde_json::to_value(&header).expect("header auth serializes"),
            serde_json::json!({"header": {"name": "X-API-Key"}})
        );
        let query = CustomAuth::Query {
            name: QueryParameterName::new("api_key").expect("query name"),
        };
        assert_eq!(
            serde_json::to_value(&query).expect("query auth serializes"),
            serde_json::json!({"query": {"name": "api_key"}})
        );
        assert_eq!(
            serde_json::from_value::<CustomAuth>(serde_json::json!({
                "query": {"name": "api_key"}
            }))
            .expect("query auth decodes"),
            query
        );
        for name in [
            "", "api key", "api&key", "api=key", "api?key", "api#key", "é",
        ] {
            assert!(
                QueryParameterName::new(name).is_err(),
                "invalid query name should be rejected: {name:?}"
            );
        }
    }

    #[test]
    fn provider_connection_validates_custom_auth_and_secret_header_metadata() {
        let mut account = account();
        account.config = ProviderConfig::Custom {
            chat_path: "/chat".into(),
            models_path: Some("/models".into()),
            streaming: true,
            auth: CustomAuth::Header {
                name: HeaderName::new("X-API-Key").expect("header name"),
            },
        };
        assert!(validate_provider_connection(&account).is_ok());

        account.api_key_ref = None;
        assert_eq!(
            validate_provider_connection(&account),
            Err(ProviderConnectionValidationError::Authentication)
        );
        account.api_key_ref = Some(SecretRef::new());
        account.endpoint = None;
        assert_eq!(
            validate_provider_connection(&account),
            Err(ProviderConnectionValidationError::Endpoint)
        );

        account.endpoint = Some("https://example.invalid".into());
        account.secret_headers = vec![SecretHeader {
            name: HeaderName::new("x-api-key").expect("header name"),
            secret_ref: SecretRef::new(),
        }];
        assert_eq!(
            validate_provider_connection(&account),
            Err(ProviderConnectionValidationError::SecretHeaders)
        );

        account.secret_headers[0].name = HeaderName::new("Host").expect("header name");
        assert_eq!(
            validate_provider_connection(&account),
            Err(ProviderConnectionValidationError::SecretHeaders)
        );

        account.secret_headers = vec![SecretHeader {
            name: HeaderName::new("X-Other").expect("header name"),
            secret_ref: account.api_key_ref.expect("api key ref"),
        }];
        assert_eq!(
            validate_provider_connection(&account),
            Err(ProviderConnectionValidationError::SecretHeaders)
        );

        account.secret_headers[0].secret_ref = SecretRef::new();
        account.config = ProviderConfig::Custom {
            chat_path: "/chat".into(),
            models_path: None,
            streaming: false,
            auth: CustomAuth::Bearer,
        };
        account.secret_headers[0].name = HeaderName::new("AUTHORIZATION").expect("header name");
        assert_eq!(
            validate_provider_connection(&account),
            Err(ProviderConnectionValidationError::SecretHeaders)
        );
    }

    #[test]
    fn custom_none_allows_missing_api_key_and_custom_paths_remain_required() {
        let mut account = account();
        account.api_key_ref = None;
        account.config = ProviderConfig::Custom {
            chat_path: "/chat".into(),
            models_path: None,
            streaming: false,
            auth: CustomAuth::None,
        };
        assert!(validate_provider_connection(&account).is_ok());

        account.endpoint = None;
        assert_eq!(
            validate_provider_connection(&account),
            Err(ProviderConnectionValidationError::Endpoint)
        );
        account.endpoint = Some("https://example.invalid".into());
        account.config = ProviderConfig::Custom {
            chat_path: "chat".into(),
            models_path: None,
            streaming: false,
            auth: CustomAuth::None,
        };
        assert_eq!(
            validate_provider_connection(&account),
            Err(ProviderConnectionValidationError::Path)
        );
    }
}
