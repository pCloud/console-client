//! Main PCloudClient struct and lifecycle management.
//!
//! This module provides the `PCloudClient` struct, which is the primary interface
//! for interacting with the pclsync C library. It manages the library lifecycle,
//! tracks authentication and crypto state, and provides safe wrappers for all
//! pclsync operations.
//!
//! # Design
//!
//! The `PCloudClient` follows a singleton pattern because the underlying pclsync
//! library maintains global state. Only one instance can exist at a time.
//!
//! # Example
//!
//! ```ignore
//! use console_client::wrapper::PCloudClient;
//!
//! // Initialize the client (only once per process)
//! let client = PCloudClient::init()?;
//!
//! // Access the client later
//! if let Some(client) = PCloudClient::get() {
//!     let guard = client.lock().unwrap();
//!     println!("Auth state: {:?}", guard.auth_state());
//! }
//! ```
//!
//! # Thread Safety
//!
//! The `PCloudClient` is protected by a `Mutex` to ensure thread-safe access.
//! The underlying pclsync library uses internal threading for callbacks and sync
//! operations, but API calls should generally be made from a single thread.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use once_cell::sync::OnceCell;

use crate::error::{FfiError, PCloudError, Result};
use crate::ffi::raw;
use crate::ffi::types::pstatus_t;

/// Global singleton for PCloudClient.
///
/// This ensures only one client instance exists, matching the pclsync library's
/// expectation of single initialization.
static PCLOUD_CLIENT: OnceCell<Arc<Mutex<PCloudClient>>> = OnceCell::new();

/// State of authentication with the pCloud service.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    /// Not authenticated - login required
    NotAuthenticated,
    /// Authentication in progress
    Authenticating,
    /// Successfully authenticated
    Authenticated,
    /// Authentication failed with error message
    Failed(String),
}

impl Default for AuthState {
    fn default() -> Self {
        AuthState::NotAuthenticated
    }
}

/// State of crypto (encryption) operations.
#[derive(Debug, Clone, PartialEq)]
pub enum CryptoState {
    /// Crypto has not been set up for this account
    NotSetup,
    /// Crypto is set up but not started (locked)
    SetupComplete,
    /// Crypto is started (unlocked)
    Started,
    /// Crypto was stopped (locked again)
    Stopped,
    /// Crypto operation failed with error message
    Failed(String),
}

impl Default for CryptoState {
    fn default() -> Self {
        CryptoState::NotSetup
    }
}

/// Main client for interacting with pCloud services.
///
/// This struct wraps the pclsync C library and provides safe Rust APIs
/// for all operations including authentication, crypto, and sync.
///
/// # Lifecycle
///
/// 1. Call `PCloudClient::init()` to initialize the library and create the client
/// 2. Use `PCloudClient::get()` to access the singleton instance
/// 3. The client is automatically cleaned up when the process exits
///
/// # State Management
///
/// The client tracks state in Rust to avoid repeated FFI calls:
/// - `auth_state`: Current authentication state
/// - `crypto_state`: Current crypto (encryption) state
/// - `fs_mounted`: Whether the virtual filesystem is mounted
///
/// Note that the C library may change state independently (e.g., from callbacks),
/// so use the `refresh_*` methods to sync state when needed.
pub struct PCloudClient {
    /// Whether the library has been initialized
    pub(crate) initialized: bool,
    /// Current authentication state
    pub(crate) auth_state: AuthState,
    /// Current crypto state
    pub(crate) crypto_state: CryptoState,
    /// Whether the FUSE filesystem is mounted
    pub(crate) fs_mounted: bool,
    /// Current filesystem mount point
    pub(crate) mountpoint: Option<PathBuf>,
}

impl PCloudClient {
    /// Initialize the pCloud client.
    ///
    /// This must be called before any other pCloud operations. It initializes
    /// the pclsync C library and creates the singleton client instance.
    ///
    /// # Returns
    ///
    /// - `Ok(Arc<Mutex<PCloudClient>>)` on success
    /// - `Err(PCloudError::Ffi(FfiError::AlreadyInitialized))` if already initialized
    /// - `Err(PCloudError::Ffi(FfiError::InitFailed))` if C library init failed
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = PCloudClient::init()?;
    /// ```
    ///
    /// # Safety
    ///
    /// This function calls the underlying `psync_init()` C function which must
    /// only be called once per process.
    pub fn init() -> Result<Arc<Mutex<Self>>> {
        // Try to set the singleton
        let client = PCLOUD_CLIENT.get_or_try_init(|| {
            // Initialize the C library
            // Safety: psync_init is safe to call once before any other operations
            let result = unsafe { raw::psync_init() };

            if result != 0 {
                return Err(PCloudError::Ffi(FfiError::InitFailed));
            }

            // Create the client instance
            let client = PCloudClient {
                initialized: true,
                auth_state: AuthState::NotAuthenticated,
                crypto_state: CryptoState::NotSetup,
                fs_mounted: false,
                mountpoint: None,
            };

            Ok(Arc::new(Mutex::new(client)))
        })?;

        Ok(Arc::clone(client))
    }

    /// Initialize with a specific database path.
    ///
    /// This allows specifying a custom path for the pclsync database file.
    /// Must be called before `init()`.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database file, or special values:
    ///   - `:memory:` for an in-memory database
    ///   - Empty string for a temporary file
    ///
    /// # Returns
    ///
    /// - `Ok(Arc<Mutex<PCloudClient>>)` on success
    /// - `Err` if initialization fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = PCloudClient::init_with_database("/path/to/db.sqlite")?;
    /// ```
    pub fn init_with_database(path: &str) -> Result<Arc<Mutex<Self>>> {
        // Set database path before init
        let c_path = crate::utils::try_to_cstring(path)?;

        // Safety: psync_set_database_path must be called before psync_init
        // and the library makes its own copy of the path
        unsafe {
            raw::psync_set_database_path(c_path.as_ptr());
        }

        Self::init()
    }

    /// Get the global client instance.
    ///
    /// Returns the singleton client instance if it has been initialized,
    /// or `None` if `init()` has not been called yet.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(client) = PCloudClient::get() {
    ///     let guard = client.lock().unwrap();
    ///     // use the client
    /// } else {
    ///     eprintln!("Client not initialized");
    /// }
    /// ```
    pub fn get() -> Option<Arc<Mutex<Self>>> {
        PCLOUD_CLIENT.get().map(Arc::clone)
    }

    /// Get the current authentication state.
    ///
    /// This returns the cached state. Use `refresh_auth_state()` to sync
    /// with the C library's actual state.
    pub fn auth_state(&self) -> &AuthState {
        &self.auth_state
    }

    /// Get the current crypto state.
    ///
    /// This returns the cached state. Use `refresh_crypto_state()` to sync
    /// with the C library's actual state.
    pub fn crypto_state(&self) -> &CryptoState {
        &self.crypto_state
    }

    /// Check if the filesystem is mounted.
    ///
    /// This returns the cached state. Use `refresh_mount_state()` to sync
    /// with the C library's actual state.
    pub fn is_mounted(&self) -> bool {
        self.fs_mounted
    }

    /// Get the current mount point path.
    ///
    /// Returns `None` if the filesystem is not mounted.
    pub fn mountpoint(&self) -> Option<&PathBuf> {
        self.mountpoint.as_ref()
    }

    /// Check if the client has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the current sync status from the C library.
    ///
    /// This queries the C library directly for the current status,
    /// bypassing any cached state.
    ///
    /// # Returns
    ///
    /// The current `pstatus_t` struct with all status fields populated.
    pub fn get_status(&self) -> pstatus_t {
        let mut status = pstatus_t::default();

        // Safety: psync_get_status fills the provided status struct
        // and the library has been initialized
        unsafe {
            raw::psync_get_status(&mut status);
        }

        status
    }

    /// Refresh the authentication state from the C library.
    ///
    /// This queries the C library and updates the cached `auth_state`.
    pub fn refresh_auth_state(&mut self) {
        let status = self.get_status();

        self.auth_state = match status.status {
            crate::ffi::types::PSTATUS_LOGIN_REQUIRED => AuthState::NotAuthenticated,
            crate::ffi::types::PSTATUS_BAD_LOGIN_DATA => {
                AuthState::Failed("Invalid credentials".to_string())
            }
            crate::ffi::types::PSTATUS_BAD_LOGIN_TOKEN => {
                AuthState::Failed("Invalid or expired token".to_string())
            }
            crate::ffi::types::PSTATUS_USER_MISMATCH => {
                AuthState::Failed("User mismatch".to_string())
            }
            _ => {
                // If we get any other status, check if logged_in flag is set
                // Note: pstatus_t should have a logged_in field from bindgen
                // For now, assume authenticated if not in an error state
                if crate::ffi::types::is_error_status(status.status) {
                    AuthState::NotAuthenticated
                } else {
                    AuthState::Authenticated
                }
            }
        };
    }

    /// Refresh the crypto state from the C library.
    ///
    /// This queries the C library and updates the cached `crypto_state`.
    pub fn refresh_crypto_state(&mut self) {
        // Safety: These functions query the crypto state and are safe after init
        let is_setup = unsafe { raw::psync_crypto_issetup() } != 0;
        let is_started = unsafe { raw::psync_crypto_isstarted() } != 0;

        self.crypto_state = if !is_setup {
            CryptoState::NotSetup
        } else if is_started {
            CryptoState::Started
        } else {
            CryptoState::Stopped
        };
    }

    /// Refresh the filesystem mount state from the C library.
    ///
    /// This queries the C library and updates the cached `fs_mounted` and
    /// `mountpoint` fields.
    pub fn refresh_mount_state(&mut self) {
        // Safety: psync_fs_isstarted returns 1 if started, 0 otherwise
        let is_started = unsafe { raw::psync_fs_isstarted() } != 0;

        self.fs_mounted = is_started;

        if is_started {
            // Get the mount point
            // Safety: psync_fs_getmountpoint returns a string that must be freed
            let ptr = unsafe { raw::psync_fs_getmountpoint() };
            if !ptr.is_null() {
                // Safety: from_cstr_and_free handles null check and frees memory
                if let Some(path) = unsafe {
                    crate::utils::cstring::from_cstr_and_free(ptr, |p| raw::psync_free(p))
                } {
                    self.mountpoint = Some(PathBuf::from(path));
                } else {
                    self.mountpoint = None;
                }
            } else {
                self.mountpoint = None;
            }
        } else {
            self.mountpoint = None;
        }
    }

    /// Refresh all state from the C library.
    ///
    /// This is a convenience method that calls all `refresh_*` methods.
    pub fn refresh_all_state(&mut self) {
        self.refresh_auth_state();
        self.refresh_crypto_state();
        self.refresh_mount_state();
    }

    /// Start the sync process.
    ///
    /// This initiates network connections and local file scanning.
    /// Status and event callbacks will be called from a dedicated thread.
    ///
    /// # Arguments
    ///
    /// * `status_callback` - Optional callback for status changes
    /// * `event_callback` - Optional callback for file/folder events
    ///
    /// # Note
    ///
    /// Currently callbacks are passed as raw function pointers.
    /// A safer callback API will be implemented in Phase 4.
    pub fn start_sync(
        &mut self,
        status_callback: crate::ffi::types::pstatus_change_callback_t,
        event_callback: crate::ffi::types::pevent_callback_t,
    ) {
        // Safety: psync_start_sync is safe to call after psync_init
        // Callbacks are called from a dedicated thread
        unsafe {
            raw::psync_start_sync(status_callback, event_callback);
        }
    }

    /// Pause sync operations.
    ///
    /// Sync is stopped but local and remote directories are still monitored.
    /// Status updates continue to be received.
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err` on failure
    pub fn pause(&mut self) -> Result<()> {
        // Safety: psync_pause is safe to call after sync has started
        let result = unsafe { raw::psync_pause() };

        if result != 0 {
            return Err(PCloudError::Ffi(FfiError::CError {
                code: result,
                message: "Failed to pause sync".to_string(),
            }));
        }

        Ok(())
    }

    /// Stop all sync operations.
    ///
    /// All network and local scan operations stop.
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err` on failure
    pub fn stop(&mut self) -> Result<()> {
        // Safety: psync_stop is safe to call after sync has started
        let result = unsafe { raw::psync_stop() };

        if result != 0 {
            return Err(PCloudError::Ffi(FfiError::CError {
                code: result,
                message: "Failed to stop sync".to_string(),
            }));
        }

        Ok(())
    }

    /// Resume sync operations.
    ///
    /// Restarts operations from both paused and stopped states.
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err` on failure
    pub fn resume(&mut self) -> Result<()> {
        // Safety: psync_resume is safe to call after pause/stop
        let result = unsafe { raw::psync_resume() };

        if result != 0 {
            return Err(PCloudError::Ffi(FfiError::CError {
                code: result,
                message: "Failed to resume sync".to_string(),
            }));
        }

        Ok(())
    }

    /// Force a rescan of local files.
    ///
    /// This is normally not needed but can be useful as a user-triggered option.
    pub fn run_local_scan(&mut self) {
        // Safety: psync_run_localscan is safe to call after sync has started
        unsafe {
            raw::psync_run_localscan();
        }
    }

    /// Notify the library of a network change.
    ///
    /// Call this when the network connection changes (e.g., WiFi access point change).
    /// Safe to call frequently.
    pub fn notify_network_change(&mut self) {
        // Safety: psync_network_exception is safe to call from any thread
        unsafe {
            raw::psync_network_exception();
        }
    }

    /// Download the directory structure (blocking).
    ///
    /// This downloads the remote directory structure into the local state.
    /// Should be called after `init()` but can be called instead of or before
    /// `start_sync()`.
    ///
    /// # Returns
    ///
    /// The status code indicating success or the reason for failure.
    pub fn download_state(&mut self) -> u32 {
        // Safety: psync_download_state blocks until complete
        unsafe { raw::psync_download_state() }
    }

    /// Get the last error code from the C library.
    ///
    /// This is useful for debugging after an operation fails.
    pub fn get_last_error(&self) -> u32 {
        // Safety: psync_get_last_error returns the last error for the current thread
        unsafe { raw::psync_get_last_error() }
    }

    /// Start the virtual filesystem (FUSE).
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err` on failure
    pub fn start_fs(&mut self) -> Result<()> {
        // Safety: psync_fs_start is safe to call after init
        let result = unsafe { raw::psync_fs_start() };

        if result != 0 {
            return Err(PCloudError::Ffi(FfiError::CError {
                code: result,
                message: "Failed to start filesystem".to_string(),
            }));
        }

        self.fs_mounted = true;
        self.refresh_mount_state();

        Ok(())
    }

    /// Stop the virtual filesystem (FUSE).
    pub fn stop_fs(&mut self) {
        // Safety: psync_fs_stop is safe to call after fs_start
        unsafe {
            raw::psync_fs_stop();
        }

        self.fs_mounted = false;
        self.mountpoint = None;
    }

    /// Internal helper to set auth state.
    pub(crate) fn set_auth_state(&mut self, state: AuthState) {
        self.auth_state = state;
    }

    /// Internal helper to set crypto state.
    pub(crate) fn set_crypto_state(&mut self, state: CryptoState) {
        self.crypto_state = state;
    }
}

impl Drop for PCloudClient {
    fn drop(&mut self) {
        if self.initialized {
            // Stop filesystem if mounted
            if self.fs_mounted {
                // Safety: psync_fs_stop is safe to call
                unsafe {
                    raw::psync_fs_stop();
                }
            }

            // Stop crypto if started
            if self.crypto_state == CryptoState::Started {
                // Safety: psync_crypto_stop is safe to call
                unsafe {
                    raw::psync_crypto_stop();
                }
            }

            // Destroy the library
            // Safety: psync_destroy cleans up all library resources
            // Should only be called once
            unsafe {
                raw::psync_destroy();
            }

            self.initialized = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_default() {
        assert_eq!(AuthState::default(), AuthState::NotAuthenticated);
    }

    #[test]
    fn test_crypto_state_default() {
        assert_eq!(CryptoState::default(), CryptoState::NotSetup);
    }

    #[test]
    fn test_auth_state_equality() {
        assert_eq!(AuthState::Authenticated, AuthState::Authenticated);
        assert_ne!(AuthState::Authenticated, AuthState::NotAuthenticated);
        assert_eq!(
            AuthState::Failed("test".to_string()),
            AuthState::Failed("test".to_string())
        );
        assert_ne!(
            AuthState::Failed("test".to_string()),
            AuthState::Failed("other".to_string())
        );
    }

    #[test]
    fn test_crypto_state_equality() {
        assert_eq!(CryptoState::Started, CryptoState::Started);
        assert_ne!(CryptoState::Started, CryptoState::Stopped);
    }

    // Note: Tests that require actual library initialization cannot be run
    // without the C library being available. Those would be integration tests.
}
