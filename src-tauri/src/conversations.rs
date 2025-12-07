use crate::models::Conversation;
use anyhow::{Context, Result};
use serde_json;
use std::fs;
use std::path::PathBuf;

pub fn get_conversations_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .context("Unable to find data directory")?
        .join("companion-chat")
        .join("conversations");

    fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

pub fn get_settings_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Unable to find config directory")?
        .join("companion-chat");

    fs::create_dir_all(&config_dir)?;
    Ok(config_dir)
}

pub fn save_conversation(conversation: &Conversation) -> Result<()> {
    let conversations_dir = get_conversations_dir()?;
    let file_path = conversations_dir.join(format!("{}.json", conversation.id));

    let json = serde_json::to_string_pretty(conversation)?;
    fs::write(&file_path, json)?;

    Ok(())
}

pub fn load_conversation(conversation_id: &str) -> Result<Option<Conversation>> {
    let conversations_dir = get_conversations_dir()?;
    let file_path = conversations_dir.join(format!("{}.json", conversation_id));

    if !file_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&file_path)?;
    let conversation: Conversation = serde_json::from_str(&content)?;
    Ok(Some(conversation))
}

pub fn load_all_conversations() -> Result<Vec<Conversation>> {
    let conversations_dir = get_conversations_dir()?;
    let mut conversations = Vec::new();

    if !conversations_dir.exists() {
        return Ok(conversations);
    }

    for entry in fs::read_dir(&conversations_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(conversation) = serde_json::from_str::<Conversation>(&content) {
                    conversations.push(conversation);
                }
            }
        }
    }

    conversations.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(conversations)
}

pub fn delete_conversation(conversation_id: &str) -> Result<()> {
    let conversations_dir = get_conversations_dir()?;
    let file_path = conversations_dir.join(format!("{}.json", conversation_id));

    if file_path.exists() {
        fs::remove_file(&file_path)?;
    }

    Ok(())
}

pub fn clear_all_conversations() -> Result<()> {
    let conversations_dir = get_conversations_dir()?;

    if conversations_dir.exists() {
        for entry in fs::read_dir(&conversations_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let _ = fs::remove_file(&path);
            }
        }
    }

    Ok(())
}

