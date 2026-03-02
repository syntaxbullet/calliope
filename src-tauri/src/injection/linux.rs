/// Linux text injection with fallback chain.
///
/// Detection order (fail-fast, try next):
/// 1. wtype  — if WAYLAND_DISPLAY is set and compositor supports zwp_virtual_keyboard_v1
/// 2. ydotool — requires ydotoold daemon + /dev/uinput access
/// 3. xdotool — X11 only (detected via DISPLAY env var); skip if no DISPLAY
/// 4. AT-SPI — final fallback via atspi-2
///
/// IMPORTANT: Do NOT use xdotool exit code to detect Wayland failure.
/// Detect compositor type explicitly via WAYLAND_DISPLAY / XDG_SESSION_TYPE.

use super::{InjectionError, Injector};
use serde::Serialize;
use std::process::Command;

pub struct LinuxInjector;

impl LinuxInjector {
    pub fn new() -> Self { Self }

    fn try_wtype(&self, text: &str) -> bool {
        if std::env::var("WAYLAND_DISPLAY").is_err() {
            return false;
        }
        Command::new("wtype")
            .arg("--")
            .arg(text)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn try_ydotool(&self, text: &str) -> bool {
        Command::new("ydotool")
            .args(["type", "--", text])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn try_xdotool(&self, text: &str) -> bool {
        // Only attempt xdotool when explicitly on X11
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        let has_display = std::env::var("DISPLAY").is_ok();
        if session == "wayland" || !has_display {
            return false;
        }
        Command::new("xdotool")
            .args(["type", "--clearmodifiers", "--", text])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn try_atspi(&self, text: &str) -> bool {
        // Use blocking AT-SPI via zbus to set text on the focused editable element.
        // We run this synchronously since inject() is called from a dedicated thread.
        match atspi_insert(text) {
            Ok(()) => true,
            Err(e) => {
                log::debug!("AT-SPI injection failed: {e}");
                false
            }
        }
    }
}

/// Insert text using AT-SPI EditableText interface on the focused accessible element.
fn atspi_insert(text: &str) -> Result<(), String> {
    use zbus::blocking::Connection;

    let conn = Connection::session().map_err(|e| format!("D-Bus session connect: {e}"))?;

    // Query the AT-SPI registry for the currently focused accessible object
    let reply: (String, zbus::zvariant::OwnedObjectPath) = conn
        .call_method(
            Some("org.a11y.atspi.Registry"),
            "/org/a11y/atspi/accessible/root",
            Some("org.a11y.atspi.Registry"),
            "GetCurrentlyFocusedAccessible",
            &(),
        )
        .map_err(|e| format!("GetCurrentlyFocusedAccessible: {e}"))?
        .body()
        .deserialize()
        .map_err(|e| format!("deserialize focused: {e}"))?;

    let (bus_name, obj_path) = reply;
    if bus_name.is_empty() || obj_path.as_str() == "/" {
        return Err("no focused accessible element".into());
    }

    // Get the current caret offset via Text interface
    let caret_offset: i32 = conn
        .call_method(
            Some(bus_name.as_str()),
            obj_path.as_str(),
            Some("org.a11y.atspi.Text"),
            "GetCaretOffset",
            &(),
        )
        .and_then(|r| r.body().deserialize().map_err(Into::into))
        .unwrap_or(0);

    // Insert text at cursor via EditableText interface
    conn.call_method(
        Some(bus_name.as_str()),
        obj_path.as_str(),
        Some("org.a11y.atspi.EditableText"),
        "InsertText",
        &(caret_offset, text, text.len() as i32),
    )
    .map_err(|e| format!("InsertText: {e}"))?;

    Ok(())
}

impl Injector for LinuxInjector {
    fn inject(&self, text: &str) -> Result<(), InjectionError> {
        if self.try_wtype(text) { return Ok(()); }
        if self.try_ydotool(text) { return Ok(()); }
        if self.try_xdotool(text) { return Ok(()); }
        if self.try_atspi(text) { return Ok(()); }
        Err(InjectionError::AllMethodsFailed)
    }
}

// ── Linux injection status (onboarding) ──────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct LinuxInjectionStatus {
    pub wayland: bool,
    pub wtype_available: bool,
    pub ydotool_available: bool,
    pub ydotoold_running: bool,
    pub uinput_accessible: bool,
    pub xdotool_available: bool,
    pub recommended_action: Option<String>,
}

fn which(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn check_status() -> LinuxInjectionStatus {
    let wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
    let wtype_available = which("wtype");
    let ydotool_available = which("ydotool");
    let xdotool_available = which("xdotool");

    let ydotoold_running = Command::new("pgrep")
        .arg("ydotoold")
        .stdout(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let uinput_accessible = std::fs::metadata("/dev/uinput")
        .map(|m| {
            use std::os::unix::fs::MetadataExt;
            // Check if readable (mode has user/group/other read bit)
            m.mode() & 0o444 != 0
        })
        .unwrap_or(false);

    let recommended_action = compute_recommendation(
        wayland,
        wtype_available,
        ydotool_available,
        ydotoold_running,
        uinput_accessible,
        xdotool_available,
    );

    LinuxInjectionStatus {
        wayland,
        wtype_available,
        ydotool_available,
        ydotoold_running,
        uinput_accessible,
        xdotool_available,
        recommended_action,
    }
}

fn compute_recommendation(
    wayland: bool,
    wtype_available: bool,
    ydotool_available: bool,
    ydotoold_running: bool,
    uinput_accessible: bool,
    xdotool_available: bool,
) -> Option<String> {
    if wayland {
        if wtype_available {
            return None; // all good
        }
        if ydotool_available && ydotoold_running && uinput_accessible {
            return None; // ydotool ready
        }
        // Suggest wtype first on Wayland
        if !wtype_available {
            return Some("Install wtype for Wayland text injection: sudo apt install wtype (or your distro's equivalent)".into());
        }
        if ydotool_available && !ydotoold_running {
            return Some("Start the ydotool daemon: sudo systemctl enable --now ydotoold".into());
        }
        if ydotool_available && !uinput_accessible {
            return Some("Grant access to /dev/uinput: sudo usermod -aG input $USER (then log out and back in)".into());
        }
        return Some("Install wtype for Wayland text injection: sudo apt install wtype".into());
    }

    // X11
    if xdotool_available {
        return None;
    }
    if ydotool_available && ydotoold_running && uinput_accessible {
        return None;
    }
    if !xdotool_available {
        return Some("Install xdotool for X11 text injection: sudo apt install xdotool".into());
    }
    None
}
