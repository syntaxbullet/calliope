/// macOS text injection via clipboard + Cmd+V.
///
/// Algorithm:
/// 1. Save current clipboard contents
/// 2. Write transcription to pasteboard
/// 3. Simulate Cmd+V via CoreGraphics CGEvent
/// 4. Wait 150ms then restore original clipboard
///
/// Requires Accessibility permission (prompted at onboarding).

use std::time::Duration;

use arboard::Clipboard;
use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

use super::{InjectionError, Injector};

/// macOS virtual keycode for 'v'
const KV_V: CGKeyCode = 9;

pub struct MacOsInjector;

impl MacOsInjector {
    pub fn new() -> Self {
        Self
    }
}

impl Injector for MacOsInjector {
    fn inject(&self, text: &str) -> Result<(), InjectionError> {
        let mut clipboard =
            Clipboard::new().map_err(|e| InjectionError::Clipboard(e.to_string()))?;

        // Save whatever is already on the clipboard
        let prev = clipboard.get_text().ok();

        // Write the transcription
        clipboard
            .set_text(text)
            .map_err(|e| InjectionError::Clipboard(e.to_string()))?;

        // Short pause so the pasteboard write is flushed before the keystroke
        std::thread::sleep(Duration::from_millis(50));

        // Simulate Cmd+V using CoreGraphics directly
        simulate_cmd_v().map_err(|e| InjectionError::Platform(e))?;

        // Wait for the paste to land before restoring
        std::thread::sleep(Duration::from_millis(150));

        if let Some(prev_text) = prev {
            let _ = clipboard.set_text(prev_text);
        }

        Ok(())
    }
}

fn simulate_cmd_v() -> Result<(), String> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "failed to create CGEventSource")?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), KV_V, true)
        .map_err(|_| "failed to create key-down event")?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);

    let key_up = CGEvent::new_keyboard_event(source, KV_V, false)
        .map_err(|_| "failed to create key-up event")?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);

    key_down.post(core_graphics::event::CGEventTapLocation::HID);
    key_up.post(core_graphics::event::CGEventTapLocation::HID);

    Ok(())
}
