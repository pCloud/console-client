//! C string conversion utilities for FFI operations.
//!
//! This module provides safe helpers for converting between Rust strings and C strings.
//! These utilities are essential for interacting with the pclsync C library.
//!
//! # Examples
//!
//! ```
//! use console_client::utils::cstring::{to_cstring, from_cstr_owned, from_cstr_ref};
//!
//! // Convert Rust string to CString for passing to C
//! let c_str = to_cstring("hello").expect("valid string");
//!
//! // The CString can be passed to C functions via .as_ptr()
//! // let result = unsafe { some_c_function(c_str.as_ptr()) };
//! ```
//!
//! # Safety Considerations
//!
//! When working with C strings:
//! - Always check for null pointers before dereferencing
//! - Be aware of string ownership - who allocates and who frees
//! - C strings from the library may need to be freed with `psync_free()`
//! - Rust strings containing null bytes cannot be converted to C strings

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Convert a Rust string slice to a `CString`.
///
/// This creates a new heap-allocated C string from the given Rust string.
/// The returned `CString` owns the memory and will free it when dropped.
///
/// # Arguments
///
/// * `s` - The Rust string to convert
///
/// # Returns
///
/// - `Some(CString)` if the string is valid (contains no null bytes)
/// - `None` if the string contains embedded null bytes
///
/// # Examples
///
/// ```
/// use console_client::utils::cstring::to_cstring;
///
/// let c_str = to_cstring("hello").unwrap();
/// assert_eq!(c_str.as_bytes(), b"hello");
///
/// // String with embedded null byte fails
/// assert!(to_cstring("hel\0lo").is_none());
/// ```
#[inline]
pub fn to_cstring(s: &str) -> Option<CString> {
    CString::new(s).ok()
}

/// Convert a Rust string slice to a `CString`, returning a `Result`.
///
/// This is an alternative to `to_cstring` that returns an error type
/// instead of `Option`, which can be more useful in error handling chains.
///
/// # Arguments
///
/// * `s` - The Rust string to convert
///
/// # Returns
///
/// - `Ok(CString)` if the string is valid
/// - `Err(NulError)` if the string contains embedded null bytes
///
/// # Examples
///
/// ```
/// use console_client::utils::cstring::try_to_cstring;
///
/// let c_str = try_to_cstring("hello")?;
/// # Ok::<(), std::ffi::NulError>(())
/// ```
#[inline]
pub fn try_to_cstring(s: &str) -> Result<CString, std::ffi::NulError> {
    CString::new(s)
}

/// Convert a C string pointer to an owned Rust `String`.
///
/// This creates a new heap-allocated `String` by copying the contents
/// of the C string. The original C string is not modified or freed.
///
/// # Safety
///
/// - The pointer must be valid and point to a null-terminated C string
/// - The pointer must remain valid for the duration of this function call
/// - The C string must be valid UTF-8
///
/// # Arguments
///
/// * `ptr` - Pointer to a null-terminated C string
///
/// # Returns
///
/// - `Some(String)` if the pointer is non-null and contains valid UTF-8
/// - `None` if the pointer is null or the string is not valid UTF-8
///
/// # Examples
///
/// ```
/// use std::ffi::CString;
/// use console_client::utils::cstring::from_cstr_owned;
///
/// let c_string = CString::new("hello").unwrap();
/// let rust_string = unsafe { from_cstr_owned(c_string.as_ptr()) };
/// assert_eq!(rust_string, Some(String::from("hello")));
///
/// // Null pointer returns None
/// let result = unsafe { from_cstr_owned(std::ptr::null()) };
/// assert!(result.is_none());
/// ```
#[inline]
pub unsafe fn from_cstr_owned(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    CStr::from_ptr(ptr).to_str().ok().map(String::from)
}

/// Convert a C string pointer to a borrowed Rust `&str`.
///
/// This creates a borrowed reference to the C string's contents without
/// copying. The returned reference is only valid as long as the underlying
/// C string memory remains valid.
///
/// # Safety
///
/// - The pointer must be valid and point to a null-terminated C string
/// - The C string memory must remain valid for the lifetime `'a`
/// - The C string must be valid UTF-8
///
/// # Arguments
///
/// * `ptr` - Pointer to a null-terminated C string
///
/// # Returns
///
/// - `Some(&str)` if the pointer is non-null and contains valid UTF-8
/// - `None` if the pointer is null or the string is not valid UTF-8
///
/// # Examples
///
/// ```
/// use std::ffi::CString;
/// use console_client::utils::cstring::from_cstr_ref;
///
/// let c_string = CString::new("hello").unwrap();
/// let rust_str = unsafe { from_cstr_ref(c_string.as_ptr()) };
/// assert_eq!(rust_str, Some("hello"));
/// ```
#[inline]
pub unsafe fn from_cstr_ref<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }

    CStr::from_ptr(ptr).to_str().ok()
}

/// Convert a C string pointer to a Rust `String`, with lossy UTF-8 handling.
///
/// This is similar to `from_cstr_owned` but handles invalid UTF-8 by replacing
/// invalid sequences with the Unicode replacement character (U+FFFD).
///
/// # Safety
///
/// - The pointer must be valid and point to a null-terminated C string
/// - The pointer must remain valid for the duration of this function call
///
/// # Arguments
///
/// * `ptr` - Pointer to a null-terminated C string
///
/// # Returns
///
/// - `Some(String)` if the pointer is non-null (possibly with replacement characters)
/// - `None` if the pointer is null
///
/// # Examples
///
/// ```
/// use std::ffi::CString;
/// use console_client::utils::cstring::from_cstr_lossy;
///
/// let c_string = CString::new("hello").unwrap();
/// let rust_string = unsafe { from_cstr_lossy(c_string.as_ptr()) };
/// assert_eq!(rust_string, Some(String::from("hello")));
/// ```
#[inline]
pub unsafe fn from_cstr_lossy(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
}

/// Convert a C string pointer to an owned `String` and free the C memory.
///
/// This is useful when the pclsync library returns a string that must be
/// freed by the caller using `psync_free()`.
///
/// # Safety
///
/// - The pointer must be valid and point to a null-terminated C string
/// - The memory must have been allocated by the pclsync library (via `psync_malloc`)
/// - The caller transfers ownership - the memory will be freed
///
/// # Arguments
///
/// * `ptr` - Pointer to a null-terminated C string allocated by pclsync
/// * `free_fn` - Function to free the memory (typically `psync_free`)
///
/// # Returns
///
/// - `Some(String)` if the pointer is non-null and contains valid UTF-8
/// - `None` if the pointer is null or the string is not valid UTF-8
///
/// # Examples
///
/// ```ignore
/// use console_client::utils::cstring::from_cstr_and_free;
/// use console_client::ffi::raw::psync_free;
///
/// let ptr = unsafe { psync_get_username() };
/// let username = unsafe { from_cstr_and_free(ptr, psync_free) };
/// ```
pub unsafe fn from_cstr_and_free<F>(ptr: *mut c_char, free_fn: F) -> Option<String>
where
    F: FnOnce(*mut std::ffi::c_void),
{
    if ptr.is_null() {
        return None;
    }

    // First, copy the string contents
    let result = CStr::from_ptr(ptr).to_str().ok().map(String::from);

    // Then free the C memory
    free_fn(ptr as *mut std::ffi::c_void);

    result
}

/// A wrapper type for C strings that need to be freed with a specific function.
///
/// This provides RAII-style management for C strings allocated by the pclsync
/// library. When dropped, it automatically frees the underlying C memory.
///
/// # Examples
///
/// ```ignore
/// use console_client::utils::cstring::OwnedCString;
/// use console_client::ffi::raw::{psync_get_username, psync_free};
///
/// let username = unsafe {
///     OwnedCString::from_ptr(
///         psync_get_username(),
///         |p| psync_free(p as *mut std::ffi::c_void)
///     )
/// };
///
/// if let Some(owned) = username {
///     println!("Username: {}", owned.to_str().unwrap_or("invalid UTF-8"));
/// }
/// // Memory is automatically freed when `owned` goes out of scope
/// ```
pub struct OwnedCString {
    ptr: *mut c_char,
    free_fn: Box<dyn FnOnce(*mut std::ffi::c_void)>,
}

impl OwnedCString {
    /// Create an `OwnedCString` from a raw pointer and free function.
    ///
    /// # Safety
    ///
    /// - `ptr` must be a valid pointer to a null-terminated C string, or null
    /// - The memory must be freeable by `free_fn`
    ///
    /// # Returns
    ///
    /// - `Some(OwnedCString)` if the pointer is non-null
    /// - `None` if the pointer is null
    pub unsafe fn from_ptr<F>(ptr: *mut c_char, free_fn: F) -> Option<Self>
    where
        F: FnOnce(*mut std::ffi::c_void) + 'static,
    {
        if ptr.is_null() {
            None
        } else {
            Some(Self {
                ptr,
                free_fn: Box::new(free_fn),
            })
        }
    }

    /// Get the string as a Rust `&str`, if valid UTF-8.
    ///
    /// # Returns
    ///
    /// - `Some(&str)` if the string is valid UTF-8
    /// - `None` if the string contains invalid UTF-8
    pub fn to_str(&self) -> Option<&str> {
        unsafe { CStr::from_ptr(self.ptr).to_str().ok() }
    }

    /// Get the string as a Rust `String`, with lossy UTF-8 conversion.
    ///
    /// Invalid UTF-8 sequences are replaced with the Unicode replacement character.
    pub fn to_string_lossy(&self) -> String {
        unsafe { CStr::from_ptr(self.ptr).to_string_lossy().into_owned() }
    }

    /// Get the raw pointer (for passing to C functions).
    ///
    /// The pointer remains valid until this `OwnedCString` is dropped.
    pub fn as_ptr(&self) -> *const c_char {
        self.ptr
    }

    /// Convert to an owned `String`, consuming the `OwnedCString`.
    ///
    /// # Returns
    ///
    /// - `Some(String)` if the string is valid UTF-8
    /// - `None` if the string contains invalid UTF-8
    pub fn into_string(self) -> Option<String> {
        let result = self.to_str().map(String::from);
        // self is dropped here, which frees the C memory
        result
    }
}

impl Drop for OwnedCString {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // Take the free function out of the box
            // We need to use a dummy function because we can't move out of self
            let free_fn = std::mem::replace(&mut self.free_fn, Box::new(|_| {}));
            free_fn(self.ptr as *mut std::ffi::c_void);
        }
    }
}

// OwnedCString cannot be safely sent between threads because the free_fn
// might not be thread-safe. We explicitly do not implement Send/Sync.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_cstring_valid() {
        let c_str = to_cstring("hello world").unwrap();
        assert_eq!(c_str.as_bytes(), b"hello world");
    }

    #[test]
    fn test_to_cstring_empty() {
        let c_str = to_cstring("").unwrap();
        assert_eq!(c_str.as_bytes(), b"");
    }

    #[test]
    fn test_to_cstring_with_null() {
        let result = to_cstring("hel\0lo");
        assert!(result.is_none());
    }

    #[test]
    fn test_try_to_cstring_valid() {
        let c_str = try_to_cstring("hello").unwrap();
        assert_eq!(c_str.as_bytes(), b"hello");
    }

    #[test]
    fn test_try_to_cstring_with_null() {
        let result = try_to_cstring("hel\0lo");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_cstr_owned_valid() {
        let c_string = CString::new("test string").unwrap();
        let result = unsafe { from_cstr_owned(c_string.as_ptr()) };
        assert_eq!(result, Some(String::from("test string")));
    }

    #[test]
    fn test_from_cstr_owned_null() {
        let result = unsafe { from_cstr_owned(std::ptr::null()) };
        assert!(result.is_none());
    }

    #[test]
    fn test_from_cstr_ref_valid() {
        let c_string = CString::new("borrowed string").unwrap();
        let result = unsafe { from_cstr_ref(c_string.as_ptr()) };
        assert_eq!(result, Some("borrowed string"));
    }

    #[test]
    fn test_from_cstr_ref_null() {
        let result = unsafe { from_cstr_ref::<'static>(std::ptr::null()) };
        assert!(result.is_none());
    }

    #[test]
    fn test_from_cstr_lossy_valid() {
        let c_string = CString::new("valid utf8").unwrap();
        let result = unsafe { from_cstr_lossy(c_string.as_ptr()) };
        assert_eq!(result, Some(String::from("valid utf8")));
    }

    #[test]
    fn test_from_cstr_lossy_null() {
        let result = unsafe { from_cstr_lossy(std::ptr::null()) };
        assert!(result.is_none());
    }

    #[test]
    fn test_from_cstr_and_free() {
        // Create a C string using libc malloc
        let test_str = "test";
        let ptr = unsafe {
            let ptr = libc::malloc(test_str.len() + 1) as *mut c_char;
            std::ptr::copy_nonoverlapping(test_str.as_ptr() as *const c_char, ptr, test_str.len());
            *ptr.add(test_str.len()) = 0; // null terminator
            ptr
        };

        let result = unsafe { from_cstr_and_free(ptr, |p| libc::free(p)) };
        assert_eq!(result, Some(String::from("test")));
    }

    #[test]
    fn test_from_cstr_and_free_null() {
        let mut free_called = false;
        let result = unsafe {
            from_cstr_and_free(std::ptr::null_mut(), |_| {
                free_called = true;
            })
        };
        assert!(result.is_none());
        assert!(!free_called); // Free should not be called for null pointer
    }

    #[test]
    fn test_unicode_roundtrip() {
        let unicode_str = "Hello, \u{4e16}\u{754c}!"; // "Hello, World!" in Chinese
        let c_str = to_cstring(unicode_str).unwrap();
        let result = unsafe { from_cstr_owned(c_str.as_ptr()) };
        assert_eq!(result, Some(String::from(unicode_str)));
    }
}
