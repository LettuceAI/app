//! Hard deletes of lorebooks and prompt documents. Live configuration that
//! names the source is cleaned in the same transaction; history keeps the id
//! and name it stored at the time of use.

use std::collections::BTreeSet;

use lettuce_context::PromptProvenance;
use lettuce_settings::{GLOBAL_SETTINGS_FORMAT_VERSION, GlobalSettings};
use lettuce_types::{
    CharacterId, ConversationId, GroupId, LorebookId, PersonaId, PromptDocumentId, Revision,
    TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;

/// The owners whose live configuration a hard delete changed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceDeletion {
    pub characters: BTreeSet<CharacterId>,
    pub personas: BTreeSet<PersonaId>,
    pub groups: BTreeSet<GroupId>,
    pub conversations: BTreeSet<ConversationId>,
    pub settings_changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SourceDeleteError {
    #[error("the lorebook or prompt was not found")]
    NotFound,
    #[error("the lorebook or prompt revision is stale")]
    Conflict,
    #[error("the prompt is protected")]
    Protected,
    #[error("the delete could not be stored")]
    Storage,
}

fn storage(_: impl std::fmt::Debug) -> SourceDeleteError {
    SourceDeleteError::Storage
}

fn parse_all<T: std::str::FromStr + Ord>(
    values: Vec<String>,
) -> Result<BTreeSet<T>, SourceDeleteError> {
    values
        .into_iter()
        .map(|value| value.parse().map_err(|_| SourceDeleteError::Storage))
        .collect()
}

impl Database {
    /// Hard deletes a lorebook with its entries. Character, persona and group
    /// bindings to it are removed, starter and conversation selections drop
    /// it, and every owner that changed moves its revision.
    pub fn delete_lorebook(
        &self,
        id: LorebookId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<SourceDeletion, SourceDeleteError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let deletion = delete_lorebook_in(&transaction, id, expected_revision, now)?;
        transaction.commit().map_err(storage)?;
        Ok(deletion)
    }

    /// Hard deletes a prompt document with its entries. A protected or
    /// required built-in is refused. Character, starter, group and
    /// conversation selections of it and the app and feature settings that
    /// name it return to their defaults.
    pub fn delete_prompt(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<SourceDeletion, SourceDeleteError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let deletion = delete_prompt_in(&transaction, id, expected_revision, now)?;
        transaction.commit().map_err(storage)?;
        Ok(deletion)
    }
}

pub(crate) fn delete_lorebook_in(
    transaction: &Transaction<'_>,
    id: LorebookId,
    expected_revision: Revision,
    now: TimestampMillis,
) -> Result<SourceDeletion, SourceDeleteError> {
    let current = crate::lorebook::lorebook_adapter::load_details(transaction, id)
        .map_err(storage)?
        .ok_or(SourceDeleteError::NotFound)?;
    if current.book.revision != expected_revision {
        return Err(SourceDeleteError::Conflict);
    }
    let [characters, personas, groups] =
        crate::lorebook::lorebook_adapter::remove_lorebook_bindings_in(transaction, id, now)
            .map_err(storage)?;
    let mut deletion = SourceDeletion {
        characters: parse_all(characters)?,
        personas: parse_all(personas)?,
        groups: parse_all(groups)?,
        ..SourceDeletion::default()
    };
    deletion.characters.extend(
        crate::catalog::character_adapter::clear_starter_lorebook_in(transaction, id, now)
            .map_err(storage)?,
    );
    deletion.conversations =
        crate::conversation::settings_references::clear_lorebook_overrides_in(transaction, id, now)
            .map_err(storage)?;
    deletion.conversations.extend(
        crate::conversation::settings_references::clear_launch_lorebooks_in(transaction, id, now)
            .map_err(storage)?,
    );
    transaction
        .execute("DELETE FROM lorebooks WHERE id=?1", [id.to_string()])
        .map_err(storage)?;
    Ok(deletion)
}

pub(crate) fn delete_prompt_in(
    transaction: &Transaction<'_>,
    id: PromptDocumentId,
    expected_revision: Revision,
    now: TimestampMillis,
) -> Result<SourceDeletion, SourceDeleteError> {
    let current = crate::catalog::prompt_adapter::load_document(transaction, id)
        .map_err(storage)?
        .ok_or(SourceDeleteError::NotFound)?;
    if current.revision != expected_revision {
        return Err(SourceDeleteError::Conflict);
    }
    if let PromptProvenance::BuiltIn {
        protected,
        required,
        ..
    } = current.provenance
        && (protected || required)
    {
        return Err(SourceDeleteError::Protected);
    }
    let mut deletion = SourceDeletion {
        characters: crate::catalog::character_adapter::clear_prompt_references_in(
            transaction,
            id,
            now,
        )
        .map_err(storage)?,
        ..SourceDeletion::default()
    };
    let groups = transaction
        .prepare(
            "UPDATE groups SET \
             group_conversation_prompt_id = CASE WHEN group_conversation_prompt_id = ?1 THEN NULL ELSE group_conversation_prompt_id END, \
             group_roleplay_prompt_id = CASE WHEN group_roleplay_prompt_id = ?1 THEN NULL ELSE group_roleplay_prompt_id END, \
             revision = revision + 1, updated_at = max(updated_at, ?2) \
             WHERE group_conversation_prompt_id = ?1 OR group_roleplay_prompt_id = ?1 RETURNING id",
        )
        .map_err(storage)?
        .query_map(params![id.to_string(), now.get()], |row| {
            row.get::<_, String>(0)
        })
        .map_err(storage)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage)?;
    deletion.groups = parse_all(groups)?;
    deletion.conversations =
        crate::conversation::settings_references::clear_prompt_overrides_in(transaction, id, now)
            .map_err(storage)?;
    deletion.settings_changed = clear_settings_prompt_in(transaction, id, now)?;
    transaction
        .execute("DELETE FROM prompt_documents WHERE id=?1", [id.to_string()])
        .map_err(storage)?;
    Ok(deletion)
}

fn clear_settings_prompt_in(
    transaction: &Transaction<'_>,
    id: PromptDocumentId,
    now: TimestampMillis,
) -> Result<bool, SourceDeleteError> {
    let Some((format_version, payload, default_prompt)) = transaction
        .query_row(
            "SELECT format_version, payload_json, default_prompt_document_id FROM app_settings WHERE id=1",
            [],
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?
    else {
        return Ok(false);
    };
    if format_version != GLOBAL_SETTINGS_FORMAT_VERSION {
        return Err(SourceDeleteError::Storage);
    }
    let mut settings: GlobalSettings = serde_json::from_str(&payload).map_err(storage)?;
    let features = settings.clear_prompts(|selected| selected == id);
    let default = default_prompt.as_deref() == Some(id.to_string().as_str());
    if !features && !default {
        return Ok(false);
    }
    transaction
        .execute(
            "UPDATE app_settings SET payload_json=?1, \
             default_prompt_document_id = CASE WHEN default_prompt_document_id = ?2 THEN NULL ELSE default_prompt_document_id END, \
             revision=revision+1, updated_at=max(updated_at, ?3) WHERE id=1",
            params![
                serde_json::to_string(&settings).map_err(storage)?,
                id.to_string(),
                now.get()
            ],
        )
        .map_err(storage)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use lettuce_characters::{CharacterDefaults, Selection};
    use lettuce_context::{
        BindingInsertionTarget, BuiltInPromptSeed, BuiltInReconcileMode, BuiltInReconcileRequest,
        CharacterLorebookBindingRepository, DetectionPolicy, GroupLorebookBindingRepository,
        KeywordMatchMode, LorebookBehaviorVersion, LorebookBindingCreate, LorebookEntryDraft,
        LorebookMetadataDraft, LorebookRepository, PersonaLorebookBindingRepository,
        PromptBehaviorVersion, PromptBootstrapPort, PromptMetadataDraft, PromptPurpose,
        PromptRepository,
    };
    use lettuce_settings::GlobalSettingsStore;
    use lettuce_types::ConversationStarterId;

    use super::*;

    const NOW: TimestampMillis = TimestampMillis::new(50);

    fn book(database: &Database, name: &str) -> lettuce_context::LorebookDetails {
        LorebookRepository::create(
            database,
            LorebookMetadataDraft {
                name: name.into(),
                detection_policy: DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
            },
            vec![LorebookEntryDraft {
                title: "Entry".into(),
                enabled: true,
                always_active: true,
                keywords: Vec::new(),
                case_sensitive: false,
                match_mode: KeywordMatchMode::Literal,
                content: "Lore".into(),
                priority: 0,
            }],
            TimestampMillis::new(1),
        )
        .expect("lorebook")
    }

    fn prompt(database: &Database, name: &str) -> lettuce_context::PromptDocument {
        PromptRepository::create_user_draft(
            database,
            PromptMetadataDraft {
                name: name.into(),
                purpose: PromptPurpose::DirectChat,
                condense: false,
                behavior_version: PromptBehaviorVersion::LegacyV1,
            },
            Vec::new(),
            TimestampMillis::new(1),
        )
        .expect("prompt")
    }

    fn character(database: &Database, defaults: &CharacterDefaults) -> CharacterId {
        let id = CharacterId::new();
        let id_text = |value: Option<PromptDocumentId>| value.map(|id| id.to_string());
        database
            .connection()
            .expect("lock")
            .execute(
                "INSERT INTO characters (id,status,name,nickname,normalized_name,normalized_nickname,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,model_profile_id,default_scene_id,default_starter_id,direct_prompt_id,group_conversation_prompt_id,group_roleplay_prompt_id,voice_profile_id,voice_legacy_locator,voice_autoplay,presentation_json,image_recommendation_json,revision,created_at,updated_at) VALUES (?1,'active','Character',NULL,'character',NULL,'{}','{}',?2,'roleplay','manual',NULL,NULL,NULL,?3,?4,?5,NULL,NULL,0,'{}',NULL,1,1,1)",
                params![
                    id.to_string(),
                    serde_json::json!({"format_version": 1, "value": defaults}).to_string(),
                    id_text(defaults.direct_prompt_id),
                    id_text(defaults.group_conversation_prompt_id),
                    id_text(defaults.group_roleplay_prompt_id),
                ],
            )
            .expect("character");
        id
    }

    fn starter(
        database: &Database,
        character_id: CharacterId,
        prompt_id: Option<PromptDocumentId>,
        lorebooks: &Selection<Vec<LorebookId>>,
    ) -> ConversationStarterId {
        let id = ConversationStarterId::new();
        database
            .connection()
            .expect("lock")
            .execute(
                "INSERT INTO conversation_starters (character_id,id,name,ordinal,scene_id,prompt_id,lorebooks_json,revision,created_at,updated_at) VALUES (?1,?2,'Starter',0,NULL,?3,?4,1,1,1)",
                params![
                    character_id.to_string(),
                    id.to_string(),
                    prompt_id.map(|id| id.to_string()),
                    serde_json::json!({"format_version": 1, "value": lorebooks}).to_string(),
                ],
            )
            .expect("starter");
        id
    }

    fn persona(database: &Database) -> PersonaId {
        let id = PersonaId::new();
        database
            .connection()
            .expect("lock")
            .execute(
                "INSERT INTO personas (id,status,title,normalized_title,nickname,normalized_nickname,description,design_description,avatar_crop_json,image_recommendation_json,revision,created_at,updated_at) VALUES (?1,'active','Persona','persona',NULL,NULL,'Description',NULL,NULL,NULL,1,1,1)",
                [id.to_string()],
            )
            .expect("persona");
        id
    }

    fn group(database: &Database, prompt_id: Option<PromptDocumentId>) -> GroupId {
        let id = GroupId::new();
        database
            .connection()
            .expect("lock")
            .execute(
                "INSERT INTO groups (id,status,name,normalized_name,chat_mode,persona_selection_kind,persona_id,speaker_selection,memory_policy,disable_character_lorebooks,group_conversation_prompt_id,group_roleplay_prompt_id,presentation_json,background_asset_id,background_blob_kind,starting_scene_id,revision,created_at,updated_at) VALUES (?1,'active','Group','group','conversation','inherit',NULL,'llm','manual',0,?2,NULL,'{}',NULL,'image',NULL,1,1,1)",
                params![id.to_string(), prompt_id.map(|id| id.to_string())],
            )
            .expect("group");
        id
    }

    fn revision(database: &Database, table: &str, id: &str) -> i64 {
        database
            .connection()
            .expect("lock")
            .query_row(
                &format!("SELECT revision FROM {table} WHERE id=?1"),
                [id],
                |row| row.get(0),
            )
            .expect("revision")
    }

    fn starter_row(database: &Database, id: ConversationStarterId) -> (Option<String>, String) {
        database
            .connection()
            .expect("lock")
            .query_row(
                "SELECT prompt_id,lorebooks_json FROM conversation_starters WHERE id=?1",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("starter row")
    }

    struct LorebookFixture {
        deleted: lettuce_context::LorebookDetails,
        kept: lettuce_context::LorebookDetails,
        character: CharacterId,
        persona: PersonaId,
        group: GroupId,
        starter: ConversationStarterId,
    }

    fn lorebook_fixture(database: &Database) -> LorebookFixture {
        let deleted = book(database, "Deleted world");
        let kept = book(database, "Kept world");
        let character = character(database, &CharacterDefaults::default());
        let persona = persona(database);
        let group = group(database, None);
        let mut revisions = [Revision::INITIAL; 3];
        for book_id in [deleted.book.id, kept.book.id] {
            let create = || LorebookBindingCreate {
                lorebook_id: book_id,
                target: BindingInsertionTarget::Append,
            };
            revisions[0] = database
                .bind_character_lorebook(character, revisions[0], create(), NOW)
                .expect("character binding")
                .owner_revision;
            revisions[1] = database
                .bind_persona_lorebook(persona, revisions[1], create(), NOW)
                .expect("persona binding")
                .owner_revision;
            revisions[2] = database
                .bind_group_lorebook(group, revisions[2], create(), NOW)
                .expect("group binding")
                .owner_revision;
        }
        let starter = starter(
            database,
            character,
            None,
            &Selection::Explicit(vec![deleted.book.id, kept.book.id]),
        );
        LorebookFixture {
            deleted,
            kept,
            character,
            persona,
            group,
            starter,
        }
    }

    #[test]
    fn lorebook_delete_cleans_live_references_and_moves_owner_revisions() {
        let database = Database::open_in_memory().expect("database");
        let fixture = lorebook_fixture(&database);
        let character_revision = revision(&database, "characters", &fixture.character.to_string());
        assert_eq!(
            database.delete_lorebook(fixture.deleted.book.id, Revision::new(9), NOW),
            Err(SourceDeleteError::Conflict)
        );
        let deletion = database
            .delete_lorebook(fixture.deleted.book.id, fixture.deleted.book.revision, NOW)
            .expect("delete");
        assert_eq!(deletion.characters, BTreeSet::from([fixture.character]));
        assert_eq!(deletion.personas, BTreeSet::from([fixture.persona]));
        assert_eq!(deletion.groups, BTreeSet::from([fixture.group]));
        assert!(
            LorebookRepository::get(&database, fixture.deleted.book.id)
                .expect("read")
                .is_none()
        );
        let entries: i64 = database
            .connection()
            .expect("lock")
            .query_row(
                "SELECT count(*) FROM lorebook_entries WHERE lorebook_id=?1",
                [fixture.deleted.book.id.to_string()],
                |row| row.get(0),
            )
            .expect("entries");
        assert_eq!(entries, 0);
        for bindings in [
            database
                .list_character_bindings(fixture.character)
                .expect("character bindings"),
            database
                .list_persona_bindings(fixture.persona)
                .expect("persona bindings"),
            database
                .list_group_bindings(fixture.group)
                .expect("group bindings"),
        ] {
            assert_eq!(bindings.len(), 1);
            assert_eq!(bindings[0].lorebook_id, fixture.kept.book.id);
            assert_eq!(bindings[0].ordinal, 0);
        }
        let (_, lorebooks) = starter_row(&database, fixture.starter);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&lorebooks).expect("json")["value"],
            serde_json::json!({"kind": "explicit", "value": [fixture.kept.book.id]})
        );
        assert_eq!(
            revision(&database, "characters", &fixture.character.to_string()),
            character_revision + 2
        );
        assert_eq!(
            database.delete_lorebook(fixture.deleted.book.id, fixture.deleted.book.revision, NOW),
            Err(SourceDeleteError::NotFound)
        );
    }

    #[test]
    fn lorebook_delete_races_a_character_edit_through_the_owner_revision() {
        let database = Database::open_in_memory().expect("database");
        let fixture = lorebook_fixture(&database);
        let seen = Revision::new(
            u64::try_from(revision(
                &database,
                "characters",
                &fixture.character.to_string(),
            ))
            .expect("revision"),
        );
        database
            .delete_lorebook(fixture.deleted.book.id, fixture.deleted.book.revision, NOW)
            .expect("delete first");
        assert!(matches!(
            database.unbind_character_lorebook(fixture.character, seen, fixture.kept.book.id, NOW),
            Err(lettuce_context::BindingRepositoryError::Conflict)
        ));

        let database = Database::open_in_memory().expect("database");
        let fixture = lorebook_fixture(&database);
        let seen = Revision::new(
            u64::try_from(revision(
                &database,
                "characters",
                &fixture.character.to_string(),
            ))
            .expect("revision"),
        );
        let edited = database
            .unbind_character_lorebook(fixture.character, seen, fixture.kept.book.id, NOW)
            .expect("edit first");
        database
            .delete_lorebook(fixture.deleted.book.id, fixture.deleted.book.revision, NOW)
            .expect("delete after the edit");
        assert!(
            database
                .list_character_bindings(fixture.character)
                .expect("bindings")
                .is_empty()
        );
        assert_eq!(
            revision(&database, "characters", &fixture.character.to_string()),
            i64::try_from(edited.owner_revision.get()).expect("revision") + 2
        );
    }

    #[test]
    fn lorebook_delete_and_character_binding_edit_are_serialized() {
        let database = std::sync::Arc::new(Database::open_in_memory().expect("database"));
        let fixture = lorebook_fixture(&database);
        let seen = Revision::new(
            u64::try_from(revision(
                &database,
                "characters",
                &fixture.character.to_string(),
            ))
            .expect("revision"),
        );
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let deletion = {
            let database = std::sync::Arc::clone(&database);
            let barrier = std::sync::Arc::clone(&barrier);
            let id = fixture.deleted.book.id;
            let revision = fixture.deleted.book.revision;
            std::thread::spawn(move || {
                barrier.wait();
                database.delete_lorebook(id, revision, NOW)
            })
        };
        let edit = {
            let database = std::sync::Arc::clone(&database);
            let character = fixture.character;
            let kept = fixture.kept.book.id;
            std::thread::spawn(move || {
                barrier.wait();
                database.unbind_character_lorebook(character, seen, kept, NOW)
            })
        };
        deletion.join().expect("deletion worker").expect("delete");
        let edited = edit.join().expect("editor worker");
        assert!(
            edited.is_ok()
                || matches!(
                    edited,
                    Err(lettuce_context::BindingRepositoryError::Conflict)
                )
        );
        let bindings = database
            .list_character_bindings(fixture.character)
            .expect("bindings");
        assert!(
            bindings
                .iter()
                .all(|binding| binding.lorebook_id != fixture.deleted.book.id)
        );
        if edited.is_ok() {
            assert!(bindings.is_empty());
        } else {
            assert_eq!(bindings.len(), 1);
            assert_eq!(bindings[0].lorebook_id, fixture.kept.book.id);
        }
    }

    #[test]
    fn lorebook_delete_failing_late_leaves_every_reference() {
        let database = Database::open_in_memory().expect("database");
        let fixture = lorebook_fixture(&database);
        let before = (
            revision(&database, "characters", &fixture.character.to_string()),
            starter_row(&database, fixture.starter),
        );
        database
            .connection()
            .expect("lock")
            .execute_batch(
                "CREATE TRIGGER fail_lorebook_delete BEFORE DELETE ON lorebooks BEGIN SELECT RAISE(ABORT, 'injected'); END;",
            )
            .expect("fault");
        assert_eq!(
            database.delete_lorebook(fixture.deleted.book.id, fixture.deleted.book.revision, NOW),
            Err(SourceDeleteError::Storage)
        );
        assert_eq!(
            (
                revision(&database, "characters", &fixture.character.to_string()),
                starter_row(&database, fixture.starter),
            ),
            before
        );
        assert_eq!(
            database
                .list_persona_bindings(fixture.persona)
                .expect("persona bindings")
                .len(),
            2
        );
    }

    #[test]
    fn prompt_delete_resets_selections_and_settings_to_defaults() {
        let database = Database::open_in_memory().expect("database");
        let deleted = prompt(&database, "Deleted prompt");
        let kept = prompt(&database, "Kept prompt");
        let mut defaults = CharacterDefaults {
            direct_prompt_id: Some(deleted.id),
            group_conversation_prompt_id: Some(kept.id),
            group_roleplay_prompt_id: Some(deleted.id),
            ..CharacterDefaults::default()
        };
        defaults.companion_soul = Some(
            serde_json::from_value(serde_json::json!({
                "prompting": {"promptTemplateId": deleted.id}
            }))
            .expect("companion soul"),
        );
        let character_id = character(&database, &defaults);
        let untouched = character(&database, &CharacterDefaults::default());
        let starter_id = starter(&database, untouched, Some(deleted.id), &Selection::Inherit);
        let group_id = group(&database, Some(deleted.id));
        let settings = GlobalSettingsStore::load(&database).expect("settings");
        let mut value = settings.settings.clone();
        value.lorebook_entry_generator.entry_prompt_id = Some(deleted.id);
        value.lorebook_generator.selection.planner_prompt_id = Some(deleted.id);
        value.help_me_reply.roleplay_prompt_id = Some(kept.id);
        let settings = GlobalSettingsStore::save(
            &database,
            value,
            settings.default_model_profile_id,
            settings.revision,
        )
        .expect("save settings");
        let settings = database
            .set_default_prompt_document(Some(deleted.id), settings.revision)
            .expect("default prompt");

        let deletion = database
            .delete_prompt(deleted.id, deleted.revision, NOW)
            .expect("delete");
        assert_eq!(
            deletion.characters,
            BTreeSet::from([character_id, untouched])
        );
        assert_eq!(deletion.groups, BTreeSet::from([group_id]));
        assert!(deletion.settings_changed);
        assert!(
            PromptRepository::get(&database, deleted.id)
                .expect("read")
                .is_none()
        );
        let loaded = lettuce_characters::CharacterRepository::get(&database, character_id);
        let row: (Option<String>, Option<String>, Option<String>, String) = database
            .connection()
            .expect("lock")
            .query_row(
                "SELECT direct_prompt_id,group_conversation_prompt_id,group_roleplay_prompt_id,defaults_json FROM characters WHERE id=?1",
                [character_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("character row");
        drop(loaded);
        assert_eq!(row.0, None);
        assert_eq!(row.1, Some(kept.id.to_string()));
        assert_eq!(row.2, None);
        assert!(!row.3.contains(&deleted.id.to_string()));
        assert_eq!(starter_row(&database, starter_id).0, None);
        let groups: (Option<String>, i64) = database
            .connection()
            .expect("lock")
            .query_row(
                "SELECT group_conversation_prompt_id,revision FROM groups WHERE id=?1",
                [group_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("group row");
        assert_eq!(groups, (None, 2));
        let after = GlobalSettingsStore::load(&database).expect("settings");
        assert_eq!(after.default_prompt_document_id, None);
        assert_eq!(
            after.settings.lorebook_entry_generator.entry_prompt_id,
            None
        );
        assert_eq!(
            after
                .settings
                .lorebook_generator
                .selection
                .planner_prompt_id,
            None
        );
        assert_eq!(
            after.settings.help_me_reply.roleplay_prompt_id,
            Some(kept.id)
        );
        assert_eq!(after.revision.get(), settings.revision.get() + 1);
    }

    #[test]
    fn protected_and_required_built_ins_cannot_be_deleted() {
        let database = Database::open_in_memory().expect("database");
        for (key, required, protected) in [("protected", false, true), ("required", true, false)] {
            let created = database
                .reconcile_built_ins(
                    BuiltInReconcileRequest {
                        seeds: vec![BuiltInPromptSeed {
                            key: key.into(),
                            aliases: Vec::new(),
                            seed_version: 1,
                            metadata: PromptMetadataDraft {
                                name: key.into(),
                                purpose: PromptPurpose::DirectChat,
                                condense: false,
                                behavior_version: PromptBehaviorVersion::LegacyV1,
                            },
                            entries: Vec::new(),
                            required,
                            protected,
                        }],
                        mode: BuiltInReconcileMode::RefreshUnedited,
                    },
                    TimestampMillis::new(1),
                )
                .expect("seed");
            let document = &created[0].document;
            assert_eq!(
                database.delete_prompt(document.id, document.revision, NOW),
                Err(SourceDeleteError::Protected)
            );
            assert!(
                PromptRepository::get(&database, document.id)
                    .expect("read")
                    .is_some()
            );
        }
    }
}
