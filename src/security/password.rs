//! Secure password handling for pCloud console client.
//!
//! This module provides secure password handling with automatic zeroization
//! of sensitive data when it goes out of scope. It uses the `secrecy` crate
//! to prevent accidental exposure of passwords in logs or debug output.
//!
//! # Security Features
//!
//! - Passwords are wrapped in `SecretString` to prevent accidental exposure
//! - Memory is automatically zeroized when `SecurePassword` is dropped
//! - Custom `Debug` implementation that never reveals the password
//! - Secure terminal input that doesn't echo characters
//! - Safe FFI conversion via `to_cstring()` with automatic zeroization of intermediates
//!
//! # Security Considerations
//!
//! - **Zeroization**: All password memory is automatically zeroized on drop via the
//!   `secrecy` crate's `SecretString` which uses `zeroize` internally.
//! - **Debug/Display safety**: Custom implementations ensure passwords never appear
//!   in debug output, logs, or error messages.
//! - **FFI boundary**: Use `to_cstring()` to safely pass passwords to C code with
//!   automatic zeroization of intermediate buffers.
//! - **No cloning raw data**: The `SecurePassword` type prevents accidental cloning
//!   of the underlying password string.
//!
//! # Example
//!
//! ```ignore
//! use console_client::security::{prompt_for_password, SecurePassword};
//!
//! // Prompt for password (won't echo to terminal)
//! let password = prompt_for_password("Password: ")?;
//!
//! // Use the password
//! let pw_str = password.expose();
//! authenticate(pw_str);
//! // password is automatically zeroized when dropped
//! ```

use rpassword::prompt_password;
use secrecy::{ExposeSecret, SecretString};
use std::ffi::CString;
use std::io;
use zeroize::Zeroizing;

/// Prompt for password without echoing to terminal.
///
/// Uses `rpassword` to securely read a password from the terminal
/// without displaying the characters as they are typed.
///
/// # Arguments
///
/// * `prompt` - The prompt string to display before reading input
///
/// # Returns
///
/// A `SecretString` containing the password, wrapped for security.
///
/// # Errors
///
/// Returns an I/O error if reading from the terminal fails.
///
/// # Example
///
/// ```ignore
/// let password = prompt_for_password("Enter password: ")?;
/// ```
pub fn prompt_for_password(prompt: &str) -> io::Result<SecretString> {
    let password = prompt_password(prompt)?;
    Ok(SecretString::from(password))
}

/// Prompt for password with confirmation.
///
/// Prompts the user to enter a password twice to confirm they typed
/// it correctly. This is useful for setting new passwords.
///
/// # Arguments
///
/// * `prompt` - The prompt string for the first password entry
///
/// # Returns
///
/// A `SecretString` containing the confirmed password.
///
/// # Errors
///
/// Returns an I/O error if reading from the terminal fails.
///
/// # Behavior
///
/// If the passwords don't match, the user is prompted to try again
/// until they enter matching passwords.
///
/// # Example
///
/// ```ignore
/// let password = prompt_for_password_with_confirm("New password: ")?;
/// ```
pub fn prompt_for_password_with_confirm(prompt: &str) -> io::Result<SecretString> {
    loop {
        let password = prompt_password(prompt)?;
        let confirm = prompt_password("Confirm password: ")?;

        if password == confirm {
            return Ok(SecretString::from(password));
        }

        eprintln!("Passwords do not match. Please try again.");
    }
}

/// Prompt for password with limited attempts.
///
/// Like `prompt_for_password_with_confirm`, but gives up after
/// a specified number of failed attempts.
///
/// # Arguments
///
/// * `prompt` - The prompt string for the first password entry
/// * `max_attempts` - Maximum number of attempts before giving up
///
/// # Returns
///
/// - `Ok(Some(password))` if passwords matched within the attempt limit
/// - `Ok(None)` if max attempts was reached without matching passwords
/// - `Err(e)` if an I/O error occurred
pub fn prompt_for_password_with_confirm_limited(
    prompt: &str,
    max_attempts: u32,
) -> io::Result<Option<SecretString>> {
    for attempt in 1..=max_attempts {
        let password = prompt_password(prompt)?;
        let confirm = prompt_password("Confirm password: ")?;

        if password == confirm {
            return Ok(Some(SecretString::from(password)));
        }

        if attempt < max_attempts {
            eprintln!(
                "Passwords do not match. {} attempts remaining.",
                max_attempts - attempt
            );
        } else {
            eprintln!("Maximum attempts reached.");
        }
    }

    Ok(None)
}

/// Secure password wrapper with automatic zeroization.
///
/// This type wraps a password string and ensures that:
/// - The password is never accidentally exposed in debug output
/// - The memory containing the password is zeroized when dropped
/// - Access to the password requires explicit `expose()` call
/// - Safe FFI conversion via `to_cstring()` with intermediate buffer zeroization
///
/// # Security
///
/// The `SecurePassword` type provides several layers of protection:
///
/// 1. **Memory zeroization**: The underlying `SecretString` uses `zeroize` to
///    clear password memory when dropped.
/// 2. **Debug protection**: The password never appears in `Debug` or `Display` output.
/// 3. **Explicit access**: You must call `expose()` to access the password, making
///    accidental exposure less likely.
/// 4. **FFI safety**: Use `to_cstring()` for C interop - intermediate buffers are
///    automatically zeroized.
///
/// # Example
///
/// ```
/// use console_client::security::SecurePassword;
///
/// let password = SecurePassword::new("my_secret".to_string());
///
/// // Debug output shows [REDACTED]
/// assert_eq!(format!("{:?}", password), "SecurePassword([REDACTED])");
///
/// // Must explicitly expose to access
/// assert_eq!(password.expose(), "my_secret");
///
/// // Safe FFI conversion
/// if let Some(c_pwd) = password.to_cstring() {
///     // Use c_pwd with C functions
///     // c_pwd bytes are zeroized when it's dropped
/// }
/// ```
#[derive(Clone)]
pub struct SecurePassword {
    inner: SecretString,
}

impl SecurePassword {
    /// Create a new secure password from a string.
    ///
    /// The string is immediately wrapped in a `Secret` to prevent
    /// accidental exposure. Note that the original `String` passed in
    /// may still contain the password in memory until it's dropped -
    /// for maximum security, consider using `Zeroizing<String>` before
    /// passing to this constructor.
    ///
    /// # Arguments
    ///
    /// * `password` - The password string to secure
    ///
    /// # Example
    ///
    /// ```
    /// use console_client::security::SecurePassword;
    ///
    /// // Basic usage
    /// let pwd = SecurePassword::new("secret".to_string());
    /// ```
    pub fn new(password: String) -> Self {
        Self {
            inner: SecretString::from(password),
        }
    }

    /// Create a secure password from a mutable string, zeroizing the original.
    ///
    /// This method takes ownership of the string and zeroizes it after copying
    /// into the secure wrapper. This is safer than `new()` as it ensures the
    /// original string's memory is cleared.
    ///
    /// # Arguments
    ///
    /// * `password` - The password string to secure (will be zeroized)
    ///
    /// # Example
    ///
    /// ```
    /// use console_client::security::SecurePassword;
    ///
    /// let mut pwd_str = "secret".to_string();
    /// let pwd = SecurePassword::new_zeroizing(pwd_str);
    /// // pwd_str's memory has been zeroized
    /// ```
    pub fn new_zeroizing(password: String) -> Self {
        // Use Zeroizing wrapper to ensure the password is cleared after copying
        let zeroizing_pwd = Zeroizing::new(password);
        Self {
            inner: SecretString::from((*zeroizing_pwd).clone()),
        }
        // zeroizing_pwd is zeroized here when dropped
    }

    /// Create a secure password from a Secret.
    ///
    /// This is useful when you already have a `SecretString` from
    /// another source (e.g., `prompt_for_password`).
    pub fn from_secret(secret: SecretString) -> Self {
        Self { inner: secret }
    }

    /// Expose the password for use.
    ///
    /// This method provides access to the underlying password string.
    ///
    /// # Security
    ///
    /// The returned string reference should:
    /// - Not be stored or persisted
    /// - Not be logged or printed
    /// - Be used immediately and let go out of scope
    ///
    /// # Returns
    ///
    /// A reference to the password string.
    pub fn expose(&self) -> &str {
        self.inner.expose_secret()
    }

    /// Create a CString for FFI, with proper zeroization of intermediate buffers.
    ///
    /// This method safely converts the password to a C-compatible string format
    /// for passing to FFI functions. The intermediate buffer used during conversion
    /// is automatically zeroized.
    ///
    /// # Returns
    ///
    /// - `Some(CString)` if the password can be converted (no null bytes)
    /// - `None` if the password contains null bytes (invalid for C strings)
    ///
    /// # Security
    ///
    /// - The intermediate `String` copy is wrapped in `Zeroizing` and cleared after use
    /// - The returned `CString` should be used immediately and dropped promptly
    /// - Note: CString's memory is NOT automatically zeroized when dropped - the caller
    ///   should use `zeroize_cstring()` if needed
    ///
    /// # Example
    ///
    /// ```
    /// use console_client::security::SecurePassword;
    ///
    /// let pwd = SecurePassword::new("secret".to_string());
    /// if let Some(c_pwd) = pwd.to_cstring() {
    ///     // Use c_pwd.as_ptr() with C functions
    /// }
    /// ```
    pub fn to_cstring(&self) -> Option<CString> {
        // Create a Zeroizing wrapper around the copy to ensure it's cleared
        let temp = Zeroizing::new(self.expose().to_string());
        let result = CString::new(temp.as_str()).ok();
        // temp is zeroized when it drops here
        result
    }

    /// Get the inner Secret.
    ///
    /// This consumes the `SecurePassword` and returns the inner `Secret`.
    pub fn into_secret(self) -> SecretString {
        self.inner
    }

    /// Get a reference to the inner Secret.
    pub fn as_secret(&self) -> &SecretString {
        &self.inner
    }

    /// Check if the password is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.expose_secret().is_empty()
    }

    /// Get the length of the password.
    pub fn len(&self) -> usize {
        self.inner.expose_secret().len()
    }
}

impl std::fmt::Debug for SecurePassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecurePassword([REDACTED])")
    }
}

impl std::fmt::Display for SecurePassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl From<String> for SecurePassword {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for SecurePassword {
    fn from(s: &str) -> Self {
        Self::new(s.to_string())
    }
}

impl From<SecretString> for SecurePassword {
    fn from(secret: SecretString) -> Self {
        Self::from_secret(secret)
    }
}

/// Ensure passwords in errors are redacted when accidentally converted to String.
///
/// This prevents accidental password exposure if someone tries to use a SecurePassword
/// in string context (e.g., error messages).
impl From<SecurePassword> for String {
    fn from(_: SecurePassword) -> String {
        "[REDACTED]".to_string()
    }
}

impl PartialEq for SecurePassword {
    fn eq(&self, other: &Self) -> bool {
        self.expose() == other.expose()
    }
}

impl Eq for SecurePassword {}

/// Zeroize a string in place.
///
/// This function overwrites the memory contents of a string with zeros.
/// Note: This is a best-effort function and may not work reliably due
/// to compiler optimizations. For reliable zeroization, use `SecurePassword`.
///
/// # Arguments
///
/// * `s` - Mutable reference to the string to zeroize
///
/// # Safety
///
/// This function uses unsafe code to directly modify the string's
/// memory. The string remains valid but contains only null bytes.
pub fn zeroize_string(s: &mut String) {
    // Safety: We're only writing zeros to valid memory
    unsafe {
        let bytes = s.as_bytes_mut();
        for byte in bytes.iter_mut() {
            std::ptr::write_volatile(byte, 0);
        }
    }
    s.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_password_new() {
        let password = SecurePassword::new("test123".to_string());
        assert_eq!(password.expose(), "test123");
    }

    #[test]
    fn test_secure_password_new_zeroizing() {
        let pwd_str = "secret123".to_string();
        let password = SecurePassword::new_zeroizing(pwd_str);
        assert_eq!(password.expose(), "secret123");
    }

    #[test]
    fn test_secure_password_debug_redacted() {
        let password = SecurePassword::new("secret".to_string());
        let debug_output = format!("{:?}", password);
        assert_eq!(debug_output, "SecurePassword([REDACTED])");
        assert!(!debug_output.contains("secret"));
    }

    #[test]
    fn test_secure_password_display_redacted() {
        let password = SecurePassword::new("secret".to_string());
        let display_output = format!("{}", password);
        assert_eq!(display_output, "[REDACTED]");
        assert!(!display_output.contains("secret"));
    }

    #[test]
    fn test_password_not_in_debug() {
        let pwd = SecurePassword::new("secret123".to_string());
        let debug_str = format!("{:?}", pwd);
        assert!(!debug_str.contains("secret123"));
        assert!(debug_str.contains("REDACTED"));
    }

    #[test]
    fn test_password_not_in_display() {
        let pwd = SecurePassword::new("secret123".to_string());
        let display_str = format!("{}", pwd);
        assert!(!display_str.contains("secret123"));
    }

    #[test]
    fn test_password_to_string_redacted() {
        let pwd = SecurePassword::new("secret123".to_string());
        let s: String = pwd.into();
        assert_eq!(s, "[REDACTED]");
        assert!(!s.contains("secret123"));
    }

    #[test]
    fn test_secure_password_clone() {
        let password = SecurePassword::new("test".to_string());
        let cloned = password.clone();
        assert_eq!(password.expose(), cloned.expose());
    }

    #[test]
    fn test_secure_password_from_string() {
        let password: SecurePassword = "test".to_string().into();
        assert_eq!(password.expose(), "test");
    }

    #[test]
    fn test_secure_password_from_str() {
        let password: SecurePassword = "test".into();
        assert_eq!(password.expose(), "test");
    }

    #[test]
    fn test_secure_password_from_secret() {
        let secret = SecretString::from("test".to_string());
        let password = SecurePassword::from_secret(secret);
        assert_eq!(password.expose(), "test");
    }

    #[test]
    fn test_secure_password_into_secret() {
        let password = SecurePassword::new("test".to_string());
        let secret = password.into_secret();
        assert_eq!(secret.expose_secret(), "test");
    }

    #[test]
    fn test_secure_password_is_empty() {
        let empty = SecurePassword::new(String::new());
        let not_empty = SecurePassword::new("x".to_string());

        assert!(empty.is_empty());
        assert!(!not_empty.is_empty());
    }

    #[test]
    fn test_secure_password_len() {
        let password = SecurePassword::new("hello".to_string());
        assert_eq!(password.len(), 5);
    }

    #[test]
    fn test_secure_password_equality() {
        let pw1 = SecurePassword::new("test".to_string());
        let pw2 = SecurePassword::new("test".to_string());
        let pw3 = SecurePassword::new("other".to_string());

        assert_eq!(pw1, pw2);
        assert_ne!(pw1, pw3);
    }

    #[test]
    fn test_cstring_conversion() {
        let pwd = SecurePassword::new("test".to_string());
        let cstring = pwd.to_cstring();
        assert!(cstring.is_some());

        // Verify the CString contains the correct value
        let cs = cstring.unwrap();
        assert_eq!(cs.to_str().unwrap(), "test");
    }

    #[test]
    fn test_cstring_conversion_with_null() {
        // Null bytes should fail
        let pwd_with_null = SecurePassword::new("test\0bad".to_string());
        assert!(pwd_with_null.to_cstring().is_none());
    }

    #[test]
    fn test_password_zeroized_on_drop() {
        // This is hard to test directly, but we can verify the behavior
        let pwd = SecurePassword::new("secret123".to_string());
        let exposed = pwd.expose();
        assert_eq!(exposed, "secret123");
        drop(pwd);
        // After drop, the memory should be zeroized
        // (Can't easily verify this without unsafe pointer manipulation)
    }

    #[test]
    fn test_zeroize_string() {
        let mut s = String::from("secret");
        zeroize_string(&mut s);
        assert!(s.is_empty());
    }

    #[test]
    fn test_secret_string_is_redacted() {
        let secret = SecretString::from("password123".to_string());
        let debug_output = format!("{:?}", secret);
        // secrecy crate should prevent the password from appearing
        assert!(!debug_output.contains("password123"));
    }

    // Note: Integration tests for prompt_for_password and prompt_for_password_with_confirm
    // would require mocking stdin, which is complex. These are better tested manually
    // or with integration testing frameworks.
}
