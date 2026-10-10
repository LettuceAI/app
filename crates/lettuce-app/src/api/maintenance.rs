use std::sync::Arc;

use lettuce_jobs::handle::CancellationToken;
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

#[derive(Debug, Default)]
pub(super) struct MaintenanceGate {
    execution: Arc<RwLock<()>>,
}

impl MaintenanceGate {
    pub(super) async fn work(
        &self,
        cancellation: &CancellationToken,
    ) -> Option<OwnedRwLockReadGuard<()>> {
        tokio::select! {
            biased;
            () = cancellation.cancelled() => None,
            lease = self.execution.clone().read_owned() => Some(lease),
        }
    }

    pub(super) async fn maintenance(
        &self,
        cancellation: &CancellationToken,
    ) -> Option<OwnedRwLockWriteGuard<()>> {
        tokio::select! {
            biased;
            () = cancellation.cancelled() => None,
            lease = self.execution.clone().write_owned() => Some(lease),
        }
    }
}
