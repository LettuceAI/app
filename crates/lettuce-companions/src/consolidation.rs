use lettuce_conversations::ResolvedInferenceProfile;
use lettuce_conversations::{ProposedToolCall, ToolChoice, ToolDefinition, ToolRequest};
use lettuce_types::{CharacterId, ConversationId, JobId, OperationRecordId, TimestampMillis};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    CONSOLIDATION_THRESHOLD, CompanionSoulIdentity, ProposedSoulFact, SoulCategory, SoulFactKind,
    SoulFactPolicy, SoulState,
};

pub const CONSOLIDATE_SOUL_TOOL_NAME: &str = "consolidate_soul";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolidationPromptValues {
    pub authored_core: String,
    pub current_core: String,
    pub accumulated_growth: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsolidationProposal {
    pub core_adjustments: Vec<ProposedSoulFact>,
    pub retire_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionConsolidationRun {
    pub job_id: JobId,
    pub growth_job_id: JobId,
    pub conversation_id: ConversationId,
    pub character_id: CharacterId,
    pub profile: ResolvedInferenceProfile,
    pub companion_name: String,
    pub authored_soul: CompanionSoulIdentity,
    pub soul: SoulState,
    pub operation_id: OperationRecordId,
    pub created_at: TimestampMillis,
    pub proposal_checkpoint: Option<CompanionConsolidationProposalCheckpoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionConsolidationProposalCheckpoint {
    pub proposal: ConsolidationProposal,
    pub reduced_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompanionConsolidationRunRepositoryError {
    #[error("companion consolidation run was not found")]
    NotFound,
    #[error("companion consolidation run conflicts with durable state")]
    Conflict,
    #[error("companion consolidation run is invalid")]
    Invalid,
    #[error("companion consolidation run storage failed")]
    Failure,
    #[error("companion consolidation run storage is corrupt")]
    Corrupt,
}

pub trait CompanionConsolidationRunRepository: Send + Sync {
    fn admit_companion_consolidation_run(
        &self,
        run: CompanionConsolidationRun,
    ) -> Result<CompanionConsolidationRun, CompanionConsolidationRunRepositoryError>;

    fn load_companion_consolidation_run(
        &self,
        job_id: JobId,
    ) -> Result<CompanionConsolidationRun, CompanionConsolidationRunRepositoryError>;

    fn load_companion_consolidation_run_for_growth(
        &self,
        growth_job_id: JobId,
    ) -> Result<CompanionConsolidationRun, CompanionConsolidationRunRepositoryError>;

    fn commit_companion_consolidation_proposal(
        &self,
        job_id: JobId,
        checkpoint: CompanionConsolidationProposalCheckpoint,
    ) -> Result<CompanionConsolidationRun, CompanionConsolidationRunRepositoryError>;
}

impl CompanionConsolidationRun {
    pub fn validate(&self) -> Result<(), CompanionConsolidationRunRepositoryError> {
        if self.job_id == self.growth_job_id
            || self.companion_name.trim().is_empty()
            || crate::validate_state(&self.soul).is_err()
            || !consolidation_ready(&self.soul, self.created_at)
            || self.created_at.get() < 0
        {
            return Err(CompanionConsolidationRunRepositoryError::Invalid);
        }
        if let Some(checkpoint) = &self.proposal_checkpoint {
            checkpoint.validate()?;
        }
        Ok(())
    }
}

impl CompanionConsolidationProposalCheckpoint {
    pub fn validate(&self) -> Result<(), CompanionConsolidationRunRepositoryError> {
        let mut proposal_ids = std::collections::HashSet::new();
        if self.reduced_at.get() < 0
            || self.proposal.core_adjustments.iter().any(|proposal| {
                proposal.id.trim().is_empty() || !proposal_ids.insert(proposal.id.as_str())
            })
            || self
                .proposal
                .retire_ids
                .iter()
                .any(|id| id.trim().is_empty())
        {
            return Err(CompanionConsolidationRunRepositoryError::Invalid);
        }
        Ok(())
    }
}

#[must_use]
pub fn consolidation_ready(state: &SoulState, effective_at: TimestampMillis) -> bool {
    state
        .facts
        .iter()
        .filter(|fact| {
            fact.is_effective_at(effective_at)
                && fact.category.is_changeable()
                && !fact.id.trim().is_empty()
        })
        .count()
        >= CONSOLIDATION_THRESHOLD
}

#[must_use]
pub fn consolidation_prompt_values(
    soul: &CompanionSoulIdentity,
    state: &SoulState,
    effective_at: TimestampMillis,
) -> ConsolidationPromptValues {
    let authored_core = [
        (SoulCategory::Essence, soul.essence.as_str()),
        (SoulCategory::Traits, soul.traits.as_str()),
    ]
    .into_iter()
    .map(|(category, value)| {
        format!(
            "- {}: {}\n",
            category.as_str(),
            if value.trim().is_empty() {
                "(empty)"
            } else {
                value.trim()
            }
        )
    })
    .collect();
    let active = state
        .facts
        .iter()
        .filter(|fact| fact.is_effective_at(effective_at) && !fact.id.trim().is_empty())
        .collect::<Vec<_>>();
    let current_core = format_entries(
        active
            .iter()
            .copied()
            .filter(|fact| matches!(fact.category, SoulCategory::Essence | SoulCategory::Traits)),
    );
    let accumulated_growth = format_entries(
        active
            .iter()
            .copied()
            .filter(|fact| fact.category.is_changeable()),
    );
    ConsolidationPromptValues {
        authored_core,
        current_core,
        accumulated_growth,
    }
}

fn format_entries<'a>(entries: impl Iterator<Item = &'a crate::SoulFact>) -> String {
    let rendered = entries
        .map(|entry| {
            format!(
                "- id={} [{} confidence={:.2} weight={:.2}{}]: {}\n",
                entry.id,
                entry.category.as_str(),
                entry.confidence,
                entry.weight,
                if entry.locked { " locked" } else { "" },
                entry.value.trim()
            )
        })
        .collect::<String>();
    if rendered.is_empty() {
        "(none)".to_owned()
    } else {
        rendered
    }
}

#[must_use]
pub fn consolidation_tool_request() -> ToolRequest {
    ToolRequest {
        definitions: vec![ToolDefinition {
            name: CONSOLIDATE_SOUL_TOOL_NAME.to_owned(),
            description: Some(
                "Fold accumulated companion growth only when sustained, high-confidence evidence warrants a very slow core change. Locked facts may inform the result but must never be retired or superseded. Both arrays may be empty."
                    .to_owned(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "coreAdjustments": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "category": { "type": "string", "enum": ["essence", "traits"] },
                                "value": { "type": "string" },
                                "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
                                "weight": { "type": "number", "minimum": 0, "maximum": 1 },
                                "supersedes": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["category", "value", "confidence", "weight"]
                        }
                    },
                    "retire": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["coreAdjustments", "retire"]
            }),
            version: 1,
        }],
        choice: ToolChoice::Required,
    }
}

#[must_use]
pub fn parse_consolidation_proposal(
    calls: &[ProposedToolCall],
    fallback_text: Option<&str>,
) -> ConsolidationProposal {
    let arguments = calls
        .iter()
        .find(|call| call.name == CONSOLIDATE_SOUL_TOOL_NAME)
        .map(|call| call.arguments.clone())
        .or_else(|| fallback_arguments(fallback_text?));
    let Some(arguments) = arguments else {
        return ConsolidationProposal {
            core_adjustments: Vec::new(),
            retire_ids: Vec::new(),
        };
    };
    let core_adjustments = arguments
        .get("coreAdjustments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_core_adjustment)
        .collect();
    ConsolidationProposal {
        core_adjustments,
        retire_ids: string_array(arguments.get("retire")),
    }
}

fn parse_core_adjustment(item: &Value) -> Option<ProposedSoulFact> {
    let category = match item.get("category").and_then(Value::as_str) {
        Some("essence") => SoulCategory::Essence,
        Some("traits") => SoulCategory::Traits,
        _ => return None,
    };
    let value = item.get("value")?.as_str()?.trim().to_owned();
    if value.is_empty() {
        return None;
    }
    Some(ProposedSoulFact {
        id: Uuid::new_v4().to_string(),
        category,
        value,
        kind: SoulFactKind::Consolidated,
        policy: SoulFactPolicy::Adaptive,
        slot: "core".to_owned(),
        confidence: item
            .get("confidence")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
        weight: item.get("weight").and_then(Value::as_f64).unwrap_or(0.0),
        valid_until: None,
        locked: false,
        source_memory_ids: Vec::new(),
        supersedes: string_array(item.get("supersedes")),
    })
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

fn fallback_arguments(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&text[start..=end]).ok()
}

#[cfg(test)]
mod tests {
    use lettuce_types::Revision;

    use super::*;
    use crate::{SoulFact, prepare_consolidation_change_set};

    fn fact(index: usize, category: SoulCategory, locked: bool) -> SoulFact {
        SoulFact {
            id: format!("fact-{index}"),
            category,
            value: format!("value-{index}"),
            kind: SoulFactKind::Add,
            policy: SoulFactPolicy::Adaptive,
            slot: format!("slot-{index}"),
            confidence: 0.9,
            evidence_count: 1,
            weight: 0.8,
            valid_from: TimestampMillis::new(1),
            valid_until: None,
            locked,
            source_memory_ids: vec![format!("memory-{index}")],
            created_at: TimestampMillis::new(1),
            supersedes: Vec::new(),
            superseded_by: None,
            superseded_at: None,
        }
    }

    fn call(name: &str, arguments: Value) -> ProposedToolCall {
        ProposedToolCall {
            provider_call_id: None,
            name: name.to_owned(),
            arguments,
            raw_arguments: None,
            provider_replay: None,
        }
    }

    #[test]
    fn required_tool_contract_matches_legacy() {
        let request = consolidation_tool_request();
        assert_eq!(request.choice, ToolChoice::Required);
        assert_eq!(request.definitions.len(), 1);
        assert_eq!(request.definitions[0].name, CONSOLIDATE_SOUL_TOOL_NAME);
        assert_eq!(
            request.definitions[0].parameters["properties"]["coreAdjustments"]["items"]["properties"]
                ["category"]["enum"],
            json!(["essence", "traits"])
        );
        request.validate().expect("valid tool request");
    }

    #[test]
    fn threshold_counts_only_active_changeable_facts() {
        let now = TimestampMillis::new(10);
        let mut facts = (0..11)
            .map(|index| fact(index, SoulCategory::Habits, false))
            .collect::<Vec<_>>();
        facts.push(fact(11, SoulCategory::Essence, false));
        let state = SoulState {
            revision: Revision::INITIAL,
            facts,
        };
        assert!(!consolidation_ready(&state, now));
        let mut ready = state;
        ready.facts.push(fact(12, SoulCategory::Likes, false));
        assert!(consolidation_ready(&ready, now));
    }

    #[test]
    fn prompt_values_copy_legacy_core_and_growth_format() {
        let soul = CompanionSoulIdentity {
            essence: "  Gentle  ".into(),
            ..CompanionSoulIdentity::default()
        };
        let state = SoulState {
            revision: Revision::INITIAL,
            facts: vec![
                fact(0, SoulCategory::Traits, true),
                fact(1, SoulCategory::Habits, false),
            ],
        };
        let values = consolidation_prompt_values(&soul, &state, TimestampMillis::new(2));
        assert_eq!(
            values.authored_core,
            "- essence: Gentle\n- traits: (empty)\n"
        );
        assert_eq!(
            values.current_core,
            "- id=fact-0 [traits confidence=0.90 weight=0.80 locked]: value-0\n"
        );
        assert_eq!(
            values.accumulated_growth,
            "- id=fact-1 [habits confidence=0.90 weight=0.80]: value-1\n"
        );
    }

    #[test]
    fn first_call_and_fallback_copy_legacy_parser() {
        let proposal = parse_consolidation_proposal(
            &[
                call("ignored", json!({})),
                call(
                    CONSOLIDATE_SOUL_TOOL_NAME,
                    json!({
                        "coreAdjustments": [
                            {"category": "essence", "value": "  More open  ", "confidence": 0.9, "weight": 0.8, "supersedes": ["old-core", ""]},
                            {"category": "likes", "value": "ignored", "confidence": 1, "weight": 1}
                        ],
                        "retire": ["fact-1", ""]
                    }),
                ),
                call(
                    CONSOLIDATE_SOUL_TOOL_NAME,
                    json!({"coreAdjustments": [], "retire": ["ignored"]}),
                ),
            ],
            Some(r#"{"coreAdjustments":[],"retire":["fallback"]}"#),
        );
        assert_eq!(proposal.core_adjustments.len(), 1);
        assert_eq!(proposal.core_adjustments[0].category, SoulCategory::Essence);
        assert_eq!(proposal.core_adjustments[0].value, "More open");
        assert_eq!(proposal.core_adjustments[0].supersedes, ["old-core"]);
        assert_eq!(proposal.retire_ids, ["fact-1"]);

        let fallback = parse_consolidation_proposal(
            &[],
            Some(r#"prefix {"coreAdjustments":[],"retire":["fact-2"]} suffix"#),
        );
        assert_eq!(fallback.retire_ids, ["fact-2"]);
    }

    #[test]
    fn existing_reducer_keeps_locked_retirements_and_core_filters_authoritative() {
        let now = TimestampMillis::new(2);
        let facts = (0..12)
            .map(|index| fact(index, SoulCategory::Habits, index == 0))
            .collect::<Vec<_>>();
        let state = SoulState {
            revision: Revision::INITIAL,
            facts,
        };
        let proposal = parse_consolidation_proposal(
            &[call(
                CONSOLIDATE_SOUL_TOOL_NAME,
                json!({
                    "coreAdjustments": [
                        {"category": "traits", "value": "Patient", "confidence": 0.9, "weight": 0.8},
                        {"category": "habits", "value": "ignored", "confidence": 1, "weight": 1}
                    ],
                    "retire": ["fact-0", "fact-1"]
                }),
            )],
            None,
        );
        let change = prepare_consolidation_change_set(
            &state,
            state.revision,
            proposal.core_adjustments,
            proposal.retire_ids,
            now,
        )
        .expect("prepare consolidation");
        assert_eq!(change.additions.len(), 1);
        assert_eq!(change.additions[0].category, SoulCategory::Traits);
        assert_eq!(change.supersessions.len(), 1);
        assert_eq!(change.supersessions[0].fact_id, "fact-1");
    }
}
