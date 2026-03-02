/// macOS Accessibility permission helpers.
///
/// Uses the ApplicationServices framework to check and prompt for
/// the Accessibility trust required by CGEvent text injection.

#[cfg(target_os = "macos")]
mod inner {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: core_foundation::base::CFTypeRef) -> bool;
    }

    /// Returns true if the current process already has Accessibility permission.
    pub fn is_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    /// If not already trusted, shows the system prompt asking the user to grant
    /// Accessibility permission. Returns the trust status after prompting.
    pub fn prompt_if_needed() -> bool {
        unsafe {
            let key = CFString::new("AXTrustedCheckOptionPrompt");
            let value = CFBoolean::true_value();
            let options = CFDictionary::from_CFType_pairs(&[(key, value)]);
            AXIsProcessTrustedWithOptions(options.as_CFTypeRef())
        }
    }
}

#[cfg(target_os = "macos")]
pub use inner::*;
