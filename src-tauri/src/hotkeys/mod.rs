/// Global hotkey manager.
///
/// Wraps tauri-plugin-global-shortcut. Supports PTT and Toggle modes.
/// On activation: starts audio capture, runs Whisper, injects text.

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::audio;
use crate::models;
use crate::settings;
use crate::state::{ActiveBufferState, ActiveSampleRate, ActiveStreamState, AppState, AppStateManager, CurrentAudioLevel, WhisperState};

/// Register hotkeys based on current settings and recording mode.
/// Called at startup and whenever hotkeys are reconfigured.
pub fn register_hotkeys(app: &AppHandle, hotkey_ptt: &str, hotkey_toggle: &str) {
    let app_ptt = app.clone();
    let app_toggle = app.clone();

    // Unregister any existing shortcuts first
    if let Err(e) = app.global_shortcut().unregister_all() {
        log::warn!("failed to unregister shortcuts: {e}");
    }

    // Push-to-talk: keydown → record, keyup → transcribe+inject
    if let Err(e) = app.global_shortcut().on_shortcut(hotkey_ptt, move |_app, _shortcut, event| {
        log::debug!("[PTT] shortcut event: {:?}", event.state());
        match event.state() {
            ShortcutState::Pressed => start_recording(&app_ptt, false),
            ShortcutState::Released => stop_and_transcribe(&app_ptt),
        }
    }) {
        log::error!("failed to register PTT hotkey '{hotkey_ptt}': {e}");
    }

    // Toggle: press once to start, press again to stop
    if let Err(e) = app.global_shortcut().on_shortcut(hotkey_toggle, move |_app, _shortcut, event| {
        if event.state() != ShortcutState::Pressed {
            return;
        }
        let state_guard = app_toggle.state::<AppStateManager>();
        match state_guard.get() {
            AppState::Idle => start_recording(&app_toggle, true),
            AppState::Recording => stop_and_transcribe(&app_toggle),
            _ => {}
        }
    }) {
        log::error!("failed to register toggle hotkey '{hotkey_toggle}': {e}");
    }
}

fn show_overlay(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("overlay") {
        // Position at top-center of the primary monitor
        if let Ok(Some(monitor)) = win.primary_monitor() {
            let screen = monitor.size();
            let scale = monitor.scale_factor();
            let win_width = (300.0 * scale) as i32;
            let x = (screen.width as i32 - win_width) / 2;
            let win_height = (64.0 * scale) as i32;
            let y = screen.height as i32 - win_height - (24.0 * scale) as i32;
            let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        }
        let _ = win.set_ignore_cursor_events(true);
        if let Err(e) = win.show() {
            log::error!("[overlay] show failed: {e}");
        }
    }
}

fn hide_overlay(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("overlay") {
        let _ = win.hide();
    }
}

fn start_recording(app: &AppHandle, is_toggle: bool) {
    log::info!("[start_recording] called, is_toggle={is_toggle}");
    log::debug!("[PTT] start_recording called");
    let state_guard = app.state::<AppStateManager>();

    // Clear the audio buffer and release excess memory from previous recordings
    {
        let buf_state = app.state::<ActiveBufferState>();
        let mut buf = buf_state.0.lock().unwrap();
        buf.clear();
        buf.shrink_to(16_000 * 30); // pre-allocate ~30s at 16kHz, release the rest
    }

    let s = settings::load(app);
    let device = audio::get_input_device(s.audio_device.as_deref());
    let device = match device {
        Some(d) => d,
        None => {
            state_guard.transition(AppState::Error("No input device found".into()), app);
            return;
        }
    };
    let config = match audio::build_stream_config(&device) {
        Some(c) => c,
        None => {
            state_guard.transition(AppState::Error("Failed to build stream config".into()), app);
            return;
        }
    };

    // Store the source sample rate for resampling after recording
    *app.state::<ActiveSampleRate>().0.lock().unwrap() = config.sample_rate.0;

    let buf_state = app.state::<ActiveBufferState>();
    let buffer = buf_state.0.clone();
    let app_audio = app.clone();

    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();

    // For toggle mode: detect silence and auto-stop
    let silence_timeout = if is_toggle {
        let t = s.silence_timeout_secs;
        log::info!("[silence] toggle mode, timeout={t}s, threshold={}", s.silence_threshold);
        if t > 0.0 { Some(t) } else { None }
    } else {
        log::info!("[silence] push-to-talk mode, silence detection disabled");
        None
    };
    let level_atomic = app.state::<CurrentAudioLevel>().0.clone();
    let app_for_autostop = app.clone();

    // Spawn a keeper thread that owns the stream for its entire lifetime.
    // This ensures the cpal::Stream is dropped on its own thread, avoiding
    // CoreAudio thread-affinity crashes on macOS.
    let join_handle = std::thread::Builder::new()
        .name("audio-keeper".into())
        .spawn(move || {
            let stream = audio::start_recording(app_audio, &device, &config, buffer);
            if stream.is_none() {
                return;
            }
            let _stream = stream.unwrap();

            if let Some(timeout_secs) = silence_timeout {
                // Poll for silence-based auto-stop
                log::info!("[silence] entering silence polling loop");
                let poll_interval = std::time::Duration::from_millis(100);
                let threshold = s.silence_threshold;
                let timeout_dur = std::time::Duration::from_secs_f32(timeout_secs);
                let mut silence_start: Option<std::time::Instant> = None;
                let mut auto_stopped = false;

                loop {
                    match stop_rx.recv_timeout(poll_interval) {
                        Ok(()) => break,                                    // manual stop
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break, // sender dropped
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}          // poll
                    }

                    let rms = f32::from_bits(level_atomic.load(std::sync::atomic::Ordering::Relaxed));
                    if rms < threshold {
                        let start = silence_start.get_or_insert_with(std::time::Instant::now);
                        if start.elapsed() >= timeout_dur {
                            log::info!("silence timeout reached ({timeout_secs}s), auto-stopping toggle recording");
                            auto_stopped = true;
                            break;
                        }
                    } else {
                        silence_start = None;
                    }
                }

                // Drop the stream before triggering transcription
                drop(_stream);

                if auto_stopped {
                    // Clear the stream handle so stop_and_transcribe won't try to join us
                    {
                        let stream_state = app_for_autostop.state::<ActiveStreamState>();
                        stream_state.0.lock().unwrap().take();
                    }
                    stop_and_transcribe(&app_for_autostop);
                }
                return;
            } else {
                // Block until stop signal (sender dropped or explicit send)
                let _ = stop_rx.recv();
            }
            // _stream is dropped here, on this thread
        })
        .expect("failed to spawn audio-keeper thread");

    let stream_state = app.state::<ActiveStreamState>();
    *stream_state.0.lock().unwrap() = Some((stop_tx, join_handle));
    state_guard.transition(AppState::Recording, app);
    show_overlay(app);
    log::debug!("[PTT] start_recording complete, now Recording");
}

fn stop_and_transcribe(app: &AppHandle) {
    log::debug!("[PTT] stop_and_transcribe called");
    let state_guard = app.state::<AppStateManager>();

    // Only proceed if we are actually recording
    let current = state_guard.get();
    if current != AppState::Recording {
        log::debug!("[PTT] not recording (state={current:?}), skipping");
        return;
    }

    // Signal the audio keeper thread to stop and wait for it to finish,
    // ensuring the cpal::Stream is fully dropped and no more audio callbacks
    // will fire before we snapshot the buffer.
    log::debug!("[PTT] stopping audio stream...");
    let join_handle = app
        .state::<ActiveStreamState>()
        .0
        .lock()
        .unwrap()
        .take()
        .map(|(_, handle)| handle); // drop stop_tx here, signalling keeper
    if let Some(handle) = join_handle {
        log::debug!("[PTT] joining audio-keeper thread...");
        let _ = handle.join();
        log::debug!("[PTT] audio-keeper thread joined");
    } else {
        log::warn!("[PTT] no active stream to stop");
    }

    state_guard.transition(AppState::Transcribing, app);

    // Snapshot the captured samples (audio-keeper is guaranteed stopped)
    let raw_samples = app.state::<ActiveBufferState>().0.lock().unwrap().clone();
    log::debug!("[PTT] buffer snapshot: {} samples", raw_samples.len());

    // Resample to 16kHz if the device used a different rate
    let source_rate = *app.state::<ActiveSampleRate>().0.lock().unwrap();
    let resampled = if source_rate != audio::TARGET_SAMPLE_RATE {
        log::debug!("[PTT] resampling from {}Hz to {}Hz", source_rate, audio::TARGET_SAMPLE_RATE);
        audio::resample(&raw_samples, source_rate, audio::TARGET_SAMPLE_RATE)
    } else {
        raw_samples.clone()
    };

    // Trim leading/trailing silence (30ms windows, RMS threshold 0.01)
    let trimmed = crate::audio::trim_silence(&resampled, 0.01, 30);
    log::debug!("[PTT] after silence trim: {} samples (trimmed {})", trimmed.len(), resampled.len() - trimmed.len());
    let samples = trimmed.to_vec();

    let s = settings::load(app);
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        log::debug!("[PTT] async transcription task started");
        let state_guard = app_handle.state::<AppStateManager>();

        if samples.is_empty() {
            log::debug!("[PTT] empty buffer, returning to Idle");
            hide_overlay(&app_handle);
            state_guard.transition(AppState::Idle, &app_handle);
            return;
        }

        let model_name = match s.active_model.clone() {
            Some(n) => n,
            None => {
                hide_overlay(&app_handle);
                state_guard.transition(AppState::Error("No model selected".into()), &app_handle);
                return;
            }
        };

        let models_dir = models::models_dir(&app_handle);
        let model_path = models_dir.join(format!("{}.bin", model_name));
        if !model_path.exists() {
            hide_overlay(&app_handle);
            state_guard.transition(
                AppState::Error(format!("Model file not found: {model_name}.bin")),
                &app_handle,
            );
            return;
        }
        let model_path_str = model_path.to_string_lossy().to_string();

        let use_gpu = s.use_gpu;
        let language = s.language.clone();
        let lang_opt = if language == "auto" { None } else { Some(language) };

        // Build the effective prompt: user prompt + dictionary words
        let effective_prompt = {
            let mut parts = Vec::new();
            if !s.whisper_prompt.is_empty() {
                parts.push(s.whisper_prompt.clone());
            }
            if !s.custom_dictionary.is_empty() {
                parts.push(format!(
                    "Vocabulary: {}.",
                    s.custom_dictionary.join(", ")
                ));
            }
            let joined = parts.join(" ");
            if joined.is_empty() { None } else { Some(joined) }
        };

        // Run synchronous whisper inference on a dedicated thread with a large stack.
        // spawn_blocking uses ~2 MB stack which is insufficient for large whisper models.
        let app_for_blocking = app_handle.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::Builder::new()
            .name("whisper-inference".into())
            .stack_size(64 * 1024 * 1024) // 64 MB
            .spawn(move || {
                log::debug!("[PTT] whisper-inference thread started");
                let whisper_state = app_for_blocking.state::<WhisperState>();
                let mut guard = match whisper_state.0.lock() {
                    Ok(g) => g,
                    Err(e) => {
                        log::error!("[PTT] WhisperState mutex poisoned: {e}");
                        let _ = tx.send(Err("WhisperState mutex poisoned".into()));
                        return;
                    }
                };

                let needs_load = match guard.as_ref() {
                    Some((loaded_name, loaded_gpu, _, count)) => {
                        loaded_name != &model_name
                            || *loaded_gpu != use_gpu
                            || *count >= WhisperState::MAX_INFERENCES_BEFORE_RELOAD
                    }
                    None => true,
                };

                if needs_load {
                    let reason = if guard.as_ref().map_or(false, |(_, _, _, c)| *c >= WhisperState::MAX_INFERENCES_BEFORE_RELOAD) {
                        "periodic refresh"
                    } else {
                        "initial load or config change"
                    };
                    log::debug!("[PTT] loading whisper model: {model_path_str} (gpu={use_gpu}, reason={reason})");
                    match crate::whisper::inner::WhisperEngine::load(&model_path_str, use_gpu) {
                        Ok(engine) => {
                            log::debug!("[PTT] model loaded successfully");
                            *guard = Some((model_name, use_gpu, engine, 0));
                        }
                        Err(e) => {
                            log::error!("[PTT] model load failed: {e}");
                            let _ = tx.send(Err(e.to_string()));
                            return;
                        }
                    }
                }

                log::debug!("[PTT] starting transcription...");
                let entry = guard.as_mut().unwrap();
                entry.3 += 1; // increment inference count
                let engine = &mut entry.2;
                let result = engine.transcribe(&samples, lang_opt.as_deref(), effective_prompt.as_deref()).map_err(|e| e.to_string());
                log::debug!("[PTT] transcription done, success={}", result.is_ok());
                let _ = tx.send(result);
            })
            .expect("failed to spawn whisper thread");

        let result = rx.await;

        let transcription = match result {
            Ok(Ok(t)) => t,
            Ok(Err(e)) => {
                hide_overlay(&app_handle);
                state_guard.transition(AppState::Error(e), &app_handle);
                return;
            }
            Err(e) => {
                hide_overlay(&app_handle);
                state_guard.transition(AppState::Error(e.to_string()), &app_handle);
                return;
            }
        };

        if transcription.text.is_empty() {
            hide_overlay(&app_handle);
            state_guard.transition(AppState::Idle, &app_handle);
            return;
        }

        // ── Post-processing (optional LLM cleanup) ─────────────────────────
        let settings = settings::load(&app_handle);
        let final_text = if settings.postprocess_enabled {
            state_guard.transition(AppState::PostProcessing, &app_handle);
            log::debug!("[PTT] running post-processing via {:?}", settings.postprocess_provider);
            let processed = crate::postprocess::process(&transcription.text, &settings).await;
            log::debug!("[PTT] post-processed text: {:?}", &processed[..processed.len().min(50)]);
            processed
        } else {
            transcription.text.clone()
        };

        log::debug!("[PTT] injecting text: {:?}", &final_text[..final_text.len().min(50)]);
        state_guard.transition(AppState::Injecting, &app_handle);

        // Run injection on a dedicated thread — enigo's CGEvent calls can crash
        // if invoked from a tokio worker thread on macOS.
        let text = final_text.clone();
        let injection_mode = settings.injection_mode.clone();
        let (inject_tx, inject_rx) = tokio::sync::oneshot::channel();
        std::thread::Builder::new()
            .name("text-injection".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(|| {
                    let injector = crate::injection::platform_injector();
                    match injection_mode {
                        crate::settings::InjectionMode::Clipboard => injector.inject(&text),
                        crate::settings::InjectionMode::Character => injector.inject_chars(&text),
                    }
                });
                let outcome = match result {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(e)) => Err(e.to_string()),
                    Err(_) => Err("text injection panicked".into()),
                };
                let _ = inject_tx.send(outcome);
            })
            .expect("failed to spawn injection thread");

        hide_overlay(&app_handle);
        match inject_rx.await {
            Ok(Ok(_)) => {
                log::debug!("[PTT] injection succeeded");
                state_guard.transition(AppState::Idle, &app_handle);
            }
            Ok(Err(e)) => {
                log::error!("[PTT] injection failed: {e}");
                // Attempt clipboard fallback regardless of mode
                log::debug!("[PTT] attempting clipboard fallback");
                match crate::injection::clipboard_fallback(&final_text) {
                    Ok(()) => {
                        let _ = app_handle.emit("clipboard-fallback", &final_text);
                        state_guard.transition(AppState::Idle, &app_handle);
                    }
                    Err(ce) => {
                        log::error!("[PTT] clipboard fallback also failed: {ce}");
                        state_guard.transition(AppState::Error(e), &app_handle);
                    }
                }
            }
            Err(_) => {
                log::error!("[PTT] injection thread dropped without responding");
                state_guard.transition(AppState::Error("injection thread crashed".into()), &app_handle);
            }
        }
    });
}
