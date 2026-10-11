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

    pub fn swap(&self, context: Arc<ApiContext>) -> Result<Arc<ApiContext>, ApiError> {
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
