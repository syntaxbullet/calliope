/// Whisper inference via whisper-cli subprocess.
///
/// Writes audio samples to a temporary WAV file, invokes the whisper-cli binary,
/// and parses the JSON output.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

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

pub struct WhisperCli {
    binary_path: PathBuf,
}

impl WhisperCli {
    pub fn new(binary_path: PathBuf) -> Self {
        Self { binary_path }
    }

    /// Write f32 samples (16kHz mono) to a temporary 16-bit PCM WAV file.
    fn write_wav(samples: &[f32], path: &Path) -> Result<(), WhisperError> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec)
            .map_err(|e| WhisperError::InferenceFailed(format!("WAV write error: {e}")))?;
        for &s in samples {
            let clamped = s.clamp(-1.0, 1.0);
            let val = (clamped * 32767.0) as i16;
            writer
                .write_sample(val)
                .map_err(|e| WhisperError::InferenceFailed(format!("WAV sample error: {e}")))?;
        }
        writer
            .finalize()
            .map_err(|e| WhisperError::InferenceFailed(format!("WAV finalize error: {e}")))?;
        Ok(())
    }

    pub fn transcribe(
        &self,
        samples: &[f32],
        model_path: &str,
        language: Option<&str>,
        prompt: Option<&str>,
        use_gpu: bool,
    ) -> Result<TranscriptionResult, WhisperError> {
        if !Path::new(model_path).exists() {
            return Err(WhisperError::ModelNotFound(model_path.to_string()));
        }

        let tmp_dir = tempfile::tempdir()
            .map_err(|e| WhisperError::InferenceFailed(format!("temp dir error: {e}")))?;
        let wav_path = tmp_dir.path().join("audio.wav");
        let out_base = tmp_dir.path().join("result");

        Self::write_wav(samples, &wav_path)?;

        let start = std::time::Instant::now();

        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("-m").arg(model_path)
            .arg("-f").arg(&wav_path)
            .arg("-oj") // output JSON
            .arg("-of").arg(&out_base); // output file base

        if let Some(lang) = language {
            if lang != "auto" {
                cmd.arg("-l").arg(lang);
            }
        }

        if let Some(p) = prompt {
            if !p.is_empty() {
                cmd.arg("--prompt").arg(p);
            }
        }

        if !use_gpu {
            cmd.arg("--no-gpu");
        }

        log::info!("running whisper-cli: {:?}", cmd);

        let output = cmd.output().map_err(|e| {
            WhisperError::InferenceFailed(format!("failed to spawn whisper-cli: {e}"))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WhisperError::InferenceFailed(format!(
                "whisper-cli exited with {}: {stderr}",
                output.status
            )));
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        // whisper-cli writes <out_base>.json
        let json_path = out_base.with_extension("json");
        let json_str = std::fs::read_to_string(&json_path).map_err(|e| {
            WhisperError::InferenceFailed(format!("failed to read output JSON: {e}"))
        })?;

        Self::parse_output(&json_str, duration_ms)
    }

    fn parse_output(json_str: &str, duration_ms: u64) -> Result<TranscriptionResult, WhisperError> {
        let val: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| WhisperError::InferenceFailed(format!("JSON parse error: {e}")))?;

        // whisper-cli JSON format: { "transcription": [{ "text": "..." }], "result": { "language": "en" } }
        let text = val["transcription"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|seg| seg["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
            .trim()
            .to_string();

        let language = val["result"]["language"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(TranscriptionResult {
            text,
            language,
            duration_ms,
        })
    }
}
