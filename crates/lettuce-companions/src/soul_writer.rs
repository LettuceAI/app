use lettuce_conversations::{ProposedToolCall, ToolChoice, ToolDefinition, ToolRequest};
use lettuce_types::TimestampMillis;
use serde::Serialize;
use serde_json::{Map, Value, json};

pub const SET_IDENTITY_TOOL_NAME: &str = "set_identity";
pub const SET_AUTHORED_FACTS_TOOL_NAME: &str = "set_authored_facts";
pub const SET_BASELINE_AFFECT_TOOL_NAME: &str = "set_baseline_affect";
pub const SET_REGULATION_STYLE_TOOL_NAME: &str = "set_regulation_style";
pub const SET_RELATIONSHIP_DEFAULTS_TOOL_NAME: &str = "set_relationship_defaults";
pub const SOUL_WRITER_DONE_TOOL_NAME: &str = "done";

const TEXT_FIELDS: &[&str] = &[
    "essence",
    "traits",
    "backstory",
    "appearance",
    "goals",
    "likes",
    "voice",
    "relationalStyle",
    "vulnerabilities",
    "fears",
    "habits",
    "boundaries",
];

const BASELINE_AFFECT_FIELDS: &[&str] = &[
    "warmth",
    "trust",
    "calm",
    "vulnerability",
    "longing",
    "hurt",
    "tension",
    "irritation",
    "affectionIntensity",
    "reassuranceNeed",
];

const REGULATION_STYLE_FIELDS: &[&str] = &[
    "suppression",
    "volatility",
    "recoverySpeed",
    "conflictAvoidance",
    "reassuranceSeeking",
    "protestBehavior",
    "emotionalTransparency",
    "attachmentActivation",
    "pride",
];

const RELATIONSHIP_DEFAULTS_FIELDS: &[&str] = &["closeness", "trust", "affection", "tension"];
const RELATIONSHIP_BIPOLAR_FIELDS: &[&str] = &["closeness", "trust", "affection"];

#[derive(Debug, Clone, PartialEq)]
pub struct SoulWriterReduction {
    pub draft: Value,
    pub results: Vec<Value>,
    pub completed: bool,
}

#[must_use]
pub fn soul_writer_tool_request() -> ToolRequest {
    let identity_properties = json!({
        "essence": { "type": "string" },
        "traits": { "type": "string" },
        "backstory": { "type": "string" },
        "appearance": { "type": "string" },
        "goals": { "type": "string" },
        "likes": { "type": "string" },
        "voice": { "type": "string" },
        "relationalStyle": { "type": "string" },
        "vulnerabilities": { "type": "string" },
        "fears": { "type": "string" },
        "habits": { "type": "string" },
        "boundaries": { "type": "string" }
    });
    ToolRequest {
        definitions: vec![
            ToolDefinition {
                name: SET_IDENTITY_TOOL_NAME.to_owned(),
                description: Some(
                    "Set or refine the durable identity text fields. All fields optional; later calls overwrite earlier values for the same field."
                        .to_owned(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": identity_properties
                }),
                version: 1,
            },
            ToolDefinition {
                name: SET_AUTHORED_FACTS_TOOL_NAME.to_owned(),
                description: Some(
                    "Replace the atomic facts extracted from the authored definition and backstory. Historical events are immutable and locked. Current facts can be superseded by a newer value in the same slot. Adaptive facts are evidence about traits, fears, goals, habits, or vulnerabilities and remain unlocked."
                        .to_owned(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "facts": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "category": { "type": "string", "enum": TEXT_FIELDS },
                                    "value": { "type": "string" },
                                    "policy": { "type": "string", "enum": ["historical", "current", "adaptive"] },
                                    "slot": { "type": "string" },
                                    "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
                                    "weight": { "type": "number", "minimum": 0, "maximum": 1 },
                                    "locked": { "type": "boolean" }
                                },
                                "required": ["category", "value", "policy", "slot", "confidence", "weight", "locked"]
                            }
                        }
                    },
                    "required": ["facts"]
                }),
                version: 1,
            },
            ToolDefinition {
                name: SET_BASELINE_AFFECT_TOOL_NAME.to_owned(),
                description: Some(
                    "Set or refine baseline affect floats in [0,1]. All fields optional; values are clamped."
                        .to_owned(),
                ),
                parameters: numeric_parameters(BASELINE_AFFECT_FIELDS),
                version: 1,
            },
            ToolDefinition {
                name: SET_REGULATION_STYLE_TOOL_NAME.to_owned(),
                description: Some(
                    "Set or refine regulation style floats in [0,1]. All fields optional; values are clamped."
                        .to_owned(),
                ),
                parameters: numeric_parameters(REGULATION_STYLE_FIELDS),
                version: 1,
            },
            ToolDefinition {
                name: SET_RELATIONSHIP_DEFAULTS_TOOL_NAME.to_owned(),
                description: Some(
                    "Set the starting relationship-with-user defaults. closeness/trust/affection are bidirectional in [-1,1] (negative = the character starts disliking/distrusting/distant from the user, 0 = neutral, positive = warm); tension is in [0,1]. All fields optional."
                        .to_owned(),
                ),
                parameters: numeric_parameters(RELATIONSHIP_DEFAULTS_FIELDS),
                version: 1,
            },
            ToolDefinition {
                name: SOUL_WRITER_DONE_TOOL_NAME.to_owned(),
                description: Some(
                    "Call once the Companion Soul is finalized. Terminal — no more setters after this."
                        .to_owned(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": { "notes": { "type": "string" } }
                }),
                version: 1,
            },
        ],
        choice: ToolChoice::Required,
    }
}

fn numeric_parameters(fields: &[&str]) -> Value {
    let properties = fields
        .iter()
        .map(|field| {
            (
                (*field).to_owned(),
                json!({ "type": "number", "minimum": 0, "maximum": 1 }),
            )
        })
        .collect::<Map<_, _>>();
    json!({ "type": "object", "properties": properties })
}

#[must_use]
pub fn normalize_soul_writer_draft(current: Option<&Value>) -> Value {
    let mut working = default_soul_value();
    let Some(current) = current.and_then(Value::as_object) else {
        return working;
    };

    if let Some(soul_in) = current.get("soul").and_then(Value::as_object)
        && let Some(soul_out) = working.get_mut("soul").and_then(Value::as_object_mut)
    {
        for key in TEXT_FIELDS {
            if let Some(Value::String(value)) = soul_in.get(*key) {
                soul_out.insert((*key).to_owned(), Value::String(value.clone()));
            }
        }
        copy_numeric_section(
            soul_in,
            soul_out,
            "baselineAffect",
            BASELINE_AFFECT_FIELDS,
            &[],
        );
        copy_numeric_section(
            soul_in,
            soul_out,
            "regulationStyle",
            REGULATION_STYLE_FIELDS,
            &[],
        );
    }
    if let Some(relationship_in) = current
        .get("relationshipDefaults")
        .and_then(Value::as_object)
        && let Some(relationship_out) = working
            .get_mut("relationshipDefaults")
            .and_then(Value::as_object_mut)
    {
        copy_numeric_values(
            relationship_in,
            relationship_out,
            RELATIONSHIP_DEFAULTS_FIELDS,
            RELATIONSHIP_BIPOLAR_FIELDS,
        );
    }
    if let Some(facts) = current.get("authoredFacts").and_then(Value::as_array)
        && let Some(root) = working.as_object_mut()
    {
        root.insert("authoredFacts".to_owned(), Value::Array(facts.clone()));
    }
    working
}

#[must_use]
pub fn reduce_soul_writer_calls(
    current: Option<&Value>,
    calls: &[ProposedToolCall],
    now: TimestampMillis,
) -> SoulWriterReduction {
    let mut draft = normalize_soul_writer_draft(current);
    let mut results = Vec::new();
    let mut completed = false;
    for call in calls {
        let (done, result) = apply_call(&mut draft, call, now);
        results.push(result);
        if done {
            completed = true;
            break;
        }
    }
    SoulWriterReduction {
        draft,
        results,
        completed,
    }
}

fn default_soul_value() -> Value {
    let text = TEXT_FIELDS
        .iter()
        .map(|field| ((*field).to_owned(), Value::String(String::new())))
        .collect::<Map<_, _>>();
    let baseline = zero_numeric_values(BASELINE_AFFECT_FIELDS);
    let regulation = zero_numeric_values(REGULATION_STYLE_FIELDS);
    let relationship = zero_numeric_values(RELATIONSHIP_DEFAULTS_FIELDS);
    let mut soul = text;
    soul.insert("baselineAffect".to_owned(), Value::Object(baseline));
    soul.insert("regulationStyle".to_owned(), Value::Object(regulation));
    json!({
        "soul": soul,
        "authoredFacts": [],
        "relationshipDefaults": relationship
    })
}

fn zero_numeric_values(fields: &[&str]) -> Map<String, Value> {
    fields
        .iter()
        .map(|field| ((*field).to_owned(), json!(0.0)))
        .collect()
}

fn copy_numeric_section(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    section: &str,
    fields: &[&str],
    signed_fields: &[&str],
) {
    let Some(source) = source.get(section).and_then(Value::as_object) else {
        return;
    };
    let Some(target) = target.get_mut(section).and_then(Value::as_object_mut) else {
        return;
    };
    copy_numeric_values(source, target, fields, signed_fields);
}

fn copy_numeric_values(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    fields: &[&str],
    signed_fields: &[&str],
) {
    for key in fields {
        if let Some(value) = source.get(*key).and_then(Value::as_f64) {
            insert_clamped(target, key, value, signed_fields.contains(key));
        }
    }
}

fn insert_clamped(target: &mut Map<String, Value>, key: &str, value: f64, signed: bool) {
    let value = if signed {
        value.clamp(-1.0, 1.0)
    } else {
        value.clamp(0.0, 1.0)
    };
    if let Some(value) = serde_json::Number::from_f64(value) {
        target.insert(key.to_owned(), Value::Number(value));
    }
}

fn apply_call(draft: &mut Value, call: &ProposedToolCall, now: TimestampMillis) -> (bool, Value) {
    match call.name.as_str() {
        SET_IDENTITY_TOOL_NAME => {
            let applied = apply_identity(draft, &call.arguments);
            (false, json!({ "ok": true, "applied": applied }))
        }
        SET_AUTHORED_FACTS_TOOL_NAME => {
            let applied = apply_authored_facts(draft, &call.arguments, now);
            (false, json!({ "ok": true, "applied": applied }))
        }
        SET_BASELINE_AFFECT_TOOL_NAME => {
            let applied = apply_numeric_section(
                draft,
                &["soul", "baselineAffect"],
                BASELINE_AFFECT_FIELDS,
                &[],
                &call.arguments,
            );
            (false, json!({ "ok": true, "applied": applied }))
        }
        SET_REGULATION_STYLE_TOOL_NAME => {
            let applied = apply_numeric_section(
                draft,
                &["soul", "regulationStyle"],
                REGULATION_STYLE_FIELDS,
                &[],
                &call.arguments,
            );
            (false, json!({ "ok": true, "applied": applied }))
        }
        SET_RELATIONSHIP_DEFAULTS_TOOL_NAME => {
            let applied = apply_numeric_section(
                draft,
                &["relationshipDefaults"],
                RELATIONSHIP_DEFAULTS_FIELDS,
                RELATIONSHIP_BIPOLAR_FIELDS,
                &call.arguments,
            );
            (false, json!({ "ok": true, "applied": applied }))
        }
        SOUL_WRITER_DONE_TOOL_NAME => (true, json!({ "ok": true, "done": true })),
        _ => (
            false,
            json!({ "ok": false, "error": "unknown_tool", "name": call.name }),
        ),
    }
}

fn apply_identity(draft: &mut Value, arguments: &Value) -> Vec<String> {
    let mut applied = Vec::new();
    let Some(arguments) = arguments.as_object() else {
        return applied;
    };
    let Some(soul) = draft.get_mut("soul").and_then(Value::as_object_mut) else {
        return applied;
    };
    for key in TEXT_FIELDS {
        if let Some(text) = arguments.get(*key).and_then(Value::as_str) {
            let text = text.trim();
            if !text.is_empty() {
                soul.insert((*key).to_owned(), Value::String(text.to_owned()));
                applied.push((*key).to_owned());
            }
        }
    }
    applied
}

fn apply_authored_facts(draft: &mut Value, arguments: &Value, now: TimestampMillis) -> usize {
    let Some(raw_facts) = arguments.get("facts") else {
        return 0;
    };
    let parsed;
    let facts = match raw_facts {
        Value::Array(facts) => facts,
        Value::String(value) => {
            parsed = serde_json::from_str::<Vec<Value>>(value).unwrap_or_default();
            &parsed
        }
        _ => return 0,
    };
    let facts = facts
        .iter()
        .filter_map(|fact| parse_authored_fact(fact, now))
        .filter_map(|fact| serde_json::to_value(fact).ok())
        .collect::<Vec<_>>();
    let count = facts.len();
    if let Some(root) = draft.as_object_mut() {
        root.insert("authoredFacts".to_owned(), Value::Array(facts));
    }
    count
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthoredFact {
    id: String,
    category: String,
    value: String,
    kind: String,
    policy: String,
    slot: String,
    confidence: f64,
    evidence_count: u32,
    weight: f64,
    valid_from: TimestampMillis,
    locked: bool,
    source_memory_ids: Vec<String>,
    created_at: TimestampMillis,
}

fn parse_authored_fact(fact: &Value, now: TimestampMillis) -> Option<AuthoredFact> {
    let category = fact.get("category")?.as_str()?.trim();
    if !TEXT_FIELDS.contains(&category) {
        return None;
    }
    let value = fact.get("value")?.as_str()?.trim();
    let policy = fact.get("policy")?.as_str()?.trim();
    let slot = fact.get("slot")?.as_str()?.trim();
    if value.is_empty()
        || slot.is_empty()
        || !matches!(policy, "historical" | "current" | "adaptive")
    {
        return None;
    }
    let confidence = fact
        .get("confidence")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    if confidence < 0.7 {
        return None;
    }
    Some(AuthoredFact {
        id: uuid::Uuid::new_v4().to_string(),
        category: category.to_owned(),
        value: value.to_owned(),
        kind: "authored".to_owned(),
        policy: policy.to_owned(),
        slot: slot.to_owned(),
        confidence,
        evidence_count: 1,
        weight: fact
            .get("weight")
            .and_then(Value::as_f64)
            .unwrap_or(1.0)
            .clamp(0.0, 1.0),
        valid_from: now,
        locked: policy == "historical"
            || fact.get("locked").and_then(Value::as_bool).unwrap_or(false),
        source_memory_ids: Vec::new(),
        created_at: now,
    })
}

fn apply_numeric_section(
    draft: &mut Value,
    path: &[&str],
    fields: &[&str],
    signed_fields: &[&str],
    arguments: &Value,
) -> Vec<String> {
    let mut applied = Vec::new();
    let Some(arguments) = arguments.as_object() else {
        return applied;
    };
    let mut node = draft;
    for segment in path {
        let Some(map) = node.as_object_mut() else {
            return applied;
        };
        let Some(next) = map.get_mut(*segment) else {
            return applied;
        };
        node = next;
    }
    let Some(target) = node.as_object_mut() else {
        return applied;
    };
    for key in fields {
        if let Some(value) = arguments.get(*key).and_then(Value::as_f64) {
            insert_clamped(target, key, value, signed_fields.contains(key));
            applied.push((*key).to_owned());
        }
    }
    applied
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
    fn required_tool_contract_matches_legacy() {
        let request = soul_writer_tool_request();
        assert_eq!(request.choice, ToolChoice::Required);
        assert_eq!(
            request
                .definitions
                .iter()
                .map(|definition| definition.name.as_str())
                .collect::<Vec<_>>(),
            [
                "set_identity",
                "set_authored_facts",
                "set_baseline_affect",
                "set_regulation_style",
                "set_relationship_defaults",
                "done"
            ]
        );
        request.validate().expect("valid tool request");
    }

    #[test]
    fn partial_current_value_is_completed_and_clamped_like_legacy() {
        let existing_fact = json!({ "id": "keep-verbatim" });
        let current = json!({
            "soul": {
                "essence": "Quiet",
                "baselineAffect": { "warmth": 2.0 },
                "regulationStyle": { "pride": -0.5 }
            },
            "relationshipDefaults": { "closeness": -2.0, "tension": 2.0 },
            "authoredFacts": [existing_fact.clone()]
        });
        let draft = normalize_soul_writer_draft(Some(&current));
        assert_eq!(draft["soul"]["essence"], "Quiet");
        assert_eq!(draft["soul"]["traits"], "");
        assert_eq!(draft["soul"]["baselineAffect"]["warmth"], 1.0);
        assert_eq!(draft["soul"]["baselineAffect"]["trust"], 0.0);
        assert_eq!(draft["soul"]["regulationStyle"]["pride"], 0.0);
        assert_eq!(draft["relationshipDefaults"]["closeness"], -1.0);
        assert_eq!(draft["relationshipDefaults"]["tension"], 1.0);
        assert_eq!(draft["authoredFacts"], json!([existing_fact]));
    }

    #[test]
    fn calls_apply_in_order_and_done_stops_later_calls() {
        let reduction = reduce_soul_writer_calls(
            None,
            &[
                call("set_identity", json!({ "traits": " First " })),
                call("set_identity", json!({ "traits": "Second" })),
                call("done", json!({})),
                call("set_identity", json!({ "traits": "Ignored" })),
            ],
            TimestampMillis::new(7),
        );
        assert!(reduction.completed);
        assert_eq!(reduction.results.len(), 3);
        assert_eq!(reduction.draft["soul"]["traits"], "Second");
        assert_eq!(reduction.results[2], json!({ "ok": true, "done": true }));
    }

    #[test]
    fn numeric_sections_use_the_legacy_bounds() {
        let reduction = reduce_soul_writer_calls(
            None,
            &[
                call(
                    "set_baseline_affect",
                    json!({ "warmth": 2.0, "hurt": -1.0 }),
                ),
                call(
                    "set_regulation_style",
                    json!({ "recoverySpeed": 1.5, "pride": -0.5 }),
                ),
                call(
                    "set_relationship_defaults",
                    json!({ "closeness": -2.0, "trust": 2.0, "tension": -1.0 }),
                ),
            ],
            TimestampMillis::new(7),
        );
        assert_eq!(reduction.draft["soul"]["baselineAffect"]["warmth"], 1.0);
        assert_eq!(reduction.draft["soul"]["baselineAffect"]["hurt"], 0.0);
        assert_eq!(
            reduction.draft["soul"]["regulationStyle"]["recoverySpeed"],
            1.0
        );
        assert_eq!(reduction.draft["soul"]["regulationStyle"]["pride"], 0.0);
        assert_eq!(reduction.draft["relationshipDefaults"]["closeness"], -1.0);
        assert_eq!(reduction.draft["relationshipDefaults"]["trust"], 1.0);
        assert_eq!(reduction.draft["relationshipDefaults"]["tension"], 0.0);
    }

    #[test]
    fn authored_facts_preserve_legacy_defaults_and_historical_locking() {
        let reduction = reduce_soul_writer_calls(
            None,
            &[call(
                "set_authored_facts",
                json!({
                    "facts": [
                        {
                            "category": "backstory",
                            "value": " Survived the winter evacuation ",
                            "policy": "historical",
                            "slot": " winter-evacuation ",
                            "confidence": 0.98,
                            "weight": 2.0,
                            "locked": false
                        },
                        {
                            "category": "fears",
                            "value": "Probably fears snow",
                            "policy": "adaptive",
                            "slot": "snow",
                            "confidence": 0.4,
                            "weight": 0.5,
                            "locked": false
                        }
                    ]
                }),
            )],
            TimestampMillis::new(7),
        );
        let facts = reduction.draft["authoredFacts"].as_array().expect("facts");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0]["category"], "backstory");
        assert_eq!(facts[0]["value"], "Survived the winter evacuation");
        assert_eq!(facts[0]["kind"], "authored");
        assert_eq!(facts[0]["locked"], true);
        assert_eq!(facts[0]["weight"], 1.0);
        assert_eq!(facts[0]["evidenceCount"], 1);
        assert_eq!(facts[0]["validFrom"], 7);
        assert_eq!(reduction.results[0], json!({ "ok": true, "applied": 1 }));
    }
}
