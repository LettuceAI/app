//! Tells listeners which change feeds a committed transaction wrote: the
//! update hook marks writes to a feed table, the commit hook reports the
//! marked feeds, and a rollback discards them.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, AtomicU64, Ordering},
};

use crate::Database;

/// A change feed table the signal watches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChangeFeed {
    Jobs,
    Conversations,
    Models,
    Settings,
}

impl ChangeFeed {
    const ALL: [Self; 4] = [
        Self::Jobs,
        Self::Conversations,
        Self::Models,
        Self::Settings,
    ];

    const fn bit(self) -> u8 {
        match self {
            Self::Jobs => 1,
            Self::Conversations => 2,
            Self::Models => 4,
            Self::Settings => 8,
        }
    }

    fn of_table(table: &str) -> Option<Self> {
        match table {
            "app_settings" | "device_settings" | "device_ui_state" => Some(Self::Settings),
            "job_changes" => Some(Self::Jobs),
            "model_changes" => Some(Self::Models),
            "conversation_changes" | "message_signals" | "memory_changes" => {
                Some(Self::Conversations)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LibraryKind {
    Character,
    Persona,
    Group,
}

type LibraryChangeListener = Arc<dyn Fn(LibraryKind, &str) + Send + Sync>;

type ChangeListener = Arc<dyn Fn() + Send + Sync>;
type SettingsChangeListener = Arc<dyn Fn(&'static str) + Send + Sync>;

#[derive(Default)]
struct LibraryChanges {
    ids: std::collections::BTreeSet<(LibraryKind, String)>,
}

pub(crate) struct ChangeSignal {
    pending: AtomicU8,
    library_changes: Mutex<LibraryChanges>,
    library_listeners: Mutex<Vec<LibraryChangeListener>>,
    settings_section: Mutex<Option<&'static str>>,
    settings_sections: Mutex<std::collections::BTreeSet<&'static str>>,
    settings_listeners: Mutex<Vec<SettingsChangeListener>>,
    settings_generation: AtomicU64,
    listeners: Mutex<Vec<(ChangeFeed, ChangeListener)>>,
}

impl std::fmt::Debug for ChangeSignal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChangeSignal")
            .finish_non_exhaustive()
    }
}

impl ChangeSignal {
    pub(crate) fn install(connection: &rusqlite::Connection) -> rusqlite::Result<Arc<Self>> {
        let signal = Arc::new(Self {
            pending: AtomicU8::new(0),
            library_changes: Mutex::new(Default::default()),
            library_listeners: Mutex::new(Vec::new()),
            settings_section: Mutex::new(None),
            settings_sections: Mutex::new(Default::default()),
            settings_listeners: Mutex::new(Vec::new()),
            settings_generation: AtomicU64::new(0),
            listeners: Mutex::new(Vec::new()),
        });
        let library = Arc::clone(&signal);
        connection.preupdate_hook(Some(
            move |_: rusqlite::hooks::Action,
                  database: &str,
                  table: &str,
                  case: &rusqlite::hooks::PreUpdateCase| {
                if database != "main" {
                    return;
                }
                if table == "persona_defaults" {
                    let mut changes = library.library_changes.lock().expect("library changes");
                    let mut record = |value: rusqlite::Result<rusqlite::types::ValueRef<'_>>| {
                        let value = value.expect("default persona column");
                        if let rusqlite::types::ValueRef::Text(id) = value {
                            changes.ids.insert((
                                LibraryKind::Persona,
                                std::str::from_utf8(id)
                                    .expect("persona identity text")
                                    .to_owned(),
                            ));
                        }
                    };
                    match case {
                        rusqlite::hooks::PreUpdateCase::Insert(accessor) => {
                            record(accessor.get_new_column_value(1))
                        }
                        rusqlite::hooks::PreUpdateCase::Delete(accessor) => {
                            record(accessor.get_old_column_value(1))
                        }
                        rusqlite::hooks::PreUpdateCase::Update {
                            old_value_accessor,
                            new_value_accessor,
                        } => {
                            record(old_value_accessor.get_old_column_value(1));
                            record(new_value_accessor.get_new_column_value(1));
                        }
                        rusqlite::hooks::PreUpdateCase::Unknown => {}
                    }
                    return;
                }
                let kind = match table {
                    "characters"
                    | "character_media"
                    | "character_presentation_asset_refs"
                    | "scenes"
                    | "scene_variants"
                    | "scene_assets"
                    | "conversation_starters"
                    | "starter_messages"
                    | "character_lorebook_bindings" => LibraryKind::Character,
                    "personas" | "persona_media" | "persona_lorebook_bindings" => {
                        LibraryKind::Persona
                    }
                    "groups"
                    | "group_members"
                    | "group_presentation_asset_refs"
                    | "group_starting_scenes"
                    | "group_scene_variants"
                    | "group_scene_assets"
                    | "group_lorebook_bindings" => LibraryKind::Group,
                    _ => return,
                };
                let value = match case {
                    rusqlite::hooks::PreUpdateCase::Insert(accessor) => {
                        accessor.get_new_column_value(0)
                    }
                    rusqlite::hooks::PreUpdateCase::Update {
                        new_value_accessor, ..
                    } => new_value_accessor.get_new_column_value(0),
                    rusqlite::hooks::PreUpdateCase::Delete(accessor) => {
                        accessor.get_old_column_value(0)
                    }
                    rusqlite::hooks::PreUpdateCase::Unknown => return,
                };
                let id = value
                    .expect("library identity column")
                    .as_str()
                    .expect("library identity text")
                    .to_owned();
                library
                    .library_changes
                    .lock()
                    .expect("library changes")
                    .ids
                    .insert((kind, id));
            },
        ))?;
        let marked = Arc::clone(&signal);
        connection.update_hook(Some(
            move |_: rusqlite::hooks::Action, _: &str, table: &str, _: i64| {
                if let Some(feed) = ChangeFeed::of_table(table) {
                    marked.pending.fetch_or(feed.bit(), Ordering::AcqRel);
                    if feed == ChangeFeed::Settings {
                        let section = match table {
                            "device_settings" => "device",
                            "device_ui_state" => "ui_state",
                            _ => marked
                                .settings_section
                                .lock()
                                .expect("settings section")
                                .as_ref()
                                .copied()
                                .unwrap_or("general"),
                        };
                        marked
                            .settings_sections
                            .lock()
                            .expect("settings sections")
                            .insert(section);
                    }
                }
            },
        ))?;
        let committed = Arc::clone(&signal);
        connection.commit_hook(Some(move || {
            let changed = committed.pending.swap(0, Ordering::AcqRel);
            committed.publish_library_changes();
            if changed & ChangeFeed::Settings.bit() != 0 {
                committed.settings_generation.fetch_add(1, Ordering::AcqRel);
                let sections = std::mem::take(
                    &mut *committed
                        .settings_sections
                        .lock()
                        .expect("settings sections"),
                );
                let listeners = committed
                    .settings_listeners
                    .lock()
                    .expect("settings listeners")
                    .clone();
                for section in sections {
                    for listener in &listeners {
                        listener(section);
                    }
                }
            }
            if changed != 0 {
                committed.notify(changed);
            }
            *committed.settings_section.lock().expect("settings section") = None;
            false
        }))?;
        let rolled_back = Arc::clone(&signal);
        connection.rollback_hook(Some(move || {
            rolled_back.pending.store(0, Ordering::Release);
            {
                let mut pending = rolled_back.library_changes.lock().expect("library changes");
                pending.ids.clear();
            }
            rolled_back
                .settings_sections
                .lock()
                .expect("settings sections")
                .clear();
            *rolled_back
                .settings_section
                .lock()
                .expect("settings section") = None;
        }))?;
        Ok(signal)
    }

    fn publish_library_changes(&self) {
        let changes = {
            let mut pending = self.library_changes.lock().expect("library changes");
            std::mem::take(&mut pending.ids)
        };
        let listeners = self
            .library_listeners
            .lock()
            .expect("library listeners")
            .clone();
        for (kind, id) in changes {
            for listener in &listeners {
                listener(kind, &id);
            }
        }
    }

    pub(crate) fn settings_section(&self, section: &'static str) {
        *self.settings_section.lock().expect("settings section") = Some(section);
    }

    pub(crate) fn settings_generation(&self) -> u64 {
        self.settings_generation.load(Ordering::Acquire)
    }

    fn notify(&self, changed: u8) {
        let listeners = self
            .listeners
            .lock()
            .map(|listeners| listeners.clone())
            .unwrap_or_default();
        for feed in ChangeFeed::ALL {
            if changed & feed.bit() == 0 {
                continue;
            }
            for (listening, listener) in &listeners {
                if *listening == feed {
                    listener();
                }
            }
        }
    }

    fn listen_library(&self, listener: LibraryChangeListener) {
        self.library_listeners
            .lock()
            .expect("library listeners")
            .push(listener);
    }

    fn listen(&self, feed: ChangeFeed, listener: ChangeListener) {
        if let Ok(mut listeners) = self.listeners.lock() {
            listeners.push((feed, listener));
        }
    }
}

impl Database {
    /// Calls `listener` after every committed transaction that changed a
    /// job, on the committing thread; the listener must not use this
    /// database.
    pub fn on_job_change(&self, listener: impl Fn() + Send + Sync + 'static) {
        self.changes.listen(ChangeFeed::Jobs, Arc::new(listener));
    }

    /// Calls `listener` after every committed transaction that changed or
    /// removed a conversation, on the committing thread; the listener must
    /// not use this database.
    pub fn on_conversation_change(&self, listener: impl Fn() + Send + Sync + 'static) {
        self.changes
            .listen(ChangeFeed::Conversations, Arc::new(listener));
    }
}

impl Database {
    pub fn on_model_change(&self, listener: impl Fn() + Send + Sync + 'static) {
        self.changes.listen(ChangeFeed::Models, Arc::new(listener));
    }
}

impl Database {
    pub fn on_settings_change(&self, listener: impl Fn() + Send + Sync + 'static) {
        self.changes
            .listen(ChangeFeed::Settings, Arc::new(listener));
    }
}

impl Database {
    pub fn on_settings_section_change(
        &self,
        listener: impl Fn(&'static str) + Send + Sync + 'static,
    ) {
        self.changes
            .settings_listeners
            .lock()
            .expect("settings listeners")
            .push(Arc::new(listener));
    }
}

impl Database {
    pub fn on_library_change(&self, listener: impl Fn(LibraryKind, &str) + Send + Sync + 'static) {
        self.changes.listen_library(Arc::new(listener));
    }
}

#[cfg(test)]
mod library_tests {
    use super::*;







    #[test]
    fn changing_the_default_refreshes_both_personas() {
        let connection = rusqlite::Connection::open_in_memory().expect("connection");
        connection.execute_batch("CREATE TABLE persona_defaults (id INTEGER, default_persona_id TEXT); INSERT INTO persona_defaults VALUES (1, 'old');").expect("schema");
        let signal = ChangeSignal::install(&connection).expect("hooks");
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        signal.listen_library(Arc::new(move |kind, id| {
            captured.lock().expect("events").push((kind, id.to_owned()));
        }));
        connection
            .execute_batch("UPDATE persona_defaults SET default_persona_id='new';")
            .expect("change");
        assert_eq!(
            *events.lock().expect("events"),
            vec![
                (LibraryKind::Persona, "new".to_owned()),
                (LibraryKind::Persona, "old".to_owned())
            ]
        );
    }

    #[test]
    fn library_ids_are_deduplicated_and_rollback_discards_deleted_rows() {
        let connection = rusqlite::Connection::open_in_memory().expect("connection");
        connection.execute_batch("CREATE TABLE characters (id TEXT PRIMARY KEY, name TEXT); CREATE TABLE character_media (character_id TEXT, asset_id TEXT);").expect("schema");
        let signal = ChangeSignal::install(&connection).expect("hooks");
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        signal.listen_library(Arc::new(move |kind, id| {
            captured.lock().expect("events").push((kind, id.to_owned()));
        }));
        connection.execute_batch("BEGIN; INSERT INTO characters VALUES ('character', 'name'); INSERT INTO character_media VALUES ('character', 'asset'); UPDATE characters SET name = 'edited'; COMMIT;").expect("commit");
        assert_eq!(
            *events.lock().expect("events"),
            vec![(LibraryKind::Character, "character".to_owned())]
        );
        events.lock().expect("events").clear();
        connection
            .execute_batch("BEGIN; DELETE FROM characters; DELETE FROM character_media; ROLLBACK;")
            .expect("rollback");
        assert!(events.lock().expect("events").is_empty());
        connection
            .execute_batch("BEGIN; DELETE FROM characters; DELETE FROM character_media; COMMIT;")
            .expect("delete");
        assert_eq!(
            *events.lock().expect("events"),
            vec![(LibraryKind::Character, "character".to_owned())]
        );
    }

    #[test]
    fn reused_transaction_statements_publish_only_their_committed_ids() {
        let connection = rusqlite::Connection::open_in_memory().expect("connection");
        connection
            .execute_batch("CREATE TABLE characters (id TEXT PRIMARY KEY);")
            .expect("schema");
        let signal = ChangeSignal::install(&connection).expect("signal");
        let events = Arc::new(Mutex::new(Vec::new()));
        let observed = events.clone();
        signal.listen_library(Arc::new(move |kind, id| {
            observed.lock().expect("events").push((kind, id.to_owned()))
        }));
        let mut begin = connection.prepare("BEGIN").expect("begin");
        let mut commit = connection.prepare("COMMIT").expect("commit");
        let mut rollback = connection.prepare("ROLLBACK").expect("rollback");
        begin.execute([]).expect("begin first");
        connection
            .execute("INSERT INTO characters VALUES ('first')", [])
            .expect("first");
        assert!(events.lock().expect("events").is_empty());
        commit.execute([]).expect("commit first");
        begin.execute([]).expect("begin empty");
        rollback.execute([]).expect("rollback empty");
        begin.execute([]).expect("begin failed");
        connection
            .execute("INSERT INTO characters VALUES ('discarded')", [])
            .expect("discarded");
        rollback.execute([]).expect("rollback write");
        begin.execute([]).expect("begin second");
        connection
            .execute("INSERT INTO characters VALUES ('second')", [])
            .expect("second");
        commit.execute([]).expect("commit second");
        assert_eq!(
            *events.lock().expect("events"),
            vec![
                (LibraryKind::Character, "first".to_owned()),
                (LibraryKind::Character, "second".to_owned())
            ]
        );
    }


}
