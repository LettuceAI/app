use lettuce_sync::{
    LocalChangeJournal, LocalChangeJournalError, SyncHello, SyncSessionError, SyncSessionId,
    SyncTransferLimits,
};
use lettuce_types::TimestampMillis;

#[derive(Debug)]
pub struct SyncHelloCoordinator<'a, J: ?Sized> {
    journal: &'a J,
}

impl<'a, J: ?Sized> SyncHelloCoordinator<'a, J> {
    #[must_use]
    pub const fn new(journal: &'a J) -> Self {
        Self { journal }
    }
}

impl<J> SyncHelloCoordinator<'_, J>
where
    J: LocalChangeJournal + ?Sized,
{
    pub fn build(
        &self,
        app_version: impl Into<String>,
        device_name: impl Into<String>,
        session_id: SyncSessionId,
        limits: SyncTransferLimits,
        now: TimestampMillis,
    ) -> Result<SyncHello, SyncHelloCoordinatorError> {
        let device_id = self
            .journal
            .local_device_id(now)
            .map_err(SyncHelloCoordinatorError::Journal)?;
        SyncHello::current(app_version, device_id, device_name, session_id, limits)
            .map_err(SyncHelloCoordinatorError::Session)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncHelloCoordinatorError {
    #[error("sync device identity is unavailable: {0}")]
    Journal(LocalChangeJournalError),
    #[error("sync hello is invalid: {0}")]
    Session(SyncSessionError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppBackend;
    use lettuce_sync::SYNC_PROTOCOL_VERSION;
    use lettuce_types::OperationId;

    #[test]
    fn application_hello_keeps_device_identity_and_rotates_session_identity() {
        let path = std::env::temp_dir().join(format!("sync-hello-{}.sqlite3", OperationId::new()));
        let first_session = SyncSessionId::new();
        let first = {
            let backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("open");
            backend
                .sync_hello()
                .build(
                    "1.2.3",
                    "Desktop",
                    first_session,
                    SyncTransferLimits::default(),
                    TimestampMillis::new(2),
                )
                .expect("first hello")
        };
        let backend = AppBackend::open(&path, TimestampMillis::new(3)).expect("reopen");
        let second = backend
            .sync_hello()
            .build(
                "1.2.3",
                "Desktop renamed",
                SyncSessionId::new(),
                SyncTransferLimits::default(),
                TimestampMillis::new(4),
            )
            .expect("second hello");

        assert_eq!(first.device_id(), second.device_id());
        assert_ne!(first.session_id(), second.session_id());
        assert_eq!(second.protocol_version(), SYNC_PROTOCOL_VERSION);
        drop(backend);
        std::fs::remove_file(path).expect("remove database");
    }
}
