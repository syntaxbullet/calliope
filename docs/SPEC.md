# Calliope — Product Specification

**Version:** 0.2 (Current Implementation)
**Status:** Phase 0 Complete

---

## Overview

Calliope is a free, open-source, fully local speech-to-text dictation application that runs on macOS, Windows, and Linux. It uses OpenAI Whisper models (via whisper.cpp) to transcribe speech and inject the result directly into any active input field, competing directly with tools like SuperWhisper and Wispr Flow while being entirely offline-capable.

---

## Core Value Proposition

- **100% local by default** — no audio ever leaves the machine
- **Cross-platform** — macOS, Windows, Linux (including Wayland) with a consistent UX
- **Zero friction** — lives in the system tray, activated by a hotkey
- **Open source** — MIT license, community-first

---

## Target Users

- Privacy-conscious knowledge workers who type heavily
- Developers and writers who want hands-free input
- Users frustrated by cloud-only or macOS-only alternatives

---

## Implemented Features

### Recording & Transcription

#### Recording Modes
- **Push-to-talk:** Hold configured hotkey to record; release to transcribe and inject
- **Toggle mode:** Press hotkey once to start recording; press again to stop and inject
- Mode is configurable in settings
- Toggle mode supports automatic silence detection with configurable timeout (0–30s, default 3s) — polls RMS every 100ms

#### Audio Capture
- Built on `cpal` — enumerates all system input devices
- Negotiates 16 kHz mono f32 stream; falls back to default config if unavailable
- Stereo-to-mono downmixing in audio callback
- Real-time RMS amplitude computation (lock-free atomic for UI polling)
- Emits `audio-level` events for waveform visualization
- Lanczos (a=3) resampling to 16 kHz for Whisper input
- Silence trimming: 30ms RMS windows with 50ms onset padding

#### Whisper Inference
- Feature-gated (`--features whisper`, off by default for fast CI)
- `WhisperEngine` wrapper over `whisper-rs` / whisper.cpp
- Acceleration backend detection: CPU, Metal, CUDA, ROCm, CoreML
- Greedy sampling (best_of: 1)
- Optional language constraint or auto-detect
- Optional initial prompt for style/vocabulary guidance
- Runs on dedicated thread (64 MB stack)

### Text Injection

Platform-specific implementations:

- **macOS:** Clipboard swap + CoreGraphics CGEvent Cmd+V; requires Accessibility permission
- **Windows:** Primary: UI Automation IValueProvider::SetValue; Fallback: clipboard swap + SendInput Ctrl+V
- **Linux:** Fallback chain — wtype → ydotool → xdotool → AT-SPI (D-Bus via zbus)
- **Clipboard fallback:** If injection fails (and enabled in settings), copies text to clipboard and emits `clipboard-fallback` event for toast notification

### Hotkey System

- Global hotkeys via `tauri-plugin-global-shortcut`
- Separate configurable hotkeys for PTT and Toggle modes
- Defaults: `Alt+Space` (PTT), `Alt+Shift+Space` (Toggle)
- Registration: unregisters old bindings, registers new on update
- PTT: keydown → start recording, keyup → stop + transcribe
- Toggle: press → start, press again → stop + transcribe

#### Recording Flow
1. Clear audio buffer
2. Get input device + build stream config
3. Spawn audio-keeper thread (64 MB stack, owns cpal::Stream — avoids macOS CoreAudio thread-affinity crashes)
4. On stop: join keeper thread, snapshot buffer, resample, trim silence
5. Run Whisper transcription on dedicated thread (64 MB stack)
6. Inject text on another thread (panics caught)
7. Clipboard fallback if injection fails (when enabled)

### Model Management

- **5 models available:** tiny (74 MB), base (142 MB), small (466 MB), large-v3-turbo (809 MB), large-v3 (1.5 GB)
- Downloaded from Hugging Face (ggerganov/whisper.cpp) as .gguf files
- Stored in OS user data directory (`~/.calliope/models/{name}.bin`)
- SHA256 verification on download (deletes on mismatch)
- `download-progress` events during download
- `hydrate_models()` scans disk on startup, marks downloaded + active status
- Model preload on startup (background thread, 64 MB stack)
- Switch active model at runtime (clears cached engine)

### System Tray

- Tray icon with menu: Show, Settings, Quit
- Click toggles popover visibility
- Popover positioning: macOS top-right below menu bar; Windows/Linux bottom-right above taskbar
- HiDPI-aware via monitor scale factor
- Set up programmatically in `setup()` (not in tauri.conf.json)

### Settings

~30 configurable fields persisted via `tauri-plugin-store` (JSON in OS app data dir):

| Category | Settings |
|----------|----------|
| Hotkeys | `hotkey_ptt`, `hotkey_toggle` |
| Recording | `recording_mode` (PushToTalk / Toggle), `silence_timeout_secs` |
| Audio | `audio_device`, `language` (auto / 100+ languages) |
| Model | `active_model` |
| Inference | `whisper_prompt`, `custom_dictionary`, `use_gpu` |
| UI | `theme` (System / Light / Dark), `launch_at_login`, `debug_logs` |
| Injection | `clipboard_fallback` |
| Post-processing | `postprocess_enabled`, `postprocess_provider`, provider-specific endpoint/model/prompt fields |
| Onboarding | `onboarding_complete` |

Emits `settings-changed` event on save.

### Post-Processing (Settings Only)

Three configurable backends — settings UI fully implemented, but **not yet wired into the transcription flow**:

- **Ollama (local):** POST to `/api/generate`, no auth
- **LM Studio (local):** OpenAI-compatible `/v1/chat/completions`
- **OpenRouter (cloud):** OpenAI-compatible with Bearer token auth

API keys stored in OS keychain via `keyring` crate. Customizable system prompt per provider. Falls back to original text if disabled or backend unreachable.

### Accessibility & Permissions

- **macOS:** `AXIsProcessTrusted()` check + system prompt for Accessibility grant
- **Linux:** Injection status checker — reports Wayland/X11, tool availability (wtype, ydotool, xdotool), ydotoold daemon status, `/dev/uinput` permissions, actionable recommendations

### API Key Management

- `save_api_key` / `get_api_key` / `delete_api_key` commands
- Stored in OS credential store via `keyring` crate

---

## User Interface

### Three Windows

| Window | Size | Purpose |
|--------|------|---------|
| Main (popover) | 280×420 | Tray popover — status, mini controls |
| Overlay | 300×64 | Recording indicator — transparent, top-center |
| Settings | 680×480 | Full settings panel |

All windows: frameless, skip taskbar, always on top. macOS private API enabled.

### Popover View

- Animated waveform (20 bars) with state label (Ready / Listening / Transcribing / Processing / Injecting / Error)
- Mini status: active model, current hotkeys, acceleration backend
- Clipboard/Character insertion mode toggle
- Settings button, close button (hides, doesn't quit)

### Overlay

- Floating pill with glassmorphism (blur 12px, semi-transparent bg, 1px border)
- 24-bar waveform with log-scaled amplitude
- State label
- Polls state every 200ms, audio level every 50ms (only when visible)

### Onboarding

4-step wizard (3 steps on Windows/Linux — skip permissions):

1. **Welcome** — intro + "Get Started"
2. **Permissions** (macOS only) — Accessibility check with 1.5s polling, skip option
3. **Model Download** — radio select, progress bar, requires ≥1 model
4. **Success** — shows hotkey instructions

### Settings Panel

5 horizontal tabs:

- **General:** Input device, insertion mode, language, silence timeout, GPU toggle, debug logs, accessibility status
- **Models:** Downloaded/Available sections, download/use/unload/delete per model, progress bars
- **Customization:** Whisper prompt textarea, custom dictionary word tags (add/remove)
- **Post-processing:** Master toggle, provider radio (Ollama/LM Studio/OpenRouter), conditional config fields, API key management
- **Hotkeys:** PTT + Toggle hotkey capture components (click to record, Escape to cancel)

### Toast Notifications

- Clipboard fallback toast: "Text copied — paste with Cmd/Ctrl+V" (OS-aware), auto-hides after 4s

### Design System

- **Typeface:** Geist (sans-serif) + Geist Mono (badges/hotkeys)
- **Color:** Monochromatic warm grays, system-adaptive light/dark + manual override
  - Light: `--bg-base: #FAFAF9`, `--text-primary: #1C1B1A`
  - Dark: `--bg-base: #141413`, `--text-primary: #F0EFED`
- **Typography:** `.text-title` 15px/600, `.text-body` 13px/400, `.text-mono` 12px
- **Icons:** Lucide (inline SVG, 16px, 1.5px stroke)
- **Motion:** Respects `prefers-reduced-motion`
- All components built from scratch (no component library)

---

## IPC Commands

All commands return `Result<T, String>`:

| Command | Purpose |
|---------|---------|
| `get_settings` / `save_settings` | Settings CRUD |
| `get_app_state` | Current state machine value |
| `list_audio_devices` / `get_audio_level` | Audio device enum + RMS |
| `list_models` / `set_active_model` / `download_model` / `delete_model` | Model management |
| `update_hotkeys` | Re-register global shortcuts |
| `get_acceleration_backend` | Detected GPU backend |
| `check_accessibility` / `request_accessibility` | macOS permissions |
| `check_linux_injection_status` | Linux tool availability |
| `save_api_key` / `get_api_key` / `delete_api_key` | Keychain CRUD |

---

## Event System

| Event | Payload | Purpose |
|-------|---------|---------|
| `app-state-changed` | `AppState` | State machine transitions |
| `audio-level` | `{ rms: number }` | Waveform visualization |
| `settings-changed` | `Settings` | UI sync |
| `download-progress` | `{ name, bytes_downloaded, total_bytes }` | Model download UI |
| `clipboard-fallback` | (none) | Toast notification trigger |

---

## State Machine

`Idle → Recording → Transcribing → PostProcessing → Injecting → Idle`

Error state can occur from any active state, with message payload.

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | Tauri v2 (Rust + WebView) |
| UI | React 19, TypeScript, Tailwind CSS v4 |
| State | Zustand 5 |
| Inference | whisper-rs 0.15 (whisper.cpp bindings) |
| Audio | cpal 0.15 |
| Clipboard | arboard 3 |
| HTTP | reqwest 0.12 |
| Keychain | keyring 2 |
| Build | Vite 7, bun |

---

## Not Yet Implemented

- Post-processing integration into transcription flow
- Download resume (partial download resumption)

---

## UX Principles

1. **Invisible by default** — lives in the tray, no attention demanded
2. **Consistent across platforms** — identical settings, hotkeys, and behavior
3. **Immediate feedback** — visual waveform on record, instant transcription
4. **Graceful degradation** — clipboard fallback if injection fails, with notification
5. **Privacy first** — no telemetry, no network calls except model downloads and optional post-processing
