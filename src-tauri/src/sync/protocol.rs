use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// 1. Grouping DB tables by dependency layers
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SyncLayer {
    Globals,
    // meta, settings, personas, models, secrets, provider_credentials, prompt_templates,
    // model_pricing_cache, creation_helper_sessions, group_characters
    Lorebooks,
    // lorebooks, lorebook_entries
    Characters,
    // characters, rules, scenes, scene_variants, character_lorebooks, chat_templates,
    // chat_template_messages
    Sessions,
    // sessions, messages, message_variants, usage_records, usage_metadata
    GroupSessions,
    // group_sessions, group_participation, group_messages, group_message_variants,
    // usage_records, usage_metadata
}

// 2. The Data Manifest (What do I have?)
// We track "last_updated" timestamps for entities that have them.
// For global tables without clear ID separation (like settings),
// we might use a single key or just always sync them (Globals layer).
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Manifest {
    // Map<ID, Last_Updated_Timestamp>
    pub lorebooks: HashMap<String, i64>,
    pub characters: HashMap<String, i64>,
    pub sessions: HashMap<String, i64>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ManifestV2 {
    pub lorebooks: HashMap<String, i64>,
    pub characters: HashMap<String, i64>,
    pub sessions: HashMap<String, i64>,
    pub group_sessions: HashMap<String, i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncDomain {
    Core,
    Tts,
    Lorebooks,
    Characters,
    Groups,
    Conversations,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DomainManifest {
    pub domain: SyncDomain,
    pub fingerprint: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncManifest {
    pub domains: Vec<DomainManifest>,
}

// 3. The Actual Messages over TCP
#[derive(Serialize, Deserialize, Debug)]
pub enum P2PMessage {
    // Handshake
    Handshake {
        #[serde(default = "default_protocol_version")]
        protocol_version: u32,
        device_name: String,
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
    SyncRequest {
        manifest: Manifest,
    },
    SyncRequestV2 {
        manifest: ManifestV2,
    },
    SyncManifest {
        manifest: SyncManifest,
    },

    // Data Transfer
    DataResponse {
        layer: SyncLayer,
        payload: Vec<u8>,
    },
    DomainSnapshot {
        domain: SyncDomain,
        payload: Vec<u8>,
    },

    // Control
    SyncComplete,
    StatusUpdate(String),
    FileTransfer {
        path: String,
        content: Vec<u8>,
    },
    Disconnect,
    Error(String),
}

fn default_protocol_version() -> u32 {
    1
}
