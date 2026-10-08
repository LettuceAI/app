//! Tells listeners which change feeds a committed transaction wrote: the
//! update hook marks writes to a feed table, the commit hook reports the
//! marked feeds, and a rollback discards them.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, Ordering},
};

use crate::Database;

/// A change feed table the signal watches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChangeFeed {
    Jobs,
    Conversations,
    Models,
}

impl ChangeFeed {
    const ALL: [Self; 3] = [Self::Jobs, Self::Conversations, Self::Models];

    const fn bit(self) -> u8 {
        match self {
            Self::Jobs => 1,
            Self::Conversations => 2,
            Self::Models => 4,
        }
    }

    fn of_table(table: &str) -> Option<Self> {
        match table {
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

pub(crate) struct ChangeSignal {
    pending: AtomicU8,
    listeners: Mutex<Vec<(ChangeFeed, ChangeListener)>>,
}

impl ChangeSignal {
    pub(crate) fn install(connection: &rusqlite::Connection) -> rusqlite::Result<Arc<Self>> {
        let signal = Arc::new(Self {
            pending: AtomicU8::new(0),
            listeners: Mutex::new(Vec::new()),
        });
        let marked = Arc::clone(&signal);
        connection.update_hook(Some(
            move |_: rusqlite::hooks::Action, _: &str, table: &str, _: i64| {
                if let Some(feed) = ChangeFeed::of_table(table) {
                    marked.pending.fetch_or(feed.bit(), Ordering::AcqRel);
                }
            },
        ))?;
        let committed = Arc::clone(&signal);
        connection.commit_hook(Some(move || {
            let changed = committed.pending.swap(0, Ordering::AcqRel);
            if changed != 0 {
                committed.notify(changed);
            }
            false
        }))?;
        let rolled_back = Arc::clone(&signal);
        connection.rollback_hook(Some(move || {
            rolled_back.pending.store(0, Ordering::Release);
        }))?;
        Ok(signal)
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
