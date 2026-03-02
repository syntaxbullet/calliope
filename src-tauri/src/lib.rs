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
        .manage(WhisperState(Mutex::new(None)))
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

            // Preload whisper model in background if one is configured
            if let Some(ref model_name) = s.active_model {
                let model_path = models_dir.join(format!("{}.bin", model_name));
                if model_path.exists() {
                    let model_path_str = model_path.to_string_lossy().to_string();
                    let model_name = model_name.clone();
                    let use_gpu = s.use_gpu;
                    let handle2 = handle.clone();
                    std::thread::Builder::new()
                        .name("whisper-preload".into())
                        .stack_size(64 * 1024 * 1024)
                        .spawn(move || {
                            log::info!("preloading whisper model: {model_name} (gpu={use_gpu})");
                            match whisper::inner::WhisperEngine::load(&model_path_str, use_gpu) {
                                Ok(engine) => {
                                    let ws = handle2.state::<WhisperState>();
                                    *ws.0.lock().unwrap() = Some((model_name.clone(), use_gpu, engine, 0));
                                    log::info!("whisper model preloaded: {model_name}");
                                }
                                Err(e) => {
                                    log::error!("failed to preload whisper model: {e}");
                                }
                            }
                        })
                        .expect("failed to spawn whisper-preload thread");
                }
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
