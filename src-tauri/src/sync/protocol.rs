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
pub struct ChangeRecord {
    pub change_id: i64,
    pub entity_type: String,
    pub entity_id: String,
    pub op: ChangeOp,
    pub payload_schema: u16,
    pub payload_hash: String,
    pub payload: Vec<u8>,
}

// 3. The Actual Messages over TCP
#[derive(Serialize, Deserialize, Debug)]
pub enum P2PMessage {
    // Handshake
    Handshake {
        #[serde(default = "default_protocol_version")]
        protocol_version: u32,
        device_name: String,
        #[serde(default)]
        device_id: String,
        salt: [u8; 16],
        challenge: [u8; 16], // Random bytes the other side must decrypt and return
    },
    AuthRequest {
        // The sender encrypts the received challenge with the derived key
        // and sends it back to prove they know the PIN.
        encrypted_challenge: Vec<u8>,
        // Sender also sends their own challenge for mutual auth
        my_challenge: [u8; 16],
    },
    AuthResponse {
        // Reply to the sender's challenge
        encrypted_challenge: Vec<u8>,
    },

    // Sync Coordination
    AdvertiseCursors {
        cursors: CursorSet,
    },

    // Data Transfer
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

    // Control
    SyncComplete,
    StatusUpdate(String),
    Disconnect,
    Error(String),
}

fn default_protocol_version() -> u32 {
    1
}
