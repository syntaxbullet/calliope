/// App state machine.
///
/// States: Idle → Recording → Transcribing → PostProcessing → Injecting → Idle
/// Any state can transition to Error.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum AppState {
    Idle,
    Recording,
    Transcribing,
    PostProcessing,
    Injecting,
    Error(String),
}

pub struct AppStateManager {
    state: Mutex<AppState>,
}

impl AppStateManager {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(AppState::Idle),
        }
    }

    pub fn get(&self) -> AppState {
        self.state.lock().unwrap().clone()
    }

    pub fn transition(&self, new_state: AppState, app: &AppHandle) {
        {
            let mut s = self.state.lock().unwrap();
            *s = new_state.clone();
        }
        let _ = app.emit("app-state-changed", &new_state);
        let _ = app.emit_to("overlay", "app-state-changed", &new_state);
        crate::tray::update_icon(app, &new_state);
        log::info!("state → {new_state:?}");
    }
}

// ── Additional managed state types ───────────────────────────────────────────

/// Holds the stop signal sender and the keeper thread's join handle.
/// Dropping the sender causes the keeper thread to exit and drop the cpal::Stream
/// on its own thread, avoiding CoreAudio thread-affinity crashes.
pub struct ActiveStreamState(
    pub Mutex<Option<(std::sync::mpsc::Sender<()>, std::thread::JoinHandle<()>)>>,
);

/// Shared audio sample buffer filled during recording.
pub struct ActiveBufferState(pub Arc<Mutex<Vec<f32>>>);

/// Sample rate of the active recording stream.
pub struct ActiveSampleRate(pub Mutex<u32>);

/// Cached Whisper model engine: (model_name, use_gpu, engine, inference_count).
/// The engine is reloaded every MAX_INFERENCES_BEFORE_RELOAD runs to prevent
/// quality degradation from accumulated whisper.cpp internal state.
pub struct WhisperState(pub Mutex<Option<(String, bool, crate::whisper::inner::WhisperEngine, u32)>>);

impl WhisperState {
    pub const MAX_INFERENCES_BEFORE_RELOAD: u32 = 10;
}

/// Lock-free current RMS audio level, updated by the audio callback.
/// Stored as f32 bits in an AtomicU32.
pub struct CurrentAudioLevel(pub Arc<AtomicU32>);

/// Tracks the currently active download (model name + abort flag).
pub struct ActiveDownloadState(pub Mutex<Option<(String, Arc<AtomicBool>)>>);

impl CurrentAudioLevel {
    pub fn new() -> Self {
        Self(Arc::new(AtomicU32::new(0)))
    }
    pub fn set(&self, level: f32) {
        self.0.store(level.to_bits(), Ordering::Relaxed);
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}
