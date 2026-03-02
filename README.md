# Calliope

Local, cross-platform, open-source speech-to-text dictation app. A privacy-first alternative to SuperWhisper and WisprFlow.

Calliope runs Whisper inference entirely on your machine — no cloud, no API keys required. Just press a hotkey and dictate.

## Features

- **Fully local** — all transcription happens on-device via whisper.cpp
- **Cross-platform** — macOS, Windows, and Linux
- **Push-to-talk & toggle mode** — with automatic silence detection
- **Text injection** — transcribed text is typed directly into the focused app
- **Optional post-processing** — clean up transcriptions with a local or cloud LLM (Ollama, LM Studio, OpenRouter)
- **GPU accelerated** — Metal (macOS), CUDA (NVIDIA), ROCm/HIPBlas (AMD)
- **Lightweight** — lives in your system tray

## Tech Stack

- [Tauri v2](https://v2.tauri.app/) (Rust + WebView)
- [whisper-rs](https://github.com/tazz4843/whisper-rs) (Rust bindings to whisper.cpp)
- React 19 + TypeScript + Tailwind CSS v4

## Development

### Prerequisites

- [Rust](https://rustup.rs/)
- [Bun](https://bun.sh/)
- Platform-specific dependencies — see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

### Setup

```bash
bun install
bun run tauri dev
```

To build with Whisper inference enabled:

```bash
bun run tauri dev -- --features whisper     # CPU only
bun run tauri dev -- --features metal       # macOS (Apple Silicon / Intel)
bun run tauri dev -- --features cuda        # NVIDIA GPU
bun run tauri dev -- --features rocm        # AMD GPU (ROCm/HIPBlas)
```

### Build

```bash
bun run tauri build --features metal        # macOS release
```

## License

MIT
