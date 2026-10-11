use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};

use lettuce_contracts::{ApiError, ApiErrorCode, ApiErrorDetails, BackupFailureReason};

use super::{ApiContext, error::api_error};

#[derive(Debug)]
pub struct ApiContextSlot {
    current: RwLock<Arc<ApiContext>>,
    recovery_required: AtomicBool,
}

fn recovery_error() -> ApiError {
    let mut error = api_error(ApiErrorCode::Unavailable, "database reload requires reopening the application");
    error.details = Some(ApiErrorDetails::Backup { reason: BackupFailureReason::RecoveryRequired });
    error
}

impl ApiContextSlot {
    pub fn new(context: Arc<ApiContext>) -> Self {
        Self {
            current: RwLock::new(context),
            recovery_required: AtomicBool::new(false),
        }
    }

    pub fn snapshot(&self) -> Result<Arc<ApiContext>, ApiError> {
        if self.recovery_required.load(Ordering::Acquire) {
            return Err(recovery_error());
        }
        let current = self.current.read().map_err(|_| recovery_error())?;
        if self.recovery_required.load(Ordering::Acquire) {
            return Err(recovery_error());
        }
        Ok(Arc::clone(&current))
    }

    pub async fn start_reloaded(&self, context: Arc<ApiContext>, job: &str) -> Result<(Arc<ApiContext>, super::ApiWorkers), ApiError> {
        let mut pending = None;
        let result: Result<_, ApiError> = async {
            context.bind_reloaded_job(job)?;
            pending = Some(super::startup(&context).await?);
            pending.as_ref().ok_or_else(recovery_error)?.started_result().await?;
            let old = self.swap(context.clone())?;
            Ok((old, pending.take().ok_or_else(recovery_error)?))
        }.await;
        if result.is_err() {
            self.require_recovery();
            let _ = Self::retire(&context, pending).await;
        }
        result.map_err(|_| recovery_error())
    }

    pub async fn retire(context: &ApiContext, workers: Option<super::ApiWorkers>) -> Result<(), ApiError> {
        if let Some(workers) = workers { workers.stop_for_restore().await }
        else {
            context.retire_database_listeners();
            context.begin_shutdown();
            context.backend().shutdown().await;
            Ok(())
        }
    }

    pub(super) fn swap(&self, context: Arc<ApiContext>) -> Result<Arc<ApiContext>, ApiError> {
        let mut current = self.current.write().map_err(|_| recovery_error())?;
        if self.recovery_required.load(Ordering::Acquire) {
            return Err(recovery_error());
        }
        Ok(std::mem::replace(&mut *current, context))
    }

    pub fn require_recovery(&self) {
        self.recovery_required.store(true, Ordering::Release);
    }
}
