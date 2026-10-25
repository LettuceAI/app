pub(super) mod conflicts;
#[cfg(test)]
mod tests;

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_jobs::handle::CancellationToken;
use lettuce_sync::{
    CausalFrontier, ConversationForkRepository, SyncBatchAcknowledgement, SyncChangeFrame,
    SyncDeviceId, SyncHello, SyncSessionId, SyncTransferLimits,
};
use tokio::sync::{oneshot, watch};

use super::ApiContext;
use crate::{
    AuthenticatedSyncTransport, AuthenticatedTcpSyncTransport, PairingPin, SyncExchangeOutcome,
    SyncPeerApprover, SyncPeerTransportError, SyncTcpListener, SyncTransportError,
    connect_authenticated_sync,
};

pub(super) struct SyncState {
    session: Mutex<Session>,
    status: watch::Sender<dto::SyncStatus>,
}

struct Session {
    active: Option<Active>,
    read_deadline: Duration,
    last_progress: Option<tokio::time::Instant>,
    progress_pending: bool,
}

struct Active {
    id: SyncSessionId,
    cancellation: CancellationToken,
    approval: Option<oneshot::Sender<bool>>,
    begin: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            session: Mutex::new(Session {
                active: None,
                read_deadline: Duration::from_secs(60),
                last_progress: None,
                progress_pending: false,
            }),
            status: watch::channel(dto::SyncStatus::Idle).0,
        }
    }
}

impl SyncState {
    fn lock(&self) -> MutexGuard<'_, Session> {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub(super) fn subscribe(&self) -> watch::Receiver<dto::SyncStatus> {
        self.status.subscribe()
    }
    pub(super) fn cancel(&self) {
        if let Some(active) = self.lock().active.as_ref() {
            active.cancellation.cancel();
        }
    }
    #[cfg(test)]
    pub(super) fn set_test_read_deadline(&self, duration: Duration) {
        self.lock().read_deadline = duration;
    }
}

fn failure(code: ApiErrorCode, reason: dto::SyncFailure) -> ApiError {
    ApiError {
        code,
        message: format!("Sync failed: {reason:?}"),
        details: Some(dto::ApiErrorDetails::Sync { reason }),
    }
}

fn transport_error(error: SyncPeerTransportError) -> ApiError {
    use SyncPeerTransportError as E;
    let (code, reason) = match error {
        E::Cancelled => (ApiErrorCode::Cancelled, dto::SyncFailure::InvalidState),
        E::AuthenticationFailed => (
            ApiErrorCode::InvalidInput,
            dto::SyncFailure::AuthenticationFailed,
        ),
        E::InvalidPin => (
            ApiErrorCode::InvalidInput,
            dto::SyncFailure::AuthenticationFailed,
        ),
        E::PairingLocked => (ApiErrorCode::Unavailable, dto::SyncFailure::PairingLocked),
        E::Declined => (ApiErrorCode::Cancelled, dto::SyncFailure::Declined),
        E::Disconnected | E::Io(_) | E::Listener(_) => (ApiErrorCode::Unavailable, dto::SyncFailure::Disconnected),
        E::PeerStalled => (ApiErrorCode::Unavailable, dto::SyncFailure::PeerStalled),
        _ => (ApiErrorCode::Unsupported, dto::SyncFailure::Protocol),
    };
    failure(code, reason)
}

fn exchange_transport(error: SyncTransportError) -> ApiError {
    let reason = match error {
        SyncTransportError::Cancelled => dto::SyncFailure::InvalidState,
        SyncTransportError::Disconnected => dto::SyncFailure::Disconnected,
        SyncTransportError::PeerStalled => dto::SyncFailure::PeerStalled,
        SyncTransportError::Declined => dto::SyncFailure::Declined,
        SyncTransportError::Protocol => dto::SyncFailure::Protocol,
    };
    failure(ApiErrorCode::Unavailable, reason)
}

fn publish(context: &ApiContext, status: dto::SyncStatus) {
    if *context.sync_state().status.borrow() == status {
        return;
    }
    context.emit(dto::ApiEvent::SyncStatusChanged {
        status: status.clone(),
    });
    context.sync_state().status.send_replace(status);
}

fn phase(context: &ApiContext, id: SyncSessionId, phase: dto::SyncPhase) {
    let mut session = context.sync_state().lock();
    if !session
        .active
        .as_ref()
        .is_some_and(|active| active.id == id && !active.cancellation.is_cancelled())
    {
        return;
    }
    if matches!(&*context.sync_state().status.borrow(), dto::SyncStatus::Syncing {phase: current, ..} if *current == phase)
    {
        return;
    }
    session.progress_pending = false;
    publish(
        context,
        dto::SyncStatus::Syncing {
            phase,
            progress: dto::SyncProgress {
                items_done: 0,
                items_total: None,
                bytes_done: 0,
                bytes_total: None,
            },
        },
    );
}

fn progress(
    context: &ApiContext,
    id: SyncSessionId,
    phase: dto::SyncPhase,
    items: u64,
    bytes: u64,
) {
    let mut session = context.sync_state().lock();
    if !session
        .active
        .as_ref()
        .is_some_and(|active| active.id == id && !active.cancellation.is_cancelled())
    {
        return;
    }
    let status = dto::SyncStatus::Syncing {
        phase,
        progress: dto::SyncProgress {
            items_done: items,
            items_total: None,
            bytes_done: bytes,
            bytes_total: None,
        },
    };
    if *context.sync_state().status.borrow() == status {
        return;
    }
    context.sync_state().status.send_replace(status.clone());
    let now = tokio::time::Instant::now();
    if session
        .last_progress
        .is_none_or(|last| now.duration_since(last) >= Duration::from_millis(250))
    {
        session.last_progress = Some(now);
        session.progress_pending = false;
        context.emit(dto::ApiEvent::SyncStatusChanged { status });
    } else {
        session.progress_pending = true;
    }
}

fn progress_deadline(context: &ApiContext, id: SyncSessionId) -> Option<tokio::time::Instant> {
    let session = context.sync_state().lock();
    if session.progress_pending
        && session
            .active
            .as_ref()
            .is_some_and(|active| active.id == id && !active.cancellation.is_cancelled())
    {
        session
            .last_progress
            .map(|last| last + Duration::from_millis(250))
    } else {
        None
    }
}

fn flush_progress(context: &ApiContext, id: SyncSessionId) {
    let mut session = context.sync_state().lock();
    if session.progress_pending
        && session
            .active
            .as_ref()
            .is_some_and(|active| active.id == id && !active.cancellation.is_cancelled())
    {
        session.last_progress = Some(tokio::time::Instant::now());
        session.progress_pending = false;
        context.emit(dto::ApiEvent::SyncStatusChanged {
            status: context.sync_state().status.borrow().clone(),
        });
    }
}

fn available(context: &ApiContext) -> Result<(), ApiError> {
    if context.shutdown_token().is_cancelled() {
        return Err(failure(
            ApiErrorCode::Cancelled,
            dto::SyncFailure::InvalidState,
        ));
    }
    if context.sync_state().lock().active.is_some() {
        return Err(failure(ApiErrorCode::Busy, dto::SyncFailure::InvalidState));
    }
    Ok(())
}

async fn hello(context: &ApiContext, id: SyncSessionId, name: String) -> Result<SyncHello, ApiError> {
    context
        .blocking(move |context| {
            let mut name = name;
            name.retain(|character| !character.is_control());
            let mut end = name.len().min(128);
            while !name.is_char_boundary(end) {
                end -= 1;
            }
            name.truncate(end);
            if name.trim().is_empty() {
                name = "Device".into();
            }
            context
                .backend()
                .sync_hello()
                .build(
                    env!("CARGO_PKG_VERSION"),
                    name,
                    id,
                    SyncTransferLimits::default(),
                    context.now(),
                )
                .map_err(|_| failure(ApiErrorCode::Unavailable, dto::SyncFailure::Storage))
        })
        .await
}

pub async fn sync_status(context: &ApiContext) -> Result<dto::SyncStatus, ApiError> {
    Ok(context.sync_state().status.borrow().clone())
}

fn ordered_addresses(mut addresses: Vec<Ipv4Addr>) -> Vec<Ipv4Addr> {
    addresses.sort_by_key(|ip| (!ip.is_private(), *ip));
    addresses.dedup();
    addresses
}

pub async fn sync_share_start(context: &ApiContext) -> Result<dto::SyncSharing, ApiError> {
    share_start_named(context, whoami::devicename()).await
}

pub(super) async fn share_start_named(
    context: &ApiContext,
    name: String,
) -> Result<dto::SyncSharing, ApiError> {
    available(context)?;
    let lease = context
        .maintenance()
        .try_work()
        .ok_or_else(|| failure(ApiErrorCode::Busy, dto::SyncFailure::InvalidState))?;
    let id = SyncSessionId::new();
    let hello = hello(context, id, name).await?;
    let interfaces = if_addrs::get_if_addrs().map_err(|_| {
        failure(
            ApiErrorCode::Unavailable,
            dto::SyncFailure::AddressesUnavailable,
        )
    })?;
    let addresses = ordered_addresses(
        interfaces
            .into_iter()
            .filter_map(|interface| match interface.ip() {
                std::net::IpAddr::V4(ip) if !ip.is_loopback() => Some(ip),
                _ => None,
            })
            .collect(),
    );
    if addresses.is_empty() {
        return Err(failure(
            ApiErrorCode::Unavailable,
            dto::SyncFailure::AddressesUnavailable,
        ));
    }
    let listener = SyncTcpListener::bind(
        SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
        hello.device_id(),
    )
    .await
    .map_err(transport_error)?;
    let sharing = dto::SyncSharing {
        addresses: addresses.into_iter().map(|ip| ip.to_string()).collect(),
        port: listener.local_addr().map_err(transport_error)?.port(),
        pin: listener.pin().expose().to_owned(),
    };
    start(
        context,
        id,
        hello,
        Endpoint::Host(listener),
        lease,
        dto::SyncStatus::Sharing {
            addresses: sharing.addresses.clone(),
            port: sharing.port,
            pin: sharing.pin.clone(),
        },
    )?;
    Ok(sharing)
}

pub async fn sync_connect(
    context: &ApiContext,
    request: dto::SyncConnectRequest,
) -> Result<dto::SyncStatus, ApiError> {
    available(context)?;
    let lease = context
        .maintenance()
        .try_work()
        .ok_or_else(|| failure(ApiErrorCode::Busy, dto::SyncFailure::InvalidState))?;
    let ip = request
        .address
        .parse::<Ipv4Addr>()
        .map_err(|_| super::error::invalid_field("address", "An IPv4 address is required"))?;
    if request.port == 0 {
        return Err(super::error::invalid_field(
            "port",
            "A nonzero port is required",
        ));
    }
    let pin = PairingPin::new(request.pin).map_err(transport_error)?;
    let id = SyncSessionId::new();
    let hello = hello(context, id, whoami::devicename()).await?;
    start(
        context,
        id,
        hello,
        Endpoint::Client(SocketAddr::from((ip, request.port)), pin),
        lease,
        dto::SyncStatus::Connecting,
    )?;
    sync_status(context).await
}

enum Endpoint {
    Host(SyncTcpListener),
    Client(SocketAddr, PairingPin),
}

fn start(
    context: &ApiContext,
    id: SyncSessionId,
    hello: SyncHello,
    endpoint: Endpoint,
    lease: tokio::sync::OwnedRwLockReadGuard<()>,
    status: dto::SyncStatus,
) -> Result<(), ApiError> {
    let mut session = context.sync_state().lock();
    if context.shutdown_token().is_cancelled() {
        return Err(failure(
            ApiErrorCode::Cancelled,
            dto::SyncFailure::InvalidState,
        ));
    }
    if session.active.is_some() {
        return Err(failure(ApiErrorCode::Busy, dto::SyncFailure::InvalidState));
    }
    let cancellation = CancellationToken::new();
    let (begin, begin_rx) = oneshot::channel();
    let read_deadline = session.read_deadline;
    session.active = Some(Active {
        id,
        cancellation: cancellation.clone(),
        approval: None,
        begin: Some(begin),
        task: None,
    });
    session.last_progress = None;
    session.progress_pending = false;
    publish(context, status);
    let context = context.clone();
    let task_context = context.clone();
    let task = tokio::spawn(async move {
        let token = cancellation.clone();
        let work = task_context.blocking(move |context| {
            tokio::runtime::Handle::current().block_on(run(
                context,
                id,
                hello,
                endpoint,
                begin_rx,
                &token,
                read_deadline,
            ))
        });
        tokio::pin!(work);
        let mut statuses = task_context.sync_state().subscribe();
        let mut deadline = None;
        let mut shutdown_seen = false;
        let result = loop {
            tokio::select! {
                biased;
                () = task_context.shutdown_token().cancelled(), if !shutdown_seen => { cancellation.cancel(); shutdown_seen = true; },
                changed = statuses.changed() => {
                    if changed.is_ok() { deadline = progress_deadline(&task_context, id); }
                },
                () = async { match deadline { Some(at) => tokio::time::sleep_until(at).await, None => std::future::pending().await } } => {
                    flush_progress(&task_context, id);
                    deadline = None;
                },
                result = &mut work => break result,
            }
        };
        drop(lease);
        let mut session = task_context.sync_state().lock();
        if session
            .active
            .as_ref()
            .is_some_and(|active| active.id == id)
        {
            session.active = None;
            let status = if cancellation.is_cancelled() {
                dto::SyncStatus::Idle
            } else {
                match result {
                    Ok(status) => status,
                    Err(reason) => dto::SyncStatus::Failed { reason },
                }
            };
            publish(&task_context, status);
        }
    });
    session.active.as_mut().expect("active session").task = Some(task);
    Ok(())
}

pub async fn sync_share_approve(
    context: &ApiContext,
    request: dto::SyncShareApproveRequest,
) -> Result<dto::SyncStatus, ApiError> {
    let mut session = context.sync_state().lock();
    if context.shutdown_token().is_cancelled() {
        return Err(failure(
            ApiErrorCode::Cancelled,
            dto::SyncFailure::InvalidState,
        ));
    }
    let dto::SyncStatus::PendingApproval { device_name } =
        context.sync_state().status.borrow().clone()
    else {
        return Err(failure(
            ApiErrorCode::Conflict,
            dto::SyncFailure::InvalidState,
        ));
    };
    let active = session
        .active
        .as_mut()
        .ok_or_else(|| failure(ApiErrorCode::Conflict, dto::SyncFailure::InvalidState))?;
    if active.cancellation.is_cancelled() {
        return Err(failure(
            ApiErrorCode::Cancelled,
            dto::SyncFailure::InvalidState,
        ));
    }
    let sender = active
        .approval
        .take()
        .ok_or_else(|| failure(ApiErrorCode::Conflict, dto::SyncFailure::InvalidState))?;
    if request.approve {
        publish(context, dto::SyncStatus::PendingStart { device_name });
    }
    sender
        .send(request.approve)
        .map_err(|_| failure(ApiErrorCode::Conflict, dto::SyncFailure::InvalidState))?;
    Ok(context.sync_state().status.borrow().clone())
}

pub async fn sync_share_begin(context: &ApiContext) -> Result<dto::SyncStatus, ApiError> {
    let mut session = context.sync_state().lock();
    if context.shutdown_token().is_cancelled() {
        return Err(failure(
            ApiErrorCode::Cancelled,
            dto::SyncFailure::InvalidState,
        ));
    }
    if !matches!(
        *context.sync_state().status.borrow(),
        dto::SyncStatus::PendingStart { .. }
    ) {
        return Err(failure(
            ApiErrorCode::Conflict,
            dto::SyncFailure::InvalidState,
        ));
    }
    let active = session
        .active
        .as_mut()
        .ok_or_else(|| failure(ApiErrorCode::Conflict, dto::SyncFailure::InvalidState))?;
    if active.cancellation.is_cancelled() {
        return Err(failure(
            ApiErrorCode::Cancelled,
            dto::SyncFailure::InvalidState,
        ));
    }
    active
        .begin
        .take()
        .ok_or_else(|| failure(ApiErrorCode::Conflict, dto::SyncFailure::InvalidState))?
        .send(())
        .map_err(|_| failure(ApiErrorCode::Conflict, dto::SyncFailure::InvalidState))?;
    Ok(context.sync_state().status.borrow().clone())
}

pub async fn sync_stop(context: &ApiContext) -> Result<dto::SyncStatus, ApiError> {
    context.stop_sync_session().await;
    Ok(dto::SyncStatus::Idle)
}

impl ApiContext {
    pub async fn stop_sync_session(&self) {
        let mut status = self.sync_state().subscribe();
        let (stopped_id, task) = {
            let mut session = self.sync_state().lock();
            match session.active.as_mut() {
                Some(active) => {
                    active.cancellation.cancel();
                    (Some(active.id), active.task.take())
                }
                None => (None, None),
            }
        };
        if let Some(task) = task {
            let _ = task.await;
        }
        loop {
            let idle = {
                let session = self.sync_state().lock();
                if session.active.is_none() {
                    publish(self, dto::SyncStatus::Idle);
                    true
                } else {
                    session.active.as_ref().map(|active| active.id) != stopped_id
                }
            };
            if idle {
                break;
            }
            if status.changed().await.is_err() {
                break;
            }
        }
    }
}

struct Approver<'a> {
    context: &'a ApiContext,
    id: SyncSessionId,
}

#[async_trait]
impl SyncPeerApprover for Approver<'_> {
    async fn approve(&self, _: SyncDeviceId, device_name: &str) -> bool {
        let (sender, receiver) = oneshot::channel();
        {
            let mut session = self.context.sync_state().lock();
            let Some(active) = session
                .active
                .as_mut()
                .filter(|active| active.id == self.id && !active.cancellation.is_cancelled())
            else {
                return false;
            };
            active.approval = Some(sender);
            publish(
                self.context,
                dto::SyncStatus::PendingApproval {
                    device_name: device_name.to_owned(),
                },
            );
        }
        receiver.await.unwrap_or(false)
    }
}

struct GatedTransport<'a> {
    inner: AuthenticatedTcpSyncTransport<'a>,
    context: &'a ApiContext,
    id: SyncSessionId,
    begin: Option<oneshot::Receiver<()>>,
    read_deadline: Duration,
}

#[async_trait]
impl AuthenticatedSyncTransport for GatedTransport<'_> {
    fn authenticated_peer(&self) -> SyncDeviceId {
        self.inner.authenticated_peer()
    }
    async fn exchange_hello(
        &mut self,
        local: SyncHello,
        cancellation: &CancellationToken,
    ) -> Result<SyncHello, SyncTransportError> {
        let hello = self.inner.exchange_hello(local, cancellation).await?;
        if let Some(mut begin) = self.begin.take() {
            tokio::select! {
                biased;
                () = cancellation.cancelled() => return Err(SyncTransportError::Cancelled),
                result = &mut begin => result.map_err(|_| SyncTransportError::Cancelled)?,
                result = self.inner.wait_for_disconnect(cancellation) => return Err(match result.err().unwrap_or(SyncPeerTransportError::Protocol) { SyncPeerTransportError::Disconnected | SyncPeerTransportError::Io(_) => SyncTransportError::Disconnected, _ => SyncTransportError::Protocol }),
            }
            self.inner.enable_read_deadline(self.read_deadline);
            phase(self.context, self.id, dto::SyncPhase::Changes);
        }
        Ok(hello)
    }
    async fn exchange_frontier(
        &mut self,
        local: CausalFrontier,
        cancellation: &CancellationToken,
    ) -> Result<CausalFrontier, SyncTransportError> {
        let frontier = self.inner.exchange_frontier(local, cancellation).await?;
        self.inner.enable_read_deadline(self.read_deadline);
        phase(self.context, self.id, dto::SyncPhase::Changes);
        Ok(frontier)
    }
    async fn exchange_changes(
        &mut self,
        local: SyncChangeFrame,
        cancellation: &CancellationToken,
    ) -> Result<SyncChangeFrame, SyncTransportError> {
        self.inner.exchange_changes(local, cancellation).await
    }
    async fn exchange_acknowledgement(
        &mut self,
        local: SyncBatchAcknowledgement,
        cancellation: &CancellationToken,
    ) -> Result<SyncBatchAcknowledgement, SyncTransportError> {
        self.inner
            .exchange_acknowledgement(local, cancellation)
            .await
    }
}

async fn run(
    context: &ApiContext,
    id: SyncSessionId,
    hello: SyncHello,
    endpoint: Endpoint,
    begin: oneshot::Receiver<()>,
    cancellation: &CancellationToken,
    read_deadline: Duration,
) -> Result<dto::SyncStatus, ApiError> {
    let root = context.app_folder().ok_or_else(|| {
        failure(
            ApiErrorCode::Unavailable,
            dto::SyncFailure::MediaUnavailable,
        )
    })?;
    let database = context.backend().database();
    let media = lettuce_media::LocalSyncMediaStore::open(
        root.join("platform-v2/media-blobs"),
        database,
        database,
    )
    .map_err(|_| {
        failure(
            ApiErrorCode::Unavailable,
            dto::SyncFailure::MediaUnavailable,
        )
    })?;
    let approver = Approver { context, id };
    let (inner, begin) = match endpoint {
        Endpoint::Host(listener) => {
            let transport = loop {
                match listener.accept(Some(&media), &approver, cancellation).await {
                    Err(
                        SyncPeerTransportError::AuthenticationFailed
                        | SyncPeerTransportError::Disconnected
                        | SyncPeerTransportError::Io(_)
                        | SyncPeerTransportError::Protocol,
                    ) => continue,
                    result => break result.map_err(transport_error)?,
                }
            };
            (transport, Some(begin))
        }
        Endpoint::Client(address, pin) => (
            connect_authenticated_sync(
                address,
                hello.device_id(),
                &pin,
                Some(&media),
                cancellation,
            )
            .await
            .map_err(transport_error)?,
            None,
        ),
    };
    let device = hello.device_id();
    let mut transport = GatedTransport {
        inner,
        context,
        id,
        begin,
        read_deadline,
    };
    let changes = context
        .backend()
        .sync_exchange()
        .run_with_progress(
            hello,
            &mut transport,
            cancellation,
            context.now(),
            |report| {
                progress(
                    context,
                    id,
                    dto::SyncPhase::Changes,
                    (report.sent + report.received) as u64,
                    0,
                )
            },
        )
        .await
        .map_err(|error| match error {
            crate::SyncExchangeError::Transport(error) => exchange_transport(error),
            _ => failure(ApiErrorCode::Unavailable, dto::SyncFailure::Storage),
        })?;
    let changes = match changes {
        SyncExchangeOutcome::Complete(report) => report,
        SyncExchangeOutcome::Pending { batch_id, .. } => {
            return Err(failure(
                ApiErrorCode::Unavailable,
                dto::SyncFailure::PendingBatch {
                    batch_id: batch_id.to_string(),
                },
            ));
        }
    };
    phase(context, id, dto::SyncPhase::Media);
    let media_report = context
        .backend()
        .sync_media(&media)
        .run_with_progress(
            &mut transport.inner,
            cancellation,
            context.now(),
            |report| {
                progress(
                    context,
                    id,
                    dto::SyncPhase::Media,
                    (report.received_assets + report.unavailable_assets) as u64,
                    report.received_bytes,
                )
            },
        )
        .await
        .map_err(|error| {
            let reason = match error {
                crate::SyncMediaExchangeError::Transport(
                    crate::MediaSyncTransportError::Disconnected,
                ) => dto::SyncFailure::Disconnected,
                crate::SyncMediaExchangeError::Transport(
                    crate::MediaSyncTransportError::PeerStalled,
                ) => dto::SyncFailure::PeerStalled,
                _ => dto::SyncFailure::MediaUnavailable,
            };
            failure(ApiErrorCode::Unavailable, reason)
        })?;
    phase(context, id, dto::SyncPhase::Secrets);
    transport
        .inner
        .serve_secrets(context.secret_store().as_ref());
    let secrets = context
        .backend()
        .sync_secrets(context.secret_store().as_ref())
        .run_with_progress(
            &mut transport.inner,
            device,
            cancellation,
            context.now(),
            |report| {
                progress(
                    context,
                    id,
                    dto::SyncPhase::Secrets,
                    (report.received + report.skipped) as u64,
                    0,
                )
            },
        )
        .await
        .map_err(|error| {
            let reason = match error {
                crate::SyncSecretExchangeError::Transport(
                    crate::SecretSyncTransportError::Disconnected,
                ) => dto::SyncFailure::Disconnected,
                crate::SyncSecretExchangeError::Transport(
                    crate::SecretSyncTransportError::PeerStalled,
                ) => dto::SyncFailure::PeerStalled,
                _ => dto::SyncFailure::Storage,
            };
            failure(ApiErrorCode::Unavailable, reason)
        })?;
    let forks = database
        .unresolved_conversation_forks()
        .map_err(|_| failure(ApiErrorCode::Unavailable, dto::SyncFailure::Storage))?
        .len();
    context.emit(dto::ApiEvent::RequiredModelsChanged);
    Ok(dto::SyncStatus::Completed {
        counts: dto::SyncCounts {
            sent_changes: changes.sent as u64,
            received_changes: changes.received as u64,
            received_assets: media_report.received_assets as u64,
            received_blobs: media_report.received_blobs as u64,
            received_bytes: media_report.received_bytes,
            unavailable_assets: media_report.unavailable_assets as u64,
            received_secrets: secrets.received as u64,
            skipped_secrets: secrets.skipped as u64,
        },
        conflicts: changes.conflicts as u64,
        forks: forks as u64,
    })
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use crate::api::tests::{Reply, harness};

    #[test]
    fn addresses_include_all_private_ranges_before_public_interfaces() {
        let addresses = ordered_addresses(vec![
            Ipv4Addr::new(8, 8, 8, 8),
            Ipv4Addr::new(192, 168, 1, 2),
            Ipv4Addr::new(10, 1, 2, 3),
            Ipv4Addr::new(172, 31, 1, 2),
            Ipv4Addr::new(172, 32, 1, 2),
            Ipv4Addr::new(10, 1, 2, 3),
        ]);
        assert_eq!(
            addresses,
            vec![
                Ipv4Addr::new(10, 1, 2, 3),
                Ipv4Addr::new(172, 31, 1, 2),
                Ipv4Addr::new(192, 168, 1, 2),
                Ipv4Addr::new(8, 8, 8, 8),
                Ipv4Addr::new(172, 32, 1, 2),
            ]
        );
    }

    #[tokio::test]
    async fn progress_coalesces_after_events_and_has_no_idle_deadline() {
        let harness = harness(Reply::Text("unused"));
        let context = &harness.context;
        let id = SyncSessionId::new();
        context.sync_state().lock().active = Some(Active {
            id,
            cancellation: CancellationToken::new(),
            approval: None,
            begin: None,
            task: None,
        });
        phase(context, id, dto::SyncPhase::Changes);
        assert!(progress_deadline(context, id).is_none());
        for items in 1..=100 {
            progress(context, id, dto::SyncPhase::Changes, items, 0);
        }
        let events = harness.events.events().into_iter().filter(|event| matches!(event,dto::ApiEvent::SyncStatusChanged {status: dto::SyncStatus::Syncing {progress,..}} if progress.items_done > 0)).count();
        assert_eq!(events, 1);
        assert!(
            matches!(sync_status(context).await.expect("current progress"),dto::SyncStatus::Syncing {progress,..} if progress.items_done == 100)
        );
        tokio::time::sleep_until(progress_deadline(context, id).expect("event schedules deadline"))
            .await;
        flush_progress(context, id);
        assert!(progress_deadline(context, id).is_none());
        let events = harness.events.events().into_iter().filter(|event| matches!(event,dto::ApiEvent::SyncStatusChanged {status: dto::SyncStatus::Syncing {progress,..}} if progress.items_done > 0)).collect::<Vec<_>>();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(events.last(),Some(dto::ApiEvent::SyncStatusChanged {status: dto::SyncStatus::Syncing {progress,..}}) if progress.items_done == 100)
        );
        context.sync_state().cancel();
        progress(context, id, dto::SyncPhase::Changes, 101, 0);
        assert!(
            matches!(sync_status(context).await.expect("cancel preserves progress"),dto::SyncStatus::Syncing {progress,..} if progress.items_done == 100)
        );
        context.sync_state().lock().active = None;
    }
    #[tokio::test]
    async fn an_old_stop_never_waits_for_or_clears_a_replacement_session() {
        let harness = harness(Reply::Text("unused"));
        let context = harness.context.clone();
        let old = SyncSessionId::new();
        let new = SyncSessionId::new();
        let worker_context = context.clone();
        let cancellation = CancellationToken::new();
        let worker_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            worker_cancellation.cancelled().await;
            let mut session = worker_context.sync_state().lock();
            session.active = Some(Active {
                id: new,
                cancellation: CancellationToken::new(),
                approval: None,
                begin: None,
                task: None,
            });
            publish(
                &worker_context,
                dto::SyncStatus::Sharing {
                    addresses: vec!["10.0.0.1".into()],
                    port: 1234,
                    pin: "123456".into(),
                },
            );
        });
        context.sync_state().lock().active = Some(Active {
            id: old,
            cancellation,
            approval: None,
            begin: None,
            task: Some(task),
        });
        tokio::time::timeout(Duration::from_secs(1), context.stop_sync_session())
            .await
            .expect("old stop must finish");
        assert!(matches!(
            sync_status(&context).await.expect("new session kept"),
            dto::SyncStatus::Sharing { .. }
        ));
        context.sync_state().cancel();
        context.sync_state().lock().active = None;
    }
}
