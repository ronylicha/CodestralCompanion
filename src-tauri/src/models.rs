use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiProvider {
    pub provider: ProviderType,
    pub api_key: Option<String>,
    pub phone_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProviderType {
    CodestralMistralAi,
    ApiMistralAi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub api_provider: ApiProvider,
    pub selected_conversation_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MistralRequest {
    pub model: String,
    pub messages: Vec<MistralMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MistralMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct MistralResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<MistralChoice>,
    pub usage: Option<MistralUsage>,
}

#[derive(Debug, Deserialize)]
pub struct MistralChoice {
    pub index: u32,
    pub message: MistralResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MistralResponseMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct MistralUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl Conversation {
    pub fn new(title: Option<String>) -> Self {
        let id = Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id,
            title: title.unwrap_or_else(|| "Nouvelle conversation".to_string()),
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn add_message(&mut self, role: MessageRole, content: String) {
        let message = Message {
            id: Uuid::new_v4().to_string(),
            role,
            content,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        self.messages.push(message);
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_provider: ApiProvider {
                provider: ProviderType::ApiMistralAi,
                api_key: None,
                phone_number: None,
            },
            selected_conversation_id: None,
        }
    }
}


