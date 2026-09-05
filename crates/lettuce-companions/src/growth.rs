use std::collections::HashSet;

use lettuce_conversations::{
    ProposedToolCall, ResolvedInferenceProfile, ToolChoice, ToolDefinition, ToolRequest,
};
use lettuce_types::{
    CharacterId, ConversationId, DynamicMemoryAttemptId, DynamicMemoryRunId, JobId,
    OperationRecordId, TimestampMillis,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    CompanionSoulIdentity, ProposedSoulFact, SoulCategory, SoulFactKind, SoulFactPolicy, SoulState,
    effective_soul_value,
};

pub const MAX_GROWTH_MEMORIES: usize = 16;
pub const RECORD_GROWTH_TOOL_NAME: &str = "record_growth";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowthMemoryEvidence {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionGrowthRun {
    pub job_id: JobId,
    pub conversation_id: ConversationId,
    pub character_id: CharacterId,
    pub memory_run_id: DynamicMemoryRunId,
    pub memory_attempt_id: DynamicMemoryAttemptId,
    pub profile: ResolvedInferenceProfile,
    pub companion_name: String,
    pub authored_soul: CompanionSoulIdentity,
    pub soul: SoulState,
    pub fresh_memories: Vec<GrowthMemoryEvidence>,
    pub operation_id: OperationRecordId,
    pub created_at: TimestampMillis,
    pub proposal_checkpoint: Option<CompanionGrowthProposalCheckpoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionGrowthProposalCheckpoint {
    #[serde(default)]
    pub usage: Option<lettuce_conversations::InferenceUsage>,
    pub proposals: Vec<ProposedSoulFact>,
    pub reduced_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompanionGrowthRunRepositoryError {
    #[error("companion growth run was not found")]
    NotFound,
    #[error("companion growth run conflicts with durable state")]
    Conflict,
    #[error("companion growth run is invalid")]
    Invalid,
    #[error("companion growth run storage failed")]
    Failure,
    #[error("companion growth run storage is corrupt")]
    Corrupt,
}

pub trait CompanionGrowthRunRepository: Send + Sync {
    fn admit_companion_growth_run(
        &self,
        run: CompanionGrowthRun,
    ) -> Result<CompanionGrowthRun, CompanionGrowthRunRepositoryError>;

    fn load_companion_growth_run(
        &self,
        job_id: JobId,
    ) -> Result<CompanionGrowthRun, CompanionGrowthRunRepositoryError>;

    fn commit_companion_growth_proposals(
        &self,
        job_id: JobId,
        checkpoint: CompanionGrowthProposalCheckpoint,
    ) -> Result<CompanionGrowthRun, CompanionGrowthRunRepositoryError>;
}

impl CompanionGrowthRun {
    pub fn validate(&self) -> Result<(), CompanionGrowthRunRepositoryError> {
        let mut memory_ids = HashSet::new();
        if self.companion_name.trim().is_empty()
            || self.fresh_memories.is_empty()
            || self.fresh_memories.len() > MAX_GROWTH_MEMORIES
            || self.fresh_memories.iter().any(|memory| {
                memory.id.trim().is_empty()
                    || memory.text.trim().is_empty()
                    || !memory_ids.insert(memory.id.as_str())
            })
            || crate::validate_state(&self.soul).is_err()
            || self.created_at.get() < 0
        {
            return Err(CompanionGrowthRunRepositoryError::Invalid);
        }
        if let Some(checkpoint) = &self.proposal_checkpoint {
            checkpoint.validate()?;
        }
        Ok(())
    }
}

impl CompanionGrowthProposalCheckpoint {
    pub fn validate(&self) -> Result<(), CompanionGrowthRunRepositoryError> {
        let mut ids = HashSet::new();
        if self.reduced_at.get() < 0
            || self
                .proposals
                .iter()
                .any(|proposal| proposal.id.trim().is_empty() || !ids.insert(&proposal.id))
        {
            return Err(CompanionGrowthRunRepositoryError::Invalid);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrowthPromptValues {
    pub changeable_categories: String,
    pub current_growth: String,
    pub new_memories: String,
}

#[must_use]
pub fn growth_prompt_values(
    soul: &CompanionSoulIdentity,
    state: &SoulState,
    fresh_memories: &[GrowthMemoryEvidence],
    effective_at: TimestampMillis,
) -> GrowthPromptValues {
    let categories = [
        (
            SoulCategory::Appearance,
            "Appearance",
            soul.appearance.as_str(),
        ),
        (SoulCategory::Goals, "Goals", soul.goals.as_str()),
        (SoulCategory::Likes, "Likes", soul.likes.as_str()),
        (SoulCategory::Voice, "Voice", soul.voice.as_str()),
        (
            SoulCategory::RelationalStyle,
            "Relational style",
            soul.relational_style.as_str(),
        ),
        (
            SoulCategory::Vulnerabilities,
            "Vulnerabilities",
            soul.vulnerabilities.as_str(),
        ),
        (SoulCategory::Fears, "Fears", soul.fears.as_str()),
        (SoulCategory::Habits, "Habits", soul.habits.as_str()),
        (
            SoulCategory::Boundaries,
            "Boundaries",
            soul.boundaries.as_str(),
        ),
    ];
    let changeable_categories = categories
        .into_iter()
        .map(|(category, label, base)| {
            let current = effective_soul_value(base, category, state, effective_at);
            format!(
                "- {label} [{}]: {}\n",
                category.as_str(),
                if current.trim().is_empty() {
                    "(empty)"
                } else {
                    current.trim()
                }
            )
        })
        .collect();
    let current_growth = {
        let rendered = state
            .facts
            .iter()
            .filter(|fact| fact.is_effective_at(effective_at))
            .map(|fact| {
                format!(
                    "- id={} [{} policy={} slot={} confidence={:.2} weight={:.2}{}]: {}\n",
                    fact.id,
                    fact.category.as_str(),
                    policy_name(fact.policy),
                    if fact.slot.is_empty() {
                        fact.category.as_str()
                    } else {
                        &fact.slot
                    },
                    fact.confidence,
                    fact.weight,
                    if fact.locked { " locked" } else { "" },
                    fact.value.trim()
                )
            })
            .collect::<String>();
        if rendered.is_empty() {
            "(none yet)".to_owned()
        } else {
            rendered
        }
    };
    let new_memories = fresh_memories
        .iter()
        .filter(|memory| !memory.text.trim().is_empty())
        .take(MAX_GROWTH_MEMORIES)
        .enumerate()
        .map(|(index, memory)| format!("{index}. {}\n", memory.text.trim()))
        .collect();
    GrowthPromptValues {
        changeable_categories,
        current_growth,
        new_memories,
    }
}

const fn policy_name(policy: SoulFactPolicy) -> &'static str {
    match policy {
        SoulFactPolicy::Current => "current",
        SoulFactPolicy::Adaptive => "adaptive",
        SoulFactPolicy::Historical => "historical",
    }
}

#[must_use]
pub fn growth_tool_request() -> ToolRequest {
    ToolRequest {
        definitions: vec![ToolDefinition {
            name: RECORD_GROWTH_TOOL_NAME.to_owned(),
            description: Some(
                "Record only well-supported changes to the companion's changeable Soul facts. Use current for replaceable present-state facts and adaptive for patterns that can accumulate. Give each fact a stable semantic slot, confidence, and weight. To revise an existing adaptive fact, set kind to adjust and list its id in supersedes. Never replace a locked fact. Pass an empty adjustments array when evidence is weak or nothing changed."
                    .to_owned(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "adjustments": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "category": { "type": "string", "enum": ["appearance", "goals", "likes", "voice", "relationalStyle", "vulnerabilities", "fears", "habits", "boundaries"] },
                                "kind": { "type": "string", "enum": ["add", "adjust"] },
                                "policy": { "type": "string", "enum": ["current", "adaptive"] },
                                "slot": { "type": "string" },
                                "value": { "type": "string" },
                                "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
                                "weight": { "type": "number", "minimum": 0, "maximum": 1 },
                                "validUntil": { "type": ["integer", "null"] },
                                "sourceIndices": { "type": "array", "items": { "type": "integer" } },
                                "supersedes": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["category", "policy", "slot", "value", "confidence", "weight"]
                        }
                    }
                },
                "required": ["adjustments"]
            }),
            version: 1,
        }],
        choice: ToolChoice::Required,
    }
}

#[must_use]
pub fn parse_growth_proposals(
    calls: &[ProposedToolCall],
    fallback_text: Option<&str>,
    memory_ids: &[String],
) -> Vec<ProposedSoulFact> {
    let adjustments = calls
        .iter()
        .find(|call| call.name == RECORD_GROWTH_TOOL_NAME)
        .and_then(|call| call.arguments.get("adjustments").cloned())
        .or_else(|| fallback_adjustments(fallback_text?));
    let Some(Value::Array(items)) = adjustments else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| parse_adjustment(&item, memory_ids))
        .collect()
}

fn parse_adjustment(item: &Value, memory_ids: &[String]) -> Option<ProposedSoulFact> {
    let category = parse_changeable_category(item.get("category")?.as_str()?)?;
    let value = item.get("value")?.as_str()?.trim().to_owned();
    if value.is_empty() {
        return None;
    }
    let policy = match item.get("policy").and_then(Value::as_str) {
        Some("current") => SoulFactPolicy::Current,
        Some("adaptive") => SoulFactPolicy::Adaptive,
        _ => return None,
    };
    let valid_until = item
        .get("validUntil")
        .and_then(Value::as_u64)
        .and_then(|value| i64::try_from(value).ok())
        .map(TimestampMillis::new);
    Some(ProposedSoulFact {
        id: Uuid::new_v4().to_string(),
        category,
        value,
        kind: if item.get("kind").and_then(Value::as_str) == Some("adjust") {
            SoulFactKind::Adjust
        } else {
            SoulFactKind::Add
        },
        policy,
        slot: item
            .get("slot")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_owned(),
        confidence: item
            .get("confidence")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
        weight: item.get("weight").and_then(Value::as_f64).unwrap_or(0.0),
        valid_until,
        locked: false,
        source_memory_ids: resolve_sources(item.get("sourceIndices"), memory_ids),
        supersedes: item
            .get("supersedes")
            .and_then(Value::as_array)
            .map(|ids| {
                ids.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn parse_changeable_category(value: &str) -> Option<SoulCategory> {
    match value {
        "appearance" => Some(SoulCategory::Appearance),
        "goals" => Some(SoulCategory::Goals),
        "likes" => Some(SoulCategory::Likes),
        "voice" => Some(SoulCategory::Voice),
        "relationalStyle" => Some(SoulCategory::RelationalStyle),
        "vulnerabilities" => Some(SoulCategory::Vulnerabilities),
        "fears" => Some(SoulCategory::Fears),
        "habits" => Some(SoulCategory::Habits),
        "boundaries" => Some(SoulCategory::Boundaries),
        _ => None,
    }
}

fn resolve_sources(indices: Option<&Value>, memory_ids: &[String]) -> Vec<String> {
    let mapped = indices
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_u64)
                .filter_map(|index| usize::try_from(index).ok())
                .filter_map(|index| memory_ids.get(index).cloned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if mapped.is_empty() {
        memory_ids.to_vec()
    } else {
        mapped
    }
}

fn fallback_adjustments(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<Value>(&text[start..=end])
        .ok()?
        .get("adjustments")
        .cloned()
}

#[cfg(test)]
mod tests {
    use lettuce_types::Revision;

    use super::*;
    use crate::SoulFact;

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
    fn tool_contract_matches_legacy_required_record_growth_call() {
        let request = growth_tool_request();
        assert_eq!(request.choice, ToolChoice::Required);
        assert_eq!(request.definitions.len(), 1);
        assert_eq!(request.definitions[0].name, RECORD_GROWTH_TOOL_NAME);
        assert_eq!(
            request.definitions[0].parameters["properties"]["adjustments"]["items"]["properties"]["category"]
                ["enum"],
            json!([
                "appearance",
                "goals",
                "likes",
                "voice",
                "relationalStyle",
                "vulnerabilities",
                "fears",
                "habits",
                "boundaries"
            ])
        );
        request.validate().expect("valid tool request");
    }

    #[test]
    fn first_record_growth_call_maps_sources_and_legacy_defaults() {
        let calls = vec![
            call("ignored", json!({})),
            call(
                RECORD_GROWTH_TOOL_NAME,
                json!({"adjustments": [{"category": "likes", "kind": "adjust", "policy": "adaptive", "slot": "food", "value": "  Likes ramen  ", "confidence": 0.8, "weight": 0.7, "validUntil": 99, "sourceIndices": [2, 88, -1], "supersedes": ["old-like"]}]}),
            ),
            call(
                RECORD_GROWTH_TOOL_NAME,
                json!({"adjustments": [{"category": "goals", "value": "ignored"}]}),
            ),
        ];
        let proposals =
            parse_growth_proposals(&calls, None, &["m0".into(), "m1".into(), "m2".into()]);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].category, SoulCategory::Likes);
        assert_eq!(proposals[0].kind, SoulFactKind::Adjust);
        assert_eq!(proposals[0].value, "Likes ramen");
        assert_eq!(proposals[0].source_memory_ids, ["m2"]);
        assert_eq!(proposals[0].supersedes, ["old-like"]);
        assert_eq!(proposals[0].valid_until, Some(TimestampMillis::new(99)));
    }

    #[test]
    fn empty_source_mapping_falls_back_to_all_memories() {
        let proposals = parse_growth_proposals(
            &[call(
                RECORD_GROWTH_TOOL_NAME,
                json!({"adjustments": [{"category": "goals", "policy": "current", "slot": "goal", "value": "Travel", "confidence": 0.8, "weight": 0.9, "sourceIndices": [12]}]}),
            )],
            None,
            &["m0".into(), "m1".into()],
        );
        assert_eq!(proposals[0].source_memory_ids, ["m0", "m1"]);
    }

    #[test]
    fn fallback_and_filters_match_legacy_parser() {
        let text = r#"prefix {"adjustments":[{"category":"backstory","policy":"adaptive","slot":"x","value":"no","confidence":1,"weight":1},{"category":"habits","policy":"adaptive","slot":"","value":"  ","confidence":1,"weight":1},{"category":"habits","policy":"adaptive","slot":"routine","value":"Paces","confidence":0.75,"weight":0.5}]} suffix"#;
        let proposals = parse_growth_proposals(&[], Some(text), &["m0".into()]);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].category, SoulCategory::Habits);
        assert_eq!(proposals[0].kind, SoulFactKind::Add);
        assert_eq!(proposals[0].source_memory_ids, ["m0"]);
    }

    #[test]
    fn malformed_first_native_call_does_not_use_text_fallback() {
        let proposals = parse_growth_proposals(
            &[call(RECORD_GROWTH_TOOL_NAME, json!({"adjustments": "bad"}))],
            Some(r#"{"adjustments":[{"category":"likes"}]}"#),
            &["m0".into()],
        );
        assert!(proposals.is_empty());
    }

    #[test]
    fn prompt_values_copy_legacy_categories_growth_and_memory_format() {
        let soul = CompanionSoulIdentity {
            appearance: "Tall".into(),
            likes: "Tea".into(),
            ..CompanionSoulIdentity::default()
        };
        let state = SoulState {
            revision: Revision::INITIAL,
            facts: vec![SoulFact {
                id: "like-coffee".into(),
                category: SoulCategory::Likes,
                value: " Coffee ".into(),
                kind: SoulFactKind::Add,
                policy: SoulFactPolicy::Adaptive,
                slot: "drink".into(),
                confidence: 0.75,
                evidence_count: 1,
                weight: 0.5,
                valid_from: TimestampMillis::new(1),
                valid_until: None,
                locked: true,
                source_memory_ids: vec!["m0".into()],
                created_at: TimestampMillis::new(1),
                supersedes: Vec::new(),
                superseded_by: None,
                superseded_at: None,
            }],
        };
        let memories = (0..18)
            .map(|index| GrowthMemoryEvidence {
                id: format!("m{index}"),
                text: if index == 1 {
                    "   ".into()
                } else {
                    format!(" memory {index} ")
                },
            })
            .collect::<Vec<_>>();

        let values = growth_prompt_values(&soul, &state, &memories, TimestampMillis::new(2));

        assert!(values.changeable_categories.starts_with(
            "- Appearance [appearance]: Tall\n- Goals [goals]: (empty)\n- Likes [likes]: Tea Coffee\n"
        ));
        assert_eq!(
            values.current_growth,
            "- id=like-coffee [likes policy=adaptive slot=drink confidence=0.75 weight=0.50 locked]: Coffee\n"
        );
        assert!(
            values
                .new_memories
                .starts_with("0. memory 0\n1. memory 2\n")
        );
        assert!(values.new_memories.ends_with("15. memory 16\n"));
        assert!(!values.new_memories.contains("memory 17"));
    }
}
