use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
struct ApprovalState {
    prompted_at: Option<usize>,
    pending: bool,
    pending_count: usize,
    skipped: bool,
}

#[derive(Clone, Default)]
pub struct DynamicMemoryApprovalManager {
    inner: Arc<Mutex<HashMap<String, ApprovalState>>>,
}

impl DynamicMemoryApprovalManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn should_prompt(
        &self,
        session_id: &str,
        total_convo: usize,
        last_window_end: usize,
        window_size: usize,
    ) -> Option<usize> {
        let mut map = self.inner.lock().ok()?;
        let state = map.entry(session_id.to_string()).or_default();
        let baseline = state.prompted_at.unwrap_or(last_window_end);
        if total_convo.saturating_sub(baseline) < window_size {
            return None;
        }
        state.prompted_at = Some(total_convo);
        state.pending = true;
        state.pending_count = total_convo.saturating_sub(last_window_end);
        Some(state.pending_count)
    }

    pub fn mark_skipped(&self, session_id: &str) {
        if let Ok(mut map) = self.inner.lock() {
            if let Some(state) = map.get_mut(session_id) {
                state.pending = false;
                state.skipped = true;
            }
        }
    }

    pub fn pending(&self, session_id: &str) -> Option<usize> {
        let map = self.inner.lock().ok()?;
        map.get(session_id)
            .filter(|state| state.pending)
            .map(|state| state.pending_count)
    }

    pub fn was_skipped(&self, session_id: &str) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|map| map.get(session_id).map(|state| state.skipped))
            .unwrap_or(false)
    }

    pub fn clear(&self, session_id: &str) {
        if let Ok(mut map) = self.inner.lock() {
            map.remove(session_id);
        }
    }
}
