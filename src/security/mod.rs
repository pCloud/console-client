//! Security-related functionality for the pCloud console client.
//!
//! This module provides comprehensive security features for handling
//! sensitive data, particularly passwords and authentication credentials.
//!
//! # Security Features
//!
//! ## Password Handling
//!
//! - **Secure storage**: Uses `secrecy` crate's `Secret<String>` for password storage
//! - **Automatic zeroization**: Memory is zeroed on drop via `zeroize` crate
//! - **Debug protection**: Custom `Debug` implementations never expose passwords
//! - **Display protection**: Custom `Display` implementations show `[REDACTED]`
//! - **Safe FFI conversion**: `to_cstring()` method for passing passwords to C code
//!   with intermediate buffer zeroization
//!
//! ## IPC Security
//!
//! - **Unix socket permissions**: IPC socket created with 0600 permissions (owner-only)
//! - **Password zeroization**: Passwords received via IPC are immediately converted to
//!   `SecurePassword` and the original strings are zeroized
//! - **Debug redaction**: `DaemonCommand::StartCrypto` never prints password in Debug output
//!
//! ## FFI Safety
//!
//! - **Null pointer checks**: All FFI boundaries check for null pointers
//! - **Panic safety**: Callback trampolines use `catch_unwind` to prevent panic unwinding
//! - **Memory ownership**: Clear documentation of who allocates/frees memory
//!
//! # Submodules
//!
//! - `password`: Secure password type and prompting utilities
//!
//! # Security Considerations
//!
//! ## What IS Protected
//!
//! - Passwords in `SecurePassword` are automatically zeroized when dropped
//! - Passwords never appear in debug output, logs, or error messages
//! - IPC communication is restricted to the current user via Unix socket permissions
//!
//! ## What is NOT Protected
//!
//! - Passwords in transit over IPC are not encrypted (but Unix socket is local-only
//!   and permission-protected)
//! - Core dumps may still contain password memory if not disabled
//! - Compiler optimizations might defeat zeroization in some edge cases
//!
//! ## Best Practices
//!
//! 1. Use `SecurePassword::new_zeroizing()` when converting from plain strings
//! 2. Keep password exposure scopes as small as possible
//! 3. Use `to_cstring()` for FFI instead of manual conversion
//! 4. Never log or print password values
//!
//! # Example
//!
//! ```
//! use console_client::security::SecurePassword;
//!
//! // Create a secure password
//! let password = SecurePassword::new("secret123".to_string());
//!
//! // Debug output is redacted
//! println!("{:?}", password); // prints "SecurePassword([REDACTED])"
//!
//! // Must explicitly expose to use
//! assert_eq!(password.expose(), "secret123");
//!
//! // Safe FFI conversion
//! if let Some(c_str) = password.to_cstring() {
//!     // Use c_str.as_ptr() with C functions
//! }
//! ```

pub mod password;

// Re-export main types and functions for convenience
pub use password::{
    prompt_for_password, prompt_for_password_with_confirm,
    prompt_for_password_with_confirm_limited, zeroize_string, SecurePassword,
};

/// Initialize security module.
///
/// This function can be used for any security-related initialization
/// that needs to happen at startup.
///
/// # Current Implementation
///
/// Currently this function is a no-op. Future versions might:
/// - Initialize secure memory allocators
/// - Set up memory locking to prevent swapping
/// - Disable core dumps for the process
pub fn init() {
    // Currently no initialization needed
    // Future versions might initialize secure memory allocators, etc.
}
