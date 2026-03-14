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
use crate::sync::protocol::{P2PMessage, SyncDomain, SyncManifest};
use crate::utils::{log_error, log_info, log_warn};
use std::path::Path;

const PROTOCOL_VERSION: u32 = 6;

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

    // Send Handshake
    framed
        .send(P2PMessage::Handshake {
            protocol_version: PROTOCOL_VERSION,
            device_name: whoami::devicename(),
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
    let (device_name, peer_protocol_version) = match framed.next().await {
        Some(Ok(P2PMessage::Handshake {
            device_name,
            protocol_version,
            ..
        })) => (device_name, protocol_version),
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
            Ok(P2PMessage::SyncManifest { manifest }) => {
                handle_sync_manifest(&app, &mut framed, manifest, peer_protocol_version).await?;
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

async fn handle_sync_manifest(
    app: &AppHandle,
    framed: &mut Framed<TcpStream, P2PCodec>,
    passenger_manifest: SyncManifest,
    peer_protocol_version: u32,
) -> Result<(), String> {
    let conn = crate::storage_manager::db::open_db(app)?;
    let local_manifest = sync_db::build_sync_manifest(&conn)?;

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

    let passenger_map = passenger_manifest
        .domains
        .into_iter()
        .map(|entry| (entry.domain, entry.fingerprint))
        .collect::<std::collections::HashMap<_, _>>();
    let domains_to_send = local_manifest
        .domains
        .iter()
        .filter(|entry| passenger_map.get(&entry.domain) != Some(&entry.fingerprint))
        .map(|entry| entry.domain)
        .collect::<Vec<_>>();

    log_info(
        app,
        "sync_driver",
        format!("Snapshot diff: {} domain(s) changed", domains_to_send.len()),
    );

    for domain in domains_to_send {
        framed
            .send(P2PMessage::StatusUpdate(
                sync_status_text(domain).to_string(),
            ))
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

        let payload = sync_db::fetch_domain_snapshot(&conn, domain)?;
        framed
            .send(P2PMessage::DomainSnapshot { domain, payload })
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        send_domain_assets(app, framed, domain).await?;
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
    let (salt, challenge, driver_protocol_version) = match framed.next().await {
        Some(Ok(P2PMessage::Handshake {
            salt,
            challenge,
            protocol_version,
            ..
        })) => (salt, challenge, protocol_version),
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
    framed
        .send(P2PMessage::Handshake {
            protocol_version: PROTOCOL_VERSION,
            device_name: whoami::devicename(),
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

    // Prepare Manifest
    let mut conn = crate::storage_manager::db::open_db(&app)?;
    let manifest = sync_db::build_sync_manifest(&conn)?;
    framed
        .send(P2PMessage::SyncManifest { manifest })
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
                    Some(Ok(P2PMessage::DomainSnapshot { domain, payload })) => {
                        log_info(&app, "sync_passenger", format!("Received snapshot for domain: {:?}", domain));
                        if let Err(e) = sync_db::apply_domain_snapshot(&mut conn, domain, &payload) {
                            log_error(&app, "sync_passenger", format!("Failed to apply domain {:?}: {}", domain, e));
                        }
                    }
                    Some(Ok(P2PMessage::StatusUpdate(msg))) => {
                         log_info(&app, "sync_passenger", format!("StatusUpdate: {}", msg));
                        state.set_status(&app, SyncStatus::Syncing {
                            phase: msg,
                            progress: None,
                        }).await;
                    }
                    Some(Ok(P2PMessage::FileTransfer { path, content })) => {
                        log_info(&app, "sync_passenger", format!("Received FileTransfer request: {}", path));

                        if path.contains("..") || path.starts_with("/") || path.contains("\\") {
                            log_warn(&app, "sync_passenger", format!("Security Warning: Attempted path traversal in sync: {}", path));
                            continue;
                        }

                        if !path.starts_with("avatars/")
                            && !path.starts_with("sessions/")
                            && !path.starts_with("images/")
                            && !path.starts_with("generated_images/")
                        {
                            log_warn(&app, "sync_passenger", format!("Security Warning: Attempted write to unauthorized directory: {}", path));
                            continue;
                        }

                        let full_path = if path.starts_with("generated_images/") {
                            match app.path().app_data_dir() {
                                Ok(root) => root.join(&path),
                                Err(e) => {
                                    log_error(&app, "sync_passenger", format!("Failed to get app data dir: {}", e));
                                    continue;
                                }
                            }
                        } else {
                            let root = match crate::utils::lettuce_dir(&app) {
                                Ok(r) => r,
                                Err(e) => {
                                    log_error(&app, "sync_passenger", format!("Failed to get lettuce dir: {}", e));
                                    continue;
                                }
                            };
                            root.join(&path)
                        };

                        log_info(&app, "sync_passenger", format!("Writing file to: {:?}", full_path));

                        if let Some(parent) = full_path.parent() {
                            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                                 log_error(&app, "sync_passenger", format!("Failed to create parent dir for {}: {}", path, e));
                                 continue;
                            }
                        }
                        if let Err(e) = tokio::fs::write(&full_path, content).await {
                             log_error(&app, "sync_passenger", format!("Failed to write file {}: {}", path, e));
                        } else {
                             log_info(&app, "sync_passenger", format!("Successfully wrote file: {}", path));
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
        SyncDomain::Conversations => "Syncing Conversations...",
    }
}

fn collect_text_ids_from_conn(
    conn: &crate::storage_manager::db::DbConnection,
    sql: &str,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

async fn send_domain_assets(
    app: &AppHandle,
    framed: &mut Framed<TcpStream, P2PCodec>,
    domain: SyncDomain,
) -> Result<(), String> {
    match domain {
        SyncDomain::Core => send_global_assets(app, framed).await,
        SyncDomain::Characters => {
            let conn = crate::storage_manager::db::open_db(app)?;
            let ids = collect_text_ids_from_conn(&conn, "SELECT id FROM characters")?;
            send_character_assets(app, framed, &ids).await
        }
        SyncDomain::Groups => {
            send_group_assets(app, framed).await?;

            let conn = crate::storage_manager::db::open_db(app)?;
            let group_ids = collect_text_ids_from_conn(&conn, "SELECT id FROM group_sessions")?;
            if !group_ids.is_empty() {
                send_group_session_assets(app, framed, &group_ids).await?;
            }

            Ok(())
        }
        SyncDomain::Conversations => {
            let session_asset_info = {
                let conn = crate::storage_manager::db::open_db(app)?;
                let mut stmt = conn
                    .prepare("SELECT id, character_id FROM sessions")
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                let rows = stmt
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                rows.collect::<Result<Vec<(String, String)>, _>>()
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            };

            if !session_asset_info.is_empty() {
                send_session_assets(app, framed, &session_asset_info).await?;
            }

            Ok(())
        }
        SyncDomain::Tts | SyncDomain::Lorebooks => Ok(()),
    }
}

async fn send_file(
    app: &AppHandle,
    framed: &mut Framed<TcpStream, P2PCodec>,
    relative_path: String,
    absolute_path: &Path,
) -> Result<(), String> {
    if absolute_path.exists() {
        let content = tokio::fs::read(absolute_path)
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        log_info(
            app,
            "sync_driver",
            format!("Sending file: {} ({} bytes)", relative_path, content.len()),
        );

        framed
            .send(P2PMessage::FileTransfer {
                path: relative_path,
                content,
            })
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    Ok(())
}

async fn send_dir_recursive(
    app: &AppHandle,
    framed: &mut Framed<TcpStream, P2PCodec>,
    absolute_dir: &Path,
    relative_prefix: &str,
) -> Result<(), String> {
    if !absolute_dir.exists() {
        return Ok(());
    }

    let mut stack = vec![(absolute_dir.to_path_buf(), relative_prefix.to_string())];
    while let Some((dir, prefix)) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&dir)
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let name = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_) => continue,
            };
            let rel = format!("{}/{}", prefix, name);
            if path.is_dir() {
                stack.push((path, rel));
            } else if path.is_file() {
                send_file(app, framed, rel, &path).await?;
            }
        }
    }

    Ok(())
}

async fn send_character_assets(
    app: &AppHandle,
    framed: &mut Framed<TcpStream, P2PCodec>,
    char_ids: &[String],
) -> Result<(), String> {
    log_info(
        app,
        "sync_driver",
        format!(
            "Starting send_character_assets for {} chars",
            char_ids.len()
        ),
    );
    let root = crate::storage_manager::legacy::storage_root(app)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let avatars_dir = root.join("avatars");
    let images_dir = root.join("images");

    let conn = crate::storage_manager::db::open_db(app)?;

    for id in char_ids {
        let mut char_dir_name = id.clone();
        let mut char_dir = avatars_dir.join(id);

        if !char_dir.exists() {
            let prefixed = format!("character-{}", id);
            let prefixed_dir = avatars_dir.join(&prefixed);
            if prefixed_dir.exists() {
                char_dir = prefixed_dir;
                char_dir_name = prefixed;
            }
        }

        log_info(
            app,
            "sync_driver",
            format!("Checking avatar dir: {:?}", char_dir),
        );
        if char_dir.exists() {
            let mut entries = tokio::fs::read_dir(&char_dir)
                .await
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                        log_info(
                            app,
                            "sync_driver",
                            format!("Sending avatar file: {}", filename),
                        );
                        send_file(
                            app,
                            framed,
                            format!("avatars/{}/{}", char_dir_name, filename),
                            &path,
                        )
                        .await?;
                    }
                }
            }
        } else {
            log_info(
                app,
                "sync_driver",
                format!(
                    "Avatar dir not found for char {} (checked raw and prefixed)",
                    id
                ),
            );
        }

        let bg_path: Option<String> = conn
            .query_row(
                "SELECT background_image_path FROM characters WHERE id = ?",
                [id],
                |row| row.get(0),
            )
            .unwrap_or(None);

        if let Some(bg_id) = bg_path {
            log_info(app, "sync_driver", format!("Found bg_id: {}", bg_id));
            if !bg_id.is_empty() && !bg_id.starts_with("data:") && !bg_id.starts_with("http") {
                for ext in &["webp", "png", "jpg", "jpeg", "gif"] {
                    let filename = format!("{}.{}", bg_id, ext);
                    let file_path = images_dir.join(&filename);
                    if file_path.exists() {
                        log_info(app, "sync_driver", format!("Sending bg file: {}", filename));
                        send_file(app, framed, format!("images/{}", filename), &file_path).await?;
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn send_session_assets(
    app: &AppHandle,
    framed: &mut Framed<TcpStream, P2PCodec>,
    sessions_with_chars: &[(String, String)],
) -> Result<(), String> {
    log_info(
        app,
        "sync_driver",
        format!(
            "Starting send_session_assets for {} sessions",
            sessions_with_chars.len()
        ),
    );
    let root = crate::storage_manager::legacy::storage_root(app)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for (session_id, char_id) in sessions_with_chars {
        let mut char_dir_name = char_id.clone();
        let mut session_base_dir = root.join("sessions").join(char_id);

        if !session_base_dir.exists() {
            let prefixed = format!("character-{}", char_id);
            let prefixed_base = root.join("sessions").join(&prefixed);
            if prefixed_base.exists() {
                session_base_dir = prefixed_base;
                char_dir_name = prefixed;
            }
        }

        let session_dir = session_base_dir.join(session_id);
        // log_info(app, "sync_driver", format!("Checking session dir: {:?}", session_dir));

        if session_dir.exists() {
            let mut entries = tokio::fs::read_dir(&session_dir)
                .await
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                        log_info(
                            app,
                            "sync_driver",
                            format!("Sending session file: {}", filename),
                        );
                        send_file(
                            app,
                            framed,
                            format!("sessions/{}/{}/{}", char_dir_name, session_id, filename),
                            &path,
                        )
                        .await?;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn send_group_session_assets(
    app: &AppHandle,
    framed: &mut Framed<TcpStream, P2PCodec>,
    group_session_ids: &[String],
) -> Result<(), String> {
    log_info(
        app,
        "sync_driver",
        format!(
            "Starting send_group_session_assets for {} sessions",
            group_session_ids.len()
        ),
    );
    let root = crate::storage_manager::legacy::storage_root(app)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let images_dir = root.join("images");
    let conn = crate::storage_manager::db::open_db(app)?;

    for id in group_session_ids {
        let bg_path: Option<String> = conn
            .query_row(
                "SELECT background_image_path FROM group_sessions WHERE id = ?",
                [id],
                |row| row.get(0),
            )
            .unwrap_or(None);

        if let Some(bg_id) = bg_path {
            if !bg_id.is_empty() && !bg_id.starts_with("data:") && !bg_id.starts_with("http") {
                for ext in &["webp", "png", "jpg", "jpeg", "gif"] {
                    let filename = format!("{}.{}", bg_id, ext);
                    let file_path = images_dir.join(&filename);
                    if file_path.exists() {
                        log_info(
                            app,
                            "sync_driver",
                            format!("Sending group bg file: {}", filename),
                        );
                        send_file(app, framed, format!("images/{}", filename), &file_path).await?;
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn send_group_assets(
    app: &AppHandle,
    framed: &mut Framed<TcpStream, P2PCodec>,
) -> Result<(), String> {
    let root = crate::storage_manager::legacy::storage_root(app)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let images_dir = root.join("images");
    let conn = crate::storage_manager::db::open_db(app)?;

    let background_ids: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT background_image_path FROM group_characters WHERE background_image_path IS NOT NULL AND background_image_path != ''")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    };

    for bg_id in background_ids {
        if bg_id.starts_with("data:") || bg_id.starts_with("http") {
            continue;
        }
        for ext in &["webp", "png", "jpg", "jpeg", "gif"] {
            let filename = format!("{}.{}", bg_id, ext);
            let file_path = images_dir.join(&filename);
            if file_path.exists() {
                send_file(app, framed, format!("images/{}", filename), &file_path).await?;
                break;
            }
        }
    }

    Ok(())
}

async fn send_global_assets(
    app: &AppHandle,
    framed: &mut Framed<TcpStream, P2PCodec>,
) -> Result<(), String> {
    log_info(app, "sync_driver", "Starting send_global_assets (Personas)");
    let root = crate::storage_manager::legacy::storage_root(app)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let avatars_dir = root.join("avatars");
    let conn = crate::storage_manager::db::open_db(app)?;

    let persona_ids: Vec<String> = {
        let stmt_sql = "SELECT id FROM personas";
        let mut stmt = conn
            .prepare(stmt_sql)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        rows.collect::<Result<_, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    };

    for id in persona_ids {
        let mut dir_name = id.clone();
        let mut dir = avatars_dir.join(&id);

        if !dir.exists() {
            let prefixed = format!("persona-{}", id);
            let prefixed_dir = avatars_dir.join(&prefixed);
            if prefixed_dir.exists() {
                dir = prefixed_dir;
                dir_name = prefixed;
            }
        }

        if dir.exists() {
            let mut entries = tokio::fs::read_dir(&dir)
                .await
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                        log_info(
                            app,
                            "sync_driver",
                            format!("Sending persona asset: {}", filename),
                        );
                        send_file(
                            app,
                            framed,
                            format!("avatars/{}/{}", dir_name, filename),
                            &path,
                        )
                        .await?;
                    }
                }
            }
        }
    }

    if let Ok(app_data_dir) = app.path().app_data_dir() {
        let generated_dir = app_data_dir.join("generated_images");
        send_dir_recursive(app, framed, &generated_dir, "generated_images").await?;
    }

    Ok(())
}
