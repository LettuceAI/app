use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use futures::{SinkExt, StreamExt};
use rand::{thread_rng, RngCore};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_util::codec::Framed;

use crate::sync::codec::P2PCodec;
use crate::sync::db as sync_db;
use crate::sync::protocol::{ChangeOp, P2PMessage, SyncDomain};
use crate::utils::{log_error, log_info, log_warn};

const PROTOCOL_VERSION: u32 = 7;

fn derive_key(pin: &str, salt: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key("lettuce_sync_v1");
    hasher.update(salt);
    hasher.update(pin.as_bytes());
    let mut output = [0u8; 32];
    hasher.finalize_xof().fill(&mut output);
    output
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", content = "details")]
pub enum SyncStatus {
    Idle,
    DriverRunning {
        ip: String,
        port: u16,
        pin: String, // Added PIN to status
        clients: usize,
    },
    PassengerConnecting,
    PassengerConnected {
        driver_ip: String,
    },
    Syncing {
        phase: String,
        progress: Option<f32>,
    },
    Error {
        message: String,
    },
    PendingApproval {
        ip: String,
        device_name: String,
    },
    PendingSyncStart {
        ip: String,
        device_name: String,
    },
    SyncCompleted,
}

pub struct SyncManagerState {
    pub status: RwLock<SyncStatus>,
    shutdown_tx: Mutex<Option<broadcast::Sender<()>>>,
    pub pending_approvals:
        RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    pub pending_starts: RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<()>>>,
    pub pin: RwLock<Option<String>>, // Added PIN storage
}

impl Default for SyncManagerState {
    fn default() -> Self {
        Self {
            status: RwLock::new(SyncStatus::Idle),
            shutdown_tx: Mutex::new(None),
            pending_approvals: RwLock::new(std::collections::HashMap::new()),
            pending_starts: RwLock::new(std::collections::HashMap::new()),
            pin: RwLock::new(None),
        }
    }
}

impl SyncManagerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_status(&self, app: &AppHandle, status: SyncStatus) {
        *self.status.write().await = status.clone();
        let _ = app.emit("sync-status-changed", status);
    }
}

// Driver Logic (Host)
pub async fn start_driver(app: AppHandle, _port: u16) -> Result<String, String> {
    let state = app.state::<SyncManagerState>();
    let mut current_tx = state.shutdown_tx.lock().await;
    if current_tx.is_some() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Sync service is already running",
        ));
    }

    let listener = TcpListener::bind("0.0.0.0:0")
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let port = listener
        .local_addr()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .port();

    let my_ip = crate::utils::get_local_ip().unwrap_or_else(|_| "0.0.0.0".into());

    // Generate PIN
    let pin: String = (0..6)
        .map(|_| {
            let mut byte = [0u8; 1];
            thread_rng().fill_bytes(&mut byte);
            (byte[0] % 10).to_string()
        })
        .collect();

    let (tx, mut rx) = broadcast::channel(1);
    *current_tx = Some(tx);
    *state.pin.write().await = Some(pin.clone());

    let app_clone = app.clone();

    state
        .set_status(
            &app,
            SyncStatus::DriverRunning {
                ip: my_ip,
                port,
                pin: pin.clone(),
                clients: 0,
            },
        )
        .await;

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = rx.recv() => {
                    break;
                }
                res = listener.accept() => {
                    match res {
                        Ok((stream, remote_addr)) => {
                            let app_inner = app_clone.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_driver_connection(app_inner.clone(), stream, remote_addr, port).await {
                                    log_error(&app_inner, "sync_driver", format!("Driver connection error: {}", e));
                                }
                            });
                        }
                        Err(e) => {
                            log_error(&app_clone, "sync_driver", format!("Listener accept error: {}", e));
                        }
                    }
                }
            }
        }
        let state = app_clone.state::<SyncManagerState>();
        state.set_status(&app_clone, SyncStatus::Idle).await;
        *state.pin.write().await = None;
    });

    Ok(pin)
}

async fn handle_driver_connection(
    app: AppHandle,
    stream: TcpStream,
    _addr: SocketAddr,
    port: u16,
) -> Result<(), String> {
    let remote_ip = stream
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let mut framed = Framed::new(stream, P2PCodec::new());

    // Security Handshake
    let state = app.state::<SyncManagerState>();

    let pin = state
        .pin
        .read()
        .await
        .clone()
        .ok_or("Driver not running or no PIN")?;

    let mut salt = [0u8; 16];
    let mut challenge = [0u8; 16];
    thread_rng().fill_bytes(&mut salt);
    thread_rng().fill_bytes(&mut challenge);
    let device_id = {
        let conn = crate::storage_manager::db::open_db(&app)?;
        sync_db::get_or_create_local_device_id(&conn)?
    };

    // Send Handshake
    framed
        .send(P2PMessage::Handshake {
            protocol_version: PROTOCOL_VERSION,
            device_name: whoami::devicename(),
            device_id,
            salt,
            challenge,
        })
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Wait for AuthRequest
    let (encrypted_challenge, their_challenge) = match framed.next().await {
        Some(Ok(P2PMessage::AuthRequest {
            encrypted_challenge,
            my_challenge,
        })) => (encrypted_challenge, my_challenge),
        Some(Ok(msg)) => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Expected AuthRequest, got {:?}", msg),
            ))
        }
        Some(Err(e)) => return Err(e.to_string()),
        None => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Connection closed during handshake",
            ))
        }
    };

    // Verify
    let key = derive_key(&pin, &salt);
    let cipher = ChaCha20Poly1305::new(&Key::from(key));

    // Try to decrypt their response to our challenge
    // The encrypted_challenge should be [Nonce 12][Ciphertext] if we follow P2PCodec pattern,
    // BUT P2PCodec is NOT encryption-enabled yet.
    // The other side manually encrypted the blob.
    // We assume they prepended the nonce.
    if encrypted_challenge.len() < 12 {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Auth challenge too short",
        ));
    }
    let mut n_bytes = [0u8; 12];
    n_bytes.copy_from_slice(&encrypted_challenge[..12]);
    let nonce = Nonce::from(n_bytes);
    let ciphertext = &encrypted_challenge[12..];

    let decrypted = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| "Auth failed (bad PIN)".to_string())?;

    if decrypted != challenge {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Auth failed (challenge mismatch)",
        ));
    }

    // Auth Success!
    // Encrypt their challenge to prove we are the driver
    let mut response_nonce_bytes = [0u8; 12];
    thread_rng().fill_bytes(&mut response_nonce_bytes);
    let response_nonce = Nonce::from(response_nonce_bytes);
    let response_ciphertext = cipher
        .encrypt(&response_nonce, their_challenge.as_ref())
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut response_payload = Vec::new();
    response_payload.extend_from_slice(&response_nonce_bytes);
    response_payload.extend_from_slice(&response_ciphertext);

    framed
        .send(P2PMessage::AuthResponse {
            encrypted_challenge: response_payload,
        })
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Enable Encryption for session
    framed.codec_mut().set_key(&key);

    // Wait for Handshake (WAIT, we already did Handshake)
    // The previous code waited for Handshake here to get device_name.
    // But we already got Handshake? No, wait.
    // The protocol was:
    // 1. Driver sends Handshake (Server -> Client)
    // 2. Client sends Handshake (Client -> Server)
    //
    // MY NEW PROTOCOL:
    // 1. Driver sends Handshake { salt, challenge }
    // 2. Client sends AuthRequest { enc_challenge, my_challenge }
    // 3. Driver sends AuthResponse
    //
    // Where does the Client send ITS device name?
    // The original code had Client send Handshake.
    // We should piggyback Device Name on AuthRequest? Or have Client send Handshake FIRST?
    // If Client sends Handshake first, it can include device name.
    // But Driver needs to send Salt/Challenge FIRST for Client to derive key.
    //
    // Adjusted Flow:
    // 1. Client connects.
    // 2. Driver sends Handshake { version, name, salt, challenge }.
    // 3. Client receives. Derives key.
    // 4. Client sends AuthRequest { enc_challenge, my_challenge, device_name? }.
    //    Ah, I didn't add `device_name` to `AuthRequest`.
    //    I should have.
    //    OR, Client sends `Handshake` packet *after* Auth? But that would be encrypted.
    //    That works! Once encrypted, Client sends `Handshake` with device name.
    //    Then Driver knows device name.
    //
    // So:
    // Driver: ... Auth Success, Enable Encryption, Send AuthResponse.
    // Client: ... Receive AuthResponse, Verify, Enable Encryption.
    // THEN Client sends `Handshake` (Encrypted).
    // Driver expects `Handshake`.

    // So after `set_key`, Driver should wait for `Handshake`.
    let (device_name, peer_device_id, peer_protocol_version) = match framed.next().await {
        Some(Ok(P2PMessage::Handshake {
            device_name,
            device_id,
            protocol_version,
            ..
        })) => (device_name, device_id, protocol_version),
        Some(Ok(msg)) => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Expected Encrypted Handshake, got {:?}", msg),
            ))
        }
        Some(Err(e)) => return Err(e.to_string()),
        None => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Connection closed after auth",
            ))
        }
    };

    // Approval Check
    let (tx, rx) = tokio::sync::oneshot::channel();

    let state = app.state::<SyncManagerState>();
    {
        state
            .pending_approvals
            .write()
            .await
            .insert(remote_ip.clone(), tx);
    }

    state
        .set_status(
            &app,
            SyncStatus::PendingApproval {
                ip: remote_ip.clone(),
                device_name: device_name.clone(),
            },
        )
        .await;

    match rx.await {
        Ok(true) => {
            // Approved
            log_info(
                &app,
                "sync_driver",
                format!("Connection from {} approved", remote_ip),
            );
        }
        _ => {
            // Rejected or dropped
            let my_ip = crate::utils::get_local_ip().unwrap_or_else(|_| "0.0.0.0".to_string());

            let pin = state.pin.read().await.clone().unwrap_or_default();
            state
                .set_status(
                    &app,
                    SyncStatus::DriverRunning {
                        ip: my_ip,
                        port,
                        pin,
                        clients: 0,
                    },
                )
                .await;

            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Connection rejected by host",
            ));
        }
    }

    let (start_tx, start_rx) = tokio::sync::oneshot::channel();
    {
        state
            .pending_starts
            .write()
            .await
            .insert(remote_ip.clone(), start_tx);
    }
    state
        .set_status(
            &app,
            SyncStatus::PendingSyncStart {
                ip: remote_ip.clone(),
                device_name: device_name.clone(),
            },
        )
        .await;

    if (start_rx.await).is_err() {
        let my_ip = crate::utils::get_local_ip().unwrap_or_else(|_| "0.0.0.0".to_string());
        let pin = state.pin.read().await.clone().unwrap_or_default();
        state
            .set_status(
                &app,
                SyncStatus::DriverRunning {
                    ip: my_ip,
                    port,
                    pin,
                    clients: 0,
                },
            )
            .await;
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Sync start cancelled",
        ));
    }

    // Main Loop
    while let Some(msg) = framed.next().await {
        match msg {
            Ok(P2PMessage::AdvertiseCursors { cursors }) => {
                handle_advertise_cursors(
                    &app,
                    &mut framed,
                    &peer_device_id,
                    cursors,
                    peer_protocol_version,
                )
                .await?;
            }
            Ok(P2PMessage::Disconnect) => break,
            Ok(other) => log_warn(
                &app,
                "sync_driver",
                format!("Driver received unexpected message: {:?}", other),
            ),
            Err(e) => return Err(e.to_string()),
        }
    }

    Ok(())
}

async fn handle_advertise_cursors(
    app: &AppHandle,
    framed: &mut Framed<TcpStream, P2PCodec>,
    _peer_device_id: &str,
    passenger_cursors: crate::sync::protocol::CursorSet,
    peer_protocol_version: u32,
) -> Result<(), String> {
    let mut conn = crate::storage_manager::db::open_db(app)?;
    sync_db::rebuild_change_log(app, &mut conn)?;

    if peer_protocol_version < PROTOCOL_VERSION {
        let warning = format!(
            "Warning: peer is outdated (v{}). Please update ASAP.",
            peer_protocol_version
        );
        log_warn(app, "sync_driver", warning.clone());
        let state = app.state::<SyncManagerState>();
        state
            .set_status(
                app,
                SyncStatus::Syncing {
                    phase: warning.clone(),
                    progress: None,
                },
            )
            .await;
        framed
            .send(P2PMessage::StatusUpdate(warning))
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    let domains_to_send = passenger_cursors.cursors;
    log_info(app, "sync_driver", "Sending incremental sync changes");

    for cursor in domains_to_send {
        let changes = sync_db::fetch_changes_since(&conn, cursor.domain, cursor.last_change_id)?;
        if changes.is_empty() {
            continue;
        }

        framed
            .send(P2PMessage::StatusUpdate(
                sync_status_text(cursor.domain).to_string(),
            ))
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

        framed
            .send(P2PMessage::PushChanges {
                domain: cursor.domain,
                changes: changes.clone(),
            })
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        if cursor.domain == SyncDomain::Assets {
            send_asset_change_contents(app, framed, &changes).await?;
        }
    }

    framed
        .send(P2PMessage::SyncComplete)
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let state = app.state::<SyncManagerState>();
    state.set_status(app, SyncStatus::SyncCompleted).await;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    Ok(())
}

pub async fn connect_as_passenger(
    app: AppHandle,
    ip: String,
    port: u16,
    pin: String,
) -> Result<(), String> {
    let state = app.state::<SyncManagerState>();
    let mut current_tx = state.shutdown_tx.lock().await;
    if current_tx.is_some() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Sync service is already running",
        ));
    }

    let addr = format!("{}:{}", ip, port);
    let stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let (tx, mut rx) = broadcast::channel(1);
    *current_tx = Some(tx);

    state
        .set_status(&app, SyncStatus::PassengerConnecting)
        .await;

    let app_clone = app.clone();
    tokio::spawn(async move {
        // Re-acquire state here to avoid lifetime issues
        let state = app_clone.state::<SyncManagerState>();

        if let Err(e) = run_passenger_session(app_clone.clone(), stream, &mut rx, pin).await {
            state
                .set_status(
                    &app_clone,
                    SyncStatus::Error {
                        message: e.to_string(),
                    },
                )
                .await;
        } else {
            // Success
            state
                .set_status(&app_clone, SyncStatus::SyncCompleted)
                .await;
        }
    });

    Ok(())
}

async fn run_passenger_session(
    app: AppHandle,
    stream: TcpStream,
    stop_signal: &mut broadcast::Receiver<()>,
    pin: String,
) -> Result<(), String> {
    let mut framed = Framed::new(stream, P2PCodec::new());
    let state = app.state::<SyncManagerState>();

    // 1. Wait for Handshake from Driver (contains Salt + Challenge)
    let (salt, challenge, driver_device_id, driver_protocol_version) = match framed.next().await {
        Some(Ok(P2PMessage::Handshake {
            salt,
            challenge,
            device_id,
            protocol_version,
            ..
        })) => (salt, challenge, device_id, protocol_version),
        Some(Ok(msg)) => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Expected Handshake, got {:?}", msg),
            ))
        }
        Some(Err(e)) => return Err(e.to_string()),
        None => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Connection closed during handshake",
            ))
        }
    };

    // 2. Derive Key & Encrypt Challenge & Send AuthRequest
    let key = derive_key(&pin, &salt);
    let cipher = ChaCha20Poly1305::new(&Key::from(key));

    let mut my_challenge = [0u8; 16];
    thread_rng().fill_bytes(&mut my_challenge);

    let mut nonce_bytes = [0u8; 12];
    thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);

    // Encrypt the Driver's challenge to prove we know the PIN
    let encrypted_challenge_ciphertext = cipher
        .encrypt(&nonce, challenge.as_ref())
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut encrypted_challenge = Vec::new();
    encrypted_challenge.extend_from_slice(&nonce_bytes);
    encrypted_challenge.extend_from_slice(&encrypted_challenge_ciphertext);

    framed
        .send(P2PMessage::AuthRequest {
            encrypted_challenge,
            my_challenge,
        })
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // 3. Wait for AuthResponse
    let encrypted_response = match framed.next().await {
        Some(Ok(P2PMessage::AuthResponse {
            encrypted_challenge,
        })) => encrypted_challenge,
        Some(Ok(msg)) => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Expected AuthResponse, got {:?}", msg),
            ))
        }
        Some(Err(e)) => return Err(e.to_string()),
        None => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Connection closed during auth",
            ))
        }
    };

    // 4. Verify Driver's response to OUR challenge
    if encrypted_response.len() < 12 {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Auth response too short",
        ));
    }
    let mut n_bytes = [0u8; 12];
    n_bytes.copy_from_slice(&encrypted_response[..12]);
    let resp_nonce = Nonce::from(n_bytes);
    let resp_ciphertext = &encrypted_response[12..];

    let decrypted_my_challenge = cipher
        .decrypt(&resp_nonce, resp_ciphertext)
        .map_err(|_| "Auth failed (Driver Sent Bad Response)".to_string())?;

    if decrypted_my_challenge != my_challenge {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Auth failed (response mismatch)",
        ));
    }

    // Auth Success! Enable Encryption.
    framed.codec_mut().set_key(&key);

    // 5. Send our Handshake (Encrypted) with Device Name
    let mut conn = crate::storage_manager::db::open_db(&app)?;
    let local_device_id = sync_db::get_or_create_local_device_id(&conn)?;
    framed
        .send(P2PMessage::Handshake {
            protocol_version: PROTOCOL_VERSION,
            device_name: whoami::devicename(),
            device_id: local_device_id,
            salt: [0u8; 16],      // Not used post-auth
            challenge: [0u8; 16], // Not used post-auth
        })
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    state
        .set_status(
            &app,
            SyncStatus::PassengerConnected {
                driver_ip: "unknown".into(),
            },
        )
        .await;

    sync_db::rebuild_change_log(&app, &mut conn)?;
    let cursors = sync_db::load_peer_cursors(&conn, &driver_device_id)?;
    framed
        .send(P2PMessage::AdvertiseCursors { cursors })
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if driver_protocol_version < PROTOCOL_VERSION {
        let warning = format!(
            "Warning: driver is outdated (v{}). Please update ASAP.",
            driver_protocol_version
        );
        log_warn(&app, "sync_passenger", warning.clone());
        state
            .set_status(
                &app,
                SyncStatus::Syncing {
                    phase: warning,
                    progress: None,
                },
            )
            .await;
    }
    state
        .set_status(
            &app,
            SyncStatus::Syncing {
                phase: "Receiving Data".into(),
                progress: None,
            },
        )
        .await;

    // Client Loop
    loop {
        tokio::select! {
            _ = stop_signal.recv() => {
                framed.send(P2PMessage::Disconnect).await.ok();
                break;
            }
            msg = framed.next() => {
                match msg {
                    Some(Ok(P2PMessage::PushChanges { domain, changes })) => {
                        log_info(&app, "sync_passenger", format!("Received {} changes for {:?}", changes.len(), domain));
                        let last_change_id = changes.last().map(|change| change.change_id).unwrap_or(0);
                        if domain == SyncDomain::Assets {
                            for change in &changes {
                                if change.op == ChangeOp::Delete {
                                    let _ = remove_asset_path(&app, &change.entity_id);
                                }
                            }
                        }
                        if let Err(e) = sync_db::apply_change_batch(&mut conn, domain, &changes) {
                            log_error(&app, "sync_passenger", format!("Failed to apply domain {:?}: {}", domain, e));
                        } else if last_change_id > 0 {
                            let _ = sync_db::record_peer_cursor(&conn, &driver_device_id, domain, last_change_id);
                        }
                    }
                    Some(Ok(P2PMessage::StatusUpdate(msg))) => {
                         log_info(&app, "sync_passenger", format!("StatusUpdate: {}", msg));
                        state.set_status(&app, SyncStatus::Syncing {
                            phase: msg,
                            progress: None,
                        }).await;
                    }
                    Some(Ok(P2PMessage::AssetContent { path, content, .. })) => {
                        log_info(&app, "sync_passenger", format!("Received asset content: {}", path));
                        if let Err(e) = write_asset_path(&app, &path, &content).await {
                            log_error(&app, "sync_passenger", format!("Failed to write asset {}: {}", path, e));
                        }
                    }
                    Some(Ok(P2PMessage::SyncComplete)) => {
                        log_info(&app, "sync_passenger", "Received SyncComplete");
                        state.set_status(&app, SyncStatus::SyncCompleted).await;
                        break;
                    }
                    Some(Ok(P2PMessage::Disconnect)) => {
                        log_info(&app, "sync_passenger", "Received Disconnect");
                        break;
                    }
                    Some(Ok(other)) => {
                        log_info(&app, "sync_passenger", format!("Received unexpected message: {:?}", other));
                    }
                    Some(Err(e)) => {
                        log_error(&app, "sync_passenger", format!("Frame error: {}", e));
                        return Err(e.to_string());
                    }
                    None => {
                        log_info(&app, "sync_passenger", "Stream ended/Connection closed");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn stop_sync(app: AppHandle) -> Result<(), String> {
    let state = app.state::<SyncManagerState>();
    let mut tx_guard = state.shutdown_tx.lock().await;

    if let Some(tx) = tx_guard.take() {
        let _ = tx.send(());
    }

    state.set_status(&app, SyncStatus::Idle).await;
    Ok(())
}

pub async fn approve_connection(app: AppHandle, ip: String, allow: bool) -> Result<(), String> {
    let state = app.state::<SyncManagerState>();
    let mut map = state.pending_approvals.write().await;

    if let Some(tx) = map.remove(&ip) {
        let _ = tx.send(allow);
    } else {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No pending connection found for this IP",
        ));
    }

    Ok(())
}

pub async fn start_sync_session(app: AppHandle, ip: String) -> Result<(), String> {
    let state = app.state::<SyncManagerState>();
    let mut map = state.pending_starts.write().await;

    if let Some(tx) = map.remove(&ip) {
        let _ = tx.send(());
    } else {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No pending sync session found for this IP",
        ));
    }

    Ok(())
}

fn sync_status_text(domain: SyncDomain) -> &'static str {
    match domain {
        SyncDomain::Core => "Syncing Core Data...",
        SyncDomain::Tts => "Syncing Voice Settings...",
        SyncDomain::Lorebooks => "Syncing Lorebooks...",
        SyncDomain::Characters => "Syncing Characters...",
        SyncDomain::Groups => "Syncing Groups...",
        SyncDomain::Sessions => "Syncing Sessions...",
        SyncDomain::Messages => "Syncing Messages...",
        SyncDomain::Assets => "Syncing Assets...",
    }
}

fn resolve_asset_path(app: &AppHandle, relative_path: &str) -> Result<std::path::PathBuf, String> {
    if relative_path.contains("..")
        || relative_path.starts_with('/')
        || relative_path.contains('\\')
    {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid asset path: {}", relative_path),
        ));
    }

    if !relative_path.starts_with("avatars/")
        && !relative_path.starts_with("sessions/")
        && !relative_path.starts_with("images/")
        && !relative_path.starts_with("generated_images/")
    {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unauthorized asset path: {}", relative_path),
        ));
    }

    if relative_path.starts_with("generated_images/") {
        Ok(app
            .path()
            .app_data_dir()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .join(relative_path))
    } else {
        Ok(crate::storage_manager::legacy::storage_root(app)?.join(relative_path))
    }
}

async fn write_asset_path(
    app: &AppHandle,
    relative_path: &str,
    content: &[u8],
) -> Result<(), String> {
    let full_path = resolve_asset_path(app, relative_path)?;

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    tokio::fs::write(&full_path, content)
        .await
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn remove_asset_path(app: &AppHandle, relative_path: &str) -> Result<(), String> {
    let full_path = resolve_asset_path(app, relative_path)?;
    if full_path.exists() {
        std::fs::remove_file(&full_path)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    Ok(())
}

async fn send_asset_change_contents(
    app: &AppHandle,
    framed: &mut Framed<TcpStream, P2PCodec>,
    changes: &[crate::sync::protocol::ChangeRecord],
) -> Result<(), String> {
    for change in changes {
        if change.op != ChangeOp::Upsert {
            continue;
        }

        let asset: sync_db::AssetRecord = bincode::deserialize(&change.payload)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let absolute_path = resolve_asset_path(app, &asset.path)?;
        if !absolute_path.exists() {
            continue;
        }

        let content = tokio::fs::read(&absolute_path)
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        framed
            .send(P2PMessage::AssetContent {
                entity_id: change.entity_id.clone(),
                path: asset.path,
                content_hash: asset.content_hash,
                content,
            })
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    Ok(())
}
