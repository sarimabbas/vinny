use objc2_application_services::{
    AXIsProcessTrusted, AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFString};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

#[derive(Debug, Clone, Copy)]
pub struct Permissions {
    pub screen_recording: bool,
    pub accessibility: bool,
}

impl Permissions {
    pub fn granted(self) -> bool {
        self.screen_recording && self.accessibility
    }
}

pub fn check() -> Permissions {
    Permissions {
        screen_recording: CGPreflightScreenCaptureAccess(),
        accessibility: unsafe { AXIsProcessTrusted() },
    }
}

pub fn request_screen_recording() {
    if !CGPreflightScreenCaptureAccess() {
        CGRequestScreenCaptureAccess();
    }
}

pub fn request_accessibility() {
    if !unsafe { AXIsProcessTrusted() } {
        let key: &CFString = unsafe { kAXTrustedCheckOptionPrompt };
        let value = CFBoolean::new(true);
        let options = CFDictionary::from_slices(&[key], &[value]);
        unsafe {
            AXIsProcessTrustedWithOptions(Some(options.as_opaque()));
        }
    }
}
