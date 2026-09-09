use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_sync::{
    CausalFrontier, IncomingBatchState, IncomingChangeError, IncomingChangeRepository,
    LocalChangeJournal, LocalChangeJournalError, SyncBatchAcknowledgement, SyncChangeBatch,
    SyncChangeFrame, SyncDeviceId, SyncHello, SyncSessionError, negotiate_sync_session,
};
use lettuce_types::{OperationId, TimestampMillis};

#[async_trait]
pub trait AuthenticatedSyncTransport: Send {
    fn authenticated_peer(&self) -> SyncDeviceId;

    async fn exchange_hello(
        &mut self,
        local: SyncHello,
        cancellation: &CancellationToken,
    ) -> Result<SyncHello, SyncTransportError>;

    async fn exchange_frontier(
        &mut self,
        local: CausalFrontier,
        cancellation: &CancellationToken,
    ) -> Result<CausalFrontier, SyncTransportError>;

    async fn exchange_changes(
        &mut self,
        local: SyncChangeFrame,
        cancellation: &CancellationToken,
    ) -> Result<SyncChangeFrame, SyncTransportError>;

    async fn exchange_acknowledgement(
        &mut self,
        local: SyncBatchAcknowledgement,
        cancellation: &CancellationToken,
    ) -> Result<SyncBatchAcknowledgement, SyncTransportError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncTransportError {
    #[error("sync transport was cancelled")]
    Cancelled,
    #[error("sync peer disconnected")]
    Disconnected,
    #[error("sync transport protocol failed")]
    Protocol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncExchangeReport {
    pub peer: SyncDeviceId,
    pub sent: usize,
    pub received: usize,
    pub conflicts: usize,
    pub final_frontier: CausalFrontier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncExchangeOutcome {
    Complete(SyncExchangeReport),
    Pending {
        report: SyncExchangeReport,
        batch_id: OperationId,
    },
}

#[derive(Debug)]
pub struct SyncExchangeCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: ?Sized> SyncExchangeCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }
}

impl<R> SyncExchangeCoordinator<'_, R>
where
    R: LocalChangeJournal + IncomingChangeRepository + ?Sized,
{
    pub async fn run<T>(
        &self,
        local_hello: SyncHello,
        transport: &mut T,
        cancellation: &CancellationToken,
        now: TimestampMillis,
    ) -> Result<SyncExchangeOutcome, SyncExchangeError>
    where
        T: AuthenticatedSyncTransport + ?Sized,
    {
        check_cancelled(cancellation)?;
        let peer_hello = transport
            .exchange_hello(local_hello.clone(), cancellation)
            .await
            .map_err(SyncExchangeError::Transport)?;
        check_cancelled(cancellation)?;
        let negotiated =
            negotiate_sync_session(&local_hello, &peer_hello, transport.authenticated_peer())
                .map_err(SyncExchangeError::Session)?;
        let local_frontier = self
            .repository
            .local_frontier()
            .map_err(SyncExchangeError::Journal)?;
        let mut remote_frontier = transport
            .exchange_frontier(local_frontier, cancellation)
            .await
            .map_err(SyncExchangeError::Transport)?;
        let mut report = SyncExchangeReport {
            peer: negotiated.peer_device_id,
            sent: 0,
            received: 0,
            conflicts: 0,
            final_frontier: CausalFrontier::new(),
        };

        loop {
            check_cancelled(cancellation)?;
            let outbound = self
                .repository
                .outbound_changes(
                    &remote_frontier,
                    negotiated.limits.max_changes_per_batch(),
                    negotiated.limits.max_batch_payload_bytes(),
                )
                .map_err(SyncExchangeError::Journal)?;
            let sent_batch_id = (!outbound.changes.is_empty()).then(OperationId::new);
            let sent_count = outbound.changes.len();
            let sent_extent = outbound
                .changes
                .last()
                .map(|change| (change.origin_device(), change.origin_sequence()));
            let outgoing = match sent_batch_id {
                Some(batch_id) => SyncChangeFrame::Batch(
                    SyncChangeBatch::new(batch_id, outbound.changes, negotiated.limits)
                        .map_err(SyncExchangeError::Session)?,
                ),
                None => SyncChangeFrame::Quiescent {
                    frontier: self
                        .repository
                        .local_frontier()
                        .map_err(SyncExchangeError::Journal)?,
                },
            };
            let incoming = transport
                .exchange_changes(outgoing, cancellation)
                .await
                .map_err(SyncExchangeError::Transport)?;
            check_cancelled(cancellation)?;
            let (received_batch_id, received_quiescent) = match incoming {
                SyncChangeFrame::Batch(batch) => {
                    let batch_id = batch.batch_id();
                    self.repository
                        .stage_incoming_batch(
                            negotiated.peer_device_id,
                            batch_id,
                            batch.batch_hash(),
                            batch.changes(),
                            now,
                        )
                        .map_err(SyncExchangeError::Incoming)?;
                    let result = self
                        .repository
                        .apply_incoming_batch(batch_id, now)
                        .map_err(SyncExchangeError::Incoming)?;
                    if result.state != IncomingBatchState::Committed {
                        report.final_frontier = result.frontier;
                        return Ok(SyncExchangeOutcome::Pending { report, batch_id });
                    }
                    report.received = report.received.saturating_add(result.applied);
                    report.conflicts = report.conflicts.saturating_add(result.conflicts);
                    (Some(batch_id), false)
                }
                SyncChangeFrame::Quiescent { .. } => (None, true),
            };
            check_cancelled(cancellation)?;
            let local_frontier = self
                .repository
                .local_frontier()
                .map_err(SyncExchangeError::Journal)?;
            let acknowledgement = transport
                .exchange_acknowledgement(
                    SyncBatchAcknowledgement {
                        batch_id: received_batch_id,
                        frontier: local_frontier.clone(),
                    },
                    cancellation,
                )
                .await
                .map_err(SyncExchangeError::Transport)?;
            if acknowledgement.batch_id != sent_batch_id {
                return Err(SyncExchangeError::AcknowledgementMismatch);
            }
            check_cancelled(cancellation)?;
            remote_frontier = self
                .repository
                .record_peer_acknowledgement(
                    negotiated.peer_device_id,
                    &acknowledgement.frontier,
                    now,
                )
                .map_err(SyncExchangeError::Journal)?;
            if sent_extent.is_some_and(|(device, sequence)| {
                remote_frontier.get(&device).copied().unwrap_or(0) < sequence
            }) {
                return Err(SyncExchangeError::AcknowledgementDidNotAdvance);
            }
            report.sent = report.sent.saturating_add(sent_count);
            report.final_frontier = local_frontier;
            if sent_batch_id.is_none() && received_quiescent {
                return Ok(SyncExchangeOutcome::Complete(report));
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncExchangeError {
    #[error("sync exchange was cancelled")]
    Cancelled,
    #[error("sync session negotiation failed: {0}")]
    Session(SyncSessionError),
    #[error("sync journal failed: {0}")]
    Journal(LocalChangeJournalError),
    #[error("incoming sync apply failed: {0}")]
    Incoming(IncomingChangeError),
    #[error("sync transport failed: {0}")]
    Transport(SyncTransportError),
    #[error("sync peer acknowledged a different batch")]
    AcknowledgementMismatch,
    #[error("sync peer acknowledgement did not cover the sent batch")]
    AcknowledgementDidNotAdvance,
}

fn check_cancelled(cancellation: &CancellationToken) -> Result<(), SyncExchangeError> {
    if cancellation.is_cancelled() {
        Err(SyncExchangeError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_characters::{Persona, PersonaDraftUpdate, PersonaRepository};
    use lettuce_database::Database;
    use lettuce_sync::{
        CanonicalPayload, ChangeOperation, ConflictChoice, NewCanonicalChange,
        PersonaConflictRepository, SyncEntity, SyncSessionId, SyncTransferLimits,
    };
    use lettuce_types::PersonaId;

    struct DatabasePeerTransport<'a> {
        remote: &'a Database,
        remote_hello: SyncHello,
        local_device: SyncDeviceId,
        local_frontier: CausalFrontier,
        received_local_batch: Option<OperationId>,
        sent_remote_batch: Option<OperationId>,
        now: TimestampMillis,
    }

    impl<'a> DatabasePeerTransport<'a> {
        fn new(
            remote: &'a Database,
            remote_hello: SyncHello,
            local_device: SyncDeviceId,
            now: TimestampMillis,
        ) -> Self {
            Self {
                remote,
                remote_hello,
                local_device,
                local_frontier: CausalFrontier::new(),
                received_local_batch: None,
                sent_remote_batch: None,
                now,
            }
        }
    }

    #[async_trait]
    impl AuthenticatedSyncTransport for DatabasePeerTransport<'_> {
        fn authenticated_peer(&self) -> SyncDeviceId {
            self.remote_hello.device_id()
        }

        async fn exchange_hello(
            &mut self,
            _: SyncHello,
            _: &CancellationToken,
        ) -> Result<SyncHello, SyncTransportError> {
            Ok(self.remote_hello.clone())
        }

        async fn exchange_frontier(
            &mut self,
            local: CausalFrontier,
            _: &CancellationToken,
        ) -> Result<CausalFrontier, SyncTransportError> {
            self.local_frontier = local;
            self.remote
                .local_frontier()
                .map_err(|_| SyncTransportError::Protocol)
        }

        async fn exchange_changes(
            &mut self,
            local: SyncChangeFrame,
            _: &CancellationToken,
        ) -> Result<SyncChangeFrame, SyncTransportError> {
            self.received_local_batch = match local {
                SyncChangeFrame::Batch(batch) => {
                    let batch_id = batch.batch_id();
                    self.remote
                        .stage_incoming_batch(
                            self.local_device,
                            batch_id,
                            batch.batch_hash(),
                            batch.changes(),
                            self.now,
                        )
                        .map_err(|_| SyncTransportError::Protocol)?;
                    let applied = self
                        .remote
                        .apply_incoming_batch(batch_id, self.now)
                        .map_err(|_| SyncTransportError::Protocol)?;
                    if applied.state != IncomingBatchState::Committed {
                        return Err(SyncTransportError::Protocol);
                    }
                    Some(batch_id)
                }
                SyncChangeFrame::Quiescent { .. } => None,
            };
            let outbound = self
                .remote
                .outbound_changes(
                    &self.local_frontier,
                    self.remote_hello.limits().max_changes_per_batch(),
                    self.remote_hello.limits().max_batch_payload_bytes(),
                )
                .map_err(|_| SyncTransportError::Protocol)?;
            if outbound.changes.is_empty() {
                self.sent_remote_batch = None;
                Ok(SyncChangeFrame::Quiescent {
                    frontier: self
                        .remote
                        .local_frontier()
                        .map_err(|_| SyncTransportError::Protocol)?,
                })
            } else {
                let batch_id = OperationId::new();
                self.sent_remote_batch = Some(batch_id);
                SyncChangeBatch::new(batch_id, outbound.changes, self.remote_hello.limits())
                    .map(SyncChangeFrame::Batch)
                    .map_err(|_| SyncTransportError::Protocol)
            }
        }

        async fn exchange_acknowledgement(
            &mut self,
            local: SyncBatchAcknowledgement,
            _: &CancellationToken,
        ) -> Result<SyncBatchAcknowledgement, SyncTransportError> {
            if local.batch_id != self.sent_remote_batch {
                return Err(SyncTransportError::Protocol);
            }
            self.local_frontier = self
                .remote
                .record_peer_acknowledgement(self.local_device, &local.frontier, self.now)
                .map_err(|_| SyncTransportError::Protocol)?;
            Ok(SyncBatchAcknowledgement {
                batch_id: self.received_local_batch,
                frontier: self
                    .remote
                    .local_frontier()
                    .map_err(|_| SyncTransportError::Protocol)?,
            })
        }
    }

    fn hello(
        database: &Database,
        device_name: &str,
        session: u128,
        now: TimestampMillis,
    ) -> SyncHello {
        crate::SyncHelloCoordinator::new(database)
            .build(
                "1.2.3",
                device_name,
                SyncSessionId::from_uuid(uuid::Uuid::from_u128(session)),
                SyncTransferLimits::default(),
                now,
            )
            .expect("hello")
    }

    async fn exchange(
        local: &Database,
        remote: &Database,
        session: u128,
        now: TimestampMillis,
    ) -> SyncExchangeOutcome {
        let local_hello = hello(local, "Local", session, now);
        let remote_hello = hello(remote, "Remote", session + 1, now);
        let mut transport =
            DatabasePeerTransport::new(remote, remote_hello, local_hello.device_id(), now);
        SyncExchangeCoordinator::new(local)
            .run(local_hello, &mut transport, &CancellationToken::new(), now)
            .await
            .expect("exchange")
    }

    #[tokio::test]
    async fn file_backed_peers_converge_replay_resolutions_and_retain_unknown_changes() {
        let left_path =
            std::env::temp_dir().join(format!("sync-exchange-left-{}.sqlite3", OperationId::new()));
        let right_path = std::env::temp_dir().join(format!(
            "sync-exchange-right-{}.sqlite3",
            OperationId::new()
        ));
        let left = Database::open(&left_path).expect("left database");
        let right = Database::open(&right_path).expect("right database");
        let persona_id = PersonaId::new();
        let created = PersonaRepository::create(
            &left,
            Persona::new(
                persona_id,
                "Shared persona".into(),
                "Base description".into(),
                TimestampMillis::new(10),
            )
            .expect("persona"),
        )
        .expect("create persona");

        assert!(matches!(
            exchange(&left, &right, 100, TimestampMillis::new(20)).await,
            SyncExchangeOutcome::Complete(_)
        ));
        assert_eq!(
            PersonaRepository::get(&right, persona_id).expect("right persona"),
            Some(created.clone())
        );

        let left_revision = PersonaRepository::revise(
            &left,
            persona_id,
            created.revision,
            PersonaDraftUpdate {
                title: "Left title".into(),
                description: created.description.clone(),
                nickname: None,
                design_description: None,
                avatar_crop: None,
                image_recommendation: None,
            },
            TimestampMillis::new(30),
        )
        .expect("left revision");
        PersonaRepository::revise(
            &right,
            persona_id,
            created.revision,
            PersonaDraftUpdate {
                title: "Right title".into(),
                description: created.description,
                nickname: None,
                design_description: None,
                avatar_crop: None,
                image_recommendation: None,
            },
            TimestampMillis::new(40),
        )
        .expect("right revision");
        let conflict_exchange = exchange(&left, &right, 200, TimestampMillis::new(50)).await;
        assert!(matches!(
            conflict_exchange,
            SyncExchangeOutcome::Complete(_)
        ));
        let conflicts = left
            .unresolved_persona_conflicts(100)
            .expect("left conflicts");
        assert_eq!(conflicts.len(), 1);
        left.resolve_persona_conflict(
            conflicts[0].id,
            conflicts[0].current.change_id,
            ConflictChoice::Current,
            OperationId::new(),
            TimestampMillis::new(60),
        )
        .expect("resolve conflict");
        assert!(matches!(
            exchange(&left, &right, 300, TimestampMillis::new(70)).await,
            SyncExchangeOutcome::Complete(_)
        ));
        let converged_left = PersonaRepository::get(&left, persona_id)
            .expect("left persona")
            .expect("left value");
        assert_eq!(
            PersonaRepository::get(&right, persona_id).expect("right persona"),
            Some(converged_left.clone())
        );
        assert!(converged_left.revision > left_revision.revision);
        assert!(
            right
                .unresolved_persona_conflicts(100)
                .expect("right conflicts")
                .is_empty()
        );

        drop(left);
        drop(right);
        let left = Database::open(&left_path).expect("reopen left");
        let right = Database::open(&right_path).expect("reopen right");
        let replay = exchange(&left, &right, 400, TimestampMillis::new(80)).await;
        let SyncExchangeOutcome::Complete(report) = replay else {
            panic!("expected complete replay");
        };
        assert_eq!(report.sent, 0);
        assert_eq!(report.received, 0);

        right
            .record_local_change(
                OperationId::new(),
                NewCanonicalChange::new(
                    SyncEntity::new("future", "future-1").expect("future entity"),
                    ChangeOperation::Insert,
                    None,
                    Some(
                        CanonicalPayload::new("future.snapshot", 1, b"future".to_vec())
                            .expect("future payload"),
                    ),
                )
                .expect("future change"),
                TimestampMillis::new(90),
            )
            .expect("record future change");
        assert!(matches!(
            exchange(&left, &right, 500, TimestampMillis::new(100)).await,
            SyncExchangeOutcome::Pending { .. }
        ));

        drop(left);
        drop(right);
        std::fs::remove_file(left_path).expect("remove left database");
        std::fs::remove_file(right_path).expect("remove right database");
    }

    #[tokio::test]
    async fn cancellation_before_exchange_keeps_both_peers_untouched() {
        let local = Database::open_in_memory().expect("local database");
        let remote = Database::open_in_memory().expect("remote database");
        let local_hello = hello(&local, "Local", 600, TimestampMillis::new(1));
        let remote_hello = hello(&remote, "Remote", 601, TimestampMillis::new(1));
        let mut transport = DatabasePeerTransport::new(
            &remote,
            remote_hello,
            local_hello.device_id(),
            TimestampMillis::new(2),
        );
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        assert_eq!(
            SyncExchangeCoordinator::new(&local)
                .run(
                    local_hello,
                    &mut transport,
                    &cancellation,
                    TimestampMillis::new(2)
                )
                .await,
            Err(SyncExchangeError::Cancelled)
        );
        assert!(local.local_frontier().expect("local frontier").is_empty());
        assert!(remote.local_frontier().expect("remote frontier").is_empty());
    }
}
