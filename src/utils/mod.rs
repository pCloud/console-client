//! Utility functions and helpers.
//!
//! This module provides:
//! - C string conversion utilities
//! - Path handling helpers
//! - Terminal output utilities
//! - QR code generation
//! - Browser opening utilities
//!
//! # Submodules
//!
//! - `cstring`: C string conversion helpers for FFI operations
//! - `terminal`: Rich terminal output (boxes, status indicators)
//! - `qrcode`: QR code generation for terminal display
//! - `browser`: Browser opening utilities

pub mod browser;
pub mod cstring;
pub mod mount;
pub mod qrcode;
pub mod terminal;

// Re-export commonly used items from cstring module
pub use cstring::{from_cstr_owned, from_cstr_ref, to_cstring, try_to_cstring};

// Re-export mountpoint utility
pub use mount::ensure_mountpoint;

// Re-export commonly used terminal utilities
pub use terminal::{print_boxed, print_status, StatusIndicator};

use std::ffi::CStr;
use std::os::raw::c_char;

/// Convert a C string pointer to a Rust String.
///
/// # Safety
///
/// The pointer must be valid and point to a null-terminated C string.
/// The string must be valid UTF-8.
///
/// # Returns
///
/// Returns `None` if the pointer is null or the string is not valid UTF-8.
///
/// # Example
///
/// ```ignore
/// use console_client::utils::from_c_str;
///
/// let rust_string = unsafe { from_c_str(c_ptr) };
/// ```
pub unsafe fn from_c_str(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    CStr::from_ptr(ptr).to_str().ok().map(String::from)
}

/// Convert a C string pointer to a Rust &str.
///
/// # Safety
///
/// The pointer must be valid and point to a null-terminated C string.
/// The string must be valid UTF-8.
/// The returned reference is only valid as long as the C string memory is valid.
///
/// # Returns
///
/// Returns `None` if the pointer is null or the string is not valid UTF-8.
pub unsafe fn from_c_str_ref<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }

    CStr::from_ptr(ptr).to_str().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_cstring() {
        let c_str = to_cstring("hello").unwrap();
        assert_eq!(c_str.as_bytes(), b"hello");
    }

    #[test]
    fn test_to_cstring_with_null() {
        let result = to_cstring("hel\0lo");
        assert!(result.is_none());
    }

    #[test]
    fn test_from_c_str_null() {
        let result = unsafe { from_c_str(std::ptr::null()) };
        assert!(result.is_none());
    }

    #[test]
    fn test_from_c_str_valid() {
        let c_string = to_cstring("hello").unwrap();
        let result = unsafe { from_c_str(c_string.as_ptr()) };
        assert_eq!(result, Some(String::from("hello")));
    }
}
