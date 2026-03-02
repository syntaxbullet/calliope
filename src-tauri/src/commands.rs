/// Tauri IPC command handlers.
///
/// All commands callable from the frontend via `invoke(...)`.

#[allow(unused_imports)]
use tauri::{AppHandle, Manager, State};
use crate::audio;
use crate::models;
use crate::settings::{self, Settings};
use crate::state::{AppState, AppStateManager};

// ── Settings ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    settings::load(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    // Apply debug log level change immediately
    log::set_max_level(if settings.debug_logs {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    });
    settings::save(&app, &settings).map_err(|e| e.to_string())
}

// ── Launch at Login ──────────────────────────────────────────────────────

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable().map_err(|e| e.to_string())?;
    } else {
        autostart.disable().map_err(|e| e.to_string())?;
    }
    let mut s = settings::load(&app);
    s.launch_at_login = enabled;
    settings::save(&app, &s).map_err(|e| e.to_string())
}

// ── App State ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_app_state(state: State<'_, AppStateManager>) -> AppState {
    state.get()
}

// ── Audio Devices ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_audio_devices() -> Vec<audio::AudioDevice> {
    audio::list_input_devices()
}

// ── Models ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_models(app: AppHandle) -> Vec<models::ModelInfo> {
    let settings = settings::load(&app);
    models::hydrate_models(&app, settings.active_model.as_deref())
}

#[tauri::command]
pub fn set_active_model(app: AppHandle, name: Option<String>) -> Result<(), String> {
    let mut s = settings::load(&app);
    s.active_model = name;
    settings::save(&app, &s).map_err(|e| e.to_string())
}

// ── Model Download / Delete ───────────────────────────────────────────────

#[tauri::command]
pub async fn download_model(app: AppHandle, name: String) -> Result<(), String> {
    let abort_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let dl_state = app.state::<crate::state::ActiveDownloadState>();
        *dl_state.0.lock().unwrap() = Some((name.clone(), abort_flag.clone()));
    }
    let result = models::download_model_file(&app, &name, abort_flag).await;
    {
        let dl_state = app.state::<crate::state::ActiveDownloadState>();
        *dl_state.0.lock().unwrap() = None;
    }
    result
}

#[tauri::command]
pub fn cancel_download(app: AppHandle) -> Result<(), String> {
    let dl_state = app.state::<crate::state::ActiveDownloadState>();
    let guard = dl_state.0.lock().unwrap();
    if let Some((_, ref flag)) = *guard {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    } else {
        Err("no active download".into())
    }
}

#[tauri::command]
pub fn delete_model(app: AppHandle, name: String) -> Result<(), String> {
    let s = settings::load(&app);
    if s.active_model.as_deref() == Some(&name) {
        let mut s = s;
        s.active_model = None;
        settings::save(&app, &s).map_err(|e| e.to_string())?;
    }
    models::delete_model_file(&app, &name)
}

// ── Audio Level ──────────────────────────────────────────────────────

#[tauri::command]
pub fn get_audio_level(level: State<'_, crate::state::CurrentAudioLevel>) -> f32 {
    level.get()
}

// ── Hotkeys ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn update_hotkeys(
    app: AppHandle,
    hotkey_ptt: String,
    hotkey_toggle: String,
) -> Result<(), String> {
    let mut s = settings::load(&app);
    s.hotkey_ptt = hotkey_ptt.clone();
    s.hotkey_toggle = hotkey_toggle.clone();
    settings::save(&app, &s).map_err(|e| e.to_string())?;
    crate::hotkeys::register_hotkeys(&app, &hotkey_ptt, &hotkey_toggle);
    Ok(())
}

// ── Acceleration ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_acceleration_backend() -> String {
    models::detect_gpu_backend()
}

#[tauri::command]
pub async fn download_gpu_backend(#[allow(unused)] app: AppHandle, backend: Option<String>) -> Result<String, String> {
    // On macOS, Metal is already bundled as the sidecar — no download needed
    #[cfg(target_os = "macos")]
    {
        let _ = backend;
        return Ok("Metal".into());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let backend = backend.unwrap_or_else(|| models::detect_gpu_backend());
        if backend == "CPU" {
            return Ok("CPU".into());
        }
        models::download_gpu_whisper_cli(&app, &backend).await?;

        // Update WhisperState to point to the new binary
        let bin_dir = app.path().app_data_dir()
            .expect("app data dir unavailable")
            .join("bin");
        let bin_name = if cfg!(target_os = "windows") { "whisper-cli.exe" } else { "whisper-cli" };
        let ws = app.state::<crate::state::WhisperState>();
        *ws.binary_path.lock().unwrap() = bin_dir.join(bin_name);

        Ok(backend)
    }
}

// ── Accessibility (macOS) ────────────────────────────────────────────────

#[tauri::command]
pub fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::accessibility::is_trusted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

// ── Linux Injection Status ────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct LinuxInjectionStatus {
    pub wayland: bool,
    pub wtype_available: bool,
    pub ydotool_available: bool,
    pub ydotoold_running: bool,
    pub uinput_accessible: bool,
    pub xdotool_available: bool,
    pub recommended_action: Option<String>,
}

#[tauri::command]
pub fn check_linux_injection_status() -> Option<LinuxInjectionStatus> {
    #[cfg(target_os = "linux")]
    {
        let s = crate::injection::linux::check_status();
        Some(LinuxInjectionStatus {
            wayland: s.wayland,
            wtype_available: s.wtype_available,
            ydotool_available: s.ydotool_available,
            ydotoold_running: s.ydotoold_running,
            uinput_accessible: s.uinput_accessible,
            xdotool_available: s.xdotool_available,
            recommended_action: s.recommended_action,
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

// ── API Key Management ───────────────────────────────────────────────────

#[tauri::command]
pub fn save_api_key(provider: String, key: String) -> Result<(), String> {
    let entry = keyring::Entry::new("com.syntaxbullet.calliope", &provider)
        .map_err(|e| e.to_string())?;
    entry.set_password(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_api_key(provider: String) -> Result<Option<String>, String> {
    let entry = keyring::Entry::new("com.syntaxbullet.calliope", &provider)
        .map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(pw) => Ok(Some(pw)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn delete_api_key(provider: String) -> Result<(), String> {
    let entry = keyring::Entry::new("com.syntaxbullet.calliope", &provider)
        .map_err(|e| e.to_string())?;
    match entry.delete_password() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn request_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::accessibility::prompt_if_needed()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}
