use lettuce_conversations::{ProposedToolCall, ToolChoice, ToolDefinition, ToolRequest};
use lettuce_types::TimestampMillis;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{ProposedSoulFact, SoulCategory, SoulFactKind, SoulFactPolicy};

pub const MAX_GROWTH_MEMORIES: usize = 16;
pub const RECORD_GROWTH_TOOL_NAME: &str = "record_growth";

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
    use super::*;

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
}
