use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncSharing {
    pub addresses: Vec<String>,
    pub port: u16,
    pub pin: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncConnectRequest {
    pub address: String,
    pub port: u16,
    pub pin: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncShareApproveRequest {
    pub approve: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SyncPhase {
    Changes,
    Media,
    Secrets,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SyncFailure {
    InvalidState,
    DeviceNameUnavailable,
    AddressesUnavailable,
    MediaUnavailable,
    AuthenticationFailed,
    PairingLocked,
    Declined,
    Disconnected,
    PeerStalled,
    Protocol,
    Storage,
    PendingBatch { batch_id: String },
    RequiredReferenceUnavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SyncStatus {
    Idle,
    Sharing {
        addresses: Vec<String>,
        port: u16,
        pin: String,
    },
    PendingApproval {
        device_name: String,
    },
    PendingStart {
        device_name: String,
    },
    Connecting,
    Syncing {
        phase: SyncPhase,
        progress: SyncProgress,
    },
    Completed {
        counts: SyncCounts,
        #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
        conflicts: u64,
        #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
        forks: u64,
    },
    Failed {
        reason: crate::ApiError,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncProgress {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub items_done: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub items_total: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub bytes_done: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub bytes_total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncCounts {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub sent_changes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub received_changes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub received_assets: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub received_blobs: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub received_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub unavailable_assets: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub received_secrets: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub skipped_secrets: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SyncConflictChoice {
    Current,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SyncConflictValue {
    Character { value: Box<crate::CharacterView> },
    Persona { value: Box<crate::PersonaView> },
    PersonaDefault { value: crate::PersonaDefaultView },
    Lorebook { value: crate::LorebookView },
    Prompt { value: crate::PromptView },
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncConflictCandidate {
    pub change_id: Option<String>,
    pub device_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub timestamp: Option<i64>,
    pub value: SyncConflictValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncConflictView {
    pub conflict_id: String,
    pub entity_kind: String,
    pub entity_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub detected_at: i64,
    pub current: SyncConflictCandidate,
    pub other: SyncConflictCandidate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncForkView {
    pub conversation_id: String,
    pub branch_id: String,
    pub holds_local: bool,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub detected_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LegacyConflictField {
    pub key: String,
    pub current: LegacyConflictCurrent,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub local: serde_json::Value,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub incoming: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LegacyConflictView {
    pub run_id: String,
    pub source_key: String,
    pub entity_kind: String,
    pub entity_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub detected_at: i64,
    pub fields: Vec<LegacyConflictField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncConflictsList {
    pub conflicts: Vec<SyncConflictView>,
    pub legacy_conflicts: Vec<LegacyConflictView>,
    pub forks: Vec<SyncForkView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncConflictResolveRequest {
    pub client_operation_id: String,
    pub conflict_id: String,
    pub expected_current_change: Option<String>,
    pub choice: SyncConflictChoice,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncConflictResolution {
    pub change_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SyncForkResolveRequest {
    pub client_operation_id: String,
    pub conversation_id: String,
    pub branch_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LegacyConflictDismissRequest {
    pub client_operation_id: String,
    pub run_id: String,
    pub source_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LegacyConflictCurrent {
    Value {
        #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
        value: serde_json::Value,
    },
    NotMapped,
}
