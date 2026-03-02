use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
#[allow(unused_imports)]
use tauri::Manager; // Required for StoreExt::store() method resolution
use tauri_plugin_store::StoreExt;

pub const STORE_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum RecordingMode {
    PushToTalk,
    Toggle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum InjectionMode {
    Clipboard,
    Character,
}

impl Default for InjectionMode {
    fn default() -> Self {
        Self::Clipboard
    }
}

/// Deserialize InjectionMode from either the new enum format or the legacy `clipboard_fallback` bool.
/// `true` (or "Clipboard") → Clipboard, `false` (or "Character") → Character.
fn deserialize_injection_mode<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<InjectionMode, D::Error> {
    use serde::de;

    struct InjectionModeVisitor;

    impl<'de> de::Visitor<'de> for InjectionModeVisitor {
        type Value = InjectionMode;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("\"Clipboard\", \"Character\", true, or false")
        }

        fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
            Ok(if v { InjectionMode::Clipboard } else { InjectionMode::Character })
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            match v {
                "Clipboard" => Ok(InjectionMode::Clipboard),
                "Character" => Ok(InjectionMode::Character),
                _ => Err(E::unknown_variant(v, &["Clipboard", "Character"])),
            }
        }
    }

    deserializer.deserialize_any(InjectionModeVisitor)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PostprocessProvider {
    Ollama,
    LmStudio,
    OpenRouter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hotkey_ptt: String,
    pub hotkey_toggle: String,
    pub recording_mode: RecordingMode,
    pub active_model: Option<String>,
    pub audio_device: Option<String>,
    pub language: String,
    pub launch_at_login: bool,
    #[serde(alias = "clipboard_fallback", deserialize_with = "deserialize_injection_mode")]
    pub injection_mode: InjectionMode,
    pub theme: Theme,
    pub postprocess_enabled: bool,
    pub postprocess_provider: Option<PostprocessProvider>,
    pub ollama_endpoint: String,
    pub ollama_model: String,
    pub ollama_system_prompt: String,
    pub lmstudio_endpoint: String,
    pub lmstudio_model: String,
    pub lmstudio_system_prompt: String,
    pub openrouter_model: String,
    pub openrouter_system_prompt: String,
    pub whisper_prompt: String,
    pub custom_dictionary: Vec<String>,
    pub silence_timeout_secs: f32,
    pub silence_threshold: f32,
    pub use_gpu: bool,
    pub debug_logs: bool,
    pub onboarding_complete: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey_ptt: "Alt+Space".into(),
            hotkey_toggle: "Alt+Shift+Space".into(),
            recording_mode: RecordingMode::PushToTalk,
            active_model: None,
            audio_device: None,
            language: "auto".into(),
            launch_at_login: false,
            injection_mode: InjectionMode::Clipboard,
            theme: Theme::System,
            postprocess_enabled: false,
            postprocess_provider: None,
            ollama_endpoint: "http://localhost:11434".into(),
            ollama_model: "llama3.2:3b".into(),
            ollama_system_prompt: "You are a transcription editor. Fix punctuation, capitalization, and formatting. Return only the corrected text, no commentary.".into(),
            lmstudio_endpoint: "http://localhost:1234".into(),
            lmstudio_model: "".into(),
            lmstudio_system_prompt: "You are a transcription editor. Fix punctuation, capitalization, and formatting. Return only the corrected text, no commentary.".into(),
            openrouter_model: "google/gemini-2.5-flash".into(),
            openrouter_system_prompt: "You are a transcription editor. Fix punctuation, capitalization, and formatting. Return only the corrected text, no commentary.".into(),
            whisper_prompt: "Dictated text with proper punctuation and capitalization.".into(),
            custom_dictionary: vec![
                "Calliope".into(),
                "Whisper".into(),
            ],
            silence_timeout_secs: 3.0,
            silence_threshold: 0.05,
            use_gpu: true,
            debug_logs: false,
            onboarding_complete: false,
        }
    }
}

pub fn load(app: &AppHandle) -> Settings {
    let store = app.store(STORE_FILE).expect("failed to open settings store");
    match store.get("settings") {
        Some(v) => match serde_json::from_value(v) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to deserialize saved settings, using defaults: {e}");
                Settings::default()
            }
        },
        None => Settings::default(),
    }
}

pub fn save(app: &AppHandle, settings: &Settings) -> tauri::Result<()> {
    let store = app.store(STORE_FILE).expect("failed to open settings store");
    store.set(
        "settings",
        serde_json::to_value(settings).expect("settings serialization failed"),
    );
    store.save().map_err(|e| tauri::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    app.emit("settings-changed", settings).map_err(|e| {
        tauri::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    })?;
    Ok(())
}
