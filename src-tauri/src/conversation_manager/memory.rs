use serde_json::Value;

use crate::chat_manager::types::MemoryEmbedding;

pub(crate) const USER_EDIT_EVENT_STATUS: &str = "user_edit";
pub(crate) const MEMORY_TOOL_EVENT_CAP: usize = 50;

pub(crate) fn event_is_reverted(event: &Value) -> bool {
    event.get("revertedAt").and_then(Value::as_u64).is_some()
}

pub(crate) fn event_advances_cursor(event: &Value) -> bool {
    !event_is_reverted(event)
        && !matches!(
            event.get("status").and_then(Value::as_str),
            Some("error") | Some(USER_EDIT_EVENT_STATUS)
        )
}

pub(crate) fn build_user_memory_edit_event(
    action: Value,
    conversation_count: usize,
    anchor_message_id: Option<String>,
) -> Value {
    let mut event = serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "windowStart": conversation_count,
        "windowEnd": conversation_count,
        "summary": "",
        "actions": [action],
        "status": USER_EDIT_EVENT_STATUS,
        "createdAt": crate::utils::now_millis().unwrap_or_default(),
    });
    if let Some(anchor) = anchor_message_id {
        event["windowMessageIds"] = serde_json::json!([anchor]);
    }
    event
}

pub(crate) struct ReplayedMemoryState {
    pub embeddings: Vec<MemoryEmbedding>,
    pub summary: String,
    pub summary_unchanged: bool,
    pub events: Vec<Value>,
}

pub(crate) fn replay_memory_state_after_rewind<F>(
    events: &[Value],
    source_embeddings: &[MemoryEmbedding],
    prior_summary: &str,
    remaining_conversation_count: usize,
    mut message_exists: F,
) -> Result<Option<ReplayedMemoryState>, String>
where
    F: FnMut(&str) -> Result<bool, String>,
{
    if events.is_empty() {
        return Ok(None);
    }

    let mut kept: Vec<Value> = Vec::new();
    let mut dropped: Vec<Value> = Vec::new();
    for event in events {
        let anchor = event
            .get("windowMessageIds")
            .and_then(Value::as_array)
            .and_then(|ids| ids.last())
            .and_then(Value::as_str);
        let keep = match anchor {
            Some(id) => message_exists(id)?,
            None => {
                event.get("windowEnd").and_then(Value::as_u64).unwrap_or(0) as usize
                    <= remaining_conversation_count
            }
        };
        if keep {
            kept.push(event.clone());
        } else {
            dropped.push(event.clone());
        }
    }
    if dropped.is_empty() {
        return Ok(None);
    }

    let mut active: Vec<MemoryEmbedding> = source_embeddings.to_vec();
    for event in dropped.iter().rev() {
        if event_is_reverted(event) {
            continue;
        }
        let Some(actions) = event.get("actions").and_then(Value::as_array) else {
            continue;
        };
        for action in actions.iter().rev() {
            if action
                .get("skipped")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                continue;
            }
            undo_action(action, &mut active);
        }
    }

    for memory in active.iter_mut() {
        match source_embeddings.iter().find(|source| source.id == memory.id) {
            Some(source) if source.text == memory.text => {}
            Some(_) | None => {
                if memory.embedding.is_empty() {
                    memory.embedding_source_version = None;
                    memory.embedding_dimensions = None;
                } else if source_embeddings
                    .iter()
                    .find(|source| source.id == memory.id)
                    .is_some_and(|source| source.text != memory.text)
                {
                    memory.embedding = Vec::new();
                    memory.embedding_source_version = None;
                    memory.embedding_dimensions = None;
                }
            }
        }
    }

    let summary = kept
        .iter()
        .rev()
        .find(|event| event_advances_cursor(event) && !event_is_reverted(event))
        .and_then(|event| event.get("summary").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();

    Ok(Some(ReplayedMemoryState {
        summary_unchanged: summary == prior_summary,
        embeddings: active,
        summary,
        events: kept,
    }))
}

fn action_string(action: &Value, key: &str) -> Option<String> {
    action
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn argument_string(action: &Value, key: &str) -> Option<String> {
    action
        .get("arguments")
        .and_then(|arguments| arguments.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn resolve_action_memory_id(action: &Value, active: &[MemoryEmbedding]) -> Option<String> {
    if let Some(id) = action_string(action, "memoryId") {
        return Some(id);
    }
    if let Some(id) = action_string(action, "deletedMemoryId") {
        return Some(id);
    }
    if let Some(id) = argument_string(action, "id") {
        return Some(id);
    }
    let text = argument_string(action, "text")?;
    if text.len() == 6
        && text.chars().all(|c| c.is_ascii_digit())
        && active.iter().any(|memory| memory.id == text)
    {
        return Some(text);
    }
    active
        .iter()
        .find(|memory| memory.text == text)
        .map(|memory| memory.id.clone())
}

fn find_active_index(active: &[MemoryEmbedding], id: &str) -> Option<usize> {
    active.iter().position(|memory| memory.id == id)
}

fn undo_action(action: &Value, active: &mut Vec<MemoryEmbedding>) {
    let Some(name) = action.get("name").and_then(Value::as_str) else {
        return;
    };
    let Some(id) = resolve_action_memory_id(action, active) else {
        return;
    };
    match name {
        "create_memory" => {
            if let Some(index) = find_active_index(active, &id) {
                active.remove(index);
            }
        }
        "update_memory" => {
            let Some(index) = find_active_index(active, &id) else {
                return;
            };
            if let Some(previous_text) = action_string(action, "previousText") {
                active[index].text = previous_text;
            }
            if let Some(previous) = action.get("previousCategory") {
                active[index].category = previous.as_str().map(str::to_string);
            }
        }
        "delete_memory" => {
            let confidence = action
                .get("arguments")
                .and_then(|arguments| arguments.get("confidence"))
                .and_then(Value::as_f64);
            let soft_delete = action
                .get("softDelete")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| confidence.is_some_and(|value| value < 0.7));
            if soft_delete {
                if let Some(index) = find_active_index(active, &id) {
                    active[index].is_cold = false;
                    active[index].importance_score = 1.0;
                }
                return;
            }
            if find_active_index(active, &id).is_some() {
                return;
            }
            let Some(snapshot) = action
                .get("memorySnapshot")
                .and_then(|value| serde_json::from_value::<MemoryEmbedding>(value.clone()).ok())
            else {
                return;
            };
            active.push(snapshot);
        }
        "pin_memory" => {
            if let Some(index) = find_active_index(active, &id) {
                active[index].is_pinned = action
                    .get("previousPinned")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
            }
        }
        "unpin_memory" => {
            if let Some(index) = find_active_index(active, &id) {
                active[index].is_pinned = action
                    .get("previousPinned")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
            }
        }
        "set_memory_cold" => {
            let Some(index) = find_active_index(active, &id) else {
                return;
            };
            let previous_cold = action
                .get("previousIsCold")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| {
                    !action
                        .get("arguments")
                        .and_then(|arguments| arguments.get("isCold"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                });
            active[index].is_cold = previous_cold;
            active[index].importance_score = if previous_cold { 0.0 } else { 1.0 };
        }
        "set_memory_observed_at" => {
            let Some(index) = find_active_index(active, &id) else {
                return;
            };
            active[index].observed_at = action.get("previousObservedAt").and_then(Value::as_u64);
            active[index].observed_time_precision = action
                .get("previousObservedTimePrecision")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        _ => {}
    }
}

pub(crate) fn resolve_last_valid_window_end<F>(
    events: &[Value],
    mut resolve_message_index: F,
) -> Result<(usize, bool), String>
where
    F: FnMut(&str) -> Result<Option<usize>, String>,
{
    if events.is_empty() {
        return Ok((0, false));
    }
    for (reverse_index, event) in events
        .iter()
        .rev()
        .filter(|event| event_advances_cursor(event))
        .enumerate()
    {
        let end_id = event
            .get("windowMessageIds")
            .and_then(Value::as_array)
            .and_then(|ids| ids.last())
            .and_then(Value::as_str);
        let Some(end_id) = end_id else {
            continue;
        };
        if let Some(window_end) = resolve_message_index(end_id)? {
            return Ok((window_end, reverse_index != 0));
        }
    }
    Ok((0, true))
}

pub(crate) fn next_window(
    last_window_end: usize,
    total_conversation_count: usize,
    window_size: usize,
    force_recent_window: bool,
) -> (usize, usize) {
    let start = if force_recent_window {
        total_conversation_count.saturating_sub(window_size)
    } else {
        last_window_end.min(total_conversation_count)
    };
    let end = if force_recent_window {
        total_conversation_count
    } else {
        start
            .saturating_add(window_size)
            .min(total_conversation_count)
    };
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::{
        event_advances_cursor, next_window, replay_memory_state_after_rewind,
        resolve_last_valid_window_end, MemoryEmbedding,
    };
    use serde_json::json;

    fn mk_memory(id: &str, text: &str) -> MemoryEmbedding {
        MemoryEmbedding {
            id: id.to_string(),
            text: text.to_string(),
            embedding: vec![0.5],
            created_at: 1,
            token_count: 3,
            is_cold: false,
            last_accessed_at: 1,
            importance_score: 1.0,
            persistence_importance: 1.0,
            prompt_importance: 1.0,
            volatility: 0.4,
            is_pinned: false,
            access_count: 0,
            embedding_source_version: Some("v4".to_string()),
            embedding_dimensions: Some(1024),
            match_score: None,
            category: None,
            observed_at: None,
            observed_time_precision: None,
            canonical_entities: Vec::new(),
            fact_signature: None,
            fact_polarity: None,
            source_role: None,
            source_message_id: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
        }
    }

    #[test]
    fn errors_do_not_advance_but_legacy_events_do() {
        assert!(!event_advances_cursor(&json!({ "status": "error" })));
        assert!(event_advances_cursor(&json!({ "status": "complete" })));
        assert!(event_advances_cursor(&json!({ "windowEnd": 2 })));
        assert!(!event_advances_cursor(&json!({ "status": "user_edit" })));
        assert!(!event_advances_cursor(
            &json!({ "status": "complete", "revertedAt": 123 })
        ));
    }

    #[test]
    fn replay_returns_none_without_dropped_events() {
        let events = vec![json!({
            "windowMessageIds": ["m1"],
            "summary": "S1",
            "actions": [],
        })];
        let replayed =
            replay_memory_state_after_rewind(&events, &[], "S1", 5, |_| Ok(true)).unwrap();
        assert!(replayed.is_none());
    }

    #[test]
    fn rewind_undoes_dropped_cycle_and_keeps_earlier_user_edit() {
        let snapshot = serde_json::to_value(mk_memory("100001", "alpha edited")).unwrap();
        let source = vec![mk_memory("100002", "beta")];
        let events = vec![
            json!({
                "windowMessageIds": ["m1"],
                "summary": "S1",
                "actions": [
                    { "name": "create_memory", "memoryId": "100001", "arguments": { "text": "alpha" } }
                ],
            }),
            json!({
                "windowMessageIds": ["m1"],
                "status": "user_edit",
                "summary": "",
                "actions": [
                    {
                        "name": "update_memory",
                        "memoryId": "100001",
                        "previousText": "alpha",
                        "arguments": { "text": "alpha edited" }
                    }
                ],
            }),
            json!({
                "windowMessageIds": ["m2"],
                "summary": "S2",
                "actions": [
                    { "name": "create_memory", "memoryId": "100002", "arguments": { "text": "beta" } },
                    {
                        "name": "delete_memory",
                        "deletedMemoryId": "100001",
                        "memorySnapshot": snapshot,
                        "arguments": { "confidence": 0.95 }
                    }
                ],
            }),
        ];

        let replayed =
            replay_memory_state_after_rewind(&events, &source, "S2", 2, |id| Ok(id == "m1"))
                .unwrap()
                .expect("state should change");

        assert_eq!(replayed.events.len(), 2);
        assert_eq!(replayed.summary, "S1");
        assert!(!replayed.summary_unchanged);
        assert_eq!(replayed.embeddings.len(), 1);
        let memory = &replayed.embeddings[0];
        assert_eq!(memory.id, "100001");
        assert_eq!(memory.text, "alpha edited");
        assert!(!memory.embedding.is_empty());
    }

    #[test]
    fn rewind_skips_already_reverted_dropped_events() {
        let snapshot = serde_json::to_value(mk_memory("100001", "alpha")).unwrap();
        let events = vec![
            json!({
                "windowMessageIds": ["m1"],
                "summary": "S1",
                "actions": [],
            }),
            json!({
                "windowMessageIds": ["m2"],
                "summary": "S2",
                "revertedAt": 123,
                "actions": [
                    {
                        "name": "delete_memory",
                        "deletedMemoryId": "100001",
                        "memorySnapshot": snapshot,
                        "arguments": { "confidence": 0.95 }
                    }
                ],
            }),
        ];
        let source = vec![mk_memory("100001", "alpha")];

        let replayed =
            replay_memory_state_after_rewind(&events, &source, "S1", 1, |id| Ok(id == "m1"))
                .unwrap()
                .expect("dropped event should still trim the log");

        assert_eq!(replayed.events.len(), 1);
        assert_eq!(replayed.embeddings.len(), 1);
        assert_eq!(replayed.embeddings[0].id, "100001");
        assert!(!replayed.embeddings[0].embedding.is_empty());
    }

    #[test]
    fn rewind_undoes_dropped_user_edit_and_clears_stale_embedding() {
        let source = vec![mk_memory("100001", "alpha edited")];
        let events = vec![
            json!({
                "windowMessageIds": ["m1"],
                "summary": "S1",
                "actions": [
                    { "name": "create_memory", "memoryId": "100001", "arguments": { "text": "alpha" } }
                ],
            }),
            json!({
                "windowMessageIds": ["m2"],
                "status": "user_edit",
                "summary": "",
                "actions": [
                    {
                        "name": "update_memory",
                        "memoryId": "100001",
                        "previousText": "alpha",
                        "arguments": { "text": "alpha edited" }
                    }
                ],
            }),
        ];

        let replayed =
            replay_memory_state_after_rewind(&events, &source, "S1", 1, |id| Ok(id == "m1"))
                .unwrap()
                .expect("state should change");

        assert_eq!(replayed.summary, "S1");
        assert!(replayed.summary_unchanged);
        assert_eq!(replayed.embeddings.len(), 1);
        let memory = &replayed.embeddings[0];
        assert_eq!(memory.text, "alpha");
        assert!(memory.embedding.is_empty());
        assert!(memory.embedding_source_version.is_none());
    }

    #[test]
    fn newest_valid_anchor_wins_and_reports_rewind() {
        let events = vec![
            json!({ "windowMessageIds": ["first"], "status": "complete" }),
            json!({ "windowMessageIds": ["failed"], "status": "error" }),
            json!({ "windowMessageIds": ["deleted"], "status": "complete" }),
        ];
        let resolved =
            resolve_last_valid_window_end(&events, |id| Ok((id == "first").then_some(4))).unwrap();
        assert_eq!(resolved, (4, true));
    }

    #[test]
    fn rewound_cycle_resumes_after_last_surviving_success() {
        let events = vec![
            json!({
                "windowMessageIds": ["m20"],
                "windowEnd": 20,
                "summary": "S20",
                "status": "complete",
                "actions": [],
            }),
            json!({
                "windowMessageIds": ["m40"],
                "windowEnd": 40,
                "summary": "S40",
                "status": "complete",
                "actions": [],
            }),
        ];
        let replayed =
            replay_memory_state_after_rewind(&events, &[], "S40", 32, |id| Ok(id == "m20"))
                .unwrap()
                .expect("latest cycle should be reverted");
        let (last_window_end, cursor_rewound) =
            resolve_last_valid_window_end(&events, |id| {
                Ok(match id {
                    "m20" => Some(20),
                    _ => None,
                })
            })
            .unwrap();

        assert!(cursor_rewound);
        assert_eq!(replayed.summary, "S20");
        assert_eq!(next_window(last_window_end, 32, 20, false), (20, 32));
    }

    #[test]
    fn catch_up_windows_are_bounded_for_local_models() {
        assert_eq!(next_window(20, 100, 20, false), (20, 40));
        assert_eq!(next_window(0, 100, 20, false), (0, 20));
        assert_eq!(next_window(20, 100, 20, true), (80, 100));
    }
}
