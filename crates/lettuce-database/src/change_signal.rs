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

type ChangeListener = Arc<dyn Fn() + Send + Sync>;
type SettingsChangeListener = Arc<dyn Fn(&'static str) + Send + Sync>;

pub(crate) struct ChangeSignal {
    pending: AtomicU8,
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
            settings_section: Mutex::new(None),
            settings_sections: Mutex::new(Default::default()),
            settings_listeners: Mutex::new(Vec::new()),
            settings_generation: AtomicU64::new(0),
            listeners: Mutex::new(Vec::new()),
        });
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
