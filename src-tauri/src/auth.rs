use crate::models::{ProviderType, Settings};
use anyhow::Result;
use serde_json;
use std::fs;

pub fn load_settings() -> Result<Settings> {
    let settings_path = crate::conversations::get_settings_dir()?.join("settings.json");

    if !settings_path.exists() {
        return Ok(Settings::default());
    }

    let content = fs::read_to_string(&settings_path)?;
    let settings: Settings = serde_json::from_str(&content)?;
    Ok(settings)
}

pub fn save_settings(settings: &Settings) -> Result<()> {
    let settings_path = crate::conversations::get_settings_dir()?.join("settings.json");
    let json = serde_json::to_string_pretty(settings)?;
    fs::write(&settings_path, json)?;
    Ok(())
}

pub fn get_api_base_url(provider: &ProviderType) -> &'static str {
    match provider {
        ProviderType::CodestralMistralAi => "https://codestral.mistral.ai/v1",
        ProviderType::ApiMistralAi => "https://api.mistral.ai/v1",
    }
}

pub fn get_api_key(settings: &Settings) -> Option<String> {
    settings.api_provider.api_key.clone()
}

pub fn validate_phone_number(phone: &str) -> bool {
    // Basic validation - phone number should contain only digits, spaces, +, -, (, )
    phone.chars().all(|c| c.is_ascii_digit() || "+-() ".contains(c)) && phone.len() >= 10
}

pub fn validate_api_key(api_key: &str) -> bool {
    // Mistral API keys are typically base64-like strings
    !api_key.is_empty() && api_key.len() >= 20
}

// Note: Registration is done on the Mistral website (codestral.mistral.ai).
// Users must register there with their phone number to obtain an API key.
// This function is kept for potential future implementation but is not currently used.
pub async fn register_with_phone(_phone_number: &str) -> Result<String> {
    anyhow::bail!("Registration must be done on the Mistral website. Please visit https://codestral.mistral.ai/ to register with your phone number and obtain your API key, then enter it in the settings.")
}

