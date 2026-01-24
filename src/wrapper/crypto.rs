//! Crypto (encryption) operations for PCloudClient.
//!
//! This module provides methods for managing pCloud's encrypted folder feature:
//! - Setup crypto with password and hint
//! - Start/stop crypto (unlock/lock encrypted folders)
//! - Check crypto status
//! - Create encrypted folders
//!
//! # Security
//!
//! Crypto passwords are handled using the `secrecy` crate to prevent accidental
//! logging or exposure. Passwords are zeroized from memory when dropped.
//!
//! # Crypto Lifecycle
//!
//! 1. **Setup**: Create encryption keys with `setup_crypto()` (once per account)
//! 2. **Start**: Unlock encrypted folders with `start_crypto()` (each session)
//! 3. **Use**: Access encrypted files normally through the virtual filesystem
//! 4. **Stop**: Lock encrypted folders with `stop_crypto()` when done
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
//! // First time setup (only needed once)
//! if !guard.is_crypto_setup() {
//!     let password = Secret::new("crypto_password".to_string());
//!     guard.setup_crypto(&password, "My password hint")?;
//! }
//!
//! // Unlock encrypted folders
//! let password = Secret::new("crypto_password".to_string());
//! guard.start_crypto(&password)?;
//!
//! // ... use encrypted files ...
//!
//! // Lock encrypted folders when done
//! guard.stop_crypto()?;
//! ```

use secrecy::{ExposeSecret, SecretString};

use crate::error::{CryptoError, PCloudError, Result};
use crate::ffi::raw;
use crate::ffi::types::{
    PSYNC_CRYPTO_HINT_CANT_CONNECT, PSYNC_CRYPTO_HINT_NOT_LOGGED_IN,
    PSYNC_CRYPTO_HINT_NOT_PROVIDED, PSYNC_CRYPTO_HINT_NOT_SUPPORTED, PSYNC_CRYPTO_HINT_SUCCESS,
    PSYNC_CRYPTO_INVALID_FOLDERID, PSYNC_CRYPTO_SETUP_ALREADY_SETUP,
    PSYNC_CRYPTO_SETUP_CANT_CONNECT, PSYNC_CRYPTO_SETUP_KEYGEN_FAILED,
    PSYNC_CRYPTO_SETUP_NOT_LOGGED_IN, PSYNC_CRYPTO_SETUP_NOT_SUPPORTED, PSYNC_CRYPTO_SETUP_SUCCESS,
    PSYNC_CRYPTO_START_ALREADY_STARTED, PSYNC_CRYPTO_START_BAD_PASSWORD,
    PSYNC_CRYPTO_START_CANT_CONNECT, PSYNC_CRYPTO_START_KEYS_DONT_MATCH,
    PSYNC_CRYPTO_START_NOT_LOGGED_IN, PSYNC_CRYPTO_START_NOT_SETUP,
    PSYNC_CRYPTO_START_NOT_SUPPORTED, PSYNC_CRYPTO_START_SUCCESS,
    PSYNC_CRYPTO_START_UNKNOWN_KEY_FORMAT, PSYNC_CRYPTO_STOP_NOT_STARTED,
    PSYNC_CRYPTO_STOP_NOT_SUPPORTED, PSYNC_CRYPTO_STOP_SUCCESS,
};
use crate::utils::cstring::{from_cstr_and_free, try_to_cstring};

use super::client::{CryptoState, PCloudClient};

impl PCloudClient {
    /// Set up crypto (encryption) for the account.
    ///
    /// This creates encryption keys with the given password. This operation
    /// only needs to be done once per account - the keys are stored on the
    /// server encrypted with your password.
    ///
    /// # Arguments
    ///
    /// * `password` - Crypto password (wrapped in Secret for security)
    /// * `hint` - Password hint to help remember the password
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(CryptoError::AlreadySetup)` if crypto is already set up
    /// - `Err(CryptoError::NotLoggedIn)` if not logged in
    /// - `Err(CryptoError::CantConnect)` on network failure
    /// - `Err(CryptoError::KeyGenFailed)` on key generation failure
    ///
    /// # Security
    ///
    /// The password is used to encrypt your private key. If you lose this
    /// password, you will NOT be able to access your encrypted files.
    /// There is no password recovery for crypto.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use secrecy::Secret;
    ///
    /// let password = Secret::new("my_crypto_password".to_string());
    /// client.setup_crypto(&password, "My memorable hint")?;
    /// ```
    pub fn setup_crypto(&mut self, password: &SecretString, hint: &str) -> Result<()> {
        let c_password = try_to_cstring(password.expose_secret())?;
        let c_hint = try_to_cstring(hint)?;

        // Safety: psync_crypto_setup is safe to call with valid C strings
        let result = unsafe { raw::psync_crypto_setup(c_password.as_ptr(), c_hint.as_ptr()) };

        match result {
            PSYNC_CRYPTO_SETUP_SUCCESS => {
                self.set_crypto_state(CryptoState::SetupComplete);
                Ok(())
            }
            PSYNC_CRYPTO_SETUP_NOT_SUPPORTED => Err(PCloudError::Crypto(CryptoError::NotSupported)),
            PSYNC_CRYPTO_SETUP_KEYGEN_FAILED => Err(PCloudError::Crypto(CryptoError::KeyGenFailed)),
            PSYNC_CRYPTO_SETUP_CANT_CONNECT => Err(PCloudError::Crypto(CryptoError::CantConnect)),
            PSYNC_CRYPTO_SETUP_NOT_LOGGED_IN => Err(PCloudError::Crypto(CryptoError::NotLoggedIn)),
            PSYNC_CRYPTO_SETUP_ALREADY_SETUP => Err(PCloudError::Crypto(CryptoError::AlreadySetup)),
            code => Err(PCloudError::Crypto(CryptoError::Unknown(code))),
        }
    }

    /// Start crypto (unlock encrypted folders).
    ///
    /// This decrypts the private key using the provided password and enables
    /// access to encrypted files. Must be called each session to access
    /// encrypted content.
    ///
    /// # Arguments
    ///
    /// * `password` - Crypto password (wrapped in Secret for security)
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(CryptoError::NotSetup)` if crypto hasn't been set up
    /// - `Err(CryptoError::BadPassword)` if password is wrong
    /// - `Err(CryptoError::AlreadyStarted)` if crypto is already unlocked
    /// - `Err(CryptoError::NotLoggedIn)` if not logged in
    ///
    /// # Example
    ///
    /// ```ignore
    /// use secrecy::Secret;
    ///
    /// let password = Secret::new("my_crypto_password".to_string());
    /// client.start_crypto(&password)?;
    ///
    /// // Encrypted files are now accessible
    /// ```
    pub fn start_crypto(&mut self, password: &SecretString) -> Result<()> {
        let c_password = try_to_cstring(password.expose_secret())?;

        // Safety: psync_crypto_start is safe to call with a valid C string
        let result = unsafe { raw::psync_crypto_start(c_password.as_ptr()) };

        match result {
            PSYNC_CRYPTO_START_SUCCESS => {
                self.set_crypto_state(CryptoState::Started);
                Ok(())
            }
            PSYNC_CRYPTO_START_NOT_SUPPORTED => Err(PCloudError::Crypto(CryptoError::NotSupported)),
            PSYNC_CRYPTO_START_ALREADY_STARTED => {
                // Not really an error - just update state
                self.set_crypto_state(CryptoState::Started);
                Ok(())
            }
            PSYNC_CRYPTO_START_CANT_CONNECT => Err(PCloudError::Crypto(CryptoError::CantConnect)),
            PSYNC_CRYPTO_START_NOT_LOGGED_IN => Err(PCloudError::Crypto(CryptoError::NotLoggedIn)),
            PSYNC_CRYPTO_START_NOT_SETUP => {
                self.set_crypto_state(CryptoState::NotSetup);
                Err(PCloudError::Crypto(CryptoError::NotSetup))
            }
            PSYNC_CRYPTO_START_UNKNOWN_KEY_FORMAT => {
                self.set_crypto_state(CryptoState::Failed("Unknown key format".to_string()));
                Err(PCloudError::Crypto(CryptoError::UnknownKeyFormat))
            }
            PSYNC_CRYPTO_START_BAD_PASSWORD => {
                self.set_crypto_state(CryptoState::Failed("Bad password".to_string()));
                Err(PCloudError::Crypto(CryptoError::BadPassword))
            }
            PSYNC_CRYPTO_START_KEYS_DONT_MATCH => {
                self.set_crypto_state(CryptoState::Failed("Keys don't match".to_string()));
                Err(PCloudError::Crypto(CryptoError::KeysDontMatch))
            }
            code => {
                self.set_crypto_state(CryptoState::Failed(format!("Unknown error: {}", code)));
                Err(PCloudError::Crypto(CryptoError::Unknown(code)))
            }
        }
    }

    /// Stop crypto (lock encrypted folders).
    ///
    /// This clears the decrypted private key from memory and makes encrypted
    /// files inaccessible until `start_crypto()` is called again.
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(CryptoError::NotStarted)` if crypto wasn't started
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.stop_crypto()?;
    /// // Encrypted files are now locked
    /// ```
    pub fn stop_crypto(&mut self) -> Result<()> {
        // Safety: psync_crypto_stop is safe to call
        let result = unsafe { raw::psync_crypto_stop() };

        match result {
            PSYNC_CRYPTO_STOP_SUCCESS => {
                self.set_crypto_state(CryptoState::Stopped);
                Ok(())
            }
            PSYNC_CRYPTO_STOP_NOT_SUPPORTED => Err(PCloudError::Crypto(CryptoError::NotSupported)),
            PSYNC_CRYPTO_STOP_NOT_STARTED => {
                // Not really an error - just update state
                self.set_crypto_state(CryptoState::Stopped);
                Ok(())
            }
            code => Err(PCloudError::Crypto(CryptoError::Unknown(code))),
        }
    }

    /// Check if crypto has been set up for this account.
    ///
    /// # Returns
    ///
    /// `true` if crypto has been set up, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if !client.is_crypto_setup() {
    ///     println!("Crypto needs to be set up first");
    /// }
    /// ```
    pub fn is_crypto_setup(&self) -> bool {
        // Safety: psync_crypto_issetup is safe to call anytime
        unsafe { raw::psync_crypto_issetup() != 0 }
    }

    /// Check if crypto is currently started (unlocked).
    ///
    /// # Returns
    ///
    /// `true` if crypto is started (encrypted files are accessible), `false` otherwise.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if client.is_crypto_started() {
    ///     println!("Encrypted files are accessible");
    /// }
    /// ```
    pub fn is_crypto_started(&self) -> bool {
        // Safety: psync_crypto_isstarted is safe to call anytime
        unsafe { raw::psync_crypto_isstarted() != 0 }
    }

    /// Check if user has an active crypto subscription.
    ///
    /// # Returns
    ///
    /// `true` if user has a crypto subscription, `false` otherwise.
    pub fn has_crypto_subscription(&self) -> bool {
        // Safety: psync_crypto_hassubscription is safe to call anytime
        unsafe { raw::psync_crypto_hassubscription() != 0 }
    }

    /// Check if the crypto service is expired.
    ///
    /// # Returns
    ///
    /// `true` if crypto subscription is expired, `false` if not expired or
    /// never set up (eligible for trial).
    pub fn is_crypto_expired(&self) -> bool {
        // Safety: psync_crypto_isexpired is safe to call anytime
        unsafe { raw::psync_crypto_isexpired() != 0 }
    }

    /// Get the crypto expiration timestamp.
    ///
    /// # Returns
    ///
    /// Unix timestamp of when crypto expires, or 0 if never set up.
    pub fn crypto_expires(&self) -> i64 {
        // Safety: psync_crypto_expires is safe to call anytime
        unsafe { raw::psync_crypto_expires() }
    }

    /// Get the crypto password hint.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(String))` with the hint if available
    /// - `Ok(None)` if no hint was provided
    /// - `Err` on failure
    ///
    /// # Example
    ///
    /// ```ignore
    /// match client.get_crypto_hint()? {
    ///     Some(hint) => println!("Hint: {}", hint),
    ///     None => println!("No hint available"),
    /// }
    /// ```
    pub fn get_crypto_hint(&self) -> Result<Option<String>> {
        let mut hint_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();

        // Safety: psync_crypto_get_hint returns a hint string that must be freed
        let result = unsafe { raw::psync_crypto_get_hint(&mut hint_ptr) };

        match result {
            PSYNC_CRYPTO_HINT_SUCCESS => {
                if hint_ptr.is_null() {
                    Ok(None)
                } else {
                    // Safety: hint_ptr was set by the C function and must be freed
                    let hint = unsafe { from_cstr_and_free(hint_ptr, |p| raw::psync_free(p)) };
                    Ok(hint)
                }
            }
            PSYNC_CRYPTO_HINT_NOT_SUPPORTED => Err(PCloudError::Crypto(CryptoError::NotSupported)),
            PSYNC_CRYPTO_HINT_NOT_PROVIDED => Ok(None),
            PSYNC_CRYPTO_HINT_CANT_CONNECT => Err(PCloudError::Crypto(CryptoError::CantConnect)),
            PSYNC_CRYPTO_HINT_NOT_LOGGED_IN => Err(PCloudError::Crypto(CryptoError::NotLoggedIn)),
            code => Err(PCloudError::Crypto(CryptoError::Unknown(code))),
        }
    }

    /// Create an encrypted folder.
    ///
    /// Creates a new folder that will be encrypted. Files placed in this
    /// folder will be automatically encrypted.
    ///
    /// # Arguments
    ///
    /// * `parent_folder_id` - ID of the parent folder (0 for root)
    /// * `name` - Name for the new encrypted folder
    ///
    /// # Returns
    ///
    /// - `Ok(folder_id)` with the new folder's ID on success
    /// - `Err(CryptoError::NotStarted)` if crypto isn't started
    /// - `Err` on other failures
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Create encrypted folder in root
    /// let folder_id = client.create_crypto_folder(0, "My Encrypted Folder")?;
    /// println!("Created folder with ID: {}", folder_id);
    /// ```
    pub fn create_crypto_folder(&mut self, parent_folder_id: u64, name: &str) -> Result<u64> {
        let c_name = try_to_cstring(name)?;
        let mut err_ptr: *const std::os::raw::c_char = std::ptr::null();
        let mut new_folder_id: u64 = 0;

        // Safety: psync_crypto_mkdir creates an encrypted folder
        let result = unsafe {
            raw::psync_crypto_mkdir(
                parent_folder_id,
                c_name.as_ptr(),
                &mut err_ptr,
                &mut new_folder_id,
            )
        };

        if result == 0 {
            Ok(new_folder_id)
        } else {
            // Get error message if available (note: err_ptr should NOT be freed)
            let error_msg = if !err_ptr.is_null() {
                unsafe { crate::utils::cstring::from_cstr_ref(err_ptr) }
                    .map(String::from)
                    .unwrap_or_else(|| format!("Error code: {}", result))
            } else {
                format!("Error code: {}", result)
            };

            Err(PCloudError::Crypto(
                CryptoError::from_generic_code(result).into_with_message(&error_msg),
            ))
        }
    }

    /// Get the ID of the first encrypted folder.
    ///
    /// # Returns
    ///
    /// - `Some(folder_id)` if an encrypted folder exists
    /// - `None` if no encrypted folders exist
    pub fn get_crypto_folder_id(&self) -> Option<u64> {
        // Safety: psync_crypto_folderid is safe to call
        let id = unsafe { raw::psync_crypto_folderid() };

        if id == PSYNC_CRYPTO_INVALID_FOLDERID {
            None
        } else {
            Some(id)
        }
    }

    /// Get all encrypted folder IDs.
    ///
    /// # Returns
    ///
    /// A vector of folder IDs for all encrypted folders.
    pub fn get_crypto_folder_ids(&self) -> Vec<u64> {
        // Safety: psync_crypto_folderids returns an array that must be freed
        let ptr = unsafe { raw::psync_crypto_folderids() };

        if ptr.is_null() {
            return Vec::new();
        }

        // Read folder IDs until we hit the sentinel value
        let mut ids = Vec::new();
        let mut i = 0;
        loop {
            // Safety: We're reading from a valid array until sentinel
            let id = unsafe { *ptr.add(i) };
            if id == PSYNC_CRYPTO_INVALID_FOLDERID {
                break;
            }
            ids.push(id);
            i += 1;
        }

        // Free the array
        // Safety: ptr was allocated by the C library
        unsafe {
            raw::psync_free(ptr as *mut std::ffi::c_void);
        }

        ids
    }

    /// Request crypto reset (delete all encrypted data).
    ///
    /// **Warning**: This is extremely destructive! It will delete all encrypted
    /// files and reset crypto completely. A confirmation email will be sent
    /// to the user.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if reset request was sent
    /// - `Err` on failure
    pub fn request_crypto_reset(&mut self) -> Result<()> {
        // Safety: psync_crypto_reset sends a reset confirmation email
        let result = unsafe { raw::psync_crypto_reset() };

        if result == 0 {
            self.set_crypto_state(CryptoState::NotSetup);
            Ok(())
        } else {
            Err(PCloudError::Crypto(CryptoError::from_generic_code(result)))
        }
    }
}

/// Helper trait to add context to CryptoError
trait CryptoErrorExt {
    fn into_with_message(self, msg: &str) -> CryptoError;
}

impl CryptoErrorExt for CryptoError {
    fn into_with_message(self, _msg: &str) -> CryptoError {
        // For most errors, just return as-is
        // The error type already contains enough information
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_state_transitions() {
        let not_setup = CryptoState::NotSetup;
        let setup_complete = CryptoState::SetupComplete;
        let started = CryptoState::Started;
        let stopped = CryptoState::Stopped;
        let failed = CryptoState::Failed("test".to_string());

        // Verify states are distinct
        assert_ne!(not_setup, setup_complete);
        assert_ne!(setup_complete, started);
        assert_ne!(started, stopped);
        assert_ne!(stopped, failed);
    }

    #[test]
    fn test_crypto_error_from_setup_code() {
        assert!(matches!(
            CryptoError::from_setup_code(PSYNC_CRYPTO_SETUP_NOT_SUPPORTED),
            CryptoError::NotSupported
        ));
        assert!(matches!(
            CryptoError::from_setup_code(PSYNC_CRYPTO_SETUP_KEYGEN_FAILED),
            CryptoError::KeyGenFailed
        ));
        assert!(matches!(
            CryptoError::from_setup_code(PSYNC_CRYPTO_SETUP_ALREADY_SETUP),
            CryptoError::AlreadySetup
        ));
    }

    #[test]
    fn test_crypto_error_from_start_code() {
        assert!(matches!(
            CryptoError::from_start_code(PSYNC_CRYPTO_START_BAD_PASSWORD),
            CryptoError::BadPassword
        ));
        assert!(matches!(
            CryptoError::from_start_code(PSYNC_CRYPTO_START_NOT_SETUP),
            CryptoError::NotSetup
        ));
        assert!(matches!(
            CryptoError::from_start_code(PSYNC_CRYPTO_START_ALREADY_STARTED),
            CryptoError::AlreadyStarted
        ));
    }

    #[test]
    fn test_crypto_error_from_stop_code() {
        assert!(matches!(
            CryptoError::from_stop_code(PSYNC_CRYPTO_STOP_NOT_STARTED),
            CryptoError::NotStarted
        ));
        assert!(matches!(
            CryptoError::from_stop_code(PSYNC_CRYPTO_STOP_NOT_SUPPORTED),
            CryptoError::NotSupported
        ));
    }

    #[test]
    fn test_secret_password_security() {
        use secrecy::SecretString;

        let password = SecretString::from("crypto_secret_password".to_string());
        let debug_output = format!("{:?}", password);

        // Verify password doesn't appear in debug output
        assert!(!debug_output.contains("crypto_secret_password"));
    }
}
