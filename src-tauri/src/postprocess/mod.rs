/// Post-processing pipeline.
///
/// Optional step between transcription and injection.
/// Disabled by default. Two backends: Ollama (local) and OpenRouter (cloud).

use crate::settings::{PostprocessProvider, Settings};

#[derive(Debug, thiserror::Error)]
pub enum PostProcessError {
    #[error("post-processing disabled")]
    Disabled,
    #[error("backend unreachable: {0}")]
    Unreachable(String),
    #[error("API error: {0}")]
    Api(String),
}

/// Process raw transcript text. Returns the processed text,
/// or the original text if post-processing is disabled or fails.
pub async fn process(text: &str, settings: &Settings) -> String {
    if !settings.postprocess_enabled {
        return text.to_string();
    }

    let result = match &settings.postprocess_provider {
        Some(PostprocessProvider::Ollama) => process_ollama(text, settings).await,
        Some(PostprocessProvider::LmStudio) => process_lmstudio(text, settings).await,
        Some(PostprocessProvider::OpenRouter) => process_openrouter(text, settings).await,
        None => return text.to_string(),
    };

    match result {
        Ok(processed) => processed,
        Err(e) => {
            log::error!("post-processing failed, returning raw text: {e}");
            text.to_string()
        }
    }
}

async fn process_ollama(text: &str, settings: &Settings) -> Result<String, PostProcessError> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/generate", settings.ollama_endpoint.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": settings.ollama_model,
        "prompt": text,
        "system": settings.ollama_system_prompt,
        "stream": false,
    });

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| PostProcessError::Unreachable(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(PostProcessError::Api(format!("{status}: {body}")));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| PostProcessError::Api(e.to_string()))?;

    json["response"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| PostProcessError::Api("missing 'response' field".into()))
}

async fn process_lmstudio(text: &str, settings: &Settings) -> Result<String, PostProcessError> {
    let client = reqwest::Client::new();
    let url = format!("{}/v1/chat/completions", settings.lmstudio_endpoint.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": settings.lmstudio_model,
        "messages": [
            { "role": "system", "content": settings.lmstudio_system_prompt },
            { "role": "user", "content": text },
        ],
    });

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| PostProcessError::Unreachable(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(PostProcessError::Api(format!("{status}: {body}")));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| PostProcessError::Api(e.to_string()))?;

    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| PostProcessError::Api("unexpected response format".into()))
}

async fn process_openrouter(text: &str, settings: &Settings) -> Result<String, PostProcessError> {
    // Retrieve API key from OS keychain
    let entry = keyring::Entry::new("com.syntaxbullet.calliope", "openrouter")
        .map_err(|e| PostProcessError::Api(format!("keyring error: {e}")))?;
    let api_key = entry
        .get_password()
        .map_err(|e| PostProcessError::Api(format!("no OpenRouter API key configured: {e}")))?;

    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": settings.openrouter_model,
        "messages": [
            { "role": "system", "content": settings.openrouter_system_prompt },
            { "role": "user", "content": text },
        ],
    });

    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| PostProcessError::Unreachable(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(PostProcessError::Api(format!("{status}: {body}")));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| PostProcessError::Api(e.to_string()))?;

    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| PostProcessError::Api("unexpected response format".into()))
}
