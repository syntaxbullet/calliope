# Calliope — Technical Architecture

**Version:** 0.1 (Planning)
**Status:** Draft

---

## Tech Stack

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| App framework | Tauri v2 (Rust + WebView) | Small binary, Rust-native Whisper bindings, cross-platform system APIs |
| UI | React 19 + TypeScript | Wide ecosystem, Tauri first-class support |
| UI styling | Tailwind CSS v4 | Utility-first, consistent design system |
| Whisper inference | `whisper-rs` (Rust crate, whisper.cpp FFI) | Native Rust, no Python runtime, Metal/CUDA acceleration |
| Audio capture | `cpal` (Rust crate) | Cross-platform audio I/O, low-level PCM access |
| Global hotkeys | `tauri-plugin-global-shortcut` | Tauri v2 plugin, handles all platforms |
| System tray | Tauri v2 built-in tray API | |
| Model storage | User data directory via Tauri path API | `~/.local/share/calliope/` (Linux), `%APPDATA%\calliope\` (Windows), `~/Library/Application Support/calliope/` (macOS) |
| State management | Zustand (frontend) + Tauri store plugin (persistence) | |

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────┐
│                    Tauri Process                      │
│                                                       │
│  ┌──────────────┐    ┌──────────────────────────┐   │
│  │  WebView UI  │◄──►│     Tauri Commands (IPC)  │   │
│  │  React + TS  │    └──────────────────────────┘   │
│  └──────────────┘              │                     │
│                                ▼                     │
│  ┌─────────────────────────────────────────────────┐│
│  │               Rust Backend Core                  ││
│  │                                                  ││
│  │  ┌───────────┐  ┌──────────────┐  ┌──────────┐ ││
│  │  │   Audio   │  │   Whisper    │  │  Inject  │ ││
│  │  │  Capture  │─►│  Inference   │─►│  Engine  │ ││
│  │  │  (cpal)   │  │ (whisper-rs) │  │          │ ││
│  │  └───────────┘  └──────────────┘  └──────────┘ ││
│  │                                                  ││
│  │  ┌───────────┐  ┌──────────────┐  ┌──────────┐ ││
│  │  │  Hotkey   │  │    Model     │  │  Post-   │ ││
│  │  │  Manager  │  │   Manager   │  │ Process  │ ││
│  │  └───────────┘  └──────────────┘  └──────────┘ ││
│  └─────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────┘
```

---

## Module Breakdown

### 1. Audio Capture (`src-tauri/src/audio/`)

- Uses `cpal` for cross-platform microphone access
- Records 16kHz mono PCM (Whisper's expected format)
- Buffers audio in memory during recording session
- Exposes events to frontend: `recording-started`, `recording-stopped`, `audio-level` (for VU meter in UI)
- Implements VAD (Voice Activity Detection) — simple energy-based threshold to strip leading/trailing silence before sending to inference
- Audio device enumeration + selection exposed via Tauri command

### 2. Whisper Inference (`src-tauri/src/whisper/`)

- Wraps `whisper-rs` with a safe async interface
- Model loading: loads `.gguf` model file into memory on demand; keeps loaded between transcriptions for performance
- Transcription runs on a dedicated Tokio thread to avoid blocking the main thread
- Acceleration:
  - macOS: Metal (via whisper.cpp Metal backend) — auto-detected
  - Windows: CUDA if available, fallback to CPU
  - Linux: CUDA if available, ROCm if available, fallback to CPU
- Returns: transcript string + confidence + detected language + word timestamps (for future use)

### 3. Text Injection Engine (`src-tauri/src/injection/`)

Platform-specific implementations behind a common `Injector` trait:

```rust
trait Injector {
    fn inject(&self, text: &str) -> Result<(), InjectionError>;
}
```

#### macOS (`injection/macos.rs`)
1. Save current clipboard contents
2. Write transcription to pasteboard
3. Post `CGEvent` Cmd+V to the system
4. Restore original clipboard after 100ms delay
5. Fallback: `AXUIElement` setValue if clipboard injection fails

Requires: Accessibility permission (prompted at onboarding)

#### Windows (`injection/windows.rs`)
1. Try `IUIAutomation::IValueProvider::SetValue` on focused element
2. Fallback: save clipboard → write text → `SendInput` Ctrl+V → restore clipboard

Requires: No special permissions (standard user can inject input)

#### Linux (`injection/linux.rs`)
Detection order (fail-fast, try next):
1. **wtype** (Wayland virtual keyboard protocol) — if `WAYLAND_DISPLAY` is set and compositor supports `zwp_virtual_keyboard_v1`
2. **ydotool** — requires `ydotoold` daemon + `/dev/uinput` access; works on Wayland + X11
3. **xdotool** — X11 only (detected via `DISPLAY` env var); skip entirely if no `DISPLAY`
4. **AT-SPI** — final fallback via `atspi-2` crate for GTK/Qt apps

If all methods fail: copy to clipboard + show notification "Text copied to clipboard — paste manually (Ctrl+V)"

> **Note on Wayland detection:** Do NOT use xdotool exit code to detect failure on Wayland — it returns 0 even when silently failing. Detect compositor type explicitly via `WAYLAND_DISPLAY` / `XDG_SESSION_TYPE` environment variables.

### 4. Hotkey Manager (`src-tauri/src/hotkeys/`)

- Wraps `tauri-plugin-global-shortcut`
- Supports two modes registered simultaneously:
  - PTT (push-to-talk): keydown → start recording; keyup → stop and transcribe
  - Toggle: keydown → if idle, start recording; if recording, stop and transcribe
- Hotkey conflict detection at startup (warn if hotkey is already registered by another app)
- Hotkeys persisted in settings; reconfigurable at runtime without restart

### 5. Model Manager (`src-tauri/src/models/`)

- Maintains a model registry (name, URL, expected SHA256 hash, size)
- Model sources: official Hugging Face `ggerganov/whisper.cpp` repository
- Download implementation: `reqwest` async HTTP client with progress events streamed to frontend
- Hash verification after download
- Models stored at: `{app_data_dir}/models/{model_name}.gguf`
- API: list available models, list downloaded models, download model (with progress), delete model, set active model

### 6. Post-Processing (`src-tauri/src/postprocess/`)

Optional pipeline step, disabled by default.

```
raw_transcript → (optional) post_processor → injected_text
```

Two backends:

**Local (Ollama):**
- HTTP call to `http://localhost:11434/api/generate` (configurable endpoint)
- Configurable model (e.g. `llama3.2:3b`, `mistral:7b`)
- System prompt: "You are a transcription editor. Fix punctuation, capitalization, and formatting. Return only the corrected text, no commentary."
- Falls back to raw transcript if Ollama is unreachable

**Cloud (OpenAI / Anthropic):**
- API key stored in OS keychain (`keyring` crate)
- Configurable provider + model
- Same system prompt pattern
- Falls back to raw transcript on error

---

## State Machine

The core recording/transcription state machine:

```
IDLE
  │  hotkey pressed (toggle) or held (ptt)
  ▼
RECORDING
  │  hotkey released (ptt) or pressed again (toggle)
  ▼
TRANSCRIBING
  │  whisper inference complete
  ▼
POST_PROCESSING  (skipped if no post-processor configured)
  │  post-processor returns
  ▼
INJECTING
  │  injection complete
  ▼
IDLE

Any state → ERROR (on failure, with message)
ERROR → IDLE (after user dismisses or timeout)
```

State is broadcast from Rust backend to frontend via Tauri events for tray icon and UI updates.

---

## Project Structure

```
calliope/
├── src-tauri/                  # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── audio/              # Audio capture (cpal)
│       ├── whisper/            # Inference (whisper-rs)
│       ├── injection/          # Text injection (platform-specific)
│       │   ├── mod.rs
│       │   ├── macos.rs
│       │   ├── windows.rs
│       │   └── linux.rs
│       ├── hotkeys/            # Global hotkey management
│       ├── models/             # Model download/management
│       ├── postprocess/        # LLM post-processing pipeline
│       └── settings/           # Settings persistence
├── src/                        # React frontend
│   ├── main.tsx
│   ├── App.tsx
│   ├── components/
│   │   ├── Onboarding/
│   │   ├── Settings/
│   │   ├── ModelManager/
│   │   └── StatusIndicator/
│   ├── hooks/
│   ├── store/                  # Zustand state
│   └── types/
├── docs/
│   ├── SPEC.md
│   ├── ARCHITECTURE.md
│   └── ROADMAP.md
└── README.md
```

---

## Build & Distribution

- **macOS:** `.dmg` + `.app` bundle; notarized; Apple Silicon native (`aarch64-apple-darwin`) + Intel (`x86_64-apple-darwin`) universal binary
- **Windows:** `.msi` installer + `.exe` portable; code-signed
- **Linux:** `.AppImage` (universal) + `.deb` + `.rpm`; AUR package

CI: GitHub Actions matrix build across all three platforms.

whisper.cpp is compiled from source as part of the Rust build (`whisper-rs` handles this via `build.rs`). Metal backend enabled for macOS builds, CUDA optional (separate build artifact).

---

## Key Dependencies

| Crate / Package | Version | Purpose |
|----------------|---------|---------|
| `tauri` | ^2.0 | App framework |
| `tauri-plugin-global-shortcut` | ^2.0 | Global hotkeys |
| `tauri-plugin-store` | ^2.0 | Settings persistence |
| `whisper-rs` | ^0.15 | Whisper.cpp Rust bindings |
| `cpal` | ^0.15 | Cross-platform audio capture |
| `reqwest` | ^0.12 | Model downloads |
| `tokio` | ^1 | Async runtime |
| `keyring` | ^2 | OS keychain for API keys |
| `serde` / `serde_json` | ^1 | Serialization |
| React | 19 | UI framework |
| Zustand | ^5 | Frontend state |
| Tailwind CSS | ^4 | Styling |
