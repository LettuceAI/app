use std::{collections::VecDeque, fmt, io, net::SocketAddr};

use async_trait::async_trait;
use bincode::Options;
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit},
};
use lettuce_jobs::handle::CancellationToken;
use lettuce_media::{LocalSyncMediaStore, MediaAssetRepository, MediaBlobRepository};
use lettuce_sync::{
    CANONICAL_CHANGE_VERSION, CanonicalChange, CanonicalPayload, CausalFrontier, HybridTimestamp,
    MAX_FRONTIER_DEVICES, MAX_SYNC_BLOB_CHUNK_BYTES, SyncBatchAcknowledgement, SyncBlobChunk,
    SyncChangeBatch, SyncChangeFrame, SyncDeviceId, SyncEntity, SyncHello, SyncMediaCatalog,
    SyncSessionError, SyncTransferLimits,
};
use lettuce_types::{ContentHash, TimestampMillis};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use crate::{
    AuthenticatedMediaSyncTransport, AuthenticatedSyncTransport, MediaSyncTransportError,
    SyncTransportError,
};

const PAIRING_PROTOCOL_VERSION: u32 = 1;
const MAX_PAIRING_FRAME_BYTES: usize = 1024;
const MAX_SYNC_FRAME_BYTES: usize = 20 * 1024 * 1024;
const MAX_PENDING_FRAMES: usize = 8;
const HOST_NONCE_PREFIX: [u8; 4] = *b"host";
const CLIENT_NONCE_PREFIX: [u8; 4] = *b"clnt";

#[derive(Clone, PartialEq, Eq)]
pub struct PairingPin(String);

impl PairingPin {
    pub fn new(value: impl Into<String>) -> Result<Self, SyncPeerTransportError> {
        let value = value.into();
        if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(SyncPeerTransportError::InvalidPin);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = [0u8; 6];
        OsRng.fill_bytes(&mut bytes);
        let value = bytes
            .into_iter()
            .map(|byte| char::from(b'0' + byte % 10))
            .collect();
        Self(value)
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PairingPin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PairingPin([REDACTED])")
    }
}

#[derive(Debug)]
pub struct SyncTcpListener {
    listener: TcpListener,
    local_device: SyncDeviceId,
    pin: PairingPin,
}

impl SyncTcpListener {
    pub async fn bind(
        address: SocketAddr,
        local_device: SyncDeviceId,
    ) -> Result<Self, SyncPeerTransportError> {
        Self::bind_with_pin(address, local_device, PairingPin::generate()).await
    }

    pub async fn bind_with_pin(
        address: SocketAddr,
        local_device: SyncDeviceId,
        pin: PairingPin,
    ) -> Result<Self, SyncPeerTransportError> {
        let listener = TcpListener::bind(address)
            .await
            .map_err(SyncPeerTransportError::Io)?;
        Ok(Self {
            listener,
            local_device,
            pin,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, SyncPeerTransportError> {
        self.listener
            .local_addr()
            .map_err(SyncPeerTransportError::Io)
    }

    #[must_use]
    pub fn pin(&self) -> PairingPin {
        self.pin.clone()
    }

    pub async fn accept<'a>(
        &self,
        media: Option<&'a dyn SyncBlobSource>,
        cancellation: &CancellationToken,
    ) -> Result<AuthenticatedTcpSyncTransport<'a>, SyncPeerTransportError> {
        let (stream, _) = tokio::select! {
            () = cancellation.cancelled() => return Err(SyncPeerTransportError::Cancelled),
            accepted = self.listener.accept() => accepted.map_err(SyncPeerTransportError::Io)?,
        };
        authenticate_host(stream, self.local_device, &self.pin, media, cancellation).await
    }
}

pub trait SyncBlobSource: Send + Sync {
    fn read_chunk(
        &self,
        content_hash: &ContentHash,
        offset: u64,
        max_bytes: usize,
    ) -> Result<Vec<u8>, SyncBlobSourceError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("sync blob source could not read the requested content")]
pub struct SyncBlobSourceError;

impl<BR, AR> SyncBlobSource for LocalSyncMediaStore<BR, AR>
where
    BR: MediaBlobRepository + Send + Sync,
    AR: MediaAssetRepository + Send + Sync,
{
    fn read_chunk(
        &self,
        content_hash: &ContentHash,
        offset: u64,
        max_bytes: usize,
    ) -> Result<Vec<u8>, SyncBlobSourceError> {
        self.read_sync_chunk(content_hash, offset, max_bytes)
            .map_err(|_| SyncBlobSourceError)
    }
}

pub async fn connect_authenticated_sync<'a>(
    address: SocketAddr,
    local_device: SyncDeviceId,
    pin: &PairingPin,
    media: Option<&'a dyn SyncBlobSource>,
    cancellation: &CancellationToken,
) -> Result<AuthenticatedTcpSyncTransport<'a>, SyncPeerTransportError> {
    let stream = tokio::select! {
        () = cancellation.cancelled() => return Err(SyncPeerTransportError::Cancelled),
        connected = TcpStream::connect(address) => connected.map_err(SyncPeerTransportError::Io)?,
    };
    authenticate_client(stream, local_device, pin, media, cancellation).await
}

#[derive(Debug, thiserror::Error)]
pub enum SyncPeerTransportError {
    #[error("pairing PIN must contain exactly six digits")]
    InvalidPin,
    #[error("sync pairing was cancelled")]
    Cancelled,
    #[error("sync pairing proof failed")]
    AuthenticationFailed,
    #[error("sync peer used the local device identity")]
    IdentityMismatch,
    #[error("sync frame exceeded its byte limit")]
    FrameTooLarge,
    #[error("sync peer sent an invalid protocol frame")]
    Protocol,
    #[error("sync peer disconnected")]
    Disconnected,
    #[error("sync socket failed")]
    Io(#[source] io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionRole {
    Host,
    Client,
}

pub struct AuthenticatedTcpSyncTransport<'a> {
    stream: TcpStream,
    peer: SyncDeviceId,
    role: ConnectionRole,
    cipher: ChaCha20Poly1305,
    send_prefix: [u8; 4],
    receive_prefix: [u8; 4],
    send_counter: u64,
    receive_counter: u64,
    media: Option<&'a dyn SyncBlobSource>,
    pending: VecDeque<SyncWireFrame>,
}

impl fmt::Debug for AuthenticatedTcpSyncTransport<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedTcpSyncTransport")
            .field("peer", &self.peer)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

impl AuthenticatedTcpSyncTransport<'_> {
    pub async fn close(&mut self) -> Result<(), SyncPeerTransportError> {
        self.stream
            .shutdown()
            .await
            .map_err(SyncPeerTransportError::Io)
    }

    async fn exchange(
        &mut self,
        local: SyncWireFrame,
        expected: ExpectedFrame,
        cancellation: &CancellationToken,
    ) -> Result<SyncWireFrame, SyncPeerTransportError> {
        match self.role {
            ConnectionRole::Host => {
                self.send(local, cancellation).await?;
                self.receive_expected(expected, cancellation).await
            }
            ConnectionRole::Client => {
                let remote = self.receive_expected(expected, cancellation).await?;
                self.send(local, cancellation).await?;
                Ok(remote)
            }
        }
    }

    async fn receive_expected(
        &mut self,
        expected: ExpectedFrame,
        cancellation: &CancellationToken,
    ) -> Result<SyncWireFrame, SyncPeerTransportError> {
        loop {
            if let Some(index) = self
                .pending
                .iter()
                .position(|frame| expected.matches(frame))
            {
                return self
                    .pending
                    .remove(index)
                    .ok_or(SyncPeerTransportError::Protocol);
            }
            let frame = self.receive(cancellation).await?;
            if let SyncWireFrame::BlobRequest {
                content_hash,
                offset,
                max_bytes,
            } = frame
            {
                let source = self.media.ok_or(SyncPeerTransportError::Protocol)?;
                let bytes = source
                    .read_chunk(&content_hash, offset, max_bytes)
                    .map_err(|_| SyncPeerTransportError::Protocol)?;
                let next_offset = offset
                    .checked_add(
                        u64::try_from(bytes.len()).map_err(|_| SyncPeerTransportError::Protocol)?,
                    )
                    .ok_or(SyncPeerTransportError::Protocol)?;
                let complete = if bytes.len() < max_bytes {
                    true
                } else {
                    source
                        .read_chunk(&content_hash, next_offset, 1)
                        .map_err(|_| SyncPeerTransportError::Protocol)?
                        .is_empty()
                };
                self.send(
                    SyncWireFrame::BlobChunk(SyncBlobChunk {
                        content_hash,
                        offset,
                        bytes,
                        complete,
                    }),
                    cancellation,
                )
                .await?;
                continue;
            }
            if expected.matches(&frame) {
                return Ok(frame);
            }
            if self.pending.len() == MAX_PENDING_FRAMES {
                return Err(SyncPeerTransportError::Protocol);
            }
            self.pending.push_back(frame);
        }
    }

    async fn send(
        &mut self,
        frame: SyncWireFrame,
        cancellation: &CancellationToken,
    ) -> Result<(), SyncPeerTransportError> {
        validate_wire_frame(&frame)?;
        let plaintext = serialize_bounded(&frame, MAX_SYNC_FRAME_BYTES)?;
        let nonce = frame_nonce(self.send_prefix, self.send_counter);
        self.send_counter = self
            .send_counter
            .checked_add(1)
            .ok_or(SyncPeerTransportError::Protocol)?;
        let encrypted = self
            .cipher
            .encrypt(&nonce, plaintext.as_slice())
            .map_err(|_| SyncPeerTransportError::Protocol)?;
        write_frame(
            &mut self.stream,
            &encrypted,
            MAX_SYNC_FRAME_BYTES,
            cancellation,
        )
        .await
    }

    async fn receive(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<SyncWireFrame, SyncPeerTransportError> {
        let encrypted = read_frame(&mut self.stream, MAX_SYNC_FRAME_BYTES, cancellation).await?;
        let nonce = frame_nonce(self.receive_prefix, self.receive_counter);
        self.receive_counter = self
            .receive_counter
            .checked_add(1)
            .ok_or(SyncPeerTransportError::Protocol)?;
        let plaintext = self
            .cipher
            .decrypt(&nonce, encrypted.as_slice())
            .map_err(|_| SyncPeerTransportError::Protocol)?;
        let frame = deserialize_bounded(&plaintext, MAX_SYNC_FRAME_BYTES)?;
        validate_wire_frame(&frame)?;
        Ok(frame)
    }
}

#[async_trait]
impl AuthenticatedSyncTransport for AuthenticatedTcpSyncTransport<'_> {
    fn authenticated_peer(&self) -> SyncDeviceId {
        self.peer
    }

    async fn exchange_hello(
        &mut self,
        local: SyncHello,
        cancellation: &CancellationToken,
    ) -> Result<SyncHello, SyncTransportError> {
        match self
            .exchange(
                SyncWireFrame::Hello(local),
                ExpectedFrame::Hello,
                cancellation,
            )
            .await
            .map_err(map_sync_error)?
        {
            SyncWireFrame::Hello(value) => Ok(value),
            _ => Err(SyncTransportError::Protocol),
        }
    }

    async fn exchange_frontier(
        &mut self,
        local: CausalFrontier,
        cancellation: &CancellationToken,
    ) -> Result<CausalFrontier, SyncTransportError> {
        match self
            .exchange(
                SyncWireFrame::Frontier(local),
                ExpectedFrame::Frontier,
                cancellation,
            )
            .await
            .map_err(map_sync_error)?
        {
            SyncWireFrame::Frontier(value) => Ok(value),
            _ => Err(SyncTransportError::Protocol),
        }
    }

    async fn exchange_changes(
        &mut self,
        local: SyncChangeFrame,
        cancellation: &CancellationToken,
    ) -> Result<SyncChangeFrame, SyncTransportError> {
        match self
            .exchange(
                SyncWireFrame::Changes(local),
                ExpectedFrame::Changes,
                cancellation,
            )
            .await
            .map_err(map_sync_error)?
        {
            SyncWireFrame::Changes(value) => Ok(value),
            _ => Err(SyncTransportError::Protocol),
        }
    }

    async fn exchange_acknowledgement(
        &mut self,
        local: SyncBatchAcknowledgement,
        cancellation: &CancellationToken,
    ) -> Result<SyncBatchAcknowledgement, SyncTransportError> {
        match self
            .exchange(
                SyncWireFrame::Acknowledgement(local),
                ExpectedFrame::Acknowledgement,
                cancellation,
            )
            .await
            .map_err(map_sync_error)?
        {
            SyncWireFrame::Acknowledgement(value) => Ok(value),
            _ => Err(SyncTransportError::Protocol),
        }
    }
}

#[async_trait]
impl AuthenticatedMediaSyncTransport for AuthenticatedTcpSyncTransport<'_> {
    async fn exchange_media_catalog(
        &mut self,
        local: SyncMediaCatalog,
        cancellation: &CancellationToken,
    ) -> Result<SyncMediaCatalog, MediaSyncTransportError> {
        match self
            .exchange(
                SyncWireFrame::MediaCatalog(local),
                ExpectedFrame::MediaCatalog,
                cancellation,
            )
            .await
            .map_err(map_media_error)?
        {
            SyncWireFrame::MediaCatalog(value) => Ok(value),
            _ => Err(MediaSyncTransportError::Protocol),
        }
    }

    async fn fetch_blob_chunk(
        &mut self,
        content_hash: &ContentHash,
        offset: u64,
        max_bytes: usize,
        cancellation: &CancellationToken,
    ) -> Result<SyncBlobChunk, MediaSyncTransportError> {
        self.send(
            SyncWireFrame::BlobRequest {
                content_hash: content_hash.clone(),
                offset,
                max_bytes,
            },
            cancellation,
        )
        .await
        .map_err(map_media_error)?;
        match self
            .receive_expected(ExpectedFrame::BlobChunk, cancellation)
            .await
            .map_err(map_media_error)?
        {
            SyncWireFrame::BlobChunk(value) => Ok(value),
            _ => Err(MediaSyncTransportError::Protocol),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum PairingFrame {
    HostChallenge {
        version: u32,
        device: SyncDeviceId,
        salt: [u8; 16],
        challenge: [u8; 32],
    },
    ClientProof {
        device: SyncDeviceId,
        challenge: [u8; 32],
        proof: [u8; 32],
    },
    HostProof {
        proof: [u8; 32],
    },
    Rejected,
}

#[derive(Debug, Serialize, Deserialize)]
enum SyncWireFrame {
    Hello(SyncHello),
    Frontier(CausalFrontier),
    Changes(SyncChangeFrame),
    Acknowledgement(SyncBatchAcknowledgement),
    MediaCatalog(SyncMediaCatalog),
    BlobRequest {
        content_hash: ContentHash,
        offset: u64,
        max_bytes: usize,
    },
    BlobChunk(SyncBlobChunk),
}

#[derive(Debug, Clone, Copy)]
enum ExpectedFrame {
    Hello,
    Frontier,
    Changes,
    Acknowledgement,
    MediaCatalog,
    BlobChunk,
}

impl ExpectedFrame {
    fn matches(self, frame: &SyncWireFrame) -> bool {
        matches!(
            (self, frame),
            (Self::Hello, SyncWireFrame::Hello(_))
                | (Self::Frontier, SyncWireFrame::Frontier(_))
                | (Self::Changes, SyncWireFrame::Changes(_))
                | (Self::Acknowledgement, SyncWireFrame::Acknowledgement(_))
                | (Self::MediaCatalog, SyncWireFrame::MediaCatalog(_))
                | (Self::BlobChunk, SyncWireFrame::BlobChunk(_))
        )
    }
}

async fn authenticate_host<'a>(
    mut stream: TcpStream,
    local_device: SyncDeviceId,
    pin: &PairingPin,
    media: Option<&'a dyn SyncBlobSource>,
    cancellation: &CancellationToken,
) -> Result<AuthenticatedTcpSyncTransport<'a>, SyncPeerTransportError> {
    let mut salt = [0u8; 16];
    let mut host_challenge = [0u8; 32];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut host_challenge);
    send_plain(
        &mut stream,
        &PairingFrame::HostChallenge {
            version: PAIRING_PROTOCOL_VERSION,
            device: local_device,
            salt,
            challenge: host_challenge,
        },
        cancellation,
    )
    .await?;
    let PairingFrame::ClientProof {
        device: peer,
        challenge: client_challenge,
        proof,
    } = receive_plain(&mut stream, cancellation).await?
    else {
        return Err(SyncPeerTransportError::Protocol);
    };
    if peer == local_device {
        let _ = send_plain(&mut stream, &PairingFrame::Rejected, cancellation).await;
        return Err(SyncPeerTransportError::IdentityMismatch);
    }
    let pairing_key = pairing_key(pin, &salt);
    let expected = pairing_proof(
        &pairing_key,
        b"client",
        local_device,
        peer,
        &host_challenge,
        &client_challenge,
    );
    if proof != expected {
        let _ = send_plain(&mut stream, &PairingFrame::Rejected, cancellation).await;
        return Err(SyncPeerTransportError::AuthenticationFailed);
    }
    let host_proof = pairing_proof(
        &pairing_key,
        b"host",
        local_device,
        peer,
        &host_challenge,
        &client_challenge,
    );
    send_plain(
        &mut stream,
        &PairingFrame::HostProof { proof: host_proof },
        cancellation,
    )
    .await?;
    Ok(authenticated_transport(
        stream,
        peer,
        ConnectionRole::Host,
        session_key(
            &pairing_key,
            local_device,
            peer,
            &host_challenge,
            &client_challenge,
        ),
        media,
    ))
}

async fn authenticate_client<'a>(
    mut stream: TcpStream,
    local_device: SyncDeviceId,
    pin: &PairingPin,
    media: Option<&'a dyn SyncBlobSource>,
    cancellation: &CancellationToken,
) -> Result<AuthenticatedTcpSyncTransport<'a>, SyncPeerTransportError> {
    let PairingFrame::HostChallenge {
        version,
        device: peer,
        salt,
        challenge: host_challenge,
    } = receive_plain(&mut stream, cancellation).await?
    else {
        return Err(SyncPeerTransportError::Protocol);
    };
    if version != PAIRING_PROTOCOL_VERSION {
        return Err(SyncPeerTransportError::Protocol);
    }
    let pairing_key = pairing_key(pin, &salt);
    let mut client_challenge = [0u8; 32];
    OsRng.fill_bytes(&mut client_challenge);
    let proof = pairing_proof(
        &pairing_key,
        b"client",
        peer,
        local_device,
        &host_challenge,
        &client_challenge,
    );
    send_plain(
        &mut stream,
        &PairingFrame::ClientProof {
            device: local_device,
            challenge: client_challenge,
            proof,
        },
        cancellation,
    )
    .await?;
    let PairingFrame::HostProof { proof } = receive_plain(&mut stream, cancellation).await? else {
        return Err(SyncPeerTransportError::AuthenticationFailed);
    };
    let expected = pairing_proof(
        &pairing_key,
        b"host",
        peer,
        local_device,
        &host_challenge,
        &client_challenge,
    );
    if proof != expected {
        return Err(SyncPeerTransportError::AuthenticationFailed);
    }
    Ok(authenticated_transport(
        stream,
        peer,
        ConnectionRole::Client,
        session_key(
            &pairing_key,
            peer,
            local_device,
            &host_challenge,
            &client_challenge,
        ),
        media,
    ))
}

fn authenticated_transport<'a>(
    stream: TcpStream,
    peer: SyncDeviceId,
    role: ConnectionRole,
    key: [u8; 32],
    media: Option<&'a dyn SyncBlobSource>,
) -> AuthenticatedTcpSyncTransport<'a> {
    let (send_prefix, receive_prefix) = match role {
        ConnectionRole::Host => (HOST_NONCE_PREFIX, CLIENT_NONCE_PREFIX),
        ConnectionRole::Client => (CLIENT_NONCE_PREFIX, HOST_NONCE_PREFIX),
    };
    AuthenticatedTcpSyncTransport {
        stream,
        peer,
        role,
        cipher: ChaCha20Poly1305::new(Key::from_slice(&key)),
        send_prefix,
        receive_prefix,
        send_counter: 0,
        receive_counter: 0,
        media,
        pending: VecDeque::new(),
    }
}

fn pairing_key(pin: &PairingPin, salt: &[u8; 16]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key("lettuce-sync-pairing-key-v1");
    hasher.update(salt);
    hasher.update(pin.expose().as_bytes());
    *hasher.finalize().as_bytes()
}

fn pairing_proof(
    key: &[u8; 32],
    role: &[u8],
    host: SyncDeviceId,
    client: SyncDeviceId,
    host_challenge: &[u8; 32],
    client_challenge: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_keyed(key);
    hasher.update(b"lettuce-sync-pairing-proof-v1\0");
    hasher.update(role);
    hasher.update(host.as_uuid().as_bytes());
    hasher.update(client.as_uuid().as_bytes());
    hasher.update(host_challenge);
    hasher.update(client_challenge);
    *hasher.finalize().as_bytes()
}

fn session_key(
    pairing_key: &[u8; 32],
    host: SyncDeviceId,
    client: SyncDeviceId,
    host_challenge: &[u8; 32],
    client_challenge: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key("lettuce-sync-session-key-v1");
    hasher.update(pairing_key);
    hasher.update(host.as_uuid().as_bytes());
    hasher.update(client.as_uuid().as_bytes());
    hasher.update(host_challenge);
    hasher.update(client_challenge);
    *hasher.finalize().as_bytes()
}

fn frame_nonce(prefix: [u8; 4], counter: u64) -> Nonce {
    let mut bytes = [0u8; 12];
    bytes[..4].copy_from_slice(&prefix);
    bytes[4..].copy_from_slice(&counter.to_be_bytes());
    *Nonce::from_slice(&bytes)
}

async fn send_plain<T: Serialize>(
    stream: &mut TcpStream,
    value: &T,
    cancellation: &CancellationToken,
) -> Result<(), SyncPeerTransportError> {
    let bytes = serialize_bounded(value, MAX_PAIRING_FRAME_BYTES)?;
    write_frame(stream, &bytes, MAX_PAIRING_FRAME_BYTES, cancellation).await
}

async fn receive_plain<T: DeserializeOwned>(
    stream: &mut TcpStream,
    cancellation: &CancellationToken,
) -> Result<T, SyncPeerTransportError> {
    let bytes = read_frame(stream, MAX_PAIRING_FRAME_BYTES, cancellation).await?;
    deserialize_bounded(&bytes, MAX_PAIRING_FRAME_BYTES)
}

fn serialize_bounded<T: Serialize>(
    value: &T,
    limit: usize,
) -> Result<Vec<u8>, SyncPeerTransportError> {
    bincode::options()
        .with_limit(u64::try_from(limit).map_err(|_| SyncPeerTransportError::Protocol)?)
        .serialize(value)
        .map_err(|_| SyncPeerTransportError::Protocol)
}

fn deserialize_bounded<T: DeserializeOwned>(
    bytes: &[u8],
    limit: usize,
) -> Result<T, SyncPeerTransportError> {
    bincode::options()
        .with_limit(u64::try_from(limit).map_err(|_| SyncPeerTransportError::Protocol)?)
        .reject_trailing_bytes()
        .deserialize(bytes)
        .map_err(|_| SyncPeerTransportError::Protocol)
}

async fn write_frame(
    stream: &mut TcpStream,
    bytes: &[u8],
    limit: usize,
    cancellation: &CancellationToken,
) -> Result<(), SyncPeerTransportError> {
    if bytes.len() > limit || u32::try_from(bytes.len()).is_err() {
        return Err(SyncPeerTransportError::FrameTooLarge);
    }
    let length = u32::try_from(bytes.len())
        .map_err(|_| SyncPeerTransportError::FrameTooLarge)?
        .to_be_bytes();
    tokio::select! {
        () = cancellation.cancelled() => Err(SyncPeerTransportError::Cancelled),
        result = async {
            stream.write_all(&length).await?;
            stream.write_all(bytes).await?;
            stream.flush().await
        } => result.map_err(map_io),
    }
}

async fn read_frame(
    stream: &mut TcpStream,
    limit: usize,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, SyncPeerTransportError> {
    let mut length = [0u8; 4];
    tokio::select! {
        () = cancellation.cancelled() => return Err(SyncPeerTransportError::Cancelled),
        result = stream.read_exact(&mut length) => result.map_err(map_io)?,
    };
    let length = u32::from_be_bytes(length) as usize;
    if length > limit {
        return Err(SyncPeerTransportError::FrameTooLarge);
    }
    let mut bytes = vec![0u8; length];
    tokio::select! {
        () = cancellation.cancelled() => Err(SyncPeerTransportError::Cancelled),
        result = stream.read_exact(&mut bytes) => result.map(|_| bytes).map_err(map_io),
    }
}

fn map_io(error: io::Error) -> SyncPeerTransportError {
    match error.kind() {
        io::ErrorKind::UnexpectedEof
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::BrokenPipe => SyncPeerTransportError::Disconnected,
        _ => SyncPeerTransportError::Io(error),
    }
}

fn map_sync_error(error: SyncPeerTransportError) -> SyncTransportError {
    match error {
        SyncPeerTransportError::Cancelled => SyncTransportError::Cancelled,
        SyncPeerTransportError::Disconnected => SyncTransportError::Disconnected,
        _ => SyncTransportError::Protocol,
    }
}

fn map_media_error(error: SyncPeerTransportError) -> MediaSyncTransportError {
    match error {
        SyncPeerTransportError::Cancelled => MediaSyncTransportError::Cancelled,
        SyncPeerTransportError::Disconnected => MediaSyncTransportError::Disconnected,
        _ => MediaSyncTransportError::Protocol,
    }
}

fn validate_wire_frame(frame: &SyncWireFrame) -> Result<(), SyncPeerTransportError> {
    match frame {
        SyncWireFrame::Hello(value) => validate_hello(value),
        SyncWireFrame::Frontier(value) => validate_frontier(value),
        SyncWireFrame::Changes(value) => validate_change_frame(value),
        SyncWireFrame::Acknowledgement(value) => validate_frontier(&value.frontier),
        SyncWireFrame::MediaCatalog(value) => {
            let validated = SyncMediaCatalog::new(value.assets().to_vec())
                .map_err(|_| SyncPeerTransportError::Protocol)?;
            if &validated == value {
                Ok(())
            } else {
                Err(SyncPeerTransportError::Protocol)
            }
        }
        SyncWireFrame::BlobRequest {
            content_hash,
            max_bytes,
            ..
        } => {
            if *max_bytes == 0
                || *max_bytes > MAX_SYNC_BLOB_CHUNK_BYTES
                || ContentHash::parse(content_hash.as_str()).as_ref() != Ok(content_hash)
            {
                Err(SyncPeerTransportError::Protocol)
            } else {
                Ok(())
            }
        }
        SyncWireFrame::BlobChunk(value) => value
            .validate(&value.content_hash, value.offset)
            .map_err(|_| SyncPeerTransportError::Protocol),
    }
}

fn validate_hello(value: &SyncHello) -> Result<(), SyncPeerTransportError> {
    let limits = SyncTransferLimits::new(
        value.limits().max_changes_per_batch(),
        value.limits().max_change_payload_bytes(),
        value.limits().max_batch_payload_bytes(),
    )
    .map_err(|_| SyncPeerTransportError::Protocol)?;
    let validated = SyncHello::new(
        value.app_version(),
        value.protocol_version(),
        value.schema_fingerprint().clone(),
        value.device_id(),
        value.device_name(),
        value.session_id(),
        limits,
    )
    .map_err(|_: SyncSessionError| SyncPeerTransportError::Protocol)?;
    if &validated == value {
        Ok(())
    } else {
        Err(SyncPeerTransportError::Protocol)
    }
}

fn validate_frontier(value: &CausalFrontier) -> Result<(), SyncPeerTransportError> {
    if value.len() > MAX_FRONTIER_DEVICES || value.values().any(|sequence| *sequence == 0) {
        Err(SyncPeerTransportError::Protocol)
    } else {
        Ok(())
    }
}

fn validate_change_frame(value: &SyncChangeFrame) -> Result<(), SyncPeerTransportError> {
    match value {
        SyncChangeFrame::Quiescent { frontier } => validate_frontier(frontier),
        SyncChangeFrame::Batch(batch) => {
            for change in batch.changes() {
                validate_change(change)?;
            }
            let validated = SyncChangeBatch::from_parts(
                batch.batch_id(),
                batch.batch_hash().clone(),
                batch.changes().to_vec(),
                SyncTransferLimits::default(),
            )
            .map_err(|_| SyncPeerTransportError::Protocol)?;
            if &validated == batch {
                Ok(())
            } else {
                Err(SyncPeerTransportError::Protocol)
            }
        }
    }
}

fn validate_change(value: &CanonicalChange) -> Result<(), SyncPeerTransportError> {
    if value.version() != CANONICAL_CHANGE_VERSION {
        return Err(SyncPeerTransportError::Protocol);
    }
    validate_frontier(value.base_frontier())?;
    let payload = value
        .payload()
        .map(|payload| {
            CanonicalPayload::new(
                payload.schema(),
                payload.version(),
                payload.bytes().to_vec(),
            )
        })
        .transpose()
        .map_err(|_| SyncPeerTransportError::Protocol)?;
    let validated = CanonicalChange::new(
        value.id(),
        value.origin_device(),
        value.origin_sequence(),
        HybridTimestamp::new(
            TimestampMillis::new(value.timestamp().wall_time().get()),
            value.timestamp().counter(),
        ),
        value.base_frontier().clone(),
        SyncEntity::new(value.entity().kind(), value.entity().id())
            .map_err(|_| SyncPeerTransportError::Protocol)?,
        value.operation(),
        value.base_revision().cloned(),
        payload,
    )
    .map_err(|_| SyncPeerTransportError::Protocol)?;
    if &validated == value {
        Ok(())
    } else {
        Err(SyncPeerTransportError::Protocol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SyncExchangeCoordinator, SyncExchangeError, SyncExchangeOutcome, SyncHelloCoordinator,
        SyncMediaCoordinator,
    };
    use lettuce_characters::{
        LifecycleStatus, Persona, PersonaMedia, PersonaMediaLink, PersonaMediaSlot,
        PersonaRepository,
    };
    use lettuce_database::Database;
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, LocalMediaBlobStore,
        MediaAssetRepository, RetentionClass,
    };
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    use lettuce_sync::{LocalChangeJournal, SyncSessionId, negotiate_sync_session};
    use lettuce_types::{OperationId, PersonaId, Revision};

    fn pin(value: &str) -> PairingPin {
        PairingPin::new(value).expect("pairing PIN")
    }

    fn hello(database: &Database, name: &str, session: u128, now: i64) -> SyncHello {
        SyncHelloCoordinator::new(database)
            .build(
                "1.2.3",
                name,
                SyncSessionId::from_uuid(uuid::Uuid::from_u128(session)),
                SyncTransferLimits::default(),
                TimestampMillis::new(now),
            )
            .expect("sync hello")
    }

    async fn authenticated_pair<'a>(
        host_device: SyncDeviceId,
        client_device: SyncDeviceId,
        host_media: Option<&'a dyn SyncBlobSource>,
        client_media: Option<&'a dyn SyncBlobSource>,
    ) -> (
        AuthenticatedTcpSyncTransport<'a>,
        AuthenticatedTcpSyncTransport<'a>,
    ) {
        let cancellation = CancellationToken::new();
        let listener = SyncTcpListener::bind_with_pin(
            "127.0.0.1:0".parse().expect("address"),
            host_device,
            pin("123456"),
        )
        .await
        .expect("listener");
        let address = listener.local_addr().expect("listener address");
        let client_pin = pin("123456");
        let (host, client) = tokio::join!(
            listener.accept(host_media, &cancellation),
            connect_authenticated_sync(
                address,
                client_device,
                &client_pin,
                client_media,
                &cancellation,
            )
        );
        (host.expect("host session"), client.expect("client session"))
    }

    #[tokio::test]
    async fn pairing_rejects_wrong_pin_duplicate_identity_and_oversized_frame() {
        let host_device = SyncDeviceId::new();
        let client_device = SyncDeviceId::new();
        let cancellation = CancellationToken::new();
        let cancelled_listener = SyncTcpListener::bind_with_pin(
            "127.0.0.1:0".parse().expect("address"),
            host_device,
            pin("123456"),
        )
        .await
        .expect("cancelled listener");
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(matches!(
            cancelled_listener.accept(None, &cancelled).await,
            Err(SyncPeerTransportError::Cancelled)
        ));
        let listener = SyncTcpListener::bind_with_pin(
            "127.0.0.1:0".parse().expect("address"),
            host_device,
            pin("123456"),
        )
        .await
        .expect("listener");
        let address = listener.local_addr().expect("address");
        let wrong_pin = pin("654321");
        let (host, client) = tokio::join!(
            listener.accept(None, &cancellation),
            connect_authenticated_sync(address, client_device, &wrong_pin, None, &cancellation,)
        );
        assert!(matches!(
            host,
            Err(SyncPeerTransportError::AuthenticationFailed)
        ));
        assert!(matches!(
            client,
            Err(SyncPeerTransportError::AuthenticationFailed)
        ));

        let listener = SyncTcpListener::bind_with_pin(
            "127.0.0.1:0".parse().expect("address"),
            host_device,
            pin("123456"),
        )
        .await
        .expect("listener");
        let address = listener.local_addr().expect("address");
        let client_pin = pin("123456");
        let (host, client) = tokio::join!(
            listener.accept(None, &cancellation),
            connect_authenticated_sync(address, host_device, &client_pin, None, &cancellation,)
        );
        assert!(matches!(
            host,
            Err(SyncPeerTransportError::IdentityMismatch)
        ));
        assert!(matches!(
            client,
            Err(SyncPeerTransportError::AuthenticationFailed)
        ));

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("raw listener");
        let address = listener.local_addr().expect("raw address");
        let raw_server = async {
            let (mut stream, _) = listener.accept().await.expect("raw accept");
            stream
                .write_all(
                    &u32::try_from(MAX_PAIRING_FRAME_BYTES + 1)
                        .expect("length")
                        .to_be_bytes(),
                )
                .await
                .expect("oversized length");
        };
        let client_pin = pin("123456");
        let raw_client =
            connect_authenticated_sync(address, client_device, &client_pin, None, &cancellation);
        let (_, result) = tokio::join!(raw_server, raw_client);
        assert!(matches!(result, Err(SyncPeerTransportError::FrameTooLarge)));

        let (mut host, mut client) =
            authenticated_pair(host_device, client_device, None, None).await;
        let host_hello = SyncHello::current(
            "1.2.3",
            host_device,
            "Host",
            SyncSessionId::new(),
            SyncTransferLimits::default(),
        )
        .expect("host hello");
        let false_client_hello = SyncHello::current(
            "1.2.3",
            SyncDeviceId::new(),
            "Client",
            SyncSessionId::new(),
            SyncTransferLimits::default(),
        )
        .expect("false client hello");
        let (received_client, received_host) = tokio::join!(
            host.exchange_hello(host_hello.clone(), &cancellation),
            client.exchange_hello(false_client_hello, &cancellation),
        );
        assert_eq!(
            negotiate_sync_session(
                &host_hello,
                &received_client.expect("client hello"),
                host.authenticated_peer(),
            ),
            Err(SyncSessionError::AuthenticatedIdentityMismatch)
        );
        assert_eq!(received_host.expect("host hello").device_id(), host_device);
    }

    fn paths(label: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("sync-tcp-{label}-{}", OperationId::new()));
        std::fs::create_dir_all(&root).expect("media root");
        let database = root.join("state.sqlite3");
        (root, database)
    }

    fn ingest_store(
        root: &std::path::Path,
        database: &std::path::Path,
    ) -> LocalMediaBlobStore<Database, Database> {
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(root).expect("snapshot"))
            .expect("authority");
        LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read capability"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write capability"),
            Database::open(database).expect("blob database"),
            Database::open(database).expect("asset database"),
        )
    }

    fn sync_store(
        root: &std::path::Path,
        database: &std::path::Path,
    ) -> LocalSyncMediaStore<Database, Database> {
        LocalSyncMediaStore::open(
            root.join("platform-v2/media-blobs"),
            Database::open(database).expect("blob database"),
            Database::open(database).expect("asset database"),
        )
        .expect("sync media store")
    }

    #[tokio::test]
    async fn loopback_peers_resume_pending_persona_media_and_converge() {
        let (source_root, source_path) = paths("source");
        let (target_root, target_path) = paths("target");
        let source = Database::open(&source_path).expect("source database");
        let target = Database::open(&target_path).expect("target database");
        let ingest = ingest_store(&source_root, &source_path);
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&2_u32.to_be_bytes());
        bytes.extend_from_slice(&3_u32.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(b"shared loopback persona bytes");
        let avatar = ingest
            .ingest(
                bytes.as_slice(),
                IngestRequest::new(
                    AssetKind::AvatarOriginal,
                    AssetOrigin::Upload,
                    RetentionClass::Persistent,
                    AssetProvenanceV1::default(),
                ),
            )
            .expect("avatar");
        let design = ingest
            .ingest(
                bytes.as_slice(),
                IngestRequest::new(
                    AssetKind::Illustration,
                    AssetOrigin::Upload,
                    RetentionClass::Persistent,
                    AssetProvenanceV1::default(),
                ),
            )
            .expect("design reference");
        assert_eq!(avatar.blob.id, design.blob.id);
        let persona = PersonaRepository::create(
            &source,
            Persona {
                id: PersonaId::new(),
                status: LifecycleStatus::Active,
                title: "Loopback persona".into(),
                description: "Carries shared media".into(),
                nickname: None,
                design_description: None,
                avatar_crop: None,
                image_recommendation: None,
                media: PersonaMedia {
                    links: vec![
                        PersonaMediaLink {
                            asset_id: avatar.asset.id,
                            slot: PersonaMediaSlot::Avatar,
                            ordinal: 0,
                        },
                        PersonaMediaLink {
                            asset_id: design.asset.id,
                            slot: PersonaMediaSlot::DesignReference,
                            ordinal: 0,
                        },
                    ],
                },
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(10),
                updated_at: TimestampMillis::new(10),
            },
        )
        .expect("persona");
        let source_hello = hello(&source, "Source", 100, 11);
        let target_hello = hello(&target, "Target", 101, 11);
        let (mut source_transport, mut target_transport) = authenticated_pair(
            source_hello.device_id(),
            target_hello.device_id(),
            None,
            None,
        )
        .await;
        let cancellation = CancellationToken::new();
        let source_exchange = SyncExchangeCoordinator::new(&source);
        let target_exchange = SyncExchangeCoordinator::new(&target);
        let (source_result, target_result) = tokio::join!(
            source_exchange.run(
                source_hello,
                &mut source_transport,
                &cancellation,
                TimestampMillis::new(12),
            ),
            async {
                let result = target_exchange
                    .run(
                        target_hello,
                        &mut target_transport,
                        &cancellation,
                        TimestampMillis::new(12),
                    )
                    .await;
                target_transport
                    .close()
                    .await
                    .expect("close pending session");
                result
            }
        );
        assert!(matches!(
            target_result,
            Ok(SyncExchangeOutcome::Pending { .. })
        ));
        assert!(matches!(
            source_result,
            Err(SyncExchangeError::Transport(
                SyncTransportError::Disconnected
            ))
        ));
        assert!(
            PersonaRepository::get(&target, persona.id)
                .expect("target persona read")
                .is_none()
        );
        drop(target);

        let target = Database::open(&target_path).expect("reopen target database");
        let source_store = sync_store(&source_root, &source_path);
        let target_store = sync_store(&target_root, &target_path);
        let source_hello = hello(&source, "Source", 200, 20);
        let target_hello = hello(&target, "Target", 201, 20);
        let (mut source_transport, mut target_transport) = authenticated_pair(
            source_hello.device_id(),
            target_hello.device_id(),
            Some(&source_store),
            Some(&target_store),
        )
        .await;
        let source_flow = async {
            SyncMediaCoordinator::new(&source, &source_store)
                .run(
                    &mut source_transport,
                    &cancellation,
                    TimestampMillis::new(21),
                )
                .await
                .expect("source media");
            SyncExchangeCoordinator::new(&source)
                .run(
                    source_hello,
                    &mut source_transport,
                    &cancellation,
                    TimestampMillis::new(22),
                )
                .await
        };
        let target_flow = async {
            let report = SyncMediaCoordinator::new(&target, &target_store)
                .run(
                    &mut target_transport,
                    &cancellation,
                    TimestampMillis::new(21),
                )
                .await
                .expect("target media");
            assert_eq!(report.received_assets, 2);
            assert_eq!(report.received_blobs, 1);
            SyncExchangeCoordinator::new(&target)
                .run(
                    target_hello,
                    &mut target_transport,
                    &cancellation,
                    TimestampMillis::new(22),
                )
                .await
        };
        let (source_result, target_result) = tokio::join!(source_flow, target_flow);
        assert!(matches!(
            source_result,
            Ok(SyncExchangeOutcome::Complete(_))
        ));
        assert!(matches!(
            target_result,
            Ok(SyncExchangeOutcome::Complete(_))
        ));
        assert_eq!(
            PersonaRepository::get(&target, persona.id).expect("target persona"),
            Some(persona)
        );
        let target_avatar = MediaAssetRepository::get(&target, avatar.asset.id)
            .expect("target avatar")
            .expect("avatar exists");
        let target_design = MediaAssetRepository::get(&target, design.asset.id)
            .expect("target design")
            .expect("design exists");
        assert_eq!(target_avatar.blob_id, target_design.blob_id);
        assert_eq!(
            target_store
                .read_sync_chunk(&avatar.blob.content_hash, 0, bytes.len())
                .expect("target bytes"),
            bytes
        );
        assert_eq!(
            source
                .outbound_changes(
                    &target.local_frontier().expect("target frontier"),
                    256,
                    16 * 1024 * 1024,
                )
                .expect("source replay")
                .changes
                .len(),
            0
        );
    }
}
