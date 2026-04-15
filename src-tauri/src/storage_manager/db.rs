use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::Duration;

use super::legacy::storage_root;
use crate::migrations;
use crate::sync::db::LOCAL_SYNC_STATE_VERSION;
use crate::utils::{log_info, log_info_global, log_warn, log_warn_global, now_millis};

pub fn db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(storage_root(app)?.join("app.db"))
}

#[tauri::command]
pub fn storage_db_size(app: tauri::AppHandle) -> Result<u64, String> {
    let db = db_path(&app)?;
    let mut total: u64 = 0;
    let wal = db.with_extension("db-wal");
    let shm = db.with_extension("db-shm");
    for path in [db, wal, shm] {
        if path.exists() {
            let meta = fs::metadata(&path)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            total = total.saturating_add(meta.len());
        }
    }
    Ok(total)
}

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use tauri::{Emitter, Manager};

pub type DbPool = Pool<SqliteConnectionManager>;
pub type DbConnection = r2d2::PooledConnection<SqliteConnectionManager>;

/// Wrapper that allows the database pool to be swapped at runtime.
/// This is used for backup restore without requiring app restart.
pub struct SwappablePool {
    pool: RwLock<DbPool>,
}

impl SwappablePool {
    pub fn new(pool: DbPool) -> Self {
        Self {
            pool: RwLock::new(pool),
        }
    }

    /// Get a connection from the current pool
    pub fn get_connection(&self) -> Result<DbConnection, String> {
        let pool = self.pool.read().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Pool lock poisoned: {}", e),
            )
        })?;
        let conn = pool.get().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to get connection from pool: {}", e),
            )
        })?;

        let state = pool.state();
        log_info_global(
            "db_status",
            format!(
                "connection acquired total={} idle={}",
                state.connections, state.idle_connections
            ),
        );

        Ok(conn)
    }

    /// Swap the pool with a new one (used after backup restore)
    pub fn swap(&self, new_pool: DbPool) -> Result<(), String> {
        let mut pool = self.pool.write().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Pool lock poisoned: {}", e),
            )
        })?;
        *pool = new_pool;
        Ok(())
    }
}

fn attach_db_logging(c: &mut Connection) {
    c.trace(Some(|stmt: &str| {
        let trimmed = stmt.trim();
        if trimmed.starts_with("PRAGMA") {
            return;
        }
        let is_write = trimmed.starts_with("INSERT")
            || trimmed.starts_with("UPDATE")
            || trimmed.starts_with("DELETE")
            || trimmed.starts_with("CREATE")
            || trimmed.starts_with("DROP")
            || trimmed.starts_with("ALTER");
        let label = if is_write { "db_write" } else { "db_read" };
        let display = if trimmed.len() > 1024 {
            format!("{}...", &trimmed[..1024])
        } else {
            trimmed.to_string()
        };
        log_info_global(label, &display);
    }));
    c.profile(Some(|stmt: &str, dur: Duration| {
        let ms = dur.as_millis();
        if ms >= 50 {
            let trimmed = stmt.trim();
            let display = if trimmed.len() > 768 {
                format!("{}...", &trimmed[..768])
            } else {
                trimmed.to_string()
            };
            log_warn_global("db_slow", format!("{}ms | {}", ms, display));
        }
    }));
}

/// Create a new pool for a given database path
pub fn create_pool_for_path(path: &PathBuf) -> Result<DbPool, String> {
    let manager = SqliteConnectionManager::file(path).with_init(|c| {
        c.busy_timeout(Duration::from_secs(5))?;
        c.execute_batch(
            r#"
                PRAGMA journal_mode=WAL;
                PRAGMA synchronous=NORMAL;
                PRAGMA temp_store=MEMORY;
                PRAGMA cache_size=-8000;
                PRAGMA busy_timeout=5000;
                PRAGMA wal_autocheckpoint=1000;
                PRAGMA mmap_size=268435456;
                PRAGMA foreign_keys=ON;
                "#,
        )?;
        attach_db_logging(c);
        Ok(())
    });

    Pool::builder().max_size(10).build(manager).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create pool: {}", e),
        )
    })
}

pub fn reload_database(app: &tauri::AppHandle) -> Result<(), String> {
    use crate::utils::log_info;

    let path = db_path(app)?;
    log_info(
        app,
        "database",
        format!("Reloading database from {:?}", path),
    );

    {
        let conn = open_db(app)?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("WAL checkpoint failed before reload: {}", e),
                )
            })?;
        log_info(app, "database", "WAL checkpoint completed before reload");
    }

    let new_pool = create_pool_for_path(&path)?;

    let conn = new_pool.get().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to get connection from new pool: {}", e),
        )
    })?;
    init_db(app, &conn)?;

    drop(conn);

    let swappable = app.state::<SwappablePool>();
    swappable.swap(new_pool)?;

    migrations::run_migrations(app)?;

    log_info(app, "database", "Database pool reloaded successfully");

    let _ = app.emit("database-reloaded", ());

    Ok(())
}

pub fn init_pool(app: &tauri::AppHandle) -> Result<DbPool, String> {
    let path = db_path(app)?;

    // Debug logging
    log_info(app, "database", format!("Database path: {:?}", path));
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            log_info(
                app,
                "database",
                format!("Creating parent directory: {:?}", parent),
            );
            fs::create_dir_all(parent).map_err(|e| {
                log_warn(
                    app,
                    "database",
                    format!("Failed to create parent directory: {:?}", e),
                );
                e.to_string()
            })?;
        }
    }

    let manager = SqliteConnectionManager::file(&path).with_init(|c| {
        c.busy_timeout(Duration::from_secs(5))?;
        c.execute_batch(
            r#"
                PRAGMA journal_mode=WAL;
                PRAGMA synchronous=NORMAL;
                PRAGMA temp_store=MEMORY;
                PRAGMA cache_size=-8000;
                PRAGMA busy_timeout=5000;
                PRAGMA wal_autocheckpoint=1000;
                PRAGMA mmap_size=268435456;
                PRAGMA foreign_keys=ON;
                "#,
        )?;
        attach_db_logging(c);
        Ok(())
    });

    let pool = Pool::builder().max_size(10).build(manager).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create pool: {}", e),
        )
    })?;

    // Initialize the database schema on the first connection
    let conn = pool.get().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to get connection from pool for init: {}", e),
        )
    })?;
    init_db(app, &conn)?;

    Ok(pool)
}

pub fn open_db(app: &tauri::AppHandle) -> Result<DbConnection, String> {
    let swappable = app.state::<SwappablePool>();
    swappable.get_connection()
}

pub fn init_db(_app: &tauri::AppHandle, conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS meta (
          key TEXT PRIMARY KEY,
          value TEXT
        );

        CREATE TABLE IF NOT EXISTS settings (
          id INTEGER PRIMARY KEY CHECK(id=1),
          default_provider_credential_id TEXT,
          default_model_id TEXT,
          app_state TEXT NOT NULL DEFAULT '{}',
          advanced_model_settings TEXT,
          prompt_template_id TEXT,
          system_prompt TEXT,
          advanced_settings TEXT,
          migration_version INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS provider_credentials (
          id TEXT PRIMARY KEY,
          provider_id TEXT NOT NULL,
          label TEXT NOT NULL,
          api_key_ref TEXT,
          api_key TEXT,
          base_url TEXT,
          default_model TEXT,
          headers TEXT
        );

        CREATE TABLE IF NOT EXISTS models (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          provider_id TEXT NOT NULL,
          provider_credential_id TEXT,
          provider_label TEXT NOT NULL,
          display_name TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          model_type TEXT NOT NULL DEFAULT 'chat',
          input_scopes TEXT,
          output_scopes TEXT,
          advanced_model_settings TEXT,
          prompt_template_id TEXT,
          system_prompt TEXT
        );

        -- Secrets (API keys and similar), stored in DB instead of JSON
        CREATE TABLE IF NOT EXISTS secrets (
          service TEXT NOT NULL,
          account TEXT NOT NULL,
          value TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(service, account)
        );

        -- System prompt templates (migrated from JSON file)
        CREATE TABLE IF NOT EXISTS prompt_templates (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          prompt_type TEXT NOT NULL DEFAULT 'undefined',
          content TEXT NOT NULL,
          entries TEXT NOT NULL DEFAULT '[]',
          condense_prompt_entries INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        -- Characters
        CREATE TABLE IF NOT EXISTS characters (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          avatar_path TEXT,
          avatar_crop_x REAL,
          avatar_crop_y REAL,
          avatar_crop_scale REAL,
          design_description TEXT,
          design_reference_image_ids TEXT,
          background_image_path TEXT,
          description TEXT,
          definition TEXT,
          nickname TEXT,
          scenario TEXT,
          creator_notes TEXT,
          creator TEXT,
          creator_notes_multilingual TEXT,
          source TEXT,
          tags TEXT,
          default_scene_id TEXT,
          default_model_id TEXT,
          fallback_model_id TEXT,
          memory_type TEXT NOT NULL DEFAULT 'manual',
          prompt_template_id TEXT,
          group_chat_prompt_template_id TEXT,
          group_chat_roleplay_prompt_template_id TEXT,
          system_prompt TEXT,
          voice_config TEXT,
          voice_autoplay INTEGER NOT NULL DEFAULT 0,
          disable_avatar_gradient INTEGER NOT NULL DEFAULT 0,
          default_chat_template_id TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS character_rules (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          character_id TEXT NOT NULL,
          idx INTEGER NOT NULL,
          rule TEXT NOT NULL,
          FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
        );

        -- Lorebooks (app-level, can be shared across characters)
        CREATE TABLE IF NOT EXISTS lorebooks (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          avatar_path TEXT,
          keyword_detection_mode TEXT NOT NULL DEFAULT 'recent_message_window',
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        -- Character <-> Lorebook mapping (many-to-many)
        CREATE TABLE IF NOT EXISTS character_lorebooks (
          character_id TEXT NOT NULL,
          lorebook_id TEXT NOT NULL,
          enabled INTEGER NOT NULL DEFAULT 1,
          display_order INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(character_id, lorebook_id),
          FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE,
          FOREIGN KEY(lorebook_id) REFERENCES lorebooks(id) ON DELETE CASCADE
        );

        -- Lorebook entries (app-level; entries belong to a lorebook)
        CREATE TABLE IF NOT EXISTS lorebook_entries (
          id TEXT PRIMARY KEY,
          lorebook_id TEXT NOT NULL,
          title TEXT NOT NULL DEFAULT '',
          enabled INTEGER NOT NULL DEFAULT 1,
          always_active INTEGER NOT NULL DEFAULT 0,
          keywords TEXT NOT NULL DEFAULT '[]',
          case_sensitive INTEGER NOT NULL DEFAULT 0,
          content TEXT NOT NULL,
          priority INTEGER NOT NULL DEFAULT 0,
          display_order INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(lorebook_id) REFERENCES lorebooks(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_lorebook_entries_lorebook ON lorebook_entries(lorebook_id);
        CREATE INDEX IF NOT EXISTS idx_lorebook_entries_enabled ON lorebook_entries(lorebook_id, enabled);
        CREATE INDEX IF NOT EXISTS idx_character_lorebooks_character ON character_lorebooks(character_id);

        CREATE TABLE IF NOT EXISTS scenes (
          id TEXT PRIMARY KEY,
          character_id TEXT NOT NULL,
          content TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          selected_variant_id TEXT,
          FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS scene_variants (
          id TEXT PRIMARY KEY,
          scene_id TEXT NOT NULL,
          content TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          FOREIGN KEY(scene_id) REFERENCES scenes(id) ON DELETE CASCADE
        );

        -- Chat templates (multi-message conversation starters)
        CREATE TABLE IF NOT EXISTS chat_templates (
          id TEXT PRIMARY KEY,
          character_id TEXT NOT NULL,
          name TEXT NOT NULL,
          scene_id TEXT,
          prompt_template_id TEXT,
          created_at INTEGER NOT NULL,
          FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS chat_template_messages (
          id TEXT PRIMARY KEY,
          template_id TEXT NOT NULL,
          idx INTEGER NOT NULL,
          role TEXT NOT NULL,
          content TEXT NOT NULL,
          FOREIGN KEY(template_id) REFERENCES chat_templates(id) ON DELETE CASCADE
        );

        -- Personas
        CREATE TABLE IF NOT EXISTS personas (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          description TEXT NOT NULL,
          nickname TEXT,
          avatar_path TEXT,
          avatar_crop_x REAL,
          avatar_crop_y REAL,
          avatar_crop_scale REAL,
          design_description TEXT,
          design_reference_image_ids TEXT,
          is_default INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        -- Sessions and messages
        CREATE TABLE IF NOT EXISTS sessions (
          id TEXT PRIMARY KEY,
          character_id TEXT NOT NULL,
          title TEXT NOT NULL,
          background_image_path TEXT,
          system_prompt TEXT,
          selected_scene_id TEXT,
          prompt_template_id TEXT,
          persona_id TEXT,
          persona_disabled INTEGER NOT NULL DEFAULT 0,
          voice_autoplay INTEGER,
          temperature REAL,
          top_p REAL,
          max_output_tokens INTEGER,
          frequency_penalty REAL,
          presence_penalty REAL,
          top_k INTEGER,
          memories TEXT NOT NULL DEFAULT '[]',
          memory_embeddings TEXT NOT NULL DEFAULT '[]',
          memory_summary TEXT,
          memory_summary_token_count INTEGER NOT NULL DEFAULT 0,
          memory_tool_events TEXT NOT NULL DEFAULT '[]',
          memory_status TEXT,
          memory_error TEXT,
          memory_progress_step INTEGER,
          archived INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE,
          FOREIGN KEY(persona_id) REFERENCES personas(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS messages (
          id TEXT PRIMARY KEY,
          session_id TEXT NOT NULL,
          role TEXT NOT NULL,
          content TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          scene_edited INTEGER NOT NULL DEFAULT 0,
          prompt_tokens INTEGER,
          completion_tokens INTEGER,
          total_tokens INTEGER,
          selected_variant_id TEXT,
          is_pinned INTEGER NOT NULL DEFAULT 0,
          memory_refs TEXT NOT NULL DEFAULT '[]',
          used_lorebook_entries TEXT NOT NULL DEFAULT '[]',
          attachments TEXT NOT NULL DEFAULT '[]',
          reasoning TEXT,
          FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS message_variants (
          id TEXT PRIMARY KEY,
          message_id TEXT NOT NULL,
          content TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          prompt_tokens INTEGER,
          completion_tokens INTEGER,
          total_tokens INTEGER,
          reasoning TEXT,
          FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
        );

        -- Smart Creator draft sessions
        CREATE TABLE IF NOT EXISTS creation_helper_sessions (
          id TEXT PRIMARY KEY,
          creation_goal TEXT NOT NULL,
          status TEXT NOT NULL,
          session_json TEXT NOT NULL,
          uploaded_images_json TEXT NOT NULL DEFAULT '{}',
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        -- Usage tracking
        CREATE TABLE IF NOT EXISTS usage_records (
          id TEXT PRIMARY KEY,
          timestamp INTEGER NOT NULL,
          session_id TEXT NOT NULL,
          character_id TEXT NOT NULL,
          character_name TEXT NOT NULL,
          model_id TEXT NOT NULL,
          model_name TEXT NOT NULL,
          provider_id TEXT NOT NULL,
          provider_label TEXT NOT NULL,
          operation_type TEXT DEFAULT 'chat',
          prompt_tokens INTEGER,
          completion_tokens INTEGER,
          total_tokens INTEGER,
          memory_tokens INTEGER,
          summary_tokens INTEGER,
          reasoning_tokens INTEGER,
          image_tokens INTEGER,
          prompt_cost REAL,
          completion_cost REAL,
          total_cost REAL,
          success INTEGER NOT NULL,
          error_message TEXT
        );

        CREATE TABLE IF NOT EXISTS usage_metadata (
          usage_id TEXT NOT NULL,
          key TEXT NOT NULL,
          value TEXT NOT NULL,
          PRIMARY KEY (usage_id, key),
          FOREIGN KEY(usage_id) REFERENCES usage_records(id) ON DELETE CASCADE
        );

        -- Model pricing cache (migrated from models_cache.json)
        CREATE TABLE IF NOT EXISTS model_pricing_cache (
          model_id TEXT PRIMARY KEY,
          pricing_json TEXT,
          cached_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS openrouter_provider_pricing_cache (
          model_id TEXT PRIMARY KEY,
          provider_pricings_json TEXT NOT NULL,
          cached_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS deferred_pricing_refreshes (
          provider_id TEXT NOT NULL,
          model_id TEXT NOT NULL,
          refresh_kind TEXT NOT NULL,
          retry_after INTEGER NOT NULL,
          last_error TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY (provider_id, model_id, refresh_kind)
        );

        -- Audio providers for TTS
        CREATE TABLE IF NOT EXISTS audio_providers (
          id TEXT PRIMARY KEY,
          provider_type TEXT NOT NULL,
          label TEXT NOT NULL,
          api_key TEXT,
          project_id TEXT,
          location TEXT DEFAULT 'us-central1',
          base_url TEXT,
          request_path TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        -- Cached voices from audio providers
        CREATE TABLE IF NOT EXISTS audio_voice_cache (
          id TEXT PRIMARY KEY,
          provider_id TEXT NOT NULL,
          voice_id TEXT NOT NULL,
          name TEXT NOT NULL,
          preview_url TEXT,
          labels TEXT,
          cached_at INTEGER NOT NULL,
          FOREIGN KEY(provider_id) REFERENCES audio_providers(id) ON DELETE CASCADE
        );

        -- User-created voice configurations
        CREATE TABLE IF NOT EXISTS user_voices (
          id TEXT PRIMARY KEY,
          provider_id TEXT NOT NULL,
          name TEXT NOT NULL,
          model_id TEXT NOT NULL,
          voice_id TEXT NOT NULL,
          prompt TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(provider_id) REFERENCES audio_providers(id) ON DELETE CASCADE
        );

        -- Group character configs (reusable group setup)
        CREATE TABLE IF NOT EXISTS group_characters (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          character_ids TEXT NOT NULL DEFAULT '[]',
          muted_character_ids TEXT NOT NULL DEFAULT '[]',
          persona_id TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          archived INTEGER NOT NULL DEFAULT 0,
          chat_type TEXT NOT NULL DEFAULT 'conversation',
          starting_scene TEXT,
          background_image_path TEXT,
          lorebook_ids TEXT NOT NULL DEFAULT '[]',
          disable_character_lorebooks INTEGER NOT NULL DEFAULT 0,
          speaker_selection_method TEXT NOT NULL DEFAULT 'llm',
          FOREIGN KEY(persona_id) REFERENCES personas(id) ON DELETE SET NULL
        );

        -- Group chat sessions (multi-character conversations)
        CREATE TABLE IF NOT EXISTS group_sessions (
          id TEXT PRIMARY KEY,
          group_character_id TEXT,
          name TEXT NOT NULL,
          character_ids TEXT NOT NULL DEFAULT '[]',
          muted_character_ids TEXT NOT NULL DEFAULT '[]',
          persona_id TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          archived INTEGER NOT NULL DEFAULT 0,
          chat_type TEXT NOT NULL DEFAULT 'conversation',
          starting_scene TEXT,
          background_image_path TEXT,
          lorebook_ids TEXT NOT NULL DEFAULT '[]',
          disable_character_lorebooks INTEGER NOT NULL DEFAULT 0,
          memories TEXT NOT NULL DEFAULT '[]',
          memory_embeddings TEXT NOT NULL DEFAULT '[]',
          memory_summary TEXT NOT NULL DEFAULT '',
          memory_summary_token_count INTEGER NOT NULL DEFAULT 0,
          memory_tool_events TEXT NOT NULL DEFAULT '[]',
          memory_status TEXT,
          memory_error TEXT,
          memory_progress_step INTEGER,
          speaker_selection_method TEXT NOT NULL DEFAULT 'llm',
          FOREIGN KEY(persona_id) REFERENCES personas(id) ON DELETE SET NULL,
          FOREIGN KEY(group_character_id) REFERENCES group_characters(id) ON DELETE SET NULL
        );

        -- Group chat participation tracking (per-character stats)
        CREATE TABLE IF NOT EXISTS group_participation (
          id TEXT PRIMARY KEY,
          session_id TEXT NOT NULL,
          character_id TEXT NOT NULL,
          speak_count INTEGER NOT NULL DEFAULT 0,
          last_spoke_turn INTEGER,
          last_spoke_at INTEGER,
          FOREIGN KEY(session_id) REFERENCES group_sessions(id) ON DELETE CASCADE
        );

        -- Group chat messages (with speaker tracking)
        CREATE TABLE IF NOT EXISTS group_messages (
          id TEXT PRIMARY KEY,
          session_id TEXT NOT NULL,
          role TEXT NOT NULL,
          content TEXT NOT NULL,
          speaker_character_id TEXT,
          turn_number INTEGER NOT NULL,
          created_at INTEGER NOT NULL,
          prompt_tokens INTEGER,
          completion_tokens INTEGER,
          total_tokens INTEGER,
          selected_variant_id TEXT,
          is_pinned INTEGER NOT NULL DEFAULT 0,
          attachments TEXT NOT NULL DEFAULT '[]',
          used_lorebook_entries TEXT NOT NULL DEFAULT '[]',
          reasoning TEXT,
          selection_reasoning TEXT,
          model_id TEXT,
          FOREIGN KEY(session_id) REFERENCES group_sessions(id) ON DELETE CASCADE
        );

        -- Group message variants (for regeneration)
        CREATE TABLE IF NOT EXISTS group_message_variants (
          id TEXT PRIMARY KEY,
          message_id TEXT NOT NULL,
          content TEXT NOT NULL,
          speaker_character_id TEXT,
          created_at INTEGER NOT NULL,
          prompt_tokens INTEGER,
          completion_tokens INTEGER,
          total_tokens INTEGER,
          reasoning TEXT,
          selection_reasoning TEXT,
          model_id TEXT,
          FOREIGN KEY(message_id) REFERENCES group_messages(id) ON DELETE CASCADE
        );

        -- Indexes
        CREATE INDEX IF NOT EXISTS idx_sessions_character ON sessions(character_id);
        CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
        CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at);
        CREATE INDEX IF NOT EXISTS idx_creation_helper_sessions_goal_updated
          ON creation_helper_sessions(creation_goal, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_scenes_character ON scenes(character_id);
        CREATE INDEX IF NOT EXISTS idx_scene_variants_scene ON scene_variants(scene_id);
        CREATE INDEX IF NOT EXISTS idx_chat_templates_character ON chat_templates(character_id);
        CREATE INDEX IF NOT EXISTS idx_ctm_template ON chat_template_messages(template_id);
        CREATE INDEX IF NOT EXISTS idx_personas_default ON personas(is_default);
        CREATE INDEX IF NOT EXISTS idx_usage_time ON usage_records(timestamp);
        CREATE INDEX IF NOT EXISTS idx_usage_provider ON usage_records(provider_id);
        CREATE INDEX IF NOT EXISTS idx_usage_model ON usage_records(model_id);
        CREATE INDEX IF NOT EXISTS idx_usage_character ON usage_records(character_id);
        CREATE INDEX IF NOT EXISTS idx_secrets_service ON secrets(service);
        CREATE INDEX IF NOT EXISTS idx_model_pricing_cached_at ON model_pricing_cache(cached_at);
        CREATE INDEX IF NOT EXISTS idx_openrouter_provider_pricing_cached_at
          ON openrouter_provider_pricing_cache(cached_at);
        CREATE INDEX IF NOT EXISTS idx_deferred_pricing_refreshes_due
          ON deferred_pricing_refreshes(provider_id, retry_after);
        CREATE INDEX IF NOT EXISTS idx_group_sessions_updated ON group_sessions(updated_at);
        CREATE INDEX IF NOT EXISTS idx_group_participation_session ON group_participation(session_id);
        CREATE INDEX IF NOT EXISTS idx_group_messages_session ON group_messages(session_id);
        CREATE INDEX IF NOT EXISTS idx_group_messages_turn ON group_messages(session_id, turn_number);
        CREATE INDEX IF NOT EXISTS idx_group_messages_speaker ON group_messages(speaker_character_id);
        CREATE INDEX IF NOT EXISTS idx_group_message_variants_message ON group_message_variants(message_id);

        -- Sync state
        CREATE TABLE IF NOT EXISTS sync_local_state (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_changes (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          domain TEXT NOT NULL,
          entity_type TEXT NOT NULL,
          entity_id TEXT NOT NULL,
          source_device_id TEXT NOT NULL DEFAULT '',
          source_created_at INTEGER NOT NULL DEFAULT 0,
          source_change_id INTEGER NOT NULL DEFAULT 0,
          op TEXT NOT NULL,
          payload_schema INTEGER NOT NULL DEFAULT 1,
          payload_hash TEXT NOT NULL,
          payload BLOB NOT NULL DEFAULT X'',
          created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_entity_heads (
          domain TEXT NOT NULL,
          entity_type TEXT NOT NULL,
          entity_id TEXT NOT NULL,
          payload_hash TEXT NOT NULL,
          payload_schema INTEGER NOT NULL DEFAULT 1,
          payload BLOB NOT NULL DEFAULT X'',
          deleted INTEGER NOT NULL DEFAULT 0,
          last_change_id INTEGER NOT NULL,
          source_device_id TEXT NOT NULL DEFAULT '',
          source_created_at INTEGER NOT NULL DEFAULT 0,
          source_change_id INTEGER NOT NULL DEFAULT 0,
          PRIMARY KEY (domain, entity_type, entity_id)
        );

        CREATE TABLE IF NOT EXISTS sync_peer_cursors (
          peer_device_id TEXT NOT NULL,
          domain TEXT NOT NULL,
          last_change_id INTEGER NOT NULL DEFAULT 0,
          PRIMARY KEY (peer_device_id, domain)
        );

        CREATE INDEX IF NOT EXISTS idx_sync_changes_domain_id ON sync_changes(domain, id);
        CREATE INDEX IF NOT EXISTS idx_sync_changes_entity ON sync_changes(domain, entity_type, entity_id, id);
      "#,
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Backward-compatible group chat schema bootstrap for existing databases:
    // older DBs have group_sessions but not group_character_id yet.
    let mut stmt_group_sessions = conn
        .prepare("PRAGMA table_info(group_sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut group_session_cols = std::collections::HashSet::new();
    let mut rows_group_sessions = stmt_group_sessions
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_group_sessions
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        group_session_cols.insert(col_name);
    }

    if !group_session_cols.contains("group_character_id") {
        conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN group_character_id TEXT",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    if !group_session_cols.contains("memory_status") {
        conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN memory_status TEXT",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    if !group_session_cols.contains("memory_error") {
        conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN memory_error TEXT",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    if !group_session_cols.contains("memory_progress_step") {
        conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN memory_progress_step INTEGER",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_group_sessions_group_character ON group_sessions(group_character_id)",
        [],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_group_characters_updated ON group_characters(updated_at)",
        [],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut stmt_sync_heads = conn
        .prepare("PRAGMA table_info(sync_entity_heads)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut sync_head_cols = std::collections::HashSet::new();
    let mut rows_sync_heads = stmt_sync_heads
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_sync_heads
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        sync_head_cols.insert(col_name);
    }

    let mut reset_sync_state = false;

    if !sync_head_cols.contains("payload_schema") {
        conn.execute(
            "ALTER TABLE sync_entity_heads ADD COLUMN payload_schema INTEGER NOT NULL DEFAULT 1",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    if !sync_head_cols.contains("payload") {
        conn.execute(
            "ALTER TABLE sync_entity_heads ADD COLUMN payload BLOB NOT NULL DEFAULT X''",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    if !sync_head_cols.contains("source_device_id") {
        conn.execute(
            "ALTER TABLE sync_entity_heads ADD COLUMN source_device_id TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        reset_sync_state = true;
    }
    if !sync_head_cols.contains("source_created_at") {
        conn.execute(
            "ALTER TABLE sync_entity_heads ADD COLUMN source_created_at INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        reset_sync_state = true;
    }
    if !sync_head_cols.contains("source_change_id") {
        conn.execute(
            "ALTER TABLE sync_entity_heads ADD COLUMN source_change_id INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        reset_sync_state = true;
    }

    let mut stmt_sync_changes = conn
        .prepare("PRAGMA table_info(sync_changes)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut sync_change_cols = std::collections::HashSet::new();
    let mut rows_sync_changes = stmt_sync_changes
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_sync_changes
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        sync_change_cols.insert(col_name);
    }

    if !sync_change_cols.contains("source_device_id") {
        conn.execute(
            "ALTER TABLE sync_changes ADD COLUMN source_device_id TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        reset_sync_state = true;
    }
    if !sync_change_cols.contains("source_created_at") {
        conn.execute(
            "ALTER TABLE sync_changes ADD COLUMN source_created_at INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        reset_sync_state = true;
    }
    if !sync_change_cols.contains("source_change_id") {
        conn.execute(
            "ALTER TABLE sync_changes ADD COLUMN source_change_id INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        reset_sync_state = true;
    }

    let sync_state_schema_version = conn
        .query_row(
            "SELECT value FROM sync_local_state WHERE key = 'sync_state_schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|value| value.parse::<u16>().ok());
    if sync_state_schema_version != Some(LOCAL_SYNC_STATE_VERSION) {
        log_warn(
            _app,
            "db",
            format!(
                "Resetting sync state because local sync_state_schema_version is {:?} instead of {}",
                sync_state_schema_version, LOCAL_SYNC_STATE_VERSION
            ),
        );
        reset_sync_state = true;
    }

    if reset_sync_state {
        conn.execute("DELETE FROM sync_changes", [])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute("DELETE FROM sync_entity_heads", [])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        conn.execute("DELETE FROM sync_peer_cursors", [])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    conn.execute(
        "INSERT OR REPLACE INTO sync_local_state (key, value) VALUES ('sync_state_schema_version', ?1)",
        params![LOCAL_SYNC_STATE_VERSION.to_string()],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Migrations: add reasoning_tokens and image_tokens to usage_records if missing
    let mut stmt = conn
        .prepare("PRAGMA table_info(usage_records)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut cols = std::collections::HashSet::new();
    let mut rows = stmt
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        cols.insert(col_name);
    }

    if !cols.contains("reasoning_tokens") {
        conn.execute(
            "ALTER TABLE usage_records ADD COLUMN reasoning_tokens INTEGER",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    if !cols.contains("image_tokens") {
        conn.execute(
            "ALTER TABLE usage_records ADD COLUMN image_tokens INTEGER",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Migrations: add memory_refs to messages if missing
    let mut stmt = conn
        .prepare("PRAGMA table_info(messages)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_memory_refs = false;
    let mut rows = stmt
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if col_name == "memory_refs" {
            has_memory_refs = true;
            break;
        }
    }
    if !has_memory_refs {
        conn.execute(
            "ALTER TABLE messages ADD COLUMN memory_refs TEXT NOT NULL DEFAULT '[]'",
            [],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    let mut has_reasoning = false;
    let mut stmt_reasoning = conn
        .prepare("PRAGMA table_info(messages)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut rows_reasoning = stmt_reasoning
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_reasoning
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if col_name == "reasoning" {
            has_reasoning = true;
            break;
        }
    }
    if !has_reasoning {
        let _ = conn.execute("ALTER TABLE messages ADD COLUMN reasoning TEXT", []);
    }

    let mut has_scene_edited = false;
    let mut stmt_scene_edited = conn
        .prepare("PRAGMA table_info(messages)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut rows_scene_edited = stmt_scene_edited
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_scene_edited
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if col_name == "scene_edited" {
            has_scene_edited = true;
            break;
        }
    }
    if !has_scene_edited {
        let _ = conn.execute(
            "ALTER TABLE messages ADD COLUMN scene_edited INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    let mut has_variant_reasoning = false;
    let mut stmt_variant_reasoning = conn
        .prepare("PRAGMA table_info(message_variants)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut rows_variant_reasoning = stmt_variant_reasoning
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_variant_reasoning
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if col_name == "reasoning" {
            has_variant_reasoning = true;
            break;
        }
    }
    if !has_variant_reasoning {
        let _ = conn.execute("ALTER TABLE message_variants ADD COLUMN reasoning TEXT", []);
    }

    let mut stmt_sessions = conn
        .prepare("PRAGMA table_info(sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_session_voice_autoplay = false;
    let mut has_session_persona_disabled = false;
    let mut rows_sessions = stmt_sessions
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_sessions
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match col_name.as_str() {
            "voice_autoplay" => has_session_voice_autoplay = true,
            "persona_disabled" => has_session_persona_disabled = true,
            _ => {}
        }
    }
    if !has_session_voice_autoplay {
        let _ = conn.execute("ALTER TABLE sessions ADD COLUMN voice_autoplay INTEGER", []);
    }
    if !has_session_persona_disabled {
        let _ = conn.execute(
            "ALTER TABLE sessions ADD COLUMN persona_disabled INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    let mut stmt_sessions_mem = conn
        .prepare("PRAGMA table_info(sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_memory_status = false;
    let mut has_memory_error = false;
    let mut rows_sessions_mem = stmt_sessions_mem
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_sessions_mem
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if col_name == "memory_status" {
            has_memory_status = true;
        } else if col_name == "memory_error" {
            has_memory_error = true;
        }
    }
    if !has_memory_status {
        let _ = conn.execute("ALTER TABLE sessions ADD COLUMN memory_status TEXT", []);
    }
    if !has_memory_error {
        let _ = conn.execute("ALTER TABLE sessions ADD COLUMN memory_error TEXT", []);
    }
    // memory_progress_step migration for sessions
    {
        let has_col = {
            let mut stmt = conn
                .prepare("PRAGMA table_info(sessions)")
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let mut rows = stmt
                .query([])
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let mut found = false;
            while let Some(row) = rows
                .next()
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            {
                let name: String = row
                    .get(1)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                if name == "memory_progress_step" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_col {
            let _ = conn.execute(
                "ALTER TABLE sessions ADD COLUMN memory_progress_step INTEGER",
                [],
            );
        }
    }

    let mut stmt_audio_providers = conn
        .prepare("PRAGMA table_info(audio_providers)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_audio_base_url = false;
    let mut has_audio_request_path = false;
    let mut rows_audio_providers = stmt_audio_providers
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_audio_providers
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match col_name.as_str() {
            "base_url" => has_audio_base_url = true,
            "request_path" => has_audio_request_path = true,
            _ => {}
        }
    }
    if !has_audio_base_url {
        let _ = conn.execute("ALTER TABLE audio_providers ADD COLUMN base_url TEXT", []);
    }
    if !has_audio_request_path {
        let _ = conn.execute(
            "ALTER TABLE audio_providers ADD COLUMN request_path TEXT",
            [],
        );
    }

    let mut stmt2 = conn
        .prepare("PRAGMA table_info(characters)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_custom_gradient_enabled = false;
    let mut has_custom_gradient_colors = false;
    let mut has_custom_text_color = false;
    let mut has_custom_text_secondary = false;
    let mut has_voice_config = false;
    let mut has_voice_autoplay = false;
    let mut has_fallback_model_id = false;
    let mut has_avatar_crop_x = false;
    let mut has_avatar_crop_y = false;
    let mut has_avatar_crop_scale = false;
    let mut rows2 = stmt2
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows2
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match col_name.as_str() {
            "custom_gradient_enabled" => has_custom_gradient_enabled = true,
            "custom_gradient_colors" => has_custom_gradient_colors = true,
            "custom_text_color" => has_custom_text_color = true,
            "custom_text_secondary" => has_custom_text_secondary = true,
            "voice_config" => has_voice_config = true,
            "voice_autoplay" => has_voice_autoplay = true,
            "fallback_model_id" => has_fallback_model_id = true,
            "avatar_crop_x" => has_avatar_crop_x = true,
            "avatar_crop_y" => has_avatar_crop_y = true,
            "avatar_crop_scale" => has_avatar_crop_scale = true,
            _ => {}
        }
    }
    if !has_custom_gradient_enabled {
        let _ = conn.execute(
            "ALTER TABLE characters ADD COLUMN custom_gradient_enabled INTEGER DEFAULT 0",
            [],
        );
    }
    if !has_custom_gradient_colors {
        let _ = conn.execute(
            "ALTER TABLE characters ADD COLUMN custom_gradient_colors TEXT",
            [],
        );
    }
    if !has_custom_text_color {
        let _ = conn.execute(
            "ALTER TABLE characters ADD COLUMN custom_text_color TEXT",
            [],
        );
    }
    if !has_custom_text_secondary {
        let _ = conn.execute(
            "ALTER TABLE characters ADD COLUMN custom_text_secondary TEXT",
            [],
        );
    }
    if !has_voice_config {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN voice_config TEXT", []);
    }
    if !has_voice_autoplay {
        let _ = conn.execute(
            "ALTER TABLE characters ADD COLUMN voice_autoplay INTEGER DEFAULT 0",
            [],
        );
    }
    if !has_fallback_model_id {
        let _ = conn.execute(
            "ALTER TABLE characters ADD COLUMN fallback_model_id TEXT",
            [],
        );
    }
    if !has_avatar_crop_x {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN avatar_crop_x REAL", []);
    }
    if !has_avatar_crop_y {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN avatar_crop_y REAL", []);
    }
    if !has_avatar_crop_scale {
        let _ = conn.execute(
            "ALTER TABLE characters ADD COLUMN avatar_crop_scale REAL",
            [],
        );
    }

    let mut stmt_personas = conn
        .prepare("PRAGMA table_info(personas)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_persona_avatar_crop_x = false;
    let mut has_persona_avatar_crop_y = false;
    let mut has_persona_avatar_crop_scale = false;
    let mut has_persona_nickname = false;
    let mut rows_personas = stmt_personas
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_personas
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match col_name.as_str() {
            "avatar_crop_x" => has_persona_avatar_crop_x = true,
            "avatar_crop_y" => has_persona_avatar_crop_y = true,
            "avatar_crop_scale" => has_persona_avatar_crop_scale = true,
            "nickname" => has_persona_nickname = true,
            _ => {}
        }
    }
    if !has_persona_avatar_crop_x {
        let _ = conn.execute("ALTER TABLE personas ADD COLUMN avatar_crop_x REAL", []);
    }
    if !has_persona_avatar_crop_y {
        let _ = conn.execute("ALTER TABLE personas ADD COLUMN avatar_crop_y REAL", []);
    }
    if !has_persona_avatar_crop_scale {
        let _ = conn.execute("ALTER TABLE personas ADD COLUMN avatar_crop_scale REAL", []);
    }
    if !has_persona_nickname {
        let _ = conn.execute("ALTER TABLE personas ADD COLUMN nickname TEXT", []);
    }

    // Migrations: add title to lorebook_entries if missing
    let mut stmt3 = conn
        .prepare("PRAGMA table_info(lorebook_entries)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_lorebook_entry_title = false;
    let mut rows3 = stmt3
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows3
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if col_name == "title" {
            has_lorebook_entry_title = true;
            break;
        }
    }
    if !has_lorebook_entry_title {
        let _ = conn.execute(
            "ALTER TABLE lorebook_entries ADD COLUMN title TEXT NOT NULL DEFAULT ''",
            [],
        );
    }

    let mut stmt_lorebooks = conn
        .prepare("PRAGMA table_info(lorebooks)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_lorebook_avatar_path = false;
    let mut has_lorebook_keyword_detection_mode = false;
    let mut rows_lorebooks = stmt_lorebooks
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_lorebooks
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match col_name.as_str() {
            "avatar_path" => has_lorebook_avatar_path = true,
            "keyword_detection_mode" => has_lorebook_keyword_detection_mode = true,
            _ => {}
        }
    }
    if !has_lorebook_avatar_path {
        let _ = conn.execute("ALTER TABLE lorebooks ADD COLUMN avatar_path TEXT", []);
    }
    if !has_lorebook_keyword_detection_mode {
        let _ = conn.execute(
            "ALTER TABLE lorebooks ADD COLUMN keyword_detection_mode TEXT NOT NULL DEFAULT 'recent_message_window'",
            [],
        );
    }

    let mut stmt_prompt_templates = conn
        .prepare("PRAGMA table_info(prompt_templates)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_prompt_entries = false;
    let mut has_condense_prompt_entries = false;
    let mut has_prompt_type = false;
    let mut rows_prompt_templates = stmt_prompt_templates
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_prompt_templates
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if col_name == "entries" {
            has_prompt_entries = true;
        }
        if col_name == "condense_prompt_entries" {
            has_condense_prompt_entries = true;
        }
        if col_name == "prompt_type" {
            has_prompt_type = true;
        }
    }
    if !has_prompt_entries {
        let _ = conn.execute(
            "ALTER TABLE prompt_templates ADD COLUMN entries TEXT NOT NULL DEFAULT '[]'",
            [],
        );
    }
    if !has_condense_prompt_entries {
        let _ = conn.execute(
            "ALTER TABLE prompt_templates ADD COLUMN condense_prompt_entries INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }
    if !has_prompt_type {
        let _ = conn.execute(
            "ALTER TABLE prompt_templates ADD COLUMN prompt_type TEXT NOT NULL DEFAULT 'undefined'",
            [],
        );
    }
    let _ = conn.execute("DROP INDEX IF EXISTS idx_prompt_templates_scope", []);
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_prompt_templates_prompt_type ON prompt_templates(prompt_type)",
        [],
    );

    let mut stmt_messages = conn
        .prepare("PRAGMA table_info(messages)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_used_lorebook_entries = false;
    let mut rows_messages = stmt_messages
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_messages
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if col_name == "used_lorebook_entries" {
            has_used_lorebook_entries = true;
            break;
        }
    }
    if !has_used_lorebook_entries {
        let _ = conn.execute(
            "ALTER TABLE messages ADD COLUMN used_lorebook_entries TEXT NOT NULL DEFAULT '[]'",
            [],
        );
    }

    let mut stmt_group_messages = conn
        .prepare("PRAGMA table_info(group_messages)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_group_used_lorebook_entries = false;
    let mut rows_group_messages = stmt_group_messages
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_group_messages
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if col_name == "used_lorebook_entries" {
            has_group_used_lorebook_entries = true;
            break;
        }
    }
    if !has_group_used_lorebook_entries {
        let _ = conn.execute(
            "ALTER TABLE group_messages ADD COLUMN used_lorebook_entries TEXT NOT NULL DEFAULT '[]'",
            [],
        );
    }

    let mut stmt_group_characters = conn
        .prepare("PRAGMA table_info(group_characters)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_group_character_lorebook_ids = false;
    let mut has_group_character_disable_lorebooks = false;
    let mut rows_group_characters = stmt_group_characters
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_group_characters
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if col_name == "lorebook_ids" {
            has_group_character_lorebook_ids = true;
        } else if col_name == "disable_character_lorebooks" {
            has_group_character_disable_lorebooks = true;
        }
    }
    if !has_group_character_lorebook_ids {
        let _ = conn.execute(
            "ALTER TABLE group_characters ADD COLUMN lorebook_ids TEXT NOT NULL DEFAULT '[]'",
            [],
        );
    }
    if !has_group_character_disable_lorebooks {
        let _ = conn.execute(
            "ALTER TABLE group_characters ADD COLUMN disable_character_lorebooks INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    let mut stmt_group_sessions = conn
        .prepare("PRAGMA table_info(group_sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_group_session_lorebook_ids = false;
    let mut has_group_session_disable_lorebooks = false;
    let mut rows_group_sessions = stmt_group_sessions
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    while let Some(row) = rows_group_sessions
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let col_name: String = row
            .get(1)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if col_name == "lorebook_ids" {
            has_group_session_lorebook_ids = true;
        } else if col_name == "disable_character_lorebooks" {
            has_group_session_disable_lorebooks = true;
        }
    }
    if !has_group_session_lorebook_ids {
        let _ = conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN lorebook_ids TEXT NOT NULL DEFAULT '[]'",
            [],
        );
    }
    if !has_group_session_disable_lorebooks {
        let _ = conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN disable_character_lorebooks INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    let default_content = crate::chat_manager::prompt_engine::default_system_prompt_template();
    let now = now_ms();
    conn
        .execute(
            "INSERT OR IGNORE INTO prompt_templates (id, name, prompt_type, content, entries, condense_prompt_entries, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, '[]', 0, ?5, ?5)",
            params![
                "prompt_app_default",
                "App Default",
                "directChat",
                default_content,
                now
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(())
}

pub fn now_ms() -> u64 {
    now_millis().unwrap_or(0)
}

fn apply_pragmas(conn: &Connection) {
    let _ = conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA temp_store=MEMORY;
        PRAGMA cache_size=-8000; -- ~8MB
        PRAGMA wal_autocheckpoint=1000;
        PRAGMA mmap_size=268435456; -- 256MB if supported
        PRAGMA optimize;
        "#,
    );
}

#[tauri::command]
pub fn db_optimize(app: tauri::AppHandle) -> Result<(), String> {
    let conn = open_db(&app)?;
    apply_pragmas(&conn);
    // Vacuum only on mobile targets
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = conn.execute_batch("VACUUM;");
    }
    Ok(())
}

/// Force a WAL checkpoint to ensure all pending writes are persisted.
/// This should be called when the app is about to be backgrounded or closed.
#[tauri::command]
pub fn db_checkpoint(app: tauri::AppHandle) -> Result<(), String> {
    let conn = open_db(&app)?;
    // PRAGMA wal_checkpoint(TRUNCATE) forces a full checkpoint and truncates the WAL file
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("WAL checkpoint failed: {}", e),
            )
        })?;
    Ok(())
}
