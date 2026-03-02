/// Model manager.
///
/// Downloads, verifies, and manages .gguf Whisper model files.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size_bytes: u64,
    pub downloaded: bool,
    pub active: bool,
    pub speed_label: String,
    pub quality_label: String,
    pub hf_url: String,
    pub sha256: String,
}

/// All available models sourced from ggerganov/whisper.cpp on Hugging Face.
pub fn available_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            name: "tiny".into(),
            size_bytes: 74_000_000,
            downloaded: false,
            active: false,
            speed_label: "Very Fast".into(),
            quality_label: "Low".into(),
            hf_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin".into(),
            sha256: "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21".into(),
        },
        ModelInfo {
            name: "base".into(),
            size_bytes: 142_000_000,
            downloaded: false,
            active: false,
            speed_label: "Fast".into(),
            quality_label: "Decent".into(),
            hf_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".into(),
            sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe".into(),
        },
        ModelInfo {
            name: "small".into(),
            size_bytes: 466_000_000,
            downloaded: false,
            active: false,
            speed_label: "Moderate".into(),
            quality_label: "Good".into(),
            hf_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".into(),
            sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b".into(),
        },
        ModelInfo {
            name: "large-v3-turbo".into(),
            size_bytes: 809_000_000,
            downloaded: false,
            active: false,
            speed_label: "Fast".into(),
            quality_label: "High".into(),
            hf_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin".into(),
            sha256: "".into(), // TODO: fill in verified hash
        },
        ModelInfo {
            name: "large-v3".into(),
            size_bytes: 1_500_000_000,
            downloaded: false,
            active: false,
            speed_label: "Slow".into(),
            quality_label: "Best".into(),
            hf_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin".into(),
            sha256: "".into(), // TODO: fill in verified hash
        },
    ]
}

/// Path to the models directory inside the app data folder.
pub fn models_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir unavailable")
        .join("models")
}

/// Check which models are already downloaded and mark the active one.
pub fn hydrate_models(app: &AppHandle, active_model: Option<&str>) -> Vec<ModelInfo> {
    let dir = models_dir(app);
    available_models()
        .into_iter()
        .map(|mut m| {
            let path = dir.join(format!("{}.bin", m.name));
            m.downloaded = path.exists();
            m.active = active_model.map(|a| a == m.name).unwrap_or(false);
            m
        })
        .collect()
}

// ── Download ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub name: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
}

/// Stream-download a model file, emitting `download-progress` events.
/// Verifies SHA256 when a hash is available and deletes the file on mismatch.
/// Checks `abort_flag` each chunk; if set, deletes the partial file and returns an error.
pub async fn download_model_file(
    app: &AppHandle,
    name: &str,
    abort_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    let model = available_models()
        .into_iter()
        .find(|m| m.name == name)
        .ok_or_else(|| format!("unknown model: {name}"))?;

    let dir = models_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(format!("{name}.bin"));

    let response = reqwest::get(&model.hf_url)
        .await
        .map_err(|e| e.to_string())?;

    let total = response.content_length().unwrap_or(model.size_bytes);
    let mut bytes_downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    let mut file = tokio::fs::File::create(&dest)
        .await
        .map_err(|e| e.to_string())?;

    let mut hasher = Sha256::new();

    while let Some(chunk) = stream.next().await {
        if abort_flag.load(std::sync::atomic::Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&dest);
            return Err("download cancelled".into());
        }
        let chunk = chunk.map_err(|e| e.to_string())?;
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        bytes_downloaded += chunk.len() as u64;
        let _ = app.emit(
            "download-progress",
            DownloadProgress {
                name: name.to_string(),
                bytes_downloaded,
                total_bytes: total,
            },
        );
    }

    file.flush().await.map_err(|e| e.to_string())?;

    if !model.sha256.is_empty() {
        let hash = format!("{:x}", hasher.finalize());
        if hash != model.sha256 {
            let _ = std::fs::remove_file(&dest);
            return Err(format!(
                "SHA256 mismatch for {name}: expected {}, got {hash}",
                model.sha256
            ));
        }
    }

    Ok(())
}

/// Delete a downloaded model file.
pub fn delete_model_file(app: &AppHandle, name: &str) -> Result<(), String> {
    let dest = models_dir(app).join(format!("{name}.bin"));
    if dest.exists() {
        std::fs::remove_file(&dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}
