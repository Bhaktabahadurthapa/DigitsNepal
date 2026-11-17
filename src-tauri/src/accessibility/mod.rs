//! Text access in other apps through the macOS Accessibility API.
//!
//! * Shortcut / typing button: the focused field's *selected* text is read
//!   (or, when nothing is selected, the whole field, which is then selected
//!   so the user sees exactly what will be replaced).
//! * Typing button (opt-in setting, off by default): while it is on, the
//!   focused text field's role, position and the end of its text are checked
//!   locally a few times a second to decide whether to show the button.
//!   Nothing is stored or sent until the user clicks it.
//!
//! No keystrokes are observed, and secure (password) input is refused.

pub mod safety;
pub use safety::TextAccessError;

/// A captured selection in another app.
pub struct Capture {
    pub text: String,
    pub pid: i32,
    /// Selection range (location, length) in UTF-16 units, if the app exposes it.
    pub range: Option<(isize, isize)>,
    #[cfg(target_os = "macos")]
    element: mac::Element,
}

/// What we inserted, so it can be undone safely.
pub struct Replacement {
    pub original: String,
    pub inserted: String,
    pub location: isize,
    pub pid: i32,
    #[cfg(target_os = "macos")]
    element: mac::Element,
}

/// A text field the user is typing in, in another app (for the typing button).
pub struct Target {
    pub pid: i32,
    /// Screen frame in points: x, y (top-left), width, height.
    pub frame: (f64, f64, f64, f64),
    /// The last part of the field's text, for local language detection only.
    pub tail: String,
    #[cfg(target_os = "macos")]
    element: mac::Element,
}

#[cfg(target_os = "macos")]
mod mac {
    use super::safety::*;
    use super::{Capture, Replacement, Target};
    use accessibility_sys::*;
    use core_foundation::base::{CFRelease, CFRetain, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::{CFString, CFStringRef};
    use std::ffi::c_void;
    use std::ptr;

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn IsSecureEventInputEnabled() -> bool;
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct CFRange {
        location: isize,
        length: isize,
    }

    /// Owned, retained AXUIElementRef. AX elements are CF objects that are
    /// safe to message from any thread; we serialise access via a Mutex.
    pub struct Element(AXUIElementRef);
    unsafe impl Send for Element {}
    unsafe impl Sync for Element {}
    impl Drop for Element {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0 as CFTypeRef) }
            }
        }
    }
    impl Element {
        fn clone_ref(&self) -> Element {
            unsafe { CFRetain(self.0 as CFTypeRef) };
            Element(self.0)
        }
    }

    fn attr(el: AXUIElementRef, name: &str) -> Option<CFTypeRef> {
        let name = CFString::new(name);
        let mut out: CFTypeRef = ptr::null();
        let err = unsafe { AXUIElementCopyAttributeValue(el, name.as_concrete_TypeRef(), &mut out) };
        (err == kAXErrorSuccess && !out.is_null()).then_some(out)
    }

    fn cf_to_string(v: CFTypeRef) -> Option<String> {
        unsafe {
            if core_foundation::base::CFGetTypeID(v) == core_foundation::string::CFString::type_id() {
                Some(CFString::wrap_under_create_rule(v as CFStringRef).to_string())
            } else {
                CFRelease(v);
                None
            }
        }
    }

    fn string_attr(el: AXUIElementRef, name: &str) -> Option<String> {
        attr(el, name).and_then(cf_to_string)
    }

    fn range_attr(el: AXUIElementRef) -> Option<(isize, isize)> {
        let v = attr(el, "AXSelectedTextRange")?;
        let mut r = CFRange::default();
        let ok = unsafe { AXValueGetValue(v as AXValueRef, kAXValueTypeCFRange, &mut r as *mut _ as *mut c_void) };
        unsafe { CFRelease(v) };
        ok.then_some((r.location, r.length))
    }

    fn string_for_range(el: AXUIElementRef, loc: isize, len: isize) -> Option<String> {
        let r = CFRange {
            location: loc,
            length: len,
        };
        let val = unsafe { AXValueCreate(kAXValueTypeCFRange, &r as *const _ as *const c_void) };
        if val.is_null() {
            return None;
        }
        let name = CFString::new("AXStringForRange");
        let mut out: CFTypeRef = ptr::null();
        let err = unsafe {
            AXUIElementCopyParameterizedAttributeValue(el, name.as_concrete_TypeRef(), val as CFTypeRef, &mut out)
        };
        unsafe { CFRelease(val as CFTypeRef) };
        if err != kAXErrorSuccess || out.is_null() {
            return None;
        }
        cf_to_string(out)
    }

    fn set_range(el: AXUIElementRef, loc: isize, len: isize) -> bool {
        let r = CFRange {
            location: loc,
            length: len,
        };
        let val = unsafe { AXValueCreate(kAXValueTypeCFRange, &r as *const _ as *const c_void) };
        if val.is_null() {
            return false;
        }
        let name = CFString::new("AXSelectedTextRange");
        let err = unsafe { AXUIElementSetAttributeValue(el, name.as_concrete_TypeRef(), val as CFTypeRef) };
        unsafe { CFRelease(val as CFTypeRef) };
        err == kAXErrorSuccess
    }

    fn set_selected_text(el: AXUIElementRef, text: &str) -> bool {
        let name = CFString::new("AXSelectedText");
        let v = CFString::new(text);
        let err = unsafe { AXUIElementSetAttributeValue(el, name.as_concrete_TypeRef(), v.as_CFTypeRef()) };
        err == kAXErrorSuccess
    }

    pub fn is_trusted(prompt: bool) -> bool {
        unsafe {
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let dict = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::from(prompt).as_CFType())]);
            AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef())
        }
    }

    pub fn secure_input_active() -> bool {
        unsafe { IsSecureEventInputEnabled() }
    }

    fn focused_element() -> Result<Element, TextAccessError> {
        let sys = unsafe { AXUIElementCreateSystemWide() };
        if sys.is_null() {
            return Err(TextAccessError::Unsupported);
        }
        let sys = Element(sys);
        unsafe { AXUIElementSetMessagingTimeout(sys.0, 1.0) };
        let name = CFString::new("AXFocusedUIElement");
        let mut out: CFTypeRef = ptr::null();
        let err = unsafe { AXUIElementCopyAttributeValue(sys.0, name.as_concrete_TypeRef(), &mut out) };
        match err {
            e if e == kAXErrorSuccess && !out.is_null() => {
                let el = Element(out as AXUIElementRef);
                unsafe { AXUIElementSetMessagingTimeout(el.0, 1.0) };
                Ok(el)
            }
            e if e == kAXErrorAPIDisabled => Err(TextAccessError::PermissionRequired),
            _ => Err(TextAccessError::Unsupported),
        }
    }

    fn is_secure(el: &Element) -> bool {
        secure_input_active()
            || is_secure_role(
                string_attr(el.0, "AXRole").as_deref(),
                string_attr(el.0, "AXSubrole").as_deref(),
            )
    }

    pub fn capture() -> Result<Capture, TextAccessError> {
        if !is_trusted(false) {
            return Err(TextAccessError::PermissionRequired);
        }
        if secure_input_active() {
            return Err(TextAccessError::SecureInput);
        }
        let el = focused_element()?;
        let mut pid = 0;
        unsafe { AXUIElementGetPid(el.0, &mut pid) };
        if pid == std::process::id() as i32 {
            // Our own window is focused; nothing to capture from another app.
            return Err(TextAccessError::NoSelection);
        }
        capture_from(el, pid)
    }

    /// Capture from the field the typing button was shown for.
    pub fn capture_target(t: &Target) -> Result<Capture, TextAccessError> {
        if !is_trusted(false) {
            return Err(TextAccessError::PermissionRequired);
        }
        if secure_input_active() {
            return Err(TextAccessError::SecureInput);
        }
        capture_from(t.element.clone_ref(), t.pid)
    }

    /// The selection, or (if nothing is selected) the whole field, selected
    /// first so replacing goes through the same safety checks.
    fn capture_from(el: Element, pid: i32) -> Result<Capture, TextAccessError> {
        if is_secure(&el) {
            return Err(TextAccessError::SecureInput);
        }
        match accept_selection(string_attr(el.0, "AXSelectedText")) {
            Ok(text) => Ok(Capture {
                text,
                pid,
                range: range_attr(el.0),
                element: el,
            }),
            Err(TextAccessError::NoSelection) => {
                let value = string_attr(el.0, "AXValue").ok_or(TextAccessError::NoSelection)?;
                let text = accept_selection(Some(value.clone()))?;
                if value.chars().count() > 4000 {
                    return Err(TextAccessError::NoSelection);
                }
                let len = utf16_len(&value);
                if !set_range(el.0, 0, len) || string_attr(el.0, "AXSelectedText").as_deref() != Some(value.as_str()) {
                    return Err(TextAccessError::Unsupported);
                }
                Ok(Capture {
                    text,
                    pid,
                    range: Some((0, len)),
                    element: el,
                })
            }
            Err(e) => Err(e),
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Pt {
        x: f64,
        y: f64,
    }

    fn point_attr(el: AXUIElementRef, name: &str, kind: AXValueType) -> Option<(f64, f64)> {
        let v = attr(el, name)?;
        let mut p = Pt::default();
        let ok = unsafe { AXValueGetValue(v as AXValueRef, kind, &mut p as *mut _ as *mut c_void) };
        unsafe { CFRelease(v) };
        ok.then_some((p.x, p.y))
    }

    const TEXT_ROLES: &[&str] = &["AXTextField", "AXTextArea", "AXComboBox"];

    /// The focused, non-secure text field in another app, if any.
    pub fn typing_target() -> Option<Target> {
        if !is_trusted(false) || secure_input_active() {
            return None;
        }
        let el = focused_element().ok()?;
        let mut pid = 0;
        unsafe { AXUIElementGetPid(el.0, &mut pid) };
        if pid == 0 || pid == std::process::id() as i32 || is_secure(&el) {
            return None;
        }
        let role = string_attr(el.0, "AXRole")?;
        if !TEXT_ROLES.contains(&role.as_str()) {
            return None;
        }
        let value = string_attr(el.0, "AXValue")?;
        if value.trim().is_empty() {
            return None;
        }
        let chars: Vec<char> = value.chars().collect();
        let tail: String = chars[chars.len().saturating_sub(400)..].iter().collect();
        let (x, y) = point_attr(el.0, "AXPosition", kAXValueTypeCGPoint)?;
        let (w, h) = point_attr(el.0, "AXSize", kAXValueTypeCGSize)?;
        Some(Target {
            pid,
            frame: (x, y, w, h),
            tail,
            element: el,
        })
    }

    pub fn activate(pid: i32) {
        if pid <= 0 {
            return;
        }
        let app = unsafe { AXUIElementCreateApplication(pid) };
        if app.is_null() {
            return;
        }
        let app = Element(app);
        let name = CFString::new("AXFrontmost");
        unsafe {
            AXUIElementSetAttributeValue(
                app.0,
                name.as_concrete_TypeRef(),
                CFBoolean::true_value().as_CFTypeRef(),
            )
        };
    }

    pub fn replace(c: &Capture, new_text: &str) -> Result<Replacement, TextAccessError> {
        let current = string_attr(c.element.0, "AXSelectedText");
        check_replacement(&c.text, current.as_deref(), new_text, is_secure(&c.element))?;
        if !set_selected_text(c.element.0, new_text) {
            return Err(TextAccessError::ReplaceFailed);
        }
        // Verify where possible: the inserted text should now be at the
        // original selection start.
        let location = c.range.map(|r| r.0).unwrap_or(-1);
        if location >= 0 {
            if let Some(found) = string_for_range(c.element.0, location, utf16_len(new_text)) {
                if found != new_text {
                    log::warn!("replacement verification mismatch");
                }
            }
        }
        Ok(Replacement {
            original: c.text.clone(),
            inserted: new_text.to_string(),
            location,
            pid: c.pid,
            element: c.element.clone_ref(),
        })
    }

    pub fn undo(r: &Replacement) -> Result<(), TextAccessError> {
        if r.location < 0 {
            return Err(TextAccessError::NothingToUndo);
        }
        let len = utf16_len(&r.inserted);
        check_undo(&r.inserted, string_for_range(r.element.0, r.location, len).as_deref())?;
        if is_secure(&r.element) {
            return Err(TextAccessError::SecureInput);
        }
        if !set_range(r.element.0, r.location, len) || !set_selected_text(r.element.0, &r.original) {
            return Err(TextAccessError::ReplaceFailed);
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub use mac::{activate, capture, capture_target, is_trusted, replace, secure_input_active, typing_target, undo};

#[cfg(not(target_os = "macos"))]
mod other {
    use super::*;
    pub fn is_trusted(_: bool) -> bool {
        false
    }
    pub fn secure_input_active() -> bool {
        false
    }
    pub fn capture() -> Result<Capture, TextAccessError> {
        Err(TextAccessError::NotMacOs)
    }
    pub fn capture_target(_: &Target) -> Result<Capture, TextAccessError> {
        Err(TextAccessError::NotMacOs)
    }
    pub fn typing_target() -> Option<Target> {
        None
    }
    pub fn activate(_: i32) {}
    pub fn replace(_: &Capture, _: &str) -> Result<Replacement, TextAccessError> {
        Err(TextAccessError::NotMacOs)
    }
    pub fn undo(_: &Replacement) -> Result<(), TextAccessError> {
        Err(TextAccessError::NotMacOs)
    }
}
#[cfg(not(target_os = "macos"))]
pub use other::*;

/// Open System Settings › Privacy & Security › Accessibility.
pub fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        // Constant URL, no user input: no injection surface.
        let _ = std::process::Command::new("/usr/bin/open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}
