use tauri::AppHandle;

use super::storage::{load_characters, load_personas, load_session, load_settings, save_session};
use crate::chat_manager::types::{Character, Persona, Session, Settings};

#[derive(Clone)]
pub struct ChatRepository {
    app: AppHandle,
}

impl ChatRepository {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub fn app(&self) -> &AppHandle {
        &self.app
    }

    pub fn load_settings(&self) -> Result<Settings, String> {
        load_settings(&self.app)
    }

    pub fn load_characters(&self) -> Result<Vec<Character>, String> {
        load_characters(&self.app)
    }

    pub fn load_personas(&self) -> Result<Vec<Persona>, String> {
        load_personas(&self.app)
    }

    pub fn load_session(&self, session_id: &str) -> Result<Option<Session>, String> {
        load_session(&self.app, session_id)
    }

    pub fn save_session(&self, session: &Session) -> Result<(), String> {
        save_session(&self.app, session)
    }
}
