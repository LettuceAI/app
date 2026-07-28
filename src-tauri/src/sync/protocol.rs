use serde::{Deserialize, Serialize};

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

    Ready,
    Sync(crate::sync::v2::protocol::SyncV2Message),
    Disconnect,
    Error(String),
}
