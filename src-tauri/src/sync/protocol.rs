use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncDomain {
    Core,
    Tts,
    Lorebooks,
    Characters,
    Groups,
    Sessions,
    Messages,
    Assets,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DomainCursor {
    pub domain: SyncDomain,
    pub last_change_id: i64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct CursorSet {
    pub cursors: Vec<DomainCursor>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeOp {
    Upsert,
    Delete,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DomainPlan {
    pub domain: SyncDomain,
    pub change_count: u32,
    pub estimated_bytes: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ChangeRecord {
    pub change_id: i64,
    pub source_device_id: String,
    pub source_created_at: i64,
    pub source_change_id: i64,
    pub entity_type: String,
    pub entity_id: String,
    pub op: ChangeOp,
    pub payload_schema: u16,
    pub payload_hash: String,
    pub payload: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum P2PMessage {
    Handshake {
        protocol_version: u32,
        app_version: String,
        device_name: String,
        device_id: String,
        salt: [u8; 16],
        challenge: [u8; 16],
    },
    AuthRequest {
        encrypted_challenge: Vec<u8>,

        my_challenge: [u8; 16],
    },
    AuthResponse {
        encrypted_challenge: Vec<u8>,
    },

    AdvertiseCursors {
        cursors: CursorSet,
    },
    SyncManifest {
        plan: Vec<DomainPlan>,
        total_changes: u32,
        total_bytes: u64,
    },

    PushChanges {
        domain: SyncDomain,
        changes: Vec<ChangeRecord>,
    },
    AssetContent {
        entity_id: String,
        path: String,
        content_hash: String,
        content: Vec<u8>,
    },
    AssetBatchComplete {
        last_change_id: i64,
    },

    SyncComplete,
    SyncApplied,
    StatusUpdate(String),
    Disconnect,
    Error(String),
    Ready,

    AssetContentStart {
        entity_id: String,
        path: String,
        content_hash: String,
        total_bytes: u64,
    },
    AssetContentChunk {
        entity_id: String,
        chunk: Vec<u8>,
    },
    AssetContentComplete {
        entity_id: String,
    },
    V2(crate::sync::v2::protocol::SyncV2Message),
}
