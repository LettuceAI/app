use crate::abort_manager::AbortRegistry;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone)]
pub struct DynamicMemoryCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl DynamicMemoryCancellationToken {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

struct DynamicMemoryRunState {
    cancelled: Arc<AtomicBool>,
    active_request_id: Option<String>,
}

#[derive(Clone)]
pub struct DynamicMemoryRunManager {
    inner: Arc<Mutex<HashMap<String, DynamicMemoryRunState>>>,
}

pub struct DynamicMemoryRunGuard {
    manager: DynamicMemoryRunManager,
    key: String,
    token: DynamicMemoryCancellationToken,
}

impl DynamicMemoryRunGuard {
    pub fn token(&self) -> DynamicMemoryCancellationToken {
        self.token.clone()
    }

    pub fn set_active_request_id(&self, request_id: Option<String>) {
        self.manager.set_active_request_id(&self.key, request_id);
    }
}

impl Drop for DynamicMemoryRunGuard {
    fn drop(&mut self) {
        self.manager.finish_run(&self.key);
    }
}

impl DynamicMemoryRunManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start_run(&self, key: String) -> DynamicMemoryRunGuard {
        let cancelled = Arc::new(AtomicBool::new(false));
        let token = DynamicMemoryCancellationToken {
            cancelled: cancelled.clone(),
        };

        if let Ok(mut map) = self.inner.lock() {
            map.insert(
                key.clone(),
                DynamicMemoryRunState {
                    cancelled,
                    active_request_id: None,
                },
            );
        }

        DynamicMemoryRunGuard {
            manager: self.clone(),
            key,
            token,
        }
    }

    pub fn cancel_run(&self, abort_registry: &AbortRegistry, key: &str) -> Result<(), String> {
        let active_request_id = {
            let mut map = self.inner.lock().map_err(|_| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    "Failed to acquire lock on dynamic memory run manager",
                )
            })?;

            let state = map
                .get_mut(key)
                .ok_or_else(|| format!("Dynamic memory run for {} is not active", key))?;

            state.cancelled.store(true, Ordering::SeqCst);
            state.active_request_id.clone()
        };

        if let Some(request_id) = active_request_id {
            let _ = abort_registry.abort(&request_id);
        }

        Ok(())
    }

    pub fn is_run_active(&self, key: &str) -> bool {
        self.inner
            .lock()
            .map(|map| map.contains_key(key))
            .unwrap_or(false)
    }

    fn set_active_request_id(&self, key: &str, request_id: Option<String>) {
        if let Ok(mut map) = self.inner.lock() {
            if let Some(state) = map.get_mut(key) {
                state.active_request_id = request_id;
            }
        }
    }

    fn finish_run(&self, key: &str) {
        if let Ok(mut map) = self.inner.lock() {
            map.remove(key);
        }
    }
}

impl Default for DynamicMemoryRunManager {
    fn default() -> Self {
        Self::new()
    }
}
