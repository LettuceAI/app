use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use super::legacy::storage_root;
use crate::migrations;
use crate::utils::{log_info, log_warn, now_millis};

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
        pool.get().map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to get connection from pool: {}", e),
            )
        })
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

/// Create a new pool for a given database path
pub fn create_pool_for_path(path: &PathBuf) -> Result<DbPool, String> {
    let manager = SqliteConnectionManager::file(path).with_init(|c| {
        c.execute_batch(
            r#"
                PRAGMA journal_mode=WAL;
                PRAGMA synchronous=NORMAL;
                PRAGMA temp_store=MEMORY;
                PRAGMA cache_size=-8000;
                PRAGMA wal_autocheckpoint=1000;
                PRAGMA mmap_size=268435456;
                PRAGMA foreign_keys=ON;
                "#,
        )
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
        c.execute_batch(
            r#"
                PRAGMA journal_mode=WAL;
                PRAGMA synchronous=NORMAL;
                PRAGMA temp_store=MEMORY;
                PRAGMA cache_size=-8000;
                PRAGMA wal_autocheckpoint=1000;
                PRAGMA mmap_size=268435456;
                PRAGMA foreign_keys=ON;
                "#,
        )
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
          scope TEXT NOT NULL,
          target_ids TEXT NOT NULL, -- JSON array of strings
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
          system_prompt TEXT,
          voice_config TEXT,
          voice_autoplay INTEGER NOT NULL DEFAULT 0,
          disable_avatar_gradient INTEGER NOT NULL DEFAULT 0,
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

        -- Personas
        CREATE TABLE IF NOT EXISTS personas (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          description TEXT NOT NULL,
          avatar_path TEXT,
          avatar_crop_x REAL,
          avatar_crop_y REAL,
          avatar_crop_scale REAL,
          is_default INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        -- Sessions and messages
        CREATE TABLE IF NOT EXISTS sessions (
          id TEXT PRIMARY KEY,
          character_id TEXT NOT NULL,
          title TEXT NOT NULL,
          system_prompt TEXT,
          selected_scene_id TEXT,
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

        -- Audio providers for TTS
        CREATE TABLE IF NOT EXISTS audio_providers (
          id TEXT PRIMARY KEY,
          provider_type TEXT NOT NULL,
          label TEXT NOT NULL,
          api_key TEXT,
          project_id TEXT,
          location TEXT DEFAULT 'us-central1',
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

        -- Group chat sessions (multi-character conversations)
        CREATE TABLE IF NOT EXISTS group_sessions (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          character_ids TEXT NOT NULL DEFAULT '[]',
          persona_id TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          archived INTEGER NOT NULL DEFAULT 0,
          chat_type TEXT NOT NULL DEFAULT 'conversation',
          starting_scene TEXT,
          background_image_path TEXT,
          memories TEXT NOT NULL DEFAULT '[]',
          memory_embeddings TEXT NOT NULL DEFAULT '[]',
          memory_summary TEXT NOT NULL DEFAULT '',
          memory_summary_token_count INTEGER NOT NULL DEFAULT 0,
          memory_tool_events TEXT NOT NULL DEFAULT '[]',
          speaker_selection_method TEXT NOT NULL DEFAULT 'llm',
          FOREIGN KEY(persona_id) REFERENCES personas(id) ON DELETE SET NULL
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
        CREATE INDEX IF NOT EXISTS idx_personas_default ON personas(is_default);
        CREATE INDEX IF NOT EXISTS idx_usage_time ON usage_records(timestamp);
        CREATE INDEX IF NOT EXISTS idx_usage_provider ON usage_records(provider_id);
        CREATE INDEX IF NOT EXISTS idx_usage_model ON usage_records(model_id);
        CREATE INDEX IF NOT EXISTS idx_usage_character ON usage_records(character_id);
        CREATE INDEX IF NOT EXISTS idx_secrets_service ON secrets(service);
        CREATE INDEX IF NOT EXISTS idx_prompt_templates_scope ON prompt_templates(scope);
        CREATE INDEX IF NOT EXISTS idx_model_pricing_cached_at ON model_pricing_cache(cached_at);
        CREATE INDEX IF NOT EXISTS idx_group_sessions_updated ON group_sessions(updated_at);
        CREATE INDEX IF NOT EXISTS idx_group_participation_session ON group_participation(session_id);
        CREATE INDEX IF NOT EXISTS idx_group_messages_session ON group_messages(session_id);
        CREATE INDEX IF NOT EXISTS idx_group_messages_turn ON group_messages(session_id, turn_number);
        CREATE INDEX IF NOT EXISTS idx_group_messages_speaker ON group_messages(speaker_character_id);
        CREATE INDEX IF NOT EXISTS idx_group_message_variants_message ON group_message_variants(message_id);
      "#,
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

    let mut stmt_prompt_templates = conn
        .prepare("PRAGMA table_info(prompt_templates)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut has_prompt_entries = false;
    let mut has_condense_prompt_entries = false;
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

    let default_content = crate::chat_manager::prompt_engine::default_system_prompt_template();
    let now = now_ms();
    conn
        .execute(
            "INSERT OR IGNORE INTO prompt_templates (id, name, scope, target_ids, content, entries, condense_prompt_entries, created_at, updated_at)
             VALUES (?1, ?2, ?3, '[]', ?4, '[]', 0, ?5, ?5)",
            params![
                "prompt_app_default",
                "App Default",
                "AppWide",
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
