/// Whisper inference module.
///
/// The actual whisper-rs integration is gated behind the `whisper` feature flag
/// to allow the project to compile without whisper.cpp during early development.
/// Enable with: `cargo build --features whisper`

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: String,
    pub duration_ms: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum WhisperError {
    #[error("no model loaded")]
    NoModel,
    #[error("model file not found: {0}")]
    ModelNotFound(String),
    #[error("inference failed: {0}")]
    InferenceFailed(String),
}

#[cfg(feature = "whisper")]
pub mod inner {
    use super::*;
    use std::path::Path;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    pub struct WhisperEngine {
        ctx: WhisperContext,
    }

    impl WhisperEngine {
        pub fn load(model_path: &str, use_gpu: bool) -> Result<Self, WhisperError> {
            if !Path::new(model_path).exists() {
                return Err(WhisperError::ModelNotFound(model_path.to_string()));
            }
            let backend = if !use_gpu {
                "CPU (GPU disabled)"
            } else if cfg!(feature = "metal") { "Metal" }
            else if cfg!(feature = "cuda") { "CUDA" }
            else if cfg!(feature = "rocm") { "ROCm" }
            else if cfg!(feature = "coreml") { "CoreML" }
            else { "CPU" };
            log::info!("Whisper backend: {backend}");
            let mut params = WhisperContextParameters::default();
            params.use_gpu(use_gpu);
            let ctx = WhisperContext::new_with_params(model_path, params)
            .map_err(|e| WhisperError::InferenceFailed(e.to_string()))?;
            Ok(Self { ctx })
        }

        pub fn transcribe(
            &mut self,
            samples: &[f32],
            language: Option<&str>,
            prompt: Option<&str>,
        ) -> Result<TranscriptionResult, WhisperError> {
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            if let Some(lang) = language {
                params.set_language(Some(lang));
            }
            if let Some(p) = prompt {
                if !p.is_empty() {
                    params.set_initial_prompt(p);
                }
            }

            let start = std::time::Instant::now();
            let mut state = self.ctx.create_state()
                .map_err(|e| WhisperError::InferenceFailed(e.to_string()))?;
            state
                .full(params, samples)
                .map_err(|e| WhisperError::InferenceFailed(e.to_string()))?;

            let mut text = String::new();
            for segment in state.as_iter() {
                if let Ok(s) = segment.to_str() {
                    text.push_str(s);
                }
            }

            let lang_id = state.full_lang_id_from_state();
            let language = whisper_rs::get_lang_str(lang_id)
                .unwrap_or("")
                .to_string();

            Ok(TranscriptionResult {
                text: text.trim().to_string(),
                language,
                duration_ms: start.elapsed().as_millis() as u64,
            })
        }
    }
}

// Stub when whisper feature is disabled
#[cfg(not(feature = "whisper"))]
pub mod inner {
    use super::*;

    pub struct WhisperEngine;

    impl WhisperEngine {
        pub fn load(_model_path: &str, _use_gpu: bool) -> Result<Self, WhisperError> {
            Err(WhisperError::InferenceFailed(
                "whisper feature not enabled; rebuild with --features whisper".into(),
            ))
        }

        pub fn transcribe(
            &mut self,
            _samples: &[f32],
            _language: Option<&str>,
            _prompt: Option<&str>,
        ) -> Result<TranscriptionResult, WhisperError> {
            Err(WhisperError::NoModel)
        }
    }
}
