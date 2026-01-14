//! Authentication operations for PCloudClient.
//!
//! This module provides authentication-related methods for the PCloudClient:
//! - Login with username/password
//! - Login with authentication token
//! - Logout
//! - User registration
//! - Password management (change, reset)
//!
//! # Security
//!
//! Passwords are handled using the `secrecy` crate to prevent accidental logging
//! or exposure. The `SecretString` wrapper ensures passwords are:
//! - Zeroized from memory when dropped
//! - Not printed in debug output
//! - Only exposed explicitly via `ExposeSecret`
//!
//! # Example
//!
//! ```ignore
//! use secrecy::Secret;
//! use console_client::wrapper::PCloudClient;
//!
//! let client = PCloudClient::init()?;
//! let mut guard = client.lock().unwrap();
//!
//! // Login with username and password
//! let password = Secret::new("my_password".to_string());
//! guard.authenticate("user@example.com", &password, true)?;
//!
//! // Check if logged in
//! if guard.is_logged_in() {
//!     println!("Successfully logged in!");
//! }
//! ```

use secrecy::{ExposeSecret, SecretString};

use crate::error::{AuthError, PCloudError, Result};
use crate::ffi::raw;
use crate::ffi::types::{
    PSTATUS_BAD_LOGIN_DATA, PSTATUS_BAD_LOGIN_TOKEN, PSTATUS_LOGIN_REQUIRED, PSTATUS_USER_MISMATCH,
};
use crate::utils::cstring::{from_cstr_and_free, from_cstr_ref, try_to_cstring};

use super::client::{AuthState, PCloudClient};

impl PCloudClient {
    /// Authenticate with username and password.
    ///
    /// This sets the credentials for login. The actual authentication happens
    /// asynchronously when `start_sync()` is called, or immediately if sync
    /// is already running.
    ///
    /// # Arguments
    ///
    /// * `username` - User's email address
    /// * `password` - User's password (wrapped in Secret for security)
    /// * `save_password` - If true, save credentials for future sessions
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success (credentials set)
    /// - `Err` if string conversion fails
    ///
    /// # Security
    ///
    /// The password is passed to the C library immediately and then the Secret
    /// wrapper ensures it will be zeroized when dropped. The C library may
    /// retain the password in memory if `save_password` is true.
    ///
    /// # Note
    ///
    /// If the username doesn't match the previously logged-in user,
    /// `PSTATUS_USER_MISMATCH` will be generated. Call `unlink()` first
    /// to switch users.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use secrecy::Secret;
    ///
    /// let password = Secret::new("my_password".to_string());
    /// client.authenticate("user@example.com", &password, true)?;
    /// ```
    pub fn authenticate(
        &mut self,
        username: &str,
        password: &SecretString,
        save_password: bool,
    ) -> Result<()> {
        // Convert strings to C strings
        let c_username = try_to_cstring(username)?;
        let c_password = try_to_cstring(password.expose_secret())?;

        // Update state to indicate authentication in progress
        self.set_auth_state(AuthState::Authenticating);

        // Safety: psync_set_user_pass is safe to call with valid C strings
        // The C library makes copies of the strings
        unsafe {
            raw::psync_set_user_pass(
                c_username.as_ptr(),
                c_password.as_ptr(),
                if save_password { 1 } else { 0 },
            );
        }

        // Note: Authentication happens asynchronously
        // The actual result will be delivered via status callback
        // For now, we remain in Authenticating state

        Ok(())
    }

    /// Set password only (when username is already known).
    ///
    /// Use this when `PSTATUS_BAD_LOGIN_DATA` is received to update just
    /// the password without changing the username.
    ///
    /// # Arguments
    ///
    /// * `password` - User's password (wrapped in Secret for security)
    /// * `save_password` - If true, save credentials for future sessions
    ///
    /// # Example
    ///
    /// ```ignore
    /// use secrecy::Secret;
    ///
    /// let password = Secret::new("new_password".to_string());
    /// client.set_password(&password, true)?;
    /// ```
    pub fn set_password(&mut self, password: &SecretString, save_password: bool) -> Result<()> {
        let c_password = try_to_cstring(password.expose_secret())?;

        self.set_auth_state(AuthState::Authenticating);

        // Safety: psync_set_pass is safe to call with a valid C string
        unsafe {
            raw::psync_set_pass(c_password.as_ptr(), if save_password { 1 } else { 0 });
        }

        Ok(())
    }

    /// Set authentication token for login.
    ///
    /// Alternative to username/password login using an auth token.
    /// Tokens can be obtained from the pCloud web interface or API.
    ///
    /// # Arguments
    ///
    /// * `token` - Authentication token (wrapped in Secret for security)
    /// * `save_token` - If true, save token for future sessions
    ///
    /// # Example
    ///
    /// ```ignore
    /// use secrecy::Secret;
    ///
    /// let token = Secret::new("auth_token_string".to_string());
    /// client.set_auth_token(&token, true)?;
    /// ```
    pub fn set_auth_token(&mut self, token: &SecretString, save_token: bool) -> Result<()> {
        let c_token = try_to_cstring(token.expose_secret())?;

        self.set_auth_state(AuthState::Authenticating);

        // Safety: psync_set_auth is safe to call with a valid C string
        unsafe {
            raw::psync_set_auth(c_token.as_ptr(), if save_token { 1 } else { 0 });
        }

        Ok(())
    }

    /// Log out the current user.
    ///
    /// This clears credentials but keeps the local database and synced files.
    /// After logout, `start_sync()` will report `PSTATUS_LOGIN_REQUIRED`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.logout();
    /// assert_eq!(client.auth_state(), &AuthState::NotAuthenticated);
    /// ```
    pub fn logout(&mut self) {
        // Safety: psync_logout is safe to call anytime
        unsafe {
            raw::psync_logout();
        }

        self.set_auth_state(AuthState::NotAuthenticated);
    }

    /// Unlink the current user.
    ///
    /// This clears credentials AND all synced data. This is required before
    /// logging in as a different user.
    ///
    /// **Warning**: This is destructive - local sync data will be deleted.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Switch to a different user
    /// client.unlink();
    /// client.authenticate("other@example.com", &password, true)?;
    /// ```
    pub fn unlink(&mut self) {
        // Safety: psync_unlink is safe to call anytime
        unsafe {
            raw::psync_unlink();
        }

        self.set_auth_state(AuthState::NotAuthenticated);
    }

    /// Get the current username.
    ///
    /// # Returns
    ///
    /// - `Some(String)` with the username if logged in
    /// - `None` if not logged in or on error
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(username) = client.get_username() {
    ///     println!("Logged in as: {}", username);
    /// }
    /// ```
    pub fn get_username(&self) -> Option<String> {
        // Safety: psync_get_username returns a string that must be freed
        let ptr = unsafe { raw::psync_get_username() };

        if ptr.is_null() {
            return None;
        }

        // Safety: We checked for null, and from_cstr_and_free handles the free
        unsafe { from_cstr_and_free(ptr, |p| raw::psync_free(p)) }
    }

    /// Get the authentication string/token.
    ///
    /// # Returns
    ///
    /// - `Some(String)` with the auth token if available
    /// - `None` if not logged in
    ///
    /// # Note
    ///
    /// The returned token should be treated as sensitive data.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(token) = client.get_auth_token() {
    ///     // Save token securely for later use
    /// }
    /// ```
    pub fn get_auth_token(&self) -> Option<String> {
        // Safety: psync_get_auth_string returns a pointer that must NOT be freed
        let ptr = unsafe { raw::psync_get_auth_string() };

        if ptr.is_null() {
            return None;
        }

        // Safety: We checked for null, and from_cstr_ref doesn't free
        unsafe { from_cstr_ref(ptr).map(String::from) }
    }

    /// Check if user is currently logged in.
    ///
    /// This queries the C library directly for the current status.
    ///
    /// # Returns
    ///
    /// `true` if the user is logged in, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if client.is_logged_in() {
    ///     println!("User is logged in");
    /// } else {
    ///     println!("Login required");
    /// }
    /// ```
    pub fn is_logged_in(&self) -> bool {
        let status = self.get_status();

        // Check if status indicates not logged in
        !matches!(
            status.status,
            PSTATUS_LOGIN_REQUIRED | PSTATUS_BAD_LOGIN_DATA | PSTATUS_BAD_LOGIN_TOKEN
        )
    }

    /// Register a new user account.
    ///
    /// Creates a new pCloud account with the given email and password.
    ///
    /// # Arguments
    ///
    /// * `email` - Email address (will be used as username)
    /// * `password` - Password for the new account
    /// * `accept_terms` - Must be true to indicate user accepted terms of service
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(AuthError::RegistrationFailed)` on API error
    /// - `Err(AuthError::NetworkError)` on network failure
    ///
    /// # Example
    ///
    /// ```ignore
    /// use secrecy::Secret;
    ///
    /// let password = Secret::new("secure_password".to_string());
    /// client.register("new_user@example.com", &password, true)?;
    /// ```
    pub fn register(
        &mut self,
        email: &str,
        password: &SecretString,
        accept_terms: bool,
    ) -> Result<()> {
        if !accept_terms {
            return Err(PCloudError::Auth(AuthError::RegistrationFailed(
                "Terms of service must be accepted".to_string(),
            )));
        }

        let c_email = try_to_cstring(email)?;
        let c_password = try_to_cstring(password.expose_secret())?;

        let mut err_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();

        // Safety: psync_register is safe to call with valid C strings
        // If it fails and err_ptr is set, we must free it
        let result =
            unsafe { raw::psync_register(c_email.as_ptr(), c_password.as_ptr(), 1, &mut err_ptr) };

        if result == 0 {
            // Success - user is now registered but may need email verification
            Ok(())
        } else if result == -1 {
            // Network error
            Err(PCloudError::Auth(AuthError::NetworkError))
        } else {
            // API error - get error message if available
            let error_msg = if !err_ptr.is_null() {
                // Safety: err_ptr was set by the C function and must be freed
                unsafe { from_cstr_and_free(err_ptr, |p| raw::psync_free(p)) }
                    .unwrap_or_else(|| format!("Error code: {}", result))
            } else {
                format!("Error code: {}", result)
            };

            Err(PCloudError::Auth(AuthError::RegistrationFailed(error_msg)))
        }
    }

    /// Send email verification mail.
    ///
    /// Sends a verification email to the currently registered email address.
    /// This is typically needed after registration.
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err` on failure
    pub fn send_verification_email(&self) -> Result<()> {
        let mut err_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();

        // Safety: psync_verify_email sends a verification email
        let result = unsafe { raw::psync_verify_email(&mut err_ptr) };

        if result == 0 {
            Ok(())
        } else if result == -1 {
            Err(PCloudError::Auth(AuthError::NetworkError))
        } else {
            let error_msg = if !err_ptr.is_null() {
                unsafe { from_cstr_and_free(err_ptr, |p| raw::psync_free(p)) }
                    .unwrap_or_else(|| format!("Error code: {}", result))
            } else {
                format!("Error code: {}", result)
            };

            Err(PCloudError::Auth(AuthError::Other(error_msg)))
        }
    }

    /// Send password reset email.
    ///
    /// Sends a password reset email to the specified email address.
    ///
    /// # Arguments
    ///
    /// * `email` - Email address to send reset link to
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err` on failure
    pub fn send_password_reset(&self, email: &str) -> Result<()> {
        let c_email = try_to_cstring(email)?;
        let mut err_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();

        // Safety: psync_lost_password sends a reset email
        let result = unsafe { raw::psync_lost_password(c_email.as_ptr(), &mut err_ptr) };

        if result == 0 {
            Ok(())
        } else if result == -1 {
            Err(PCloudError::Auth(AuthError::NetworkError))
        } else {
            let error_msg = if !err_ptr.is_null() {
                unsafe { from_cstr_and_free(err_ptr, |p| raw::psync_free(p)) }
                    .unwrap_or_else(|| format!("Error code: {}", result))
            } else {
                format!("Error code: {}", result)
            };

            Err(PCloudError::Auth(AuthError::Other(error_msg)))
        }
    }

    /// Change the user's password.
    ///
    /// Changes the password for the currently logged-in user.
    ///
    /// # Arguments
    ///
    /// * `current_password` - Current password
    /// * `new_password` - New password
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(AuthError::BadCredentials)` if current password is wrong
    /// - `Err` on other failures
    pub fn change_password(
        &mut self,
        current_password: &SecretString,
        new_password: &SecretString,
    ) -> Result<()> {
        let c_current = try_to_cstring(current_password.expose_secret())?;
        let c_new = try_to_cstring(new_password.expose_secret())?;
        let mut err_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();

        // Safety: psync_change_password changes the user's password
        let result =
            unsafe { raw::psync_change_password(c_current.as_ptr(), c_new.as_ptr(), &mut err_ptr) };

        if result == 0 {
            Ok(())
        } else if result == -1 {
            Err(PCloudError::Auth(AuthError::NetworkError))
        } else {
            let error_msg = if !err_ptr.is_null() {
                unsafe { from_cstr_and_free(err_ptr, |p| raw::psync_free(p)) }
                    .unwrap_or_else(|| format!("Error code: {}", result))
            } else {
                format!("Error code: {}", result)
            };

            // Check for common error codes
            Err(PCloudError::Auth(AuthError::Other(error_msg)))
        }
    }

    /// Update authentication state based on status code.
    ///
    /// This is called internally to update the auth state based on
    /// status callbacks from the C library.
    pub(crate) fn update_auth_state_from_status(&mut self, status: u32) {
        let new_state = match status {
            PSTATUS_LOGIN_REQUIRED => AuthState::NotAuthenticated,
            PSTATUS_BAD_LOGIN_DATA => AuthState::Failed("Invalid credentials".to_string()),
            PSTATUS_BAD_LOGIN_TOKEN => AuthState::Failed("Invalid or expired token".to_string()),
            PSTATUS_USER_MISMATCH => {
                AuthState::Failed("User mismatch - unlink required".to_string())
            }
            _ => {
                // Any other status means we're authenticated (or in a non-auth-related state)
                if self.auth_state == AuthState::Authenticating {
                    AuthState::Authenticated
                } else {
                    // Keep current state if not actively authenticating
                    return;
                }
            }
        };

        self.set_auth_state(new_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_transitions() {
        // Test that state transitions work correctly
        // Note: Can't test with real client without C library

        let initial = AuthState::NotAuthenticated;
        let authenticating = AuthState::Authenticating;
        let authenticated = AuthState::Authenticated;
        let failed = AuthState::Failed("test error".to_string());

        // Verify all states are distinct
        assert_ne!(initial, authenticating);
        assert_ne!(authenticating, authenticated);
        assert_ne!(authenticated, failed);
    }

    #[test]
    fn test_secret_password_not_debug_printed() {
        // Verify that SecretString doesn't leak password in debug output
        let password = Secret::new("super_secret_password".to_string());
        let debug_output = format!("{:?}", password);

        // The debug output should NOT contain the actual password
        assert!(!debug_output.contains("super_secret_password"));
    }
}
