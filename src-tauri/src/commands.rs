use crate::mistral_client::{MistralClient, ApiProvider, Message};
use tauri::{State, AppHandle};
use tauri_plugin_store::StoreExt;
use serde_json::json;
use std::sync::Mutex;
use uuid::Uuid;
use std::collections::HashMap;

// Using a simple in-memory cache for now for active conversations state, 
// relying on store plugin for persistence.

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct AppSettings {
    pub api_key: String,
    pub provider: ApiProvider,
}

#[derive(Default)]
pub struct AppState {
    // In a real app we might cache loaded conversations here
    // For now we will read/write from disk/store directly to ensure persistence
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    conversation_id: String,
    content: String,
    api_key: String,
    provider: ApiProvider
) -> Result<String, String> {
    let client = MistralClient::new(api_key, provider);
    
    // Load conversation history using the store
    let store = app.store("conversations.json").map_err(|e| e.to_string())?;
    
    let mut messages = Vec::new();
    let mut current_conversation: Option<Conversation> = None;

    if let Some(val) = store.get(&conversation_id) {
         if let Ok(conv) = serde_json::from_value::<Conversation>(val) {
             messages = conv.messages.clone();
             current_conversation = Some(conv);
         }
    }

    if current_conversation.is_none() {
        return Err("Conversation not found".to_string());
    }

    // Add user message
    messages.push(Message { role: "user".to_string(), content: content.clone() });

    // Call API
    let response_content = client.chat(messages.clone())
        .await
        .map_err(|e| e.to_string())?;

    // Add assistant message
    messages.push(Message { role: "assistant".to_string(), content: response_content.clone() });

    // Update conversation
    if let Some(mut conv) = current_conversation {
        // Auto-name conversation based on first user message if still default title
        if conv.title == "New Conversation" && !content.is_empty() {
            // Take first 50 chars of the user message as the title
            let auto_title: String = content.chars().take(50).collect();
            conv.title = if auto_title.len() < content.len() {
                format!("{}...", auto_title.trim())
            } else {
                auto_title.trim().to_string()
            };
        }
        
        conv.messages = messages;
        store.set(conversation_id, json!(conv));
        store.save().map_err(|e| e.to_string())?;
    }

    Ok(response_content)
}

#[tauri::command]
pub async fn create_conversation(app: AppHandle, title: Option<String>) -> Result<Conversation, String> {
    let store = app.store("conversations.json").map_err(|e| e.to_string())?;
    let id = Uuid::new_v4().to_string();
    
    let conversation = Conversation {
        id: id.clone(),
        title: title.unwrap_or_else(|| "New Conversation".to_string()),
        messages: Vec::new(),
         created_at: chrono::Utc::now().timestamp(),
    };

    store.set(id, json!(conversation));
    store.save().map_err(|e| e.to_string())?;

    Ok(conversation)
}

#[tauri::command]
pub async fn get_conversations(app: AppHandle) -> Result<Vec<Conversation>, String> {
    let store = app.store("conversations.json").map_err(|e| e.to_string())?;
    let mut conversations = Vec::new();

    // Iterate over all keys in the store
    // Note: The store API might need to be used carefully. 
    // If the store is large, this is inefficient, but for a local chat app it's fine.
    // simpler: The store entries method gives us key-values.
    
    for (key, value) in store.entries() {
         if let Ok(conv) = serde_json::from_value::<Conversation>(value.clone()) {
             conversations.push(conv);
         }
    }
    
    // Sort by created_at desc
    conversations.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(conversations)
}

#[tauri::command]
pub async fn delete_conversation(app: AppHandle, conversation_id: String) -> Result<(), String> {
    let store = app.store("conversations.json").map_err(|e| e.to_string())?;
    store.delete(&conversation_id);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn rename_conversation(app: AppHandle, conversation_id: String, new_title: String) -> Result<(), String> {
    let store = app.store("conversations.json").map_err(|e| e.to_string())?;
    if let Some(val) = store.get(&conversation_id) {
         if let Ok(mut conv) = serde_json::from_value::<Conversation>(val) {
             conv.title = new_title;
             store.set(conversation_id, json!(conv));
             store.save().map_err(|e| e.to_string())?;
         }
    }
    Ok(())
}

#[tauri::command]
pub async fn clear_history(app: AppHandle) -> Result<(), String> {
    let store = app.store("conversations.json").map_err(|e| e.to_string())?;
    store.clear();
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_app_settings(app: AppHandle) -> Result<AppSettings, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    
    // Default settings
    let mut settings = AppSettings::default();
    
    if let Some(val) = store.get("config") {
        if let Ok(s) = serde_json::from_value::<AppSettings>(val) {
            settings = s;
        }
    }
    
    Ok(settings)
}

#[tauri::command]
pub async fn update_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("config", json!(settings));
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn test_api_connection(api_key: String, provider: ApiProvider) -> Result<String, String> {
    let client = MistralClient::new(api_key, provider);
    // Simple test message
    let messages = vec![Message { role: "user".to_string(), content: "Hello".to_string() }];
    
    match client.chat(messages).await {
        Ok(_) => Ok("Connection successful".to_string()),
        Err(e) => Err(format!("Connection failed: {}", e)),
    }
}
