use std::collections::BTreeMap;

use lettuce_types::MemoryId;
use serde::{Deserialize, Serialize};

use crate::{DynamicMemoryBackgroundRoundSettlement, DynamicMemoryInferenceRound, DynamicMemoryRun, MemoryToolOutcome};

/// Evidence for review, not a claim that any proposed addition is true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryDuplicateCandidate {
    pub existing_text: Option<String>,
    pub proposed_texts: Vec<String>,
}

pub type MemoryDuplicateCandidates = BTreeMap<MemoryId, MemoryDuplicateCandidate>;

/// Reconstructs only settled calls, in durable round/call order. Repeated
/// proposal strings are deliberately retained, including after recovery.
pub fn collect_duplicate_candidates(
    run: &DynamicMemoryRun,
    rounds: &[DynamicMemoryInferenceRound],
    settlements: &[DynamicMemoryBackgroundRoundSettlement],
) -> MemoryDuplicateCandidates {
    let mut texts = run.starting_memory.items.iter()
        .map(|item| (item.id, item.text.clone())).collect::<BTreeMap<_, _>>();
    let mut result = MemoryDuplicateCandidates::new();
    let mut ordered = rounds.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|round| round.ordinal);
    for round in ordered {
        if round.run_id != run.id { continue; }
        let Some(settlement) = settlements.iter().find(|settlement| {
            settlement.run_id == run.id && settlement.attempt_id == round.attempt_id
                && settlement.round_ordinal == round.ordinal && settlement.space_id == run.space_id
        }) else { continue; };
        let mut calls = round.calls.iter().collect::<Vec<_>>();
        calls.sort_by_key(|call| call.ordinal);
        for call in calls {
            if call.call.name != "create_memory" { continue; }
            let Some(text) = call.call.arguments.get("text").and_then(serde_json::Value::as_str) else { continue; };
            let Some(outcome) = settlement.results.iter().find(|result| result.execution_id == call.id).map(|result| &result.outcome) else { continue; };
            match outcome {
                MemoryToolOutcome::Created { id, short_id, memories } => {
                    if let Some(memory) = memories.iter().find(|memory| memory.short_id == *short_id) {
                        texts.insert(*id, memory.text.clone());
                    }
                }
                MemoryToolOutcome::DuplicateSkipped { existing_id, .. } => {
                    result.entry(*existing_id).or_insert_with(|| MemoryDuplicateCandidate {
                        existing_text: texts.get(existing_id).cloned(), proposed_texts: Vec::new(),
                    }).proposed_texts.push(text.to_owned());
                }
                _ => {}
            }
        }
    }
    result
}
