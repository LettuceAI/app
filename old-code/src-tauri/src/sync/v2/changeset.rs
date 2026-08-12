use std::io::Read;

use fallible_streaming_iterator::FallibleStreamingIterator;
use rusqlite::hooks::Action;
use rusqlite::session::{ChangesetItem, ChangesetIter};
use rusqlite::types::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowOperation {
    Insert,
    Update,
    Delete,
}

impl RowOperation {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }

    pub(crate) fn is_delete(self) -> bool {
        self == Self::Delete
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RowChange {
    pub table_name: String,
    pub primary_key: Vec<Value>,
    pub primary_key_bytes: Vec<u8>,
    pub primary_key_hash: String,
    pub operation: RowOperation,
}

pub(crate) fn inspect_changeset(changeset: &[u8]) -> Result<Vec<RowChange>, rusqlite::Error> {
    let mut bytes = changeset;
    let input: &mut dyn Read = &mut bytes;
    let mut iterator = ChangesetIter::start_strm(&input)?;
    let mut changes = Vec::new();
    while let Some(item) = iterator.next()? {
        changes.push(inspect_item(item)?);
    }
    Ok(changes)
}

pub(crate) fn inspect_item(item: &ChangesetItem) -> Result<RowChange, rusqlite::Error> {
    let operation = item.op()?;
    let row_operation = match operation.code() {
        Action::SQLITE_INSERT => RowOperation::Insert,
        Action::SQLITE_UPDATE => RowOperation::Update,
        Action::SQLITE_DELETE => RowOperation::Delete,
        _ => {
            return Err(rusqlite::Error::InvalidParameterName(
                "unsupported changeset operation".to_string(),
            ))
        }
    };

    let mut primary_key = Vec::new();
    for (column, is_primary_key) in item.pk()?.iter().enumerate() {
        if *is_primary_key == 0 {
            continue;
        }
        let value = if row_operation == RowOperation::Insert {
            item.new_value(column)?
        } else {
            item.old_value(column)?
        };
        primary_key.push(value.into());
    }

    if primary_key.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "changeset table {} has no primary key",
            operation.table_name()
        )));
    }

    let primary_key_bytes = encode_values(&primary_key);
    let mut identity = Vec::with_capacity(operation.table_name().len() + primary_key_bytes.len() + 1);
    identity.extend_from_slice(operation.table_name().as_bytes());
    identity.push(0);
    identity.extend_from_slice(&primary_key_bytes);

    Ok(RowChange {
        table_name: operation.table_name().to_string(),
        primary_key,
        primary_key_bytes,
        primary_key_hash: blake3::hash(&identity).to_hex().to_string(),
        operation: row_operation,
    })
}

fn encode_values(values: &[Value]) -> Vec<u8> {
    let mut encoded = Vec::new();
    for value in values {
        match value {
            Value::Null => encoded.push(0),
            Value::Integer(value) => {
                encoded.push(1);
                encoded.extend_from_slice(&value.to_le_bytes());
            }
            Value::Real(value) => {
                encoded.push(2);
                encoded.extend_from_slice(&value.to_bits().to_le_bytes());
            }
            Value::Text(value) => {
                encoded.push(3);
                encode_bytes(&mut encoded, value.as_bytes());
            }
            Value::Blob(value) => {
                encoded.push(4);
                encode_bytes(&mut encoded, value);
            }
        }
    }
    encoded
}

pub(crate) fn row_identity_hash(table_name: &str, primary_key: &[Value]) -> String {
    let primary_key_bytes = encode_values(primary_key);
    let mut identity = Vec::with_capacity(table_name.len() + primary_key_bytes.len() + 1);
    identity.extend_from_slice(table_name.as_bytes());
    identity.push(0);
    identity.extend_from_slice(&primary_key_bytes);
    blake3::hash(&identity).to_hex().to_string()
}

fn encode_bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value);
}
