use std::collections::HashSet;

use lettuce_conversations::MessageRole;
use lettuce_types::{MemoryId, MessageId, TimestampMillis};
use serde::{Deserialize, Serialize};

use crate::{MemoryCategory, MemoryChangeSet, MemoryItem, MemoryOrigin, MemoryPolicy, MemoryShortId, MemorySpaceSnapshot, Score};

pub const CONSOLIDATION_MAX_GROUPS: usize = 4;
pub const CONSOLIDATION_MAX_PROPOSALS: usize = 16;
pub const CONSOLIDATION_MAX_REPLACEMENTS: usize = 4;
pub const CONSOLIDATION_MAX_INPUT_BYTES: usize = 8192;
pub const CONSOLIDATION_MAX_OUTPUT_TOKENS: u32 = 2048;
pub const CONSOLIDATION_MAX_OUTPUT_BYTES: usize = 16384;
pub const CONSOLIDATION_REQUEST_SECONDS: u64 = 20;
pub const CONSOLIDATION_PHASE_SECONDS: u64 = 90;
pub const CONSOLIDATION_MEMORY_BYTES: usize = 280;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidationSource {
    pub message_id: MessageId,
    pub role: MessageRole,
    pub observed_at: TimestampMillis,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConsolidationEvidence {
    Base { quote: String },
    Message { message_id: MessageId, quote: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidationDetail {
    pub text: String,
    pub evidence: ConsolidationEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidationReplacement {
    pub category: MemoryCategory,
    pub details: Vec<ConsolidationDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConsolidationResponse {
    NoChange,
    Replace {
        base_preserved: bool,
        no_conflicts: bool,
        new_information: bool,
        replacements: Vec<ConsolidationReplacement>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationStatus { Pending, Dispatched, Ready, Applied, NoChange, Skipped, Rejected }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidationApplied {
    pub revision: lettuce_types::Revision,
    pub at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidationGroup {
    pub base_id: MemoryId,
    pub base: Option<MemoryItem>,
    pub proposals: Vec<String>,
    pub status: ConsolidationStatus,
    pub response: Option<ConsolidationResponse>,
    /// Full one-to-many successor membership. Every member also has a
    /// reverse `supersedes` edge to the base; the old singular field is only
    /// a compatibility anchor and is never the sole record of successors.
    pub replacement_ids: Vec<MemoryId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied: Option<ConsolidationApplied>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidationPhase {
    pub groups: Vec<ConsolidationGroup>,
    pub sources: Vec<ConsolidationSource>,
    pub omitted_groups: usize,
}

#[derive(Debug, Clone)]
pub struct ConsolidationCommit {
    pub run_id: lettuce_types::DynamicMemoryRunId,
    pub attempt_id: lettuce_types::DynamicMemoryAttemptId,
    pub expected: ConsolidationGroup,
    pub next: ConsolidationGroup,
    pub replacements: Vec<MemoryItem>,
    pub policy: MemoryPolicy,
    pub embedding_revision: String,
    pub embedding_dimensions: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolidationText {
    pub text: String,
    pub category: MemoryCategory,
    pub source_message_id: Option<MessageId>,
}

impl ConsolidationStatus {
    pub fn can_transition(self, next: Self) -> bool {
        matches!((self, next),
            (Self::Pending, Self::Dispatched | Self::Skipped | Self::Rejected)
            | (Self::Dispatched, Self::Ready | Self::Skipped | Self::Rejected)
            | (Self::Ready, Self::Applied | Self::NoChange | Self::Skipped | Self::Rejected))
    }
}

impl ConsolidationGroup {
    pub fn finish(mut self, status: ConsolidationStatus, reason: &str) -> Self {
        self.status = status;
        self.reason = Some(reason.to_owned());
        self
    }
}

/// Validates structure and cited spans, not semantic entailment or factual
/// completeness. Those remain the LLM's judgment and regression-eval gates.
pub fn consolidation_texts(group: &ConsolidationGroup, sources: &[ConsolidationSource]) -> Result<Vec<ConsolidationText>, &'static str> {
    let base = group.base.as_ref().ok_or("missing_base")?;
    let Some(ConsolidationResponse::Replace { base_preserved: true, no_conflicts: true, new_information: true, replacements }) = &group.response else { return Err("no_safe_enrichment"); };
    if replacements.is_empty() || replacements.len() > CONSOLIDATION_MAX_REPLACEMENTS { return Err("replacement_count"); }
    let mut base_cited = false;
    let mut addition_cited = false;
    let mut texts = Vec::new();
    let mut unique = HashSet::new();
    for replacement in replacements {
        if replacement.details.is_empty() || replacement.details.len() > 3 { return Err("detail_count"); }
        let mut source = None;
        for detail in &replacement.details {
            if detail.text.trim().is_empty() { return Err("empty_detail"); }
            match &detail.evidence {
                ConsolidationEvidence::Base { quote } => {
                    if quote.trim().is_empty() || !base.text.contains(quote) { return Err("invalid_base_evidence"); }
                    base_cited = true;
                }
                ConsolidationEvidence::Message { message_id, quote } => {
                    if quote.trim().is_empty() || !sources.iter().any(|source| source.message_id == *message_id && source.text.contains(quote)) { return Err("unsupported_evidence"); }
                    addition_cited = true;
                    source = Some(*message_id);
                }
            }
        }
        let raw = replacement.details.iter().map(|detail| detail.text.as_str()).collect::<Vec<_>>().join(" ");
        // Check the bytes before normalization; never shorten a response to
        // make it fit. The ordinary normalizer currently counts characters.
        if raw.len() > CONSOLIDATION_MEMORY_BYTES { return Err("text_bytes"); }
        let text = crate::normalize_memory_text(&raw).map_err(|_| "invalid_text")?;
        if text != raw.split_whitespace().collect::<Vec<_>>().join(" ") { return Err("rewritten_during_validation"); }
        if !unique.insert(text.clone()) { return Err("repeated_replacement"); }
        texts.push(ConsolidationText { text, category: replacement.category, source_message_id: source });
    }
    if !base_cited || !addition_cited { return Err("missing_evidence"); }
    if texts.len() == 1 && texts[0].text == base.text { return Err("no_enrichment"); }
    Ok(texts)
}

/// Controlled replacement only: ordinary create-memory deduplication is not
/// called. The caller commits this complete change and the receipt together.
pub fn replace_duplicate_group(snapshot: &MemorySpaceSnapshot, group: &ConsolidationGroup, replacements: Vec<MemoryItem>, policy: &MemoryPolicy, at: TimestampMillis) -> Result<MemoryChangeSet, &'static str> {
    let base = group.base.as_ref().ok_or("missing_base")?;
    if base.id != group.base_id || base.superseded_by.is_some() || !snapshot.items.iter().any(|item| item == base) { return Err("stale_base"); }
    if replacements.is_empty() || replacements.len() > CONSOLIDATION_MAX_REPLACEMENTS { return Err("replacement_count"); }
    if snapshot.items.len().saturating_add(replacements.len()) > policy.max_entries { return Err("capacity"); }
    let mut items = snapshot.items.clone();
    let anchor = replacements[0].id;
    for mut item in replacements {
        if item.text.len() > CONSOLIDATION_MEMORY_BYTES || item.validate().is_err() || item.token_count.is_none() || items.iter().any(|old| old.id == item.id) { return Err("invalid_replacement"); }
        item.short_id = MemoryShortId::allocate(item.id, |id| items.iter().any(|old| old.short_id == id));
        item.is_pinned = base.is_pinned;
        item.is_cold = false;
        item.origin = MemoryOrigin::Model;
        item.supersedes = vec![base.id];
        item.superseded_by = None;
        item.superseded_at = None;
        item.importance = Score::FULL;
        items.push(item);
    }
    let old = items.iter_mut().find(|item| item.id == base.id).ok_or("missing_base")?;
    old.is_pinned = false;
    old.is_cold = true;
    old.superseded_by = Some(anchor);
    old.superseded_at = Some(at);
    let hot = items.iter().filter(|item| !item.is_cold).try_fold(0u32, |sum, item| item.token_count.map(|count| sum.saturating_add(count))).ok_or("unknown_token_count")?;
    if hot > policy.hot_token_budget { return Err("hot_budget"); }
    let change = MemoryChangeSet { space_id: snapshot.id, expected_revision: snapshot.revision, items };
    change.validate().map_err(|_| "invalid_replacement")?;
    Ok(change)
}

/// Source deletion and cycle revert undo the complete operation. Later user
/// edits retain ownership of their items/fields, just as ordinary tools do.
pub fn undo_duplicate_consolidation(items: &mut Vec<MemoryItem>, group: &ConsolidationGroup, retained: &[crate::MemoryManualHistory]) {
    let (Some(base), Some(applied)) = (&group.base, &group.applied) else { return; };
    let outcomes = group.replacement_ids.iter().map(|id| crate::MemoryToolOutcome::Created {
        id: *id, short_id: MemoryShortId::derived(*id), memories: Vec::new(),
    }).collect::<Vec<_>>();
    crate::undo_memory_tool_outcomes_preserving(items, std::slice::from_ref(base), &outcomes, applied.revision, retained);
    let remaining = items.iter().find(|item| item.supersedes.contains(&base.id)).map(|item| item.id);
    if let Some(successor) = remaining {
        if let Some(item) = items.iter_mut().find(|item| item.id == base.id)
            && (item.superseded_by.is_none() || item.superseded_by.is_some_and(|id| group.replacement_ids.contains(&id)))
        { item.superseded_by = Some(successor); item.superseded_at = Some(applied.at); }
        return;
    }
    let edits = retained.iter().filter(|edit| edit.resulting_revision > applied.revision && edit.before_item.iter().chain(&edit.after_item).any(|item| item.id == base.id)).collect::<Vec<_>>();
    if edits.iter().any(|edit| matches!(edit.edit.mutation, crate::MemoryManualMutation::Delete { .. })) { return; }
    if !items.iter().any(|item| item.id == base.id) {
        let mut restored = base.clone();
        restored.short_id = MemoryShortId::allocate(base.id, |short| items.iter().any(|item| item.short_id == short));
        items.push(restored);
        return;
    }
    let item = items.iter_mut().find(|item| item.id == base.id).expect("base exists");
    if item.superseded_by.is_some_and(|id| !group.replacement_ids.contains(&id)) { return; }
    item.superseded_by = base.superseded_by; item.superseded_at = base.superseded_at;
    let pin_owned = edits.iter().any(|edit| matches!(edit.edit.mutation, crate::MemoryManualMutation::Pin { .. } | crate::MemoryManualMutation::Add { .. }));
    let temperature_owned = edits.iter().any(|edit| matches!(edit.edit.mutation, crate::MemoryManualMutation::Temperature { .. } | crate::MemoryManualMutation::Pin { pinned: true, .. } | crate::MemoryManualMutation::Add { .. }));
    if !pin_owned { item.is_pinned = base.is_pinned; }
    if !temperature_owned { item.is_cold = base.is_cold; item.importance = base.importance; }
    if item.is_pinned { item.is_cold = false; }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_types::{MemorySpaceId, Revision};

    fn item(text: &str) -> MemoryItem {
        let id = MemoryId::new();
        let mut item = MemoryItem::written(id, MemoryShortId::derived(id), text.into(), TimestampMillis::new(10));
        item.token_count = Some(5);
        item
    }

    fn fixture() -> (ConsolidationGroup, Vec<ConsolidationSource>, MemorySpaceSnapshot, MemoryPolicy) {
        let mut base = item("Mara works at the hospital."); base.is_pinned = true;
        let source = ConsolidationSource { message_id: MessageId::new(), role: MessageRole::User, observed_at: TimestampMillis::new(11), text: "Mara works night shifts at the hospital.".into() };
        let group = ConsolidationGroup {
            base_id: base.id, base: Some(base.clone()), proposals: vec![source.text.clone(), source.text.clone()], status: ConsolidationStatus::Ready,
            response: Some(ConsolidationResponse::Replace { base_preserved: true, no_conflicts: true, new_information: true, replacements: vec![ConsolidationReplacement { category: MemoryCategory::Profile, details: vec![
                ConsolidationDetail { text: base.text.clone(), evidence: ConsolidationEvidence::Base { quote: base.text.clone() } },
                ConsolidationDetail { text: "She works night shifts.".into(), evidence: ConsolidationEvidence::Message { message_id: source.message_id, quote: "night shifts".into() } },
            ] }] }), replacement_ids: Vec::new(), applied: None, reason: None,
        };
        let snapshot = MemorySpaceSnapshot { id: MemorySpaceId::new(), revision: Revision::INITIAL, items: vec![base] };
        let policy = MemoryPolicy { max_entries: 10, hot_token_budget: 1000, cold_threshold: Score::ZERO, delete_confidence_default: Score::FULL, max_hard_delete_ratio_per_cycle: Score::ZERO, decay_rate: Score::ZERO };
        (group, vec![source], snapshot, policy)
    }

    #[test]
    fn replacement_set_moves_pin_priority_and_retains_the_cold_base() {
        let (group, _, snapshot, policy) = fixture();
        let outputs = vec![item("Mara works at the hospital."), item("Mara works night shifts.")];
        let ids = outputs.iter().map(|item| item.id).collect::<Vec<_>>();
        let change = replace_duplicate_group(&snapshot, &group, outputs, &policy, TimestampMillis::new(12)).expect("replacement");
        let base = &change.items[0];
        assert!(base.is_cold && !base.is_pinned);
        assert_eq!(base.superseded_by, Some(ids[0]));
        for item in &change.items[1..] { assert!(item.is_pinned && !item.is_cold); assert_eq!(item.supersedes, vec![base.id]); }
    }

    #[test]
    fn unpinned_base_cannot_gain_a_model_selected_pin() {
        let (mut group, _, mut snapshot, policy) = fixture();
        snapshot.items[0].is_pinned = false; group.base = Some(snapshot.items[0].clone());
        let mut output = item("Mara works at night."); output.is_pinned = true;
        let change = replace_duplicate_group(&snapshot, &group, vec![output], &policy, TimestampMillis::new(12)).expect("replacement");
        assert!(change.items.iter().all(|item| !item.is_pinned));
    }

    #[test]
    fn stale_base_and_insufficient_capacity_leave_the_entire_group_unchanged() {
        let (group, _, mut snapshot, mut policy) = fixture();
        let before = snapshot.clone();
        policy.max_entries = 1;
        assert_eq!(replace_duplicate_group(&snapshot, &group, vec![item("new")], &policy, TimestampMillis::new(12)), Err("capacity"));
        assert_eq!(snapshot, before);
        policy.max_entries = 10; snapshot.items[0].text = "A concurrent edit".into();
        assert_eq!(replace_duplicate_group(&snapshot, &group, vec![item("new")], &policy, TimestampMillis::new(12)), Err("stale_base"));
    }

    #[test]
    fn complete_group_must_fit_the_hot_budget_without_demoting_a_replacement() {
        let (group, _, snapshot, mut policy) = fixture(); policy.hot_token_budget = 9;
        assert_eq!(replace_duplicate_group(&snapshot, &group, vec![item("one"), item("two")], &policy, TimestampMillis::new(12)), Err("hot_budget"));
    }

    #[test]
    fn evidence_is_checked_against_frozen_sources_and_all_outputs_are_validated() {
        let (mut group, sources, _, _) = fixture();
        assert!(consolidation_texts(&group, &sources).is_ok());
        let Some(ConsolidationResponse::Replace { replacements, .. }) = &mut group.response else { panic!("response"); };
        replacements[0].details[1].evidence = ConsolidationEvidence::Message { message_id: MessageId::new(), quote: "night shifts".into() };
        assert_eq!(consolidation_texts(&group, &sources), Err("unsupported_evidence"));
    }

    #[test]
    fn byte_limit_is_independent_of_declared_detail_count_and_unicode_characters() {
        let (mut group, sources, _, _) = fixture();
        let Some(ConsolidationResponse::Replace { replacements, .. }) = &mut group.response else { panic!("response"); };
        let mut extra = replacements[0].details[1].clone(); extra.text = "é".repeat(140);
        replacements[0].details.truncate(1);
        replacements.push(ConsolidationReplacement { category: MemoryCategory::Profile, details: vec![extra] });
        assert!(consolidation_texts(&group, &sources).is_ok());
        let Some(ConsolidationResponse::Replace { replacements, .. }) = &mut group.response else { panic!("response"); };
        replacements[1].details[0].text.push('é');
        assert_eq!(consolidation_texts(&group, &sources), Err("text_bytes"));
    }

    #[test]
    fn unresolved_conflicts_and_four_declared_details_reject_the_complete_set() {
        let (mut group, sources, _, _) = fixture();
        let Some(ConsolidationResponse::Replace { no_conflicts, .. }) = &mut group.response else { panic!("response"); }; *no_conflicts = false;
        assert_eq!(consolidation_texts(&group, &sources), Err("no_safe_enrichment"));
        let Some(ConsolidationResponse::Replace { no_conflicts, replacements, .. }) = &mut group.response else { panic!("response"); }; *no_conflicts = true;
        let extra = replacements[0].details[0].clone(); replacements[0].details.extend([extra.clone(), extra]);
        assert_eq!(consolidation_texts(&group, &sources), Err("detail_count"));
    }
}
