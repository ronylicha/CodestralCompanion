use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc, Duration};
use crate::mistral_client::Message;

/// Saved chat session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedChat {
    pub id: String,
    pub title: String,
    pub project_path: String,
    pub messages: Vec<Message>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SavedChat {
    pub fn new(project_path: &str) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        Self {
            id,
            title: "Nouvelle conversation".to_string(),
            project_path: project_path.to_string(),
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Generate title from first user message
    pub fn auto_title(&mut self) {
        if let Some(first_user_msg) = self.messages.iter().find(|m| m.role == "user") {
            let content = &first_user_msg.content;
            // Take first 40 chars or first sentence
            let title = if let Some(dot_pos) = content.find('.') {
                if dot_pos < 60 {
                    &content[..dot_pos]
                } else {
                    &content[..content.len().min(40)]
                }
            } else {
                &content[..content.len().min(40)]
            };
            self.title = title.trim().to_string();
            if self.title.len() < content.len() {
                self.title.push_str("...");
            }
        }
    }

    /// Format time elapsed since last message
    pub fn time_ago(&self) -> String {
        let now = Utc::now();
        let diff = now.signed_duration_since(self.updated_at);
        
        if diff < Duration::minutes(1) {
            "à l'instant".to_string()
        } else if diff < Duration::hours(1) {
            format!("il y a {} min", diff.num_minutes())
        } else if diff < Duration::hours(24) {
            format!("il y a {} h", diff.num_hours())
        } else if diff < Duration::days(7) {
            format!("il y a {} j", diff.num_days())
        } else {
            self.updated_at.format("%d/%m/%Y").to_string()
        }
    }
}

/// Chat storage manager
pub struct ChatStorage {
    storage_dir: PathBuf,
}

impl ChatStorage {
    pub fn new() -> Result<Self, String> {
        let config_dir = dirs::config_dir()
            .ok_or("Cannot find config directory")?
            .join("com.rony.companion-chat")
            .join("cli-chats");
        
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Cannot create chat storage dir: {}", e))?;
        
        Ok(Self { storage_dir: config_dir })
    }

    /// Save a chat session
    pub fn save(&self, chat: &SavedChat) -> Result<(), String> {
        let path = self.storage_dir.join(format!("{}.json", chat.id));
        let json = serde_json::to_string_pretty(chat)
            .map_err(|e| format!("Serialize error: {}", e))?;
        fs::write(&path, json)
            .map_err(|e| format!("Write error: {}", e))?;
        Ok(())
    }

    /// Load a chat session by ID
    pub fn load(&self, id: &str) -> Result<SavedChat, String> {
        let path = self.storage_dir.join(format!("{}.json", id));
        let json = fs::read_to_string(&path)
            .map_err(|e| format!("Read error: {}", e))?;
        serde_json::from_str(&json)
            .map_err(|e| format!("Parse error: {}", e))
    }

    /// List all saved chats, sorted by updated_at (most recent first)
    pub fn list(&self) -> Result<Vec<SavedChat>, String> {
        let mut chats = Vec::new();
        
        let entries = fs::read_dir(&self.storage_dir)
            .map_err(|e| format!("Read dir error: {}", e))?;
        
        for entry in entries.flatten() {
            if entry.path().extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(json) = fs::read_to_string(entry.path()) {
                    if let Ok(chat) = serde_json::from_str::<SavedChat>(&json) {
                        chats.push(chat);
                    }
                }
            }
        }
        
        // Sort by updated_at descending
        chats.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        
        Ok(chats)
    }

    /// List chats for a specific project
    pub fn list_for_project(&self, project_path: &str) -> Result<Vec<SavedChat>, String> {
        let all = self.list()?;
        Ok(all.into_iter().filter(|c| c.project_path == project_path).collect())
    }

    /// Delete a chat
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let path = self.storage_dir.join(format!("{}.json", id));
        fs::remove_file(&path)
            .map_err(|e| format!("Delete error: {}", e))
    }
}
