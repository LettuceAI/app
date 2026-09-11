use std::collections::{BTreeMap, BTreeSet};

use lettuce_characters::InteractionMode;
use lettuce_companions::{CompanionScheduledNote, ScheduledNoteRecurrence};
use lettuce_types::{CharacterId, TimestampMillis};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind, LegacyBackupDocumentKind,
    LegacyBackupGroupSessionPlan,
};

const NOTE_LIMIT: usize = 100_000;
const TEXT_LIMIT: usize = 1_000_000;

#[derive(Debug)]
pub struct LegacyBackupScheduledNotePlan {
    pub notes: Vec<LegacyBackupScheduledNote>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub source: LegacyBackupGroupSessionPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupScheduledNote {
    pub ordinal: u64,
    pub note: CompanionScheduledNote,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupScheduledNoteError {
    #[error("legacy backup scheduled-note document is malformed")]
    Malformed { field: String },
    #[error("legacy backup scheduled-note document exceeds its record limit")]
    LimitExceeded,
    #[error("legacy backup scheduled-note graph contains an orphaned link")]
    Orphan { field: String },
}

#[derive(Deserialize)]
struct NoteRow {
    id: String,
    character_id: String,
    label: String,
    content: String,
    available_at: i64,
    expires_at: Option<i64>,
    recurrence: String,
    recurrence_window_ms: Option<i64>,
    enabled: bool,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub fn plan_legacy_backup_scheduled_notes(
    source: LegacyBackupGroupSessionPlan,
) -> Result<LegacyBackupScheduledNotePlan, LegacyBackupScheduledNoteError> {
    let document = source
        .source
        .source
        .source
        .source
        .source
        .authored
        .configuration
        .source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::CompanionScheduledNotes);
    let mut notices = source.notices.clone();
    let notes = match document {
        Some(document) => {
            let rows: Vec<NoteRow> =
                serde_json::from_slice(&document.bytes).map_err(|_| malformed("$"))?;
            map_notes(rows, &source, &mut notices)?
        }
        None => {
            notices.push(notice(LegacyBackupConversionNoticeKind::Absent, "$"));
            Vec::new()
        }
    };
    notices.sort();
    notices.dedup();
    Ok(LegacyBackupScheduledNotePlan {
        notes,
        notices,
        source,
    })
}

fn map_notes(
    rows: Vec<NoteRow>,
    source: &LegacyBackupGroupSessionPlan,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupScheduledNote>, LegacyBackupScheduledNoteError> {
    if rows.len() > NOTE_LIMIT {
        return Err(LegacyBackupScheduledNoteError::LimitExceeded);
    }
    let authored = &source.source.source.source.source.source.authored;
    let companion_ids = authored
        .characters
        .iter()
        .filter(|character| character.defaults.interaction_mode == InteractionMode::Companion)
        .map(|character| character.id)
        .collect::<BTreeSet<_>>();
    let all_character_ids = authored
        .characters
        .iter()
        .map(|character| character.id)
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut notes = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("[{index}]");
        report_extra(&path, &row.extra, notices);
        let id = Uuid::parse_str(&row.id).map_err(|_| malformed(format!("{path}.id")))?;
        if !ids.insert(id) {
            return Err(malformed(format!("{path}.id")));
        }
        let character_id = row
            .character_id
            .parse::<CharacterId>()
            .map_err(|_| malformed(format!("{path}.character_id")))?;
        if !all_character_ids.contains(&character_id) {
            return Err(orphan(format!("{path}.character_id")));
        }
        if !companion_ids.contains(&character_id) {
            return Err(malformed(format!("{path}.character_id")));
        }
        validate_text(&row.label, &format!("{path}.label"))?;
        validate_text(&row.content, &format!("{path}.content"))?;
        if row.content.is_empty() {
            return Err(malformed(format!("{path}.content")));
        }
        let recurrence = match row.recurrence.as_str() {
            "none" => ScheduledNoteRecurrence::None,
            "daily" => ScheduledNoteRecurrence::Daily,
            "weekly" => ScheduledNoteRecurrence::Weekly,
            "monthly" => ScheduledNoteRecurrence::Monthly,
            "yearly" => ScheduledNoteRecurrence::Yearly,
            _ => return Err(malformed(format!("{path}.recurrence"))),
        };
        let available_at = timestamp(row.available_at, &format!("{path}.available_at"))?;
        let expires_at = row
            .expires_at
            .map(|value| timestamp(value, &format!("{path}.expires_at")))
            .transpose()?;
        if expires_at.is_some_and(|value| value <= available_at) {
            return Err(malformed(format!("{path}.expires_at")));
        }
        let created_at = timestamp(row.created_at, &format!("{path}.created_at"))?;
        let updated_at = timestamp(row.updated_at, &format!("{path}.updated_at"))?;
        if created_at > updated_at {
            return Err(malformed(format!("{path}.updated_at")));
        }
        let note = CompanionScheduledNote {
            id,
            character_id,
            label: row.label,
            content: row.content,
            available_at,
            expires_at,
            recurrence,
            recurrence_window_ms: row
                .recurrence_window_ms
                .map(|value| count(value, &format!("{path}.recurrence_window_ms")))
                .transpose()?,
            enabled: row.enabled,
            created_at,
            updated_at,
        };
        note.validate()
            .map_err(|_| malformed(format!("{path}.schedule")))?;
        notes.push(LegacyBackupScheduledNote {
            ordinal: u64::try_from(index)
                .map_err(|_| LegacyBackupScheduledNoteError::LimitExceeded)?,
            note,
        });
    }
    Ok(notes)
}

fn validate_text(value: &str, field: &str) -> Result<(), LegacyBackupScheduledNoteError> {
    if value.chars().count() > TEXT_LIMIT || value.contains('\0') {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn timestamp(value: i64, field: &str) -> Result<TimestampMillis, LegacyBackupScheduledNoteError> {
    if value < 0 {
        Err(malformed(field))
    } else {
        Ok(TimestampMillis::new(value))
    }
}

fn count(value: i64, field: &str) -> Result<u64, LegacyBackupScheduledNoteError> {
    u64::try_from(value).map_err(|_| malformed(field))
}

fn report_extra(
    path: &str,
    extra: &BTreeMap<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) {
    for field in extra.keys() {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            &format!("{path}.{field}"),
        ));
    }
}

fn notice(kind: LegacyBackupConversionNoticeKind, field: &str) -> LegacyBackupConversionNotice {
    LegacyBackupConversionNotice {
        kind,
        document: LegacyBackupDocumentKind::CompanionScheduledNotes,
        field: field.to_owned(),
    }
}

fn malformed(field: impl Into<String>) -> LegacyBackupScheduledNoteError {
    LegacyBackupScheduledNoteError::Malformed {
        field: field.into(),
    }
}

fn orphan(field: impl Into<String>) -> LegacyBackupScheduledNoteError {
    LegacyBackupScheduledNoteError::Orphan {
        field: field.into(),
    }
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use serde_json::{Value, json};
    use zeroize::Zeroizing;

    use super::*;
    use crate::{
        LegacyBackupDocument, LegacyBackupInventory, plan_legacy_backup_asr,
        plan_legacy_backup_authored, plan_legacy_backup_authored_media,
        plan_legacy_backup_configuration, plan_legacy_backup_direct_sessions,
        plan_legacy_backup_group_sessions, plan_legacy_backup_pricing, plan_legacy_backup_usage,
    };

    fn id(value: u128) -> String {
        Uuid::from_u128(value).to_string()
    }

    fn document(kind: LegacyBackupDocumentKind, value: Value) -> LegacyBackupDocument {
        LegacyBackupDocument {
            kind,
            bytes: Zeroizing::new(serde_json::to_vec(&value).expect("fixture document")),
        }
    }

    fn source(
        notes: Option<Value>,
        character_id: &str,
        companion: bool,
    ) -> LegacyBackupGroupSessionPlan {
        let mut documents = vec![document(
            LegacyBackupDocumentKind::Characters,
            json!([{
                "id": character_id,
                "name": "Mira",
                "mode": if companion { "companion" } else { "roleplay" },
                "created_at": 1,
                "updated_at": 1
            }]),
        )];
        if let Some(notes) = notes {
            documents.push(document(
                LegacyBackupDocumentKind::CompanionScheduledNotes,
                notes,
            ));
        }
        let inventory = LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("99".repeat(32)).expect("source hash"),
            documents,
            media: Vec::new(),
        };
        let configuration =
            plan_legacy_backup_configuration(inventory).expect("configuration plan");
        let authored = plan_legacy_backup_authored(configuration).expect("authored plan");
        let media = plan_legacy_backup_authored_media(authored).expect("media plan");
        let asr = plan_legacy_backup_asr(media).expect("ASR plan");
        let usage = plan_legacy_backup_usage(asr).expect("usage plan");
        let pricing = plan_legacy_backup_pricing(usage).expect("pricing plan");
        let direct = plan_legacy_backup_direct_sessions(pricing).expect("direct session plan");
        plan_legacy_backup_group_sessions(direct).expect("group session plan")
    }

    fn note(note_id: &str, character_id: &str) -> Value {
        json!({
            "id": note_id,
            "character_id": character_id,
            "label": " Anniversary ",
            "content": " Remember the first meeting. ",
            "available_at": 1_000,
            "expires_at": 9_000,
            "recurrence": "yearly",
            "recurrence_window_ms": 86_400_000,
            "enabled": true,
            "created_at": 100,
            "updated_at": 200
        })
    }

    #[test]
    fn scheduled_notes_preserve_exact_fields_and_export_order() {
        let character_id = id(1);
        let first = id(2);
        let second = id(3);
        let rows = json!([
            note(&first, &character_id),
            {
                "id": second,
                "character_id": character_id,
                "label": "Later",
                "content": "A daily reminder",
                "available_at": 2_000,
                "expires_at": null,
                "recurrence": "daily",
                "recurrence_window_ms": null,
                "enabled": false,
                "created_at": 150,
                "updated_at": 250
            }
        ]);
        let plan = plan_legacy_backup_scheduled_notes(source(Some(rows), &character_id, true))
            .expect("scheduled-note plan");
        assert_eq!(plan.notes.len(), 2);
        assert_eq!(plan.notes[0].ordinal, 0);
        assert_eq!(plan.notes[0].note.label, " Anniversary ");
        assert_eq!(plan.notes[0].note.content, " Remember the first meeting. ");
        assert_eq!(
            plan.notes[0].note.recurrence,
            ScheduledNoteRecurrence::Yearly
        );
        assert!(!plan.notes[1].note.enabled);
    }

    #[test]
    fn missing_scheduled_note_document_is_explicit() {
        let character_id = id(10);
        let plan = plan_legacy_backup_scheduled_notes(source(None, &character_id, true))
            .expect("empty scheduled-note plan");
        assert!(plan.notes.is_empty());
        assert!(plan.notices.iter().any(|notice| {
            notice.document == LegacyBackupDocumentKind::CompanionScheduledNotes
                && notice.kind == LegacyBackupConversionNoticeKind::Absent
        }));
    }

    #[test]
    fn scheduled_notes_reject_duplicates_wrong_owners_and_invalid_windows() {
        let character_id = id(20);
        let note_id = id(21);
        let duplicate = json!([note(&note_id, &character_id), note(&note_id, &character_id)]);
        assert!(matches!(
            plan_legacy_backup_scheduled_notes(source(Some(duplicate), &character_id, true)),
            Err(LegacyBackupScheduledNoteError::Malformed { ref field })
                if field == "[1].id"
        ));

        let wrong_owner = json!([note(&note_id, &character_id)]);
        assert!(matches!(
            plan_legacy_backup_scheduled_notes(source(Some(wrong_owner), &character_id, false)),
            Err(LegacyBackupScheduledNoteError::Malformed { ref field })
                if field == "[0].character_id"
        ));

        let mut invalid = note(&note_id, &character_id);
        invalid["expires_at"] = json!(1_000);
        assert!(matches!(
            plan_legacy_backup_scheduled_notes(source(Some(json!([invalid])), &character_id, true)),
            Err(LegacyBackupScheduledNoteError::Malformed { ref field })
                if field == "[0].expires_at"
        ));
    }
}
