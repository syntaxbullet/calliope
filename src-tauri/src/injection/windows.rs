/// Windows text injection via UI Automation or SendInput.
///
/// Algorithm:
/// 1. Try IUIAutomation::IValueProvider::SetValue on focused element
/// 2. Fallback: clipboard swap + SendInput Ctrl+V

use super::{InjectionError, Injector};

use arboard::Clipboard;
use std::thread;
use std::time::Duration;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IValueProvider, UIA_ValuePatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_CONTROL, VK_V,
};
use windows::core::Interface;

pub struct WindowsInjector;

impl WindowsInjector {
    pub fn new() -> Self {
        Self
    }

    fn try_uia(&self, text: &str) -> Result<(), String> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().map_err(|e| e.to_string())?;

            let result = (|| -> Result<(), String> {
                let automation: IUIAutomation =
                    windows::Win32::System::Com::CoCreateInstance(
                        &CUIAutomation,
                        None,
                        windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
                    ).map_err(|e| format!("CoCreateInstance failed: {e}"))?;

                let focused = automation
                    .GetFocusedElement()
                    .map_err(|e| format!("GetFocusedElement failed: {e}"))?;

                let pattern = focused
                    .GetCurrentPattern(UIA_ValuePatternId)
                    .map_err(|e| format!("GetCurrentPattern failed: {e}"))?;

                let value_provider: IValueProvider = pattern
                    .cast()
                    .map_err(|e| format!("cast to IValueProvider failed: {e}"))?;

                let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let bstr = windows::core::BSTR::from_wide(&wide[..wide.len() - 1])
                    .map_err(|e| format!("BSTR creation failed: {e}"))?;

                value_provider
                    .SetValue(&bstr)
                    .map_err(|e| format!("SetValue failed: {e}"))?;

                Ok(())
            })();

            CoUninitialize();
            result
        }
    }

    fn clipboard_paste(&self, text: &str) -> Result<(), InjectionError> {
        let mut clipboard =
            Clipboard::new().map_err(|e| InjectionError::Clipboard(e.to_string()))?;

        // Save current clipboard content
        let saved = clipboard.get_text().ok();

        // Set our text
        clipboard
            .set_text(text)
            .map_err(|e| InjectionError::Clipboard(e.to_string()))?;

        // Send Ctrl+V
        unsafe {
            let inputs = [
                make_key_input(VK_CONTROL, false),
                make_key_input(VK_V, false),
                make_key_input(VK_V, true),
                make_key_input(VK_CONTROL, true),
            ];
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }

        // Wait for paste to complete, then restore
        thread::sleep(Duration::from_millis(150));
        if let Some(prev) = saved {
            let _ = clipboard.set_text(&prev);
        }

        Ok(())
    }
}

unsafe fn make_key_input(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
    let mut flags = windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0);
    if key_up {
        flags = KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

impl Injector for WindowsInjector {
    fn inject(&self, text: &str) -> Result<(), InjectionError> {
        // Try UIA first
        match self.try_uia(text) {
            Ok(()) => {
                log::debug!("Windows injection: UIA SetValue succeeded");
                return Ok(());
            }
            Err(e) => {
                log::debug!("Windows injection: UIA failed ({e}), falling back to clipboard+SendInput");
            }
        }

        // Fallback to clipboard paste
        self.clipboard_paste(text)
    }
}
