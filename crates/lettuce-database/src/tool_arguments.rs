use lettuce_conversations::ProposedToolCall;
use serde_json::value::RawValue;

/// Keep the provider's JSON representation inside the versioned envelope.
/// SQLite's `json()` removes whitespace but preserves key order, escapes and
/// number spelling, so reserializing a Value can break the raw-arguments CHECK
/// even when both documents describe the same object.
pub(crate) fn encode_tool_arguments(
    call: &ProposedToolCall,
    format_version: u32,
) -> Result<String, serde_json::Error> {
    call.validate().map_err(serde::ser::Error::custom)?;
    match call.raw_arguments.as_deref() {
        Some(raw) => {
            let value: &RawValue = serde_json::from_str(raw)?;
            crate::encode_versioned(&value, format_version)
        }
        None => crate::encode_versioned(&call.arguments, format_version),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn raw_tool_arguments_satisfy_the_existing_sqlite_check_without_losing_wire_text() {
        let connection = rusqlite::Connection::open_in_memory().expect("connection");
        connection.execute_batch(
            "CREATE TABLE tool_arguments (
                arguments_json TEXT NOT NULL,
                raw_arguments TEXT CHECK (
                    raw_arguments IS NULL OR (
                        length(CAST(raw_arguments AS BLOB)) <= 262144
                        AND json_valid(raw_arguments)
                        AND json(raw_arguments) = json(json_extract(arguments_json, '$.value'))
                    )
                )
            ) STRICT;",
        ).expect("existing raw arguments constraint");
        for raw in [
            r#"{"text":"tea","category":"preference"}"#,
            r#" { "z": {"y":2,"b":3}, "a":[{"z":1,"a":2}] } "#,
            r#"{"text":"\u00e7ay","category":"preference"}"#,
            r#"{"z":1e+03,"a":-0,"fraction":1.2500}"#,
            r#"{"text":"tea","category":"preference","text":"coffee"}"#,
        ] {
            let call = ProposedToolCall {
                provider_call_id: Some("provider-call".into()),
                name: "create_memory".into(),
                arguments: serde_json::from_str(raw).expect("provider arguments"),
                raw_arguments: Some(raw.into()),
                provider_replay: None,
            };
            let encoded = encode_tool_arguments(&call, 1).expect("encode arguments");
            connection.execute("INSERT INTO tool_arguments VALUES (?1,?2)",
                rusqlite::params![encoded, raw]).expect("lexically matching JSON passes CHECK");
            let decoded: serde_json::Value = crate::decode_versioned(&encoded, 1).expect("decode arguments");
            assert_eq!(decoded, call.arguments);
            assert_eq!(call.raw_arguments.as_deref(), Some(raw));
        }
        let call = ProposedToolCall {
            provider_call_id: None, name: "done".into(), arguments: json!({}),
            raw_arguments: None, provider_replay: None,
        };
        let encoded = encode_tool_arguments(&call, 1).expect("arguments without raw text");
        connection.execute("INSERT INTO tool_arguments VALUES (?1,NULL)", [encoded]).expect("no raw text");
    }

    #[test]
    fn tool_arguments_reject_malformed_or_semantically_different_raw_json() {
        for raw in ["{", "[]", r#"{"text":"coffee"}"#] {
            let call = ProposedToolCall {
                provider_call_id: None, name: "create_memory".into(),
                arguments: json!({"text":"tea"}), raw_arguments: Some(raw.into()),
                provider_replay: None,
            };
            assert!(encode_tool_arguments(&call, 1).is_err(), "{raw}");
        }
    }
}
