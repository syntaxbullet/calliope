/// Text injection engine.
///
/// Platform-specific implementations behind a common `Injector` trait.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
pub(crate) mod linux;

#[derive(Debug, thiserror::Error)]
pub enum InjectionError {
    #[error("all injection methods failed")]
    AllMethodsFailed,
    #[error("clipboard error: {0}")]
    Clipboard(String),
    #[error("platform error: {0}")]
    Platform(String),
}

pub trait Injector: Send + Sync {
    /// Inject text via clipboard+paste (the default for most platforms).
    fn inject(&self, text: &str) -> Result<(), InjectionError>;

    /// Inject text character-by-character via simulated keystrokes.
    /// Default implementation uses enigo.
    fn inject_chars(&self, text: &str) -> Result<(), InjectionError> {
        use enigo::{Enigo, Keyboard, Settings as EnigoSettings};
        let mut enigo = Enigo::new(&EnigoSettings::default())
            .map_err(|e| InjectionError::Platform(e.to_string()))?;
        enigo.text(text)
            .map_err(|e| InjectionError::Platform(e.to_string()))?;
        Ok(())
    }
}

/// Copy text to clipboard as a last-resort fallback.
pub fn clipboard_fallback(text: &str) -> Result<(), InjectionError> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| InjectionError::Clipboard(e.to_string()))?;
    clipboard
        .set_text(text)
        .map_err(|e| InjectionError::Clipboard(e.to_string()))?;
    Ok(())
}

/// Create the appropriate injector for the current platform.
pub fn platform_injector() -> Box<dyn Injector> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacOsInjector::new());
    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsInjector::new());
    #[cfg(target_os = "linux")]
    return Box::new(linux::LinuxInjector::new());
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    compile_error!("unsupported platform");
}
