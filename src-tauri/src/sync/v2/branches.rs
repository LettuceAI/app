use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, OptionalExtension, Transaction};

pub fn materialize_message_forks(tx: &Transaction<'_>) -> Result<usize, rusqlite::Error> {
    let mut materialized = 0;
    for config in [
        BranchConfig {
            session_table: "sessions",
            message_table: "messages",
            variant_table: "message_variants",
            title_column: "title",
            branch_seed: "direct",
            update_companion_effects: true,
        },
        BranchConfig {
            session_table: "group_sessions",
            message_table: "group_messages",
            variant_table: "group_message_variants",
            title_column: "name",
            branch_seed: "group",
            update_companion_effects: false,
        },
    ] {
        if table_exists(tx, config.message_table)? {
            materialized += materialize_table_forks(tx, config)?;
        }
    }
    Ok(materialized)
}

#[derive(Clone, Copy)]
struct BranchConfig {
    session_table: &'static str,
    message_table: &'static str,
    variant_table: &'static str,
    title_column: &'static str,
    branch_seed: &'static str,
    update_companion_effects: bool,
}

fn materialize_table_forks(
    tx: &Transaction<'_>,
    config: BranchConfig,
) -> Result<usize, rusqlite::Error> {
    let mut materialized = 0;
    while let Some(fork) = next_fork(tx, config)? {
        let winner = fork.children[0].clone();
        for losing_child in fork.children.into_iter().skip(1) {
            materialize_branch(
                tx,
                config,
                &fork.session_id,
                fork.parent_message_id.as_deref(),
                &winner,
                &losing_child,
            )?;
            materialized += 1;
        }
    }
    Ok(materialized)
}

struct MessageFork {
    session_id: String,
    parent_message_id: Option<String>,
    children: Vec<String>,
}

fn next_fork(
    tx: &Transaction<'_>,
    config: BranchConfig,
) -> Result<Option<MessageFork>, rusqlite::Error> {
    let message_table = quote_identifier(config.message_table);
    let fork = tx
        .query_row(
            &format!(
                "SELECT session_id, parent_message_id
                 FROM {message_table}
                 GROUP BY session_id, parent_message_id
                 HAVING COUNT(*) > 1
                 ORDER BY session_id ASC, parent_message_id ASC
                 LIMIT 1"
            ),
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()?;
    let Some((session_id, parent_message_id)) = fork else {
        return Ok(None);
    };

    let mut statement = tx.prepare(&format!(
        "SELECT id
         FROM {message_table}
         WHERE session_id = ?1
           AND (
             parent_message_id = ?2
             OR (parent_message_id IS NULL AND ?2 IS NULL)
           )
         ORDER BY id ASC"
    ))?;
    let children = statement
        .query_map(params![session_id, parent_message_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(MessageFork {
        session_id,
        parent_message_id,
        children,
    }))
}

fn materialize_branch(
    tx: &Transaction<'_>,
    config: BranchConfig,
    source_session_id: &str,
    fork_parent_id: Option<&str>,
    _winning_child_id: &str,
    losing_child_id: &str,
) -> Result<(), rusqlite::Error> {
    let branch_id = deterministic_uuid(&format!(
        "sync-message-branch\0{}\0{source_session_id}\0{}\0{losing_child_id}",
        config.branch_seed,
        fork_parent_id.unwrap_or("<root>")
    ));
    let session_table = quote_identifier(config.session_table);
    let title_column = quote_identifier(config.title_column);
    let (source_title, source_root): (String, Option<String>) = tx.query_row(
        &format!(
            "SELECT {title_column}, root_session_id
             FROM {session_table}
             WHERE id = ?1"
        ),
        [source_session_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let root_session_id = source_root.unwrap_or_else(|| source_session_id.to_string());
    let branch_title = format!("{source_title} (branch)");

    let mut session_overrides = HashMap::new();
    session_overrides.insert("id", Value::Text(branch_id.clone()));
    session_overrides.insert(
        "parent_session_id",
        Value::Text(source_session_id.to_string()),
    );
    session_overrides.insert(
        "branched_from_message_id",
        fork_parent_id
            .map(|id| Value::Text(deterministic_message_id(&branch_id, id)))
            .unwrap_or(Value::Null),
    );
    session_overrides.insert("root_session_id", Value::Text(root_session_id));
    session_overrides.insert(config.title_column, Value::Text(branch_title));
    copy_row(
        tx,
        config.session_table,
        source_session_id,
        &session_overrides,
    )?;

    let prefix = ancestor_path(tx, config.message_table, source_session_id, fork_parent_id)?;
    let mut cloned_parent_id = None;
    for source_message_id in prefix {
        let cloned_id = deterministic_message_id(&branch_id, &source_message_id);
        let mut message_overrides = HashMap::new();
        message_overrides.insert("id", Value::Text(cloned_id.clone()));
        message_overrides.insert("session_id", Value::Text(branch_id.clone()));
        message_overrides.insert(
            "parent_message_id",
            cloned_parent_id
                .as_ref()
                .map(|id: &String| Value::Text(id.clone()))
                .unwrap_or(Value::Null),
        );
        copy_row(
            tx,
            config.message_table,
            &source_message_id,
            &message_overrides,
        )?;
        clone_message_variants(
            tx,
            config.variant_table,
            &branch_id,
            &source_message_id,
            &cloned_id,
        )?;
        cloned_parent_id = Some(cloned_id);
    }

    let subtree = descendant_ids(
        tx,
        config.message_table,
        source_session_id,
        losing_child_id,
    )?;
    let message_table = quote_identifier(config.message_table);
    for message_id in &subtree {
        tx.execute(
            &format!("UPDATE {message_table} SET session_id = ?1 WHERE id = ?2"),
            params![branch_id, message_id],
        )?;
    }
    tx.execute(
        &format!(
            "UPDATE {message_table}
             SET parent_message_id = ?1
             WHERE id = ?2"
        ),
        params![cloned_parent_id, losing_child_id],
    )?;

    if config.update_companion_effects && table_exists(tx, "companion_turn_effects")? {
        for message_id in &subtree {
            tx.execute(
                "UPDATE companion_turn_effects
                 SET session_id = ?1
                 WHERE assistant_message_id = ?2",
                params![branch_id, message_id],
            )?;
        }
    }
    tx.execute(
        &format!(
            "UPDATE {session_table} AS target
             SET updated_at = (
               SELECT COALESCE(MAX(created_at), target.updated_at)
               FROM {message_table}
               WHERE session_id = target.id
             )
             WHERE id IN (?1, ?2)"
        ),
        params![source_session_id, branch_id],
    )?;
    Ok(())
}

fn ancestor_path(
    tx: &Transaction<'_>,
    message_table: &str,
    session_id: &str,
    message_id: Option<&str>,
) -> Result<Vec<String>, rusqlite::Error> {
    let Some(message_id) = message_id else {
        return Ok(Vec::new());
    };
    let message_table = quote_identifier(message_table);
    let mut statement = tx.prepare(&format!(
        "WITH RECURSIVE ancestors(id, parent_message_id, depth) AS (
           SELECT id, parent_message_id, 0
           FROM {message_table}
           WHERE id = ?1 AND session_id = ?2
           UNION ALL
           SELECT parent.id, parent.parent_message_id, ancestors.depth + 1
           FROM {message_table} AS parent
           JOIN ancestors ON parent.id = ancestors.parent_message_id
           WHERE parent.session_id = ?2
         )
         SELECT id FROM ancestors ORDER BY depth DESC"
    ))?;
    let ids = statement
        .query_map(params![message_id, session_id], |row| row.get(0))?
        .collect();
    ids
}

fn descendant_ids(
    tx: &Transaction<'_>,
    message_table: &str,
    session_id: &str,
    root_message_id: &str,
) -> Result<Vec<String>, rusqlite::Error> {
    let message_table = quote_identifier(message_table);
    let mut statement = tx.prepare(&format!(
        "WITH RECURSIVE descendants(id) AS (
           SELECT id FROM {message_table} WHERE id = ?1 AND session_id = ?2
           UNION ALL
           SELECT child.id
           FROM {message_table} AS child
           JOIN descendants ON child.parent_message_id = descendants.id
           WHERE child.session_id = ?2
         )
         SELECT id FROM descendants"
    ))?;
    let ids = statement
        .query_map(params![root_message_id, session_id], |row| row.get(0))?
        .collect();
    ids
}

fn clone_message_variants(
    tx: &Transaction<'_>,
    variant_table: &str,
    branch_id: &str,
    source_message_id: &str,
    cloned_message_id: &str,
) -> Result<(), rusqlite::Error> {
    let variant_table_quoted = quote_identifier(variant_table);
    let mut statement = tx.prepare(&format!(
        "SELECT id FROM {variant_table_quoted} WHERE message_id = ?1 ORDER BY id"
    ))?;
    let variant_ids = statement
        .query_map([source_message_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for variant_id in variant_ids {
        let mut overrides = HashMap::new();
        overrides.insert(
            "id",
            Value::Text(deterministic_uuid(&format!(
                "sync-message-variant\0{branch_id}\0{variant_id}"
            ))),
        );
        overrides.insert(
            "message_id",
            Value::Text(cloned_message_id.to_string()),
        );
        copy_row(tx, variant_table, &variant_id, &overrides)?;
    }
    Ok(())
}

fn copy_row(
    tx: &Transaction<'_>,
    table: &str,
    source_id: &str,
    overrides: &HashMap<&str, Value>,
) -> Result<(), rusqlite::Error> {
    let quoted_table = quote_identifier(table);
    let mut statement = tx.prepare(&format!("PRAGMA table_info({quoted_table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;

    let column_list = columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let mut values = Vec::new();
    let select_list = columns
        .iter()
        .map(|column| {
            if let Some(value) = overrides.get(column.as_str()) {
                values.push(value.clone());
                format!("?{}", values.len())
            } else {
                quote_identifier(column)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    values.push(Value::Text(source_id.to_string()));
    tx.execute(
        &format!(
            "INSERT OR IGNORE INTO {quoted_table} ({column_list})
             SELECT {select_list}
             FROM {quoted_table}
             WHERE id = ?{}",
            values.len()
        ),
        params_from_iter(values),
    )?;
    Ok(())
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn table_exists(tx: &Transaction<'_>, table: &str) -> Result<bool, rusqlite::Error> {
    tx.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sqlite_schema
           WHERE type = 'table' AND name = ?1
         )",
        [table],
        |row| row.get(0),
    )
}

fn deterministic_message_id(branch_id: &str, source_message_id: &str) -> String {
    deterministic_uuid(&format!(
        "sync-message-copy\0{branch_id}\0{source_message_id}"
    ))
}

fn deterministic_uuid(seed: &str) -> String {
    let hash = blake3::hash(seed.as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&hash.as_bytes()[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use super::materialize_message_forks;
    use crate::sync::v2::{apply_remote_revision, capture_transaction, create_schema};

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE sessions (
               id TEXT PRIMARY KEY,
               title TEXT NOT NULL,
               parent_session_id TEXT,
               branched_from_message_id TEXT,
               root_session_id TEXT,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
             );
             CREATE TABLE messages (
               id TEXT PRIMARY KEY,
               session_id TEXT NOT NULL,
               parent_message_id TEXT,
               role TEXT NOT NULL,
               content TEXT NOT NULL,
               created_at INTEGER NOT NULL,
               FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE,
               FOREIGN KEY(parent_message_id) REFERENCES messages(id) ON DELETE SET NULL
             );
             CREATE TABLE message_variants (
               id TEXT PRIMARY KEY,
               message_id TEXT NOT NULL,
               content TEXT NOT NULL,
               FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
             );
             CREATE TABLE companion_turn_effects (
               id TEXT PRIMARY KEY,
               session_id TEXT NOT NULL,
               assistant_message_id TEXT NOT NULL
             );
             CREATE TRIGGER messages_assign_parent_after_insert
             AFTER INSERT ON messages
             WHEN NEW.parent_message_id IS NULL
             BEGIN
               UPDATE messages
               SET parent_message_id = (
                 SELECT previous.id
                 FROM messages AS previous
                 WHERE previous.session_id = NEW.session_id
                   AND previous.id != NEW.id
                 ORDER BY previous.rowid DESC
                 LIMIT 1
               )
               WHERE id = NEW.id;
             END;",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions VALUES ('chat', 'Original', NULL, NULL, 'chat', 1, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages VALUES ('shared', 'chat', NULL, 'user', 'hello', 1)",
            [],
        )
        .unwrap();
        conn
    }

    fn setup_replica() -> Connection {
        let conn = setup();
        create_schema(&conn).unwrap();
        conn
    }

    fn setup_group_replica() -> Connection {
        let conn = setup();
        conn.execute_batch(
            "CREATE TABLE group_sessions (
               id TEXT PRIMARY KEY,
               name TEXT NOT NULL,
               parent_session_id TEXT,
               branched_from_message_id TEXT,
               root_session_id TEXT,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
             );
             CREATE TABLE group_messages (
               id TEXT PRIMARY KEY,
               session_id TEXT NOT NULL,
               parent_message_id TEXT,
               role TEXT NOT NULL,
               content TEXT NOT NULL,
               created_at INTEGER NOT NULL
             );
             CREATE TABLE group_message_variants (
               id TEXT PRIMARY KEY,
               message_id TEXT NOT NULL,
               content TEXT NOT NULL
             );
             INSERT INTO group_sessions
             VALUES ('group-chat', 'Group', NULL, NULL, 'group-chat', 1, 1);
             INSERT INTO group_messages
             VALUES ('group-shared', 'group-chat', NULL, 'user', 'hello', 1);",
        )
        .unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn divergent_tails_become_deterministic_separate_sessions() {
        let mut conn = setup();
        conn.execute(
            "INSERT INTO messages VALUES ('tail-a', 'chat', 'shared', 'user', 'A', 2)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages VALUES ('tail-b', 'chat', 'shared', 'user', 'B', 2)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages VALUES ('reply-b', 'chat', 'tail-b', 'assistant', 'B reply', 3)",
            [],
        )
        .unwrap();

        let tx = conn.transaction().unwrap();
        assert_eq!(materialize_message_forks(&tx).unwrap(), 1);
        tx.commit().unwrap();

        let original = conn
            .prepare("SELECT content FROM messages WHERE session_id = 'chat' ORDER BY created_at")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(original, vec!["hello", "A"]);

        let branch_id: String = conn
            .query_row(
                "SELECT id FROM sessions WHERE parent_session_id = 'chat'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let branch = conn
            .prepare(
                "SELECT content FROM messages WHERE session_id = ?1 ORDER BY created_at, id",
            )
            .unwrap()
            .query_map([&branch_id], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(branch, vec!["hello", "B", "B reply"]);
        let branch_parent: Option<String> = conn
            .query_row(
                "SELECT parent_message_id FROM messages
                 WHERE session_id = ?1 AND content = 'B'",
                [&branch_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(branch_parent.is_some());
        assert_eq!(materialize_message_forks(&conn.unchecked_transaction().unwrap()).unwrap(), 0);
    }

    #[test]
    fn root_forks_do_not_mix() {
        let mut conn = setup();
        conn.execute("DELETE FROM messages", []).unwrap();
        for (id, content) in [("root-a", "A"), ("root-b", "B")] {
            conn.execute(
                "INSERT INTO messages VALUES (?1, 'chat', NULL, 'user', ?2, 1)",
                params![id, content],
            )
            .unwrap();
        }
        conn.execute(
            "UPDATE messages SET parent_message_id = NULL WHERE id = 'root-b'",
            [],
        )
        .unwrap();
        let tx = conn.transaction().unwrap();
        assert_eq!(materialize_message_forks(&tx).unwrap(), 1);
        tx.commit().unwrap();
        let original_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id = 'chat'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(original_count, 1);
    }

    #[test]
    fn concurrent_replica_tails_converge_as_separate_branches() {
        let left = setup_replica();
        let right = setup_replica();

        let left_tail = capture_transaction(&left, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO messages VALUES (
                   'tail-a', 'chat', 'shared', 'user', 'A', 2
                 )",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        let right_tail = capture_transaction(&right, "device-b", 100, |tx| {
            tx.execute(
                "INSERT INTO messages VALUES (
                   'tail-b', 'chat', 'shared', 'user', 'B', 2
                 )",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();

        apply_remote_revision(&left, &right_tail, 200).unwrap();
        apply_remote_revision(&right, &left_tail, 200).unwrap();

        fn transcript_rows(conn: &Connection) -> Vec<(String, String, String)> {
            let mut statement = conn
                .prepare(
                    "SELECT session_id, id, content
                     FROM messages
                     ORDER BY session_id, created_at, id",
                )
                .unwrap();
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        }

        assert_eq!(transcript_rows(&left), transcript_rows(&right));
        let original_tail_count: i64 = left
            .query_row(
                "SELECT COUNT(*) FROM messages
                 WHERE session_id = 'chat' AND parent_message_id = 'shared'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(original_tail_count, 1);
        let branch_count: i64 = left
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE parent_session_id = 'chat'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(branch_count, 1);
    }

    #[test]
    fn captured_insert_includes_the_trigger_assigned_parent() {
        let source = setup_replica();
        let target = setup_replica();
        let revision = capture_transaction(&source, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO messages VALUES (
                   'next', 'chat', NULL, 'user', 'next', 2
                 )",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();

        apply_remote_revision(&target, &revision, 200).unwrap();
        let parent: Option<String> = target
            .query_row(
                "SELECT parent_message_id FROM messages WHERE id = 'next'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(parent.as_deref(), Some("shared"));
    }

    #[test]
    fn concurrent_group_tails_converge_as_separate_group_sessions() {
        let left = setup_group_replica();
        let right = setup_group_replica();
        let left_tail = capture_transaction(&left, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO group_messages VALUES (
                   'group-tail-a', 'group-chat', 'group-shared', 'user', 'A', 2
                 )",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        let right_tail = capture_transaction(&right, "device-b", 100, |tx| {
            tx.execute(
                "INSERT INTO group_messages VALUES (
                   'group-tail-b', 'group-chat', 'group-shared', 'user', 'B', 2
                 )",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();

        apply_remote_revision(&left, &right_tail, 200).unwrap();
        apply_remote_revision(&right, &left_tail, 200).unwrap();

        let rows = |conn: &Connection| {
            let mut statement = conn
                .prepare(
                    "SELECT session_id, id, content
                     FROM group_messages
                     ORDER BY session_id, created_at, id",
                )
                .unwrap();
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .collect::<Result<Vec<(String, String, String)>, _>>()
                .unwrap()
        };
        assert_eq!(rows(&left), rows(&right));
        let original_tail_count: i64 = left
            .query_row(
                "SELECT COUNT(*) FROM group_messages
                 WHERE session_id = 'group-chat'
                   AND parent_message_id = 'group-shared'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(original_tail_count, 1);
        let branch_count: i64 = left
            .query_row(
                "SELECT COUNT(*) FROM group_sessions
                 WHERE parent_session_id = 'group-chat'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(branch_count, 1);
    }
}
