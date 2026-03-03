#[cfg(target_os = "macos")]
pub mod accessibility;
pub mod audio;
pub mod commands;
pub mod hotkeys;
pub mod injection;
pub mod models;
pub mod postprocess;
pub mod settings;
pub mod state;
pub mod tray;
pub mod whisper;

use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
use tauri::Manager;
use state::{ActiveBufferState, ActiveDownloadState, ActiveSampleRate, ActiveStreamState, AppStateManager, CurrentAudioLevel, WhisperState};
use tray::TrayIconState;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Init logger at info level; setup hook will promote to debug if setting is on
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .manage(AppStateManager::new())
        .manage(ActiveStreamState(Mutex::new(None)))
        .manage(ActiveBufferState(Arc::new(Mutex::new(Vec::new()))))
        .manage(WhisperState { binary_path: Mutex::new(std::path::PathBuf::new()) })
        .manage(ActiveSampleRate(Mutex::new(16_000)))
        .manage(CurrentAudioLevel::new())
        .manage(TrayIconState(Mutex::new(None)))
        .manage(ActiveDownloadState(Mutex::new(None)))
        .setup(|app| {
            let handle = app.handle().clone();
            let s = settings::load(&handle);

            // Apply debug log level from settings
            if s.debug_logs {
                log::set_max_level(log::LevelFilter::Debug);
            }

            // Sync launch-at-login with persisted setting
            {
                use tauri_plugin_autostart::ManagerExt;
                let autostart = app.autolaunch();
                if s.launch_at_login {
                    let _ = autostart.enable();
                } else {
                    let _ = autostart.disable();
                }
            }

            // Register global hotkeys
            hotkeys::register_hotkeys(&handle, &s.hotkey_ptt, &s.hotkey_toggle);

            // Ensure models directory exists
            let models_dir = models::models_dir(&handle);
            std::fs::create_dir_all(&models_dir)
                .expect("failed to create models directory");

            // Resolve whisper-cli binary path:
            // - macOS: bundled sidecar is already Metal-accelerated
            // - Windows/Linux: prefer GPU binary in app data dir (downloaded at
            //   runtime), fall back to bundled CPU sidecar
            {
                let bin_dir = handle.path().app_data_dir()
                    .expect("app data dir unavailable")
                    .join("bin");
                let bin_name = if cfg!(target_os = "windows") { "whisper-cli.exe" } else { "whisper-cli" };
                let gpu_bin = bin_dir.join(bin_name);
                let binary_path = if gpu_bin.exists() {
                    gpu_bin
                } else {
                    // Bundled sidecar — Tauri places it next to the app binary
                    let exe_dir = std::env::current_exe()
                        .expect("failed to get exe path")
                        .parent()
                        .expect("exe has no parent dir")
                        .to_path_buf();
                    exe_dir.join(bin_name)
                };
                log::info!("whisper-cli binary: {}", binary_path.display());
                let ws = handle.state::<WhisperState>();
                *ws.binary_path.lock().unwrap() = binary_path;
            }

            // Hide dock icon on macOS (become accessory app)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Setup tray icon
            tray::setup(&handle)?;

            log::info!("Calliope started");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide windows instead of closing them to keep the app in tray
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::get_app_state,
            commands::list_audio_devices,
            commands::list_models,
            commands::set_active_model,
            commands::update_hotkeys,
            commands::download_model,
            commands::cancel_download,
            commands::get_audio_level,
            commands::delete_model,
            commands::get_acceleration_backend,
            commands::check_accessibility,
            commands::request_accessibility,
            commands::check_linux_injection_status,
            commands::save_api_key,
            commands::get_api_key,
            commands::delete_api_key,
            commands::set_launch_at_login,
            commands::download_gpu_backend,
            commands::check_gpu_binary_installed,
            commands::check_whisper_cli_available,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
