use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use futures::{
    future::join,
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use rand::{thread_rng, RngCore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex, RwLock, Semaphore};
use tokio_util::codec::Framed;
use uuid::Uuid;

use crate::sync::codec::P2PCodec;
use crate::sync::protocol::P2PMessage;
use crate::sync::v2::protocol::{
    validate_hello, BlobDescriptor, BlobRequestDescriptor, SyncHello, SyncLimits,
    SyncV2Message,
    SYNC_V2_PROTOCOL_VERSION,
};
use crate::sync::v2::{
    apply_staged_batch, begin_blob_receive, build_outbound_batch,
    cached_schema_fingerprint, finish_blob_receive, get_or_create_device_id,
    load_frontier, plan_outbound, record_peer_acknowledgement,
    revision_batch_hash, stage_revision_batch, write_blob_chunk, BatchApplyResult,
    BlobReceiveState, ChangeRevision,
};
use crate::utils::{log_error, log_info};

const QUIESCENT_ACK: &str = "quiescent";
const DATABASE_LOCK_RETRY_DELAYS_MS: &[u64] = &[100, 250, 500, 1_000, 2_000];
static SYNC_DATABASE_USERS: AtomicUsize = AtomicUsize::new(0);

struct SyncDatabaseActivityGuard;

impl SyncDatabaseActivityGuard {
    fn begin() -> Self {
        SYNC_DATABASE_USERS.fetch_add(1, Ordering::AcqRel);
        Self
    }
}

impl Drop for SyncDatabaseActivityGuard {
    fn drop(&mut self) {
        SYNC_DATABASE_USERS.fetch_sub(1, Ordering::AcqRel);
    }
}

pub fn is_sync_database_active() -> bool {
    SYNC_DATABASE_USERS.load(Ordering::Acquire) > 0
}

async fn stage_and_apply_batch_with_retry(
    app: &AppHandle,
    peer_device_id: &str,
    batch_id: &str,
    batch_hash: &str,
    revisions: &[ChangeRevision],
    now_ms: i64,
    stop: &mut broadcast::Receiver<()>,
    state: &SyncManagerState,
    stats: &TransferStats,
) -> Result<BatchApplyResult, String> {
    let mut retry = 0usize;
    loop {
        let result = {
            let conn = crate::storage_manager::db::open_db(app)?;
            stage_revision_batch(
                &conn,
                peer_device_id,
                batch_id,
                batch_hash,
                revisions,
                now_ms,
            )
            .and_then(|_| apply_staged_batch(&conn, batch_id, now_ms))
        };
        match result {
            Ok(result) => return Ok(result),
            Err(error)
                if error.is_retryable_lock()
                    && retry < DATABASE_LOCK_RETRY_DELAYS_MS.len() =>
            {
                let delay_ms = DATABASE_LOCK_RETRY_DELAYS_MS[retry];
                retry += 1;
                log_info(
                    app,
                    "sync_v2_driver",
                    format!(
                        "Database busy while applying batch {batch_id}; retry {retry}/{} in {delay_ms}ms",
                        DATABASE_LOCK_RETRY_DELAYS_MS.len()
                    ),
                );
                state
                    .set_status(
                        app,
                        stats.database_wait_status(
                            retry as u64,
                            DATABASE_LOCK_RETRY_DELAYS_MS.len() as u64,
                            delay_ms,
                        ),
                    )
                    .await;
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_millis(delay_ms)) => {}
                    _ = stop.recv() => return Err("Sync cancelled".to_string()),
                }
                state
                    .set_status(app, stats.status("Applying changes"))
                    .await;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

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
    Sharing {
        ip: String,
        port: u16,
        pin: String,
        clients: usize,
    },
    Connecting,
    WaitingForApproval {
        peer: String,
    },
    Syncing {
        phase: String,
        progress: Option<f32>,
        items_done: Option<u64>,
        items_total: Option<u64>,
        bytes_done: Option<u64>,
        bytes_total: Option<u64>,
        items_sent: Option<u64>,
        items_received: Option<u64>,
        bytes_sent: Option<u64>,
        bytes_received: Option<u64>,
        conflicts_detected: Option<u64>,
        branches_created: Option<u64>,
        database_wait_attempt: Option<u64>,
        database_wait_total: Option<u64>,
        database_wait_ms: Option<u64>,
    },
    Error {
        message: String,
    },
    PendingApproval {
        ip: String,
        device_name: String,
    },
    PendingStart {
        ip: String,
        device_name: String,
    },
    Completed,
}

#[derive(Debug, Default)]
struct TransferStats {
    planned_items: u64,
    planned_bytes: u64,
    items_sent: u64,
    items_received: u64,
    bytes_sent: u64,
    bytes_received: u64,
    conflicts_detected: u64,
    branches_created: u64,
}

impl TransferStats {
    fn status(&self, phase: impl Into<String>) -> SyncStatus {
        let items_done = self.items_sent.saturating_add(self.items_received);
        let bytes_done = self.bytes_sent.saturating_add(self.bytes_received);
        let progress = if self.planned_bytes > 0 {
            Some((bytes_done as f32 / self.planned_bytes as f32).clamp(0.0, 1.0))
        } else if self.planned_items > 0 {
            Some((items_done as f32 / self.planned_items as f32).clamp(0.0, 1.0))
        } else {
            None
        };
        SyncStatus::Syncing {
            phase: phase.into(),
            progress,
            items_done: Some(items_done),
            items_total: Some(self.planned_items),
            bytes_done: Some(bytes_done),
            bytes_total: Some(self.planned_bytes),
            items_sent: Some(self.items_sent),
            items_received: Some(self.items_received),
            bytes_sent: Some(self.bytes_sent),
            bytes_received: Some(self.bytes_received),
            conflicts_detected: Some(self.conflicts_detected),
            branches_created: Some(self.branches_created),
            database_wait_attempt: None,
            database_wait_total: None,
            database_wait_ms: None,
        }
    }

    fn database_wait_status(
        &self,
        attempt: u64,
        total: u64,
        delay_ms: u64,
    ) -> SyncStatus {
        let mut status = self.status("Waiting for local database");
        if let SyncStatus::Syncing {
            database_wait_attempt,
            database_wait_total,
            database_wait_ms,
            ..
        } = &mut status
        {
            *database_wait_attempt = Some(attempt);
            *database_wait_total = Some(total);
            *database_wait_ms = Some(delay_ms);
        }
        status
    }
}

pub struct SyncManagerState {
    pub status: RwLock<SyncStatus>,
    shutdown_tx: Mutex<Option<broadcast::Sender<()>>>,
    pub pending_approvals:
        RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    pub pending_starts:
        RwLock<HashMap<String, tokio::sync::oneshot::Sender<()>>>,
    pub pin: RwLock<Option<String>>,
    sharing_port: RwLock<Option<u16>>,
    connection_slot: Semaphore,
}

impl Default for SyncManagerState {
    fn default() -> Self {
        Self {
            status: RwLock::new(SyncStatus::Idle),
            shutdown_tx: Mutex::new(None),
            pending_approvals: RwLock::new(HashMap::new()),
            pending_starts: RwLock::new(HashMap::new()),
            pin: RwLock::new(None),
            sharing_port: RwLock::new(None),
            connection_slot: Semaphore::new(1),
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

    async fn clear_pending(&self) {
        self.pending_approvals.write().await.clear();
        self.pending_starts.write().await.clear();
    }
}

async fn set_sharing_status(
    app: &AppHandle,
    state: &SyncManagerState,
    port: u16,
) {
    let ip = crate::utils::get_local_ip().unwrap_or_else(|_| "0.0.0.0".to_string());
    let pin = state.pin.read().await.clone().unwrap_or_default();
    state
        .set_status(
            app,
            SyncStatus::Sharing {
                ip,
                port,
                pin,
                clients: 0,
            },
        )
        .await;
}

pub async fn share_device(app: AppHandle, _port: u16) -> Result<String, String> {
    let state = app.state::<SyncManagerState>();
    let mut shutdown = state.shutdown_tx.lock().await;
    if shutdown.is_some() {
        let pin = state.pin.read().await.clone();
        drop(shutdown);
        if let Some(pin) = pin {
            if let Some(port) = *state.sharing_port.read().await {
                set_sharing_status(&app, state.inner(), port).await;
            }
            return Ok(pin);
        }
        return Err("A sync session is already running".to_string());
    }

    let listener = TcpListener::bind("0.0.0.0:0")
        .await
        .map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();
    let pin: String = (0..6)
        .map(|_| {
            let mut byte = [0u8; 1];
            thread_rng().fill_bytes(&mut byte);
            (byte[0] % 10).to_string()
        })
        .collect();
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel(4);
    let task_tx = shutdown_tx.clone();
    *shutdown = Some(shutdown_tx);
    *state.pin.write().await = Some(pin.clone());
    *state.sharing_port.write().await = Some(port);
    state.clear_pending().await;
    drop(shutdown);
    set_sharing_status(&app, state.inner(), port).await;

    let app_clone = app.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, address)) => {
                            let connection_app = app_clone.clone();
                            tokio::spawn(async move {
                                if let Err(error) =
                                    handle_driver_connection(connection_app.clone(), stream, address, port).await
                                {
                                    log_error(
                                        &connection_app,
                                        "sync_v2_driver",
                                        format!("Connection failed: {error}"),
                                    );
                                    let state = connection_app.state::<SyncManagerState>();
                                    if state.shutdown_tx.lock().await.is_some() {
                                        set_sharing_status(
                                            &connection_app,
                                            state.inner(),
                                            port,
                                        )
                                        .await;
                                    }
                                }
                            });
                        }
                        Err(error) => {
                            log_error(
                                &app_clone,
                                "sync_v2_driver",
                                format!("Accept failed: {error}"),
                            );
                        }
                    }
                }
            }
        }

        let state = app_clone.state::<SyncManagerState>();
        state.clear_pending().await;
        let should_clear = {
            let mut current = state.shutdown_tx.lock().await;
            match current.as_ref() {
                Some(sender) if sender.same_channel(&task_tx) => {
                    current.take();
                    true
                }
                None => true,
                _ => false,
            }
        };
        if should_clear {
            *state.pin.write().await = None;
            *state.sharing_port.write().await = None;
            state.set_status(&app_clone, SyncStatus::Idle).await;
        }
    });

    Ok(pin)
}

async fn handle_driver_connection(
    app: AppHandle,
    stream: TcpStream,
    _address: SocketAddr,
    port: u16,
) -> Result<(), String> {
    let remote_ip = stream
        .peer_addr()
        .map(|address| address.ip().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let state = app.state::<SyncManagerState>();
    let _connection_slot = match state.connection_slot.try_acquire() {
        Ok(slot) => slot,
        Err(_) => {
            let mut framed = Framed::new(stream, P2PCodec::new());
            let _ = framed
                .send(P2PMessage::Error(
                    "This device is already syncing with another device".to_string(),
                ))
                .await;
            return Ok(());
        }
    };
    let pin = state
        .pin
        .read()
        .await
        .clone()
        .ok_or_else(|| "Sync sharing is not active".to_string())?;
    let mut framed = Framed::new(stream, P2PCodec::new());

    let mut salt = [0u8; 16];
    let mut challenge = [0u8; 16];
    thread_rng().fill_bytes(&mut salt);
    thread_rng().fill_bytes(&mut challenge);
    let local_device_id = {
        let conn = crate::storage_manager::db::open_db(&app)?;
        get_or_create_device_id(&conn).map_err(|error| error.to_string())?
    };
    framed
        .send(P2PMessage::Handshake {
            protocol_version: SYNC_V2_PROTOCOL_VERSION,
            app_version: app.package_info().version.to_string(),
            device_name: whoami::devicename(),
            device_id: local_device_id,
            salt,
            challenge,
        })
        .await
        .map_err(|error| error.to_string())?;

    let (encrypted_challenge, peer_challenge) = match receive_message(&mut framed).await? {
        P2PMessage::AuthRequest {
            encrypted_challenge,
            my_challenge,
        } => (encrypted_challenge, my_challenge),
        message => return Err(format!("Expected authentication request, got {message:?}")),
    };
    let key = derive_key(&pin, &salt);
    let cipher = ChaCha20Poly1305::new(&Key::from(key));
    let decrypted = decrypt_challenge(&cipher, &encrypted_challenge, "authentication request")?;
    if decrypted != challenge {
        return Err("Authentication challenge did not match".to_string());
    }
    let response = encrypt_challenge(&cipher, &peer_challenge)?;
    framed
        .send(P2PMessage::AuthResponse {
            encrypted_challenge: response,
        })
        .await
        .map_err(|error| error.to_string())?;
    framed.codec_mut().set_key(&key);

    let (peer_name, peer_device_id, peer_app_version, peer_protocol) =
        match receive_message(&mut framed).await? {
            P2PMessage::Handshake {
                device_name,
                device_id,
                app_version,
                protocol_version,
                ..
            } => (device_name, device_id, app_version, protocol_version),
            message => return Err(format!("Expected encrypted handshake, got {message:?}")),
        };
    validate_transport_version(&app, &peer_name, &peer_app_version, peer_protocol)?;

    let (approval_tx, approval_rx) = tokio::sync::oneshot::channel();
    state
        .pending_approvals
        .write()
        .await
        .insert(remote_ip.clone(), approval_tx);
    state
        .set_status(
            &app,
            SyncStatus::PendingApproval {
                ip: remote_ip.clone(),
                device_name: peer_name.clone(),
            },
        )
        .await;
    let approved = approval_rx
        .await
        .map_err(|_| "Connection approval was cancelled".to_string())?;
    state.pending_approvals.write().await.remove(&remote_ip);
    if !approved {
        let _ = framed
            .send(P2PMessage::Error("Connection was declined".to_string()))
            .await;
        set_sharing_status(&app, state.inner(), port).await;
        return Ok(());
    }

    let (start_tx, start_rx) = tokio::sync::oneshot::channel();
    state
        .pending_starts
        .write()
        .await
        .insert(remote_ip.clone(), start_tx);
    state
        .set_status(
            &app,
            SyncStatus::PendingStart {
                ip: remote_ip.clone(),
                device_name: peer_name,
            },
        )
        .await;
    start_rx
        .await
        .map_err(|_| "Sync start was cancelled".to_string())?;
    state.pending_starts.write().await.remove(&remote_ip);
    framed
        .send(P2PMessage::Ready)
        .await
        .map_err(|error| error.to_string())?;

    let mut stop = state
        .shutdown_tx
        .lock()
        .await
        .as_ref()
        .ok_or_else(|| "Sync service stopped".to_string())?
        .subscribe();
    match run_v2_replication(&app, framed, &peer_device_id, &mut stop).await {
        Ok(()) => {
            state.set_status(&app, SyncStatus::Completed).await;
            log_info(
                &app,
                "sync_v2_driver",
                format!("Completed sync with {peer_device_id}"),
            );
        }
        Err(error) if error == "Sync cancelled" => {
            state.set_status(&app, SyncStatus::Idle).await;
        }
        Err(error) => {
            log_error(
                &app,
                "sync_v2_driver",
                format!("Replication failed: {error}"),
            );
            state
                .set_status(&app, SyncStatus::Error { message: error })
                .await;
        }
    }
    Ok(())
}

pub async fn connect_device(
    app: AppHandle,
    ip: String,
    port: u16,
    pin: String,
) -> Result<(), String> {
    let state = app.state::<SyncManagerState>();
    let mut shutdown = state.shutdown_tx.lock().await;
    if shutdown.is_some() {
        return Err("A sync session is already running".to_string());
    }
    state
        .set_status(&app, SyncStatus::Connecting)
        .await;
    let stream = match TcpStream::connect(format!("{ip}:{port}")).await {
        Ok(stream) => stream,
        Err(error) => {
            let message = error.to_string();
            state
                .set_status(
                    &app,
                    SyncStatus::Error {
                        message: message.clone(),
                    },
                )
                .await;
            return Err(message);
        }
    };
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel(4);
    let task_tx = shutdown_tx.clone();
    *shutdown = Some(shutdown_tx);
    drop(shutdown);

    let app_clone = app.clone();
    tokio::spawn(async move {
        let result =
            run_passenger_session(app_clone.clone(), stream, &mut shutdown_rx, pin).await;
        let state = app_clone.state::<SyncManagerState>();
        let owns_session = {
            let mut current = state.shutdown_tx.lock().await;
            match current.as_ref() {
                Some(sender) if sender.same_channel(&task_tx) => {
                    current.take();
                    true
                }
                None => true,
                _ => false,
            }
        };
        if !owns_session {
            return;
        }
        match result {
            Ok(()) => {
                state
                    .set_status(&app_clone, SyncStatus::Completed)
                    .await;
            }
            Err(error) if error == "Sync cancelled" => {
                state.set_status(&app_clone, SyncStatus::Idle).await;
            }
            Err(error) => {
                state
                    .set_status(&app_clone, SyncStatus::Error { message: error })
                    .await;
            }
        }
    });
    Ok(())
}

async fn run_passenger_session(
    app: AppHandle,
    stream: TcpStream,
    stop: &mut broadcast::Receiver<()>,
    pin: String,
) -> Result<(), String> {
    let mut framed = Framed::new(stream, P2PCodec::new());
    let (salt, challenge, peer_device_id, peer_name, peer_version, peer_protocol) =
        match receive_message(&mut framed).await? {
            P2PMessage::Handshake {
                salt,
                challenge,
                device_id,
                device_name,
                app_version,
                protocol_version,
            } => (
                salt,
                challenge,
                device_id,
                device_name,
                app_version,
                protocol_version,
            ),
            message => return Err(format!("Expected handshake, got {message:?}")),
        };
    validate_transport_version(&app, &peer_name, &peer_version, peer_protocol)?;

    let key = derive_key(&pin, &salt);
    let cipher = ChaCha20Poly1305::new(&Key::from(key));
    let mut local_challenge = [0u8; 16];
    thread_rng().fill_bytes(&mut local_challenge);
    framed
        .send(P2PMessage::AuthRequest {
            encrypted_challenge: encrypt_challenge(&cipher, &challenge)?,
            my_challenge: local_challenge,
        })
        .await
        .map_err(|error| error.to_string())?;
    let encrypted_response = match receive_message(&mut framed).await? {
        P2PMessage::AuthResponse {
            encrypted_challenge,
        } => encrypted_challenge,
        message => return Err(format!("Expected authentication response, got {message:?}")),
    };
    if decrypt_challenge(&cipher, &encrypted_response, "authentication response")?
        != local_challenge
    {
        return Err("Authentication response did not match".to_string());
    }
    framed.codec_mut().set_key(&key);

    let local_device_id = {
        let conn = crate::storage_manager::db::open_db(&app)?;
        get_or_create_device_id(&conn).map_err(|error| error.to_string())?
    };
    framed
        .send(P2PMessage::Handshake {
            protocol_version: SYNC_V2_PROTOCOL_VERSION,
            app_version: app.package_info().version.to_string(),
            device_name: whoami::devicename(),
            device_id: local_device_id,
            salt: [0; 16],
            challenge: [0; 16],
        })
        .await
        .map_err(|error| error.to_string())?;
    app.state::<SyncManagerState>()
        .set_status(
            &app,
            SyncStatus::WaitingForApproval {
                peer: peer_name,
            },
        )
        .await;
    match receive_message(&mut framed).await? {
        P2PMessage::Ready => {}
        P2PMessage::Error(message) => return Err(message),
        message => return Err(format!("Expected sync approval, got {message:?}")),
    }
    run_v2_replication(&app, framed, &peer_device_id, stop).await
}

async fn run_v2_replication(
    app: &AppHandle,
    mut framed: Framed<TcpStream, P2PCodec>,
    authenticated_peer_device_id: &str,
    stop: &mut broadcast::Receiver<()>,
) -> Result<(), String> {
    let _database_activity = SyncDatabaseActivityGuard::begin();
    let state = app.state::<SyncManagerState>();
    state
        .set_status(app, TransferStats::default().status("Verifying devices"))
        .await;
    let local_hello = local_hello(app)?;
    framed
        .send(P2PMessage::Sync(SyncV2Message::Hello(
            local_hello.clone(),
        )))
        .await
        .map_err(|error| error.to_string())?;
    let peer_hello = match receive_message(&mut framed).await? {
        P2PMessage::Sync(SyncV2Message::Hello(hello)) => hello,
        P2PMessage::Error(message) => return Err(message),
        message => return Err(format!("Expected sync v2 hello, got {message:?}")),
    };
    if peer_hello.device_id != authenticated_peer_device_id {
        return Err("Peer identity changed after authentication".to_string());
    }
    let limits = validate_hello(&local_hello, &peer_hello).map_err(|error| error.to_string())?;

    state
        .set_status(app, TransferStats::default().status("Comparing changes"))
        .await;
    let local_frontier = {
        let conn = crate::storage_manager::db::open_db(app)?;
        load_frontier(&conn).map_err(|error| error.to_string())?
    };
    framed
        .send(P2PMessage::Sync(SyncV2Message::Frontier(
            local_frontier,
        )))
        .await
        .map_err(|error| error.to_string())?;
    let mut remote_frontier = match receive_message(&mut framed).await? {
        P2PMessage::Sync(SyncV2Message::Frontier(frontier)) => frontier,
        message => return Err(format!("Expected sync frontier, got {message:?}")),
    };

    let outbound_plan = {
        let conn = crate::storage_manager::db::open_db(app)?;
        plan_outbound(&conn, &remote_frontier).map_err(|error| error.to_string())?
    };
    framed
        .send(P2PMessage::Sync(SyncV2Message::Plan(
            outbound_plan.clone(),
        )))
        .await
        .map_err(|error| error.to_string())?;
    let inbound_plan = match receive_message(&mut framed).await? {
        P2PMessage::Sync(SyncV2Message::Plan(plan)) => plan,
        message => return Err(format!("Expected sync plan, got {message:?}")),
    };
    let mut stats = TransferStats {
        planned_items: outbound_plan
            .estimated_revisions
            .saturating_add(inbound_plan.estimated_revisions),
        planned_bytes: outbound_plan
            .estimated_bytes
            .saturating_add(inbound_plan.estimated_bytes),
        ..TransferStats::default()
    };
    state
        .set_status(app, stats.status("Exchanging changes"))
        .await;

    let (mut sink, mut stream) = framed.split();
    loop {
        let revisions = {
            let conn = crate::storage_manager::db::open_db(app)?;
            build_outbound_batch(
                &conn,
                &remote_frontier,
                limits.max_revisions_per_batch as usize,
                limits.max_batch_bytes as usize,
            )
            .map_err(|error| error.to_string())?
        };
        let sent_batch_id = if revisions.is_empty() {
            None
        } else {
            Some(Uuid::new_v4().to_string())
        };
        let outgoing = if let Some(batch_id) = &sent_batch_id {
            P2PMessage::Sync(SyncV2Message::RevisionBatch {
                batch_id: batch_id.clone(),
                batch_hash: revision_batch_hash(&revisions)
                    .map_err(|error| error.to_string())?,
                revisions: revisions.clone(),
            })
        } else {
            let frontier = {
                let conn = crate::storage_manager::db::open_db(app)?;
                load_frontier(&conn).map_err(|error| error.to_string())?
            };
            P2PMessage::Sync(SyncV2Message::Quiescent { frontier })
        };

        let incoming = exchange(&mut sink, &mut stream, outgoing, stop).await?;
        let (received_batch_id, received_quiescent) = match incoming {
            P2PMessage::Sync(SyncV2Message::RevisionBatch {
                batch_id,
                batch_hash,
                revisions,
            }) => {
                state.set_status(app, stats.status("Applying changes")).await;
                let now = crate::utils::now_millis()? as i64;
                let result = stage_and_apply_batch_with_retry(
                    app,
                    &peer_hello.device_id,
                    &batch_id,
                    &batch_hash,
                    &revisions,
                    now,
                    stop,
                    state.inner(),
                    &stats,
                )
                .await?;
                stats.items_received = stats
                    .items_received
                    .saturating_add(result.revisions_applied as u64);
                stats.bytes_received = stats.bytes_received.saturating_add(
                    revisions
                        .iter()
                        .map(|revision| revision.changeset.len() as u64)
                        .sum(),
                );
                stats.conflicts_detected = stats
                    .conflicts_detected
                    .saturating_add(result.conflicts_created as u64);
                stats.branches_created = stats
                    .branches_created
                    .saturating_add(result.branches_created as u64);
                if result.branches_created > 0 {
                    state
                        .set_status(app, stats.status("Separating chat branches"))
                        .await;
                }
                (batch_id, false)
            }
            P2PMessage::Sync(SyncV2Message::Quiescent { .. }) => {
                (QUIESCENT_ACK.to_string(), true)
            }
            P2PMessage::Sync(SyncV2Message::Cancel { reason }) => return Err(reason),
            P2PMessage::Error(message) => return Err(message),
            message => return Err(format!("Expected revision batch, got {message:?}")),
        };

        let local_frontier = {
            let conn = crate::storage_manager::db::open_db(app)?;
            load_frontier(&conn).map_err(|error| error.to_string())?
        };
        let ack = P2PMessage::Sync(SyncV2Message::BatchAck {
            batch_id: received_batch_id,
            frontier: local_frontier,
        });
        let peer_ack = exchange(&mut sink, &mut stream, ack, stop).await?;
        let (acknowledged_batch_id, acknowledged_frontier) = match peer_ack {
            P2PMessage::Sync(SyncV2Message::BatchAck { batch_id, frontier }) => {
                (batch_id, frontier)
            }
            P2PMessage::Sync(SyncV2Message::Cancel { reason }) => return Err(reason),
            P2PMessage::Error(message) => return Err(message),
            message => return Err(format!("Expected batch acknowledgement, got {message:?}")),
        };
        let expected_ack = sent_batch_id.as_deref().unwrap_or(QUIESCENT_ACK);
        if acknowledged_batch_id != expected_ack {
            return Err(format!(
                "Peer acknowledged batch {acknowledged_batch_id}, expected {expected_ack}"
            ));
        }
        if sent_batch_id.is_some() {
            stats.items_sent = stats.items_sent.saturating_add(revisions.len() as u64);
            stats.bytes_sent = stats.bytes_sent.saturating_add(
                revisions
                    .iter()
                    .map(|revision| revision.changeset.len() as u64)
                    .sum(),
            );
        }
        remote_frontier = acknowledged_frontier;
        {
            let conn = crate::storage_manager::db::open_db(app)?;
            record_peer_acknowledgement(
                &conn,
                &peer_hello.device_id,
                &remote_frontier,
                crate::utils::now_millis()? as i64,
            )
            .map_err(|error| error.to_string())?;
        }
        state
            .set_status(app, stats.status("Exchanging changes"))
            .await;

        if sent_batch_id.is_none() && received_quiescent {
            break;
        }
    }

    sync_assets(
        app,
        &mut sink,
        &mut stream,
        &local_hello,
        &peer_hello,
        &limits,
        &mut stats,
        stop,
    )
    .await?;
    let frontier = {
        let conn = crate::storage_manager::db::open_db(app)?;
        load_frontier(&conn).map_err(|error| error.to_string())?
    };
    match exchange(
        &mut sink,
        &mut stream,
        P2PMessage::Sync(SyncV2Message::Complete { frontier }),
        stop,
    )
    .await?
    {
        P2PMessage::Sync(SyncV2Message::Complete { .. }) => Ok(()),
        P2PMessage::Sync(SyncV2Message::Cancel { reason }) => Err(reason),
        message => Err(format!("Expected sync completion, got {message:?}")),
    }
}

async fn sync_assets(
    app: &AppHandle,
    sink: &mut SplitSink<Framed<TcpStream, P2PCodec>, P2PMessage>,
    stream: &mut SplitStream<Framed<TcpStream, P2PCodec>>,
    local_hello: &SyncHello,
    peer_hello: &SyncHello,
    limits: &SyncLimits,
    stats: &mut TransferStats,
    stop: &mut broadcast::Receiver<()>,
) -> Result<(), String> {
    let state = app.state::<SyncManagerState>();
    state
        .set_status(app, stats.status("Comparing files"))
        .await;
    let local_assets = crate::sync::v2::assets::collect_local_assets(app)?;
    let local_inventory = local_assets
        .values()
        .map(|asset| asset.descriptor.clone())
        .collect::<Vec<_>>();
    let peer_inventory = match exchange(
        sink,
        stream,
        P2PMessage::Sync(SyncV2Message::BlobInventory {
            blobs: local_inventory,
        }),
        stop,
    )
    .await?
    {
        P2PMessage::Sync(SyncV2Message::BlobInventory { blobs }) => blobs,
        P2PMessage::Sync(SyncV2Message::Cancel { reason }) => return Err(reason),
        message => return Err(format!("Expected blob inventory, got {message:?}")),
    };

    let cache_root = crate::sync::v2::assets::blob_cache_root(app)?;
    let local_path_hashes = local_assets
        .values()
        .flat_map(|asset| {
            asset
                .descriptor
                .relative_paths
                .iter()
                .cloned()
                .map(|path| (path, asset.descriptor.content_hash.clone()))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut requested_descriptors = HashMap::new();
    let mut local_requests = Vec::new();
    for mut descriptor in peer_inventory {
        validate_blob_descriptor(&descriptor)?;
        crate::sync::v2::assets::retain_remote_winners(
            &mut descriptor,
            &local_path_hashes,
        );
        if descriptor.relative_paths.is_empty() {
            continue;
        }
        let already_materialized = descriptor.relative_paths.iter().try_fold(
            true,
            |all_present, path| {
                crate::sync::v2::assets::destination_has_hash(
                    app,
                    path,
                    &descriptor.content_hash,
                )
                .map(|present| all_present && present)
            },
        )?;
        if already_materialized {
            continue;
        }
        let receive_state = {
            let conn = crate::storage_manager::db::open_db(app)?;
            begin_blob_receive(
                &conn,
                &cache_root,
                &descriptor.content_hash,
                descriptor.size_bytes,
                crate::utils::now_millis()? as i64,
            )
            .map_err(|error| error.to_string())?
        };
        match receive_state {
            BlobReceiveState::Complete { path } => {
                crate::sync::v2::assets::materialize_blob(
                    app,
                    &path,
                    &descriptor.relative_paths,
                    &descriptor.content_hash,
                )?;
            }
            BlobReceiveState::Ready { offset } => {
                local_requests.push(BlobRequestDescriptor {
                    content_hash: descriptor.content_hash.clone(),
                    offset,
                });
                requested_descriptors
                    .insert(descriptor.content_hash.clone(), descriptor);
            }
        }
    }
    local_requests.sort_by(|left, right| left.content_hash.cmp(&right.content_hash));
    let peer_requests = match exchange(
        sink,
        stream,
        P2PMessage::Sync(SyncV2Message::BlobRequests {
            requests: local_requests.clone(),
        }),
        stop,
    )
    .await?
    {
        P2PMessage::Sync(SyncV2Message::BlobRequests { mut requests }) => {
            requests.sort_by(|left, right| left.content_hash.cmp(&right.content_hash));
            requests
        }
        P2PMessage::Sync(SyncV2Message::Cancel { reason }) => return Err(reason),
        message => return Err(format!("Expected blob requests, got {message:?}")),
    };

    stats.planned_items = stats
        .planned_items
        .saturating_add(local_requests.len() as u64)
        .saturating_add(peer_requests.len() as u64);
    stats.planned_bytes = stats.planned_bytes.saturating_add(
        local_requests
            .iter()
            .filter_map(|request| {
                requested_descriptors
                    .get(&request.content_hash)
                    .map(|descriptor| descriptor.size_bytes.saturating_sub(request.offset))
            })
            .sum::<u64>(),
    );
    stats.planned_bytes = stats.planned_bytes.saturating_add(
        peer_requests
            .iter()
            .filter_map(|request| local_assets.get(&request.content_hash).map(|asset| {
                asset
                    .descriptor
                    .size_bytes
                    .saturating_sub(request.offset)
            }))
            .sum::<u64>(),
    );
    state
        .set_status(app, stats.status("Exchanging files"))
        .await;

    if local_hello.device_id < peer_hello.device_id {
        send_requested_blobs(
            app,
            sink,
            stream,
            &local_assets,
            &peer_requests,
            limits.blob_chunk_bytes as usize,
            stats,
            stop,
        )
        .await?;
        receive_requested_blobs(
            app,
            sink,
            stream,
            &cache_root,
            &local_requests,
            &requested_descriptors,
            stats,
            stop,
        )
        .await?;
    } else {
        receive_requested_blobs(
            app,
            sink,
            stream,
            &cache_root,
            &local_requests,
            &requested_descriptors,
            stats,
            stop,
        )
        .await?;
        send_requested_blobs(
            app,
            sink,
            stream,
            &local_assets,
            &peer_requests,
            limits.blob_chunk_bytes as usize,
            stats,
            stop,
        )
        .await?;
    }
    Ok(())
}

async fn send_requested_blobs(
    app: &AppHandle,
    sink: &mut SplitSink<Framed<TcpStream, P2PCodec>, P2PMessage>,
    stream: &mut SplitStream<Framed<TcpStream, P2PCodec>>,
    local_assets: &std::collections::BTreeMap<String, crate::sync::v2::assets::LocalAsset>,
    requests: &[BlobRequestDescriptor],
    chunk_bytes: usize,
    stats: &mut TransferStats,
    stop: &mut broadcast::Receiver<()>,
) -> Result<(), String> {
    for request in requests {
        let asset = local_assets
            .get(&request.content_hash)
            .ok_or_else(|| format!("Requested blob {} is unavailable", request.content_hash))?;
        if request.offset > asset.descriptor.size_bytes {
            return Err(format!(
                "Peer requested invalid offset {} for blob {}",
                request.offset, request.content_hash
            ));
        }
        let mut file = File::open(&asset.source_path).map_err(|error| error.to_string())?;
        file.seek(SeekFrom::Start(request.offset))
            .map_err(|error| error.to_string())?;
        let mut offset = request.offset;
        let mut buffer = vec![0u8; chunk_bytes.max(1)];
        loop {
            let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            sink.send(P2PMessage::Sync(SyncV2Message::BlobChunk {
                content_hash: request.content_hash.clone(),
                offset,
                bytes: buffer[..read].to_vec(),
            }))
            .await
            .map_err(|error| error.to_string())?;
            offset = offset.saturating_add(read as u64);
            stats.bytes_sent = stats.bytes_sent.saturating_add(read as u64);
            app.state::<SyncManagerState>()
                .set_status(app, stats.status("Exchanging files"))
                .await;
        }
        sink.send(P2PMessage::Sync(SyncV2Message::BlobComplete {
            content_hash: request.content_hash.clone(),
            size_bytes: asset.descriptor.size_bytes,
        }))
        .await
        .map_err(|error| error.to_string())?;
        match receive_split(stream, stop).await? {
            P2PMessage::Sync(SyncV2Message::BlobAck { content_hash })
                if content_hash == request.content_hash =>
            {
                stats.items_sent = stats.items_sent.saturating_add(1);
            }
            P2PMessage::Sync(SyncV2Message::Cancel { reason }) => return Err(reason),
            message => return Err(format!("Expected blob acknowledgement, got {message:?}")),
        }
    }
    Ok(())
}

async fn receive_requested_blobs(
    app: &AppHandle,
    sink: &mut SplitSink<Framed<TcpStream, P2PCodec>, P2PMessage>,
    stream: &mut SplitStream<Framed<TcpStream, P2PCodec>>,
    cache_root: &std::path::Path,
    requests: &[BlobRequestDescriptor],
    descriptors: &HashMap<String, BlobDescriptor>,
    stats: &mut TransferStats,
    stop: &mut broadcast::Receiver<()>,
) -> Result<(), String> {
    for request in requests {
        let descriptor = descriptors
            .get(&request.content_hash)
            .ok_or_else(|| format!("Missing descriptor for blob {}", request.content_hash))?;
        let mut expected_offset = request.offset;
        loop {
            match receive_split(stream, stop).await? {
                P2PMessage::Sync(SyncV2Message::BlobChunk {
                    content_hash,
                    offset,
                    bytes,
                }) if content_hash == request.content_hash => {
                    if offset != expected_offset {
                        return Err(format!(
                            "Blob {} expected offset {}, received {}",
                            content_hash, expected_offset, offset
                        ));
                    }
                    expected_offset = {
                        let conn = crate::storage_manager::db::open_db(app)?;
                        write_blob_chunk(
                            &conn,
                            cache_root,
                            &content_hash,
                            offset,
                            &bytes,
                        )
                        .map_err(|error| error.to_string())?
                    };
                    stats.bytes_received =
                        stats.bytes_received.saturating_add(bytes.len() as u64);
                    app.state::<SyncManagerState>()
                        .set_status(app, stats.status("Exchanging files"))
                        .await;
                }
                P2PMessage::Sync(SyncV2Message::BlobComplete {
                    content_hash,
                    size_bytes,
                }) if content_hash == request.content_hash => {
                    if size_bytes != descriptor.size_bytes {
                        return Err(format!(
                            "Blob {} completed with inconsistent size",
                            content_hash
                        ));
                    }
                    let path = {
                        let conn = crate::storage_manager::db::open_db(app)?;
                        finish_blob_receive(&conn, cache_root, &content_hash)
                            .map_err(|error| error.to_string())?
                    };
                    crate::sync::v2::assets::materialize_blob(
                        app,
                        &path,
                        &descriptor.relative_paths,
                        &content_hash,
                    )?;
                    sink.send(P2PMessage::Sync(SyncV2Message::BlobAck {
                        content_hash,
                    }))
                    .await
                    .map_err(|error| error.to_string())?;
                    stats.items_received = stats.items_received.saturating_add(1);
                    break;
                }
                P2PMessage::Sync(SyncV2Message::Cancel { reason }) => return Err(reason),
                message => return Err(format!("Expected blob content, got {message:?}")),
            }
        }
    }
    Ok(())
}

fn validate_blob_descriptor(descriptor: &BlobDescriptor) -> Result<(), String> {
    if descriptor.content_hash.len() != 64
        || !descriptor
            .content_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || descriptor.relative_paths.is_empty()
    {
        return Err("Peer advertised an invalid blob descriptor".to_string());
    }
    Ok(())
}

async fn receive_split(
    stream: &mut SplitStream<Framed<TcpStream, P2PCodec>>,
    stop: &mut broadcast::Receiver<()>,
) -> Result<P2PMessage, String> {
    tokio::select! {
        _ = stop.recv() => Err("Sync cancelled".to_string()),
        message = stream.next() => match message {
            Some(Ok(message)) => Ok(message),
            Some(Err(error)) => Err(error.to_string()),
            None => Err("Peer disconnected during sync".to_string()),
        }
    }
}

fn local_hello(app: &AppHandle) -> Result<SyncHello, String> {
    let conn = crate::storage_manager::db::open_db(app)?;
    crate::sync::v2::ensure_current_database_seeded(
        &conn,
        crate::utils::now_millis()? as i64,
    )
    .map_err(|error| error.to_string())?;
    Ok(SyncHello {
        app_version: app.package_info().version.to_string(),
        sync_protocol_version: SYNC_V2_PROTOCOL_VERSION,
        schema_fingerprint: cached_schema_fingerprint(&conn)
            .map_err(|error| error.to_string())?,
        device_id: get_or_create_device_id(&conn).map_err(|error| error.to_string())?,
        session_id: Uuid::new_v4().to_string(),
        limits: SyncLimits::default(),
    })
}

async fn exchange(
    sink: &mut SplitSink<Framed<TcpStream, P2PCodec>, P2PMessage>,
    stream: &mut SplitStream<Framed<TcpStream, P2PCodec>>,
    outgoing: P2PMessage,
    stop: &mut broadcast::Receiver<()>,
) -> Result<P2PMessage, String> {
    let operation = async {
        let (sent, received) = join(sink.send(outgoing), stream.next()).await;
        sent.map_err(|error| error.to_string())?;
        match received {
            Some(Ok(message)) => Ok(message),
            Some(Err(error)) => Err(error.to_string()),
            None => Err("Peer disconnected during sync".to_string()),
        }
    };
    tokio::select! {
        _ = stop.recv() => Err("Sync cancelled".to_string()),
        result = operation => result,
    }
}

async fn receive_message(
    framed: &mut Framed<TcpStream, P2PCodec>,
) -> Result<P2PMessage, String> {
    match framed.next().await {
        Some(Ok(message)) => Ok(message),
        Some(Err(error)) => Err(error.to_string()),
        None => Err("Peer disconnected".to_string()),
    }
}

fn validate_transport_version(
    app: &AppHandle,
    peer_name: &str,
    peer_app_version: &str,
    peer_protocol: u32,
) -> Result<(), String> {
    let local_version = app.package_info().version.to_string();
    if peer_app_version == local_version && peer_protocol == SYNC_V2_PROTOCOL_VERSION {
        return Ok(());
    }
    let peer = if peer_name.is_empty() {
        "The other device"
    } else {
        peer_name
    };
    Err(format!(
        "Sync requires exactly the same LettuceAI version on both devices. {peer} uses app {peer_app_version} with sync protocol {peer_protocol}; this device uses app {local_version} with sync protocol {SYNC_V2_PROTOCOL_VERSION}."
    ))
}

fn encrypt_challenge(
    cipher: &ChaCha20Poly1305,
    challenge: &[u8; 16],
) -> Result<Vec<u8>, String> {
    let mut nonce_bytes = [0u8; 12];
    thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, challenge.as_ref())
        .map_err(|error| error.to_string())?;
    let mut payload = Vec::with_capacity(12 + ciphertext.len());
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&ciphertext);
    Ok(payload)
}

fn decrypt_challenge(
    cipher: &ChaCha20Poly1305,
    payload: &[u8],
    label: &str,
) -> Result<Vec<u8>, String> {
    if payload.len() < 12 {
        return Err(format!("{label} is too short"));
    }
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&payload[..12]);
    cipher
        .decrypt(&Nonce::from(nonce_bytes), &payload[12..])
        .map_err(|_| format!("{label} could not be decrypted"))
}

pub async fn stop_sync(app: AppHandle) -> Result<(), String> {
    let state = app.state::<SyncManagerState>();
    if let Some(sender) = state.shutdown_tx.lock().await.take() {
        let _ = sender.send(());
    }
    state.clear_pending().await;
    *state.pin.write().await = None;
    *state.sharing_port.write().await = None;
    state.set_status(&app, SyncStatus::Idle).await;
    Ok(())
}

pub async fn approve_connection(
    app: AppHandle,
    ip: String,
    allow: bool,
) -> Result<(), String> {
    let state = app.state::<SyncManagerState>();
    let sender = state
        .pending_approvals
        .write()
        .await
        .remove(&ip)
        .ok_or_else(|| "No pending connection exists for this address".to_string())?;
    let _ = sender.send(allow);
    Ok(())
}

pub async fn start_sync_session(app: AppHandle, ip: String) -> Result<(), String> {
    let state = app.state::<SyncManagerState>();
    let sender = state
        .pending_starts
        .write()
        .await
        .remove(&ip)
        .ok_or_else(|| "No pending sync session exists for this address".to_string())?;
    let _ = sender.send(());
    Ok(())
}
