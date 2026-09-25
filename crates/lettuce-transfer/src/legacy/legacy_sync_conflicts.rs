use std::collections::BTreeMap;

use serde_json::Value;

use crate::{LegacyBackupCompatibilityPlan, LegacyIdScope, LegacyMessageConflict};

/// The losing message versions among the preserved `sync_v2_conflicts` rows:
/// for each conflict on `messages` or `group_messages`, the recorded side
/// whose content differs from what the legacy message shows now. Other
/// conflicts stay only in the import provenance.
#[must_use]
pub fn legacy_message_conflicts(
    source: &LegacyBackupCompatibilityPlan,
    preserved: &[crate::LegacyPreservedRow],
    scope: LegacyIdScope,
) -> Vec<LegacyMessageConflict> {
    let current = source
        .direct_sessions()
        .sessions
        .iter()
        .flat_map(|session| {
            session
                .messages
                .iter()
                .map(|message| (message.source_id.as_str(), message.content.as_str()))
        })
        .chain(source.group_sessions().sessions.iter().flat_map(|session| {
            session
                .messages
                .iter()
                .map(|message| (message.source_id.as_str(), message.content.as_str()))
        }))
        .collect::<BTreeMap<_, _>>();
    let mut conflicts = Vec::new();
    for row in preserved
        .iter()
        .filter(|row| row.source_table == "sync_v2_conflicts")
    {
        let Ok(Value::Object(row)) = serde_json::from_str::<Value>(&row.row_json) else {
            continue;
        };
        if !matches!(
            row.get("table_name").and_then(Value::as_str),
            Some("messages" | "group_messages")
        ) {
            continue;
        }
        let Some(conflict_key) = row.get("conflict_id").and_then(Value::as_str) else {
            continue;
        };
        let detected_at = row
            .get("detected_at")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let losing = ["local_row", "incoming_row"]
            .into_iter()
            .filter_map(|column| row.get(column)?.get("hex")?.as_str())
            .filter_map(decode_hex)
            .filter_map(|bytes| stored_row(&bytes))
            .find_map(|columns| {
                let id = columns.get("id")?.as_str()?;
                let content = columns.get("content")?.as_str()?;
                (current.get(id).copied() != Some(content))
                    .then(|| (id.to_owned(), content.to_owned()))
            });
        if let Some((id, content)) = losing
            && current.contains_key(id.as_str())
        {
            conflicts.push(LegacyMessageConflict {
                conflict_key: conflict_key.to_owned(),
                message_id: lettuce_types::MessageId::from_uuid(scope.source(&id)),
                content,
                detected_at: lettuce_types::TimestampMillis::new(detected_at),
            });
        }
    }
    conflicts
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(hex.get(index..index + 2)?, 16).ok())
        .collect()
}

/// Decodes a legacy row snapshot: bincode's default encoding of
/// `{ columns: Vec<String>, primary_key: Vec<StoredValue>, values:
/// Option<Vec<StoredValue>> }` with fixed little-endian integers and u64
/// lengths, where `StoredValue` is `Null`, `Integer(i64)`, `Real(f64)`,
/// `Text(String)` or `Blob(String)`. Returns the columns with their values.
fn stored_row(bytes: &[u8]) -> Option<BTreeMap<String, Value>> {
    let mut reader = Reader { bytes, offset: 0 };
    let columns = (0..reader.length()?)
        .map(|_| reader.string())
        .collect::<Option<Vec<_>>>()?;
    for _ in 0..reader.length()? {
        reader.value()?;
    }
    let values = match reader.byte()? {
        0 => return Some(BTreeMap::new()),
        1 => (0..reader.length()?)
            .map(|_| reader.value())
            .collect::<Option<Vec<_>>>()?,
        _ => return None,
    };
    (reader.offset == bytes.len() && values.len() == columns.len())
        .then(|| columns.into_iter().zip(values).collect())
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let end = self.offset.checked_add(N)?;
        let value = self.bytes.get(self.offset..end)?.try_into().ok()?;
        self.offset = end;
        Some(value)
    }

    fn byte(&mut self) -> Option<u8> {
        self.take::<1>().map(|[value]| value)
    }

    fn length(&mut self) -> Option<usize> {
        usize::try_from(u64::from_le_bytes(self.take()?)).ok()
    }

    fn string(&mut self) -> Option<String> {
        let length = self.length()?;
        let end = self.offset.checked_add(length)?;
        let text = std::str::from_utf8(self.bytes.get(self.offset..end)?).ok()?;
        self.offset = end;
        Some(text.to_owned())
    }

    fn value(&mut self) -> Option<Value> {
        match u32::from_le_bytes(self.take()?) {
            0 => Some(Value::Null),
            1 => Some(Value::from(i64::from_le_bytes(self.take()?))),
            2 => Some(Value::from(f64::from_le_bytes(self.take()?))),
            3 | 4 => self.string().map(Value::String),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(columns: &[&str], values: &[Value]) -> String {
        let mut bytes = Vec::new();
        let string = |bytes: &mut Vec<u8>, text: &str| {
            bytes.extend_from_slice(&(text.len() as u64).to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
        };
        bytes.extend_from_slice(&(columns.len() as u64).to_le_bytes());
        for column in columns {
            string(&mut bytes, column);
        }
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        string(&mut bytes, "message-1");
        bytes.push(1);
        bytes.extend_from_slice(&(values.len() as u64).to_le_bytes());
        for value in values {
            match value {
                Value::Null => bytes.extend_from_slice(&0_u32.to_le_bytes()),
                Value::Number(number) => {
                    bytes.extend_from_slice(&1_u32.to_le_bytes());
                    bytes.extend_from_slice(&number.as_i64().expect("integer").to_le_bytes());
                }
                Value::String(text) => {
                    bytes.extend_from_slice(&3_u32.to_le_bytes());
                    string(&mut bytes, text);
                }
                _ => unreachable!("fixture values"),
            }
        }
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn legacy_row_snapshots_decode_their_columns() {
        let hex = encode(
            &["id", "content", "created_at", "reasoning"],
            &[
                Value::from("message-1"),
                Value::from("Edited here"),
                Value::from(5),
                Value::Null,
            ],
        );
        let columns = stored_row(&decode_hex(&hex).expect("hex")).expect("row");
        assert_eq!(columns["content"], "Edited here");
        assert_eq!(columns["created_at"], 5);
        assert_eq!(columns["reasoning"], Value::Null);
        assert!(stored_row(&decode_hex(&hex[..hex.len() - 2]).expect("hex")).is_none());
    }
}
