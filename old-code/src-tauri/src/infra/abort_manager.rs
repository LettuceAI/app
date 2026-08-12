use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

#[derive(Debug)]
pub struct AbortHandle {
    tx: Option<oneshot::Sender<()>>,
}

impl AbortHandle {
    pub fn new(tx: oneshot::Sender<()>) -> Self {
        Self { tx: Some(tx) }
    }

    pub fn abort(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(());
        }
    }
}

#[derive(Default)]
struct AbortRegistryState {
    handles: HashMap<String, Vec<AbortHandle>>,
    aborted: HashSet<String>,
}

#[derive(Clone)]
pub struct AbortRegistry {
    inner: Arc<Mutex<AbortRegistryState>>,
}

impl AbortRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AbortRegistryState::default())),
        }
    }

    pub fn register(&self, request_id: String) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();

        if let Ok(mut state) = self.inner.lock() {
            if state.aborted.contains(&request_id) {
                let _ = tx.send(());
                return rx;
            }
            state
                .handles
                .entry(request_id)
                .or_default()
                .push(AbortHandle::new(tx));
        }

        rx
    }

    pub fn abort(&self, request_id: &str) -> Result<(), String> {
        if let Ok(mut state) = self.inner.lock() {
            state.aborted.insert(request_id.to_string());
            if let Some(handles) = state.handles.remove(request_id) {
                for mut handle in handles {
                    handle.abort();
                }
            }
            Ok(())
        } else {
            Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Failed to acquire lock on abort registry",
            ))
        }
    }

    pub fn unregister(&self, request_id: &str) {
        if let Ok(mut state) = self.inner.lock() {
            let remove_entry = if let Some(handles) = state.handles.get_mut(request_id) {
                handles.pop();
                handles.is_empty()
            } else {
                false
            };
            if remove_entry {
                state.handles.remove(request_id);
            }
        }
    }

    pub fn take_aborted(&self, request_id: &str) -> bool {
        if let Ok(mut state) = self.inner.lock() {
            state.aborted.remove(request_id)
        } else {
            false
        }
    }

    pub fn abort_all(&self) {
        if let Ok(mut state) = self.inner.lock() {
            let pending: Vec<(String, Vec<AbortHandle>)> = state.handles.drain().collect();
            for (request_id, handles) in pending {
                state.aborted.insert(request_id);
                for mut handle in handles {
                    handle.abort();
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn is_registered(&self, request_id: &str) -> bool {
        if let Ok(state) = self.inner.lock() {
            state
                .handles
                .get(request_id)
                .map(|handles| !handles.is_empty())
                .unwrap_or(false)
        } else {
            false
        }
    }
}

impl Default for AbortRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abort_before_register_fires_receiver_and_keeps_flag() {
        let registry = AbortRegistry::new();
        registry.abort("req-1").unwrap();

        let mut rx = registry.register("req-1".to_string());
        assert!(rx.try_recv().is_ok());
        assert!(registry.take_aborted("req-1"));
        assert!(!registry.take_aborted("req-1"));
    }

    #[test]
    fn abort_after_register_fires_receiver() {
        let registry = AbortRegistry::new();
        let mut rx = registry.register("req-2".to_string());

        assert!(rx.try_recv().is_err());
        registry.abort("req-2").unwrap();
        assert!(rx.try_recv().is_ok());
        assert!(registry.take_aborted("req-2"));
    }

    #[test]
    fn register_without_abort_stays_pending() {
        let registry = AbortRegistry::new();
        let mut rx = registry.register("req-3".to_string());

        assert!(rx.try_recv().is_err());
        assert!(!registry.take_aborted("req-3"));
        registry.unregister("req-3");
    }
}
