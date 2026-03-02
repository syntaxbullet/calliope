// ── App State ─────────────────────────────────────────────────────────────

/**
 * Mirrors Rust's `AppState` enum, serialized with `#[serde(tag = "type", content = "message")]`.
 */
export type AppState =
  | { type: "Idle" }
  | { type: "Recording" }
  | { type: "Transcribing" }
  | { type: "PostProcessing" }
  | { type: "Injecting" }
  | { type: "Error"; message: string };

// ── Recording Mode ────────────────────────────────────────────────────────

export type RecordingMode = "PushToTalk" | "Toggle";

// ── Theme ─────────────────────────────────────────────────────────────────

export type Theme = "System" | "Light" | "Dark";

// ── Injection Mode ───────────────────────────────────────────────────────

export type InjectionMode = "Clipboard" | "Character";

// ── Settings ──────────────────────────────────────────────────────────────

export interface Settings {
  hotkey_ptt: string;
  hotkey_toggle: string;
  recording_mode: RecordingMode;
  active_model: string | null;
  audio_device: string | null;
  language: string;
  launch_at_login: boolean;
  injection_mode: InjectionMode;
  theme: Theme;
  postprocess_enabled: boolean;
  postprocess_provider: "Ollama" | "LmStudio" | "OpenRouter" | null;
  ollama_endpoint: string;
  ollama_model: string;
  ollama_system_prompt: string;
  lmstudio_endpoint: string;
  lmstudio_model: string;
  lmstudio_system_prompt: string;
  openrouter_model: string;
  openrouter_system_prompt: string;
  whisper_prompt: string;
  custom_dictionary: string[];
  silence_timeout_secs: number;
  silence_threshold: number;
  use_gpu: boolean;
  debug_logs: boolean;
  onboarding_complete: boolean;
}

export const DEFAULT_SETTINGS: Settings = {
  hotkey_ptt: "Alt+Space",
  hotkey_toggle: "Alt+Shift+Space",
  recording_mode: "PushToTalk",
  active_model: null,
  audio_device: null,
  language: "auto",
  launch_at_login: false,
  injection_mode: "Clipboard",
  theme: "System",
  postprocess_enabled: false,
  postprocess_provider: null,
  ollama_endpoint: "http://localhost:11434",
  ollama_model: "llama3.2:3b",
  ollama_system_prompt:
    "You are a transcription editor. Fix punctuation, capitalization, and formatting. Return only the corrected text, no commentary.",
  lmstudio_endpoint: "http://localhost:1234",
  lmstudio_model: "",
  lmstudio_system_prompt:
    "You are a transcription editor. Fix punctuation, capitalization, and formatting. Return only the corrected text, no commentary.",
  openrouter_model: "google/gemini-2.5-flash",
  openrouter_system_prompt:
    "You are a transcription editor. Fix punctuation, capitalization, and formatting. Return only the corrected text, no commentary.",
  whisper_prompt: "Dictated text with proper punctuation and capitalization.",
  custom_dictionary: ["Calliope", "Whisper"],
  silence_timeout_secs: 3,
  silence_threshold: 0.05,
  use_gpu: true,
  debug_logs: false,
  onboarding_complete: false,
};

// ── Model Info ────────────────────────────────────────────────────────────

export interface ModelInfo {
  name: string;
  size_bytes: number;
  downloaded: boolean;
  active: boolean;
  speed_label: string;
  quality_label: string;
  hf_url: string;
  sha256: string;
}

// ── Audio Device ──────────────────────────────────────────────────────────

export interface AudioDevice {
  id: string;
  name: string;
  is_default: boolean;
}

// ── Linux Injection Status ────────────────────────────────────────────────

export interface LinuxInjectionStatus {
  wayland: boolean;
  wtype_available: boolean;
  ydotool_available: boolean;
  ydotoold_running: boolean;
  uinput_accessible: boolean;
  xdotool_available: boolean;
  recommended_action: string | null;
}

// ── Tauri Events emitted from Rust backend ────────────────────────────────

export interface AudioLevelEvent {
  rms: number; // 0.0 – 1.0
}

export interface TranscriptionResult {
  text: string;
  language: string;
  duration_ms: number;
}
