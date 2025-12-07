use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;
use anyhow::{Result, anyhow};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ApiProvider {
    Codestral, // codestral.mistral.ai
    MistralAi, // api.mistral.ai
}

impl Default for ApiProvider {
    fn default() -> Self {
        ApiProvider::MistralAi
    }
}

pub struct MistralClient {
    client: Client,
    api_key: String,
    provider: ApiProvider,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: Message,
}

impl MistralClient {
    pub fn new(api_key: String, provider: ApiProvider) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_default();

        Self {
            client,
            api_key,
            provider,
        }
    }

    fn get_base_url(&self) -> &str {
        match self.provider {
            ApiProvider::Codestral => "https://codestral.mistral.ai/v1/chat/completions",
            ApiProvider::MistralAi => "https://api.mistral.ai/v1/chat/completions",
        }
    }

    // Default models for each provider
    fn get_model(&self) -> &str {
        match self.provider {
            ApiProvider::Codestral => "codestral-latest", 
            ApiProvider::MistralAi => "mistral-large-latest",
        }
    }

    pub async fn chat(&self, messages: Vec<Message>) -> Result<String> {
        let url = self.get_base_url();
        let model = self.get_model();

        let request_body = ChatRequest {
            model: model.to_string(),
            messages,
            stream: false, // Streaming can be added later
        };

        let response = self.client.post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
             let error_text = response.text().await?;
             return Err(anyhow!("API Error: {}", error_text));
        }

        let chat_response: ChatResponse = response.json().await?;

        if let Some(choice) = chat_response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow!("No response content found"))
        }
    }
}
