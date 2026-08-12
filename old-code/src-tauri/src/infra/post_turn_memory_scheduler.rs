use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct PostTurnMemoryJob {
    pub session_id: String,
    pub user_message_id: Option<String>,
    pub assistant_message_id: String,
    pub enqueued_at: u64,
    pub track_effect: bool,
    pub relationship_delta: serde_json::Value,
    pub emotion_delta: serde_json::Value,
    pub signal_changes: serde_json::Value,
}

#[derive(Clone, Debug, Default)]
struct SessionQueueState {
    active: bool,
    dirty: bool,
    pending: Vec<PostTurnMemoryJob>,
}

#[derive(Clone, Default)]
pub struct PostTurnMemoryScheduler {
    inner: Arc<Mutex<HashMap<String, SessionQueueState>>>,
}

impl PostTurnMemoryScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&self, job: PostTurnMemoryJob) -> bool {
        let mut map = match self.inner.lock() {
            Ok(map) => map,
            Err(_) => return false,
        };

        let state = map.entry(job.session_id.clone()).or_default();
        state.dirty = true;
        state.pending.push(job);
        if state.active {
            return false;
        }

        state.active = true;
        true
    }

    pub fn begin_iteration(&self, session_id: &str) -> Vec<PostTurnMemoryJob> {
        if let Ok(mut map) = self.inner.lock() {
            if let Some(state) = map.get_mut(session_id) {
                state.dirty = false;
                return std::mem::take(&mut state.pending);
            }
        }
        Vec::new()
    }

    pub fn finish_iteration(&self, session_id: &str) -> bool {
        let mut map = match self.inner.lock() {
            Ok(map) => map,
            Err(_) => return false,
        };

        let Some(state) = map.get_mut(session_id) else {
            return false;
        };

        if state.dirty {
            return true;
        }

        map.remove(session_id);
        false
    }
}
