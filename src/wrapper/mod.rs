//! Safe Rust wrappers over the pclsync FFI bindings.
//!
//! This module provides:
//! - `PCloudClient`: Main client struct with safe methods
//! - `AuthState`: Authentication state tracking
//! - `CryptoState`: Crypto (encryption) state tracking
//! - Authentication operations (login, logout, registration)
//! - Crypto operations (setup, start, stop)
//! - Filesystem operations (mount, unmount, sync folders)
//!
//! # Design
//!
//! The wrapper layer converts unsafe FFI calls into safe Rust APIs:
//! - All unsafe code is isolated and well-documented
//! - C error codes are converted to Rust `Result` types
//! - Memory ownership is clearly defined
//! - RAII patterns ensure proper cleanup
//! - Passwords are protected using the `secrecy` crate
//!
//! # Example
//!
//! ```ignore
//! use secrecy::Secret;
//! use console_client::wrapper::{PCloudClient, AuthState, CryptoState, SyncType};
//!
//! // Initialize the client (singleton - only once per process)
//! let client = PCloudClient::init()?;
//!
//! // Access the client
//! {
//!     let mut guard = client.lock().unwrap();
//!
//!     // Authenticate
//!     let password = Secret::new("my_password".to_string());
//!     guard.authenticate("user@example.com", &password, true)?;
//!
//!     // Start sync
//!     guard.start_sync(None, None);
//!
//!     // Mount the FUSE filesystem
//!     guard.mount_filesystem("/home/user/pCloud")?;
//!
//!     // Setup and start crypto (if needed)
//!     if !guard.is_crypto_setup() {
//!         let crypto_pass = Secret::new("crypto_password".to_string());
//!         guard.setup_crypto(&crypto_pass, "my hint")?;
//!     }
//!
//!     let crypto_pass = Secret::new("crypto_password".to_string());
//!     guard.start_crypto(&crypto_pass)?;
//!
//!     // Add a sync folder
//!     let sync_id = guard.add_sync_by_path(
//!         "/home/user/Documents",
//!         "/Documents",
//!         SyncType::Full
//!     )?;
//!
//!     // Check states
//!     println!("Auth state: {:?}", guard.auth_state());
//!     println!("Crypto state: {:?}", guard.crypto_state());
//!     println!("Mounted at: {:?}", guard.mountpoint());
//! }
//!
//! // Client will be cleaned up when dropped (filesystem unmounted automatically)
//! ```
//!
//! # Thread Safety
//!
//! The `PCloudClient` is wrapped in `Arc<Mutex<>>` to ensure thread-safe access.
//! The underlying pclsync library uses internal threading, but API calls should
//! generally be made from a single thread.
//!
//! # Submodules
//!
//! - `client`: Main `PCloudClient` struct and lifecycle management
//! - `auth`: Authentication operations (login, logout, registration)
//! - `crypto`: Crypto operations (setup, start, stop)
//! - `filesystem`: Filesystem mount/unmount and sync folder management

mod auth;
mod client;
mod crypto;
mod filesystem;

// Re-export the main types
pub use client::{AuthState, CryptoState, PCloudClient};
pub use filesystem::{SyncFolder, SyncType};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_accessible() {
        let state = AuthState::NotAuthenticated;
        assert_eq!(state, AuthState::NotAuthenticated);
    }

    #[test]
    fn test_crypto_state_accessible() {
        let state = CryptoState::NotSetup;
        assert_eq!(state, CryptoState::NotSetup);
    }

    #[test]
    fn test_states_debug() {
        // Ensure Debug is implemented
        let auth = AuthState::Authenticated;
        let crypto = CryptoState::Started;
        assert!(!format!("{:?}", auth).is_empty());
        assert!(!format!("{:?}", crypto).is_empty());
    }

    #[test]
    fn test_states_clone() {
        // Ensure Clone is implemented
        let auth = AuthState::Failed("test".to_string());
        let auth_clone = auth.clone();
        assert_eq!(auth, auth_clone);

        let crypto = CryptoState::Failed("test".to_string());
        let crypto_clone = crypto.clone();
        assert_eq!(crypto, crypto_clone);
    }
}
