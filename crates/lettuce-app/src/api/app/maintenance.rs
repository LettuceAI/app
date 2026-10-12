use std::sync::Arc;

use lettuce_jobs::handle::CancellationToken;
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

#[derive(Debug, Default)]
pub(in crate::api) struct MaintenanceGate {
    execution: Arc<RwLock<()>>,
}

impl MaintenanceGate {
    pub(in crate::api) fn try_work(&self) -> Option<OwnedRwLockReadGuard<()>> {
        self.execution.clone().try_read_owned().ok()
    }

    pub(in crate::api) async fn work(
        &self,
        cancellation: &CancellationToken,
    ) -> Option<OwnedRwLockReadGuard<()>> {
        tokio::select! {
            biased;
            () = cancellation.cancelled() => None,
            lease = self.execution.clone().read_owned() => Some(lease),
        }
    }

    pub(in crate::api) async fn maintenance(
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
