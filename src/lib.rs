//! pCloud Console Client Library
//!
//! This library provides safe Rust bindings to the pclsync C library and
//! supporting functionality for the pCloud console client.
//!
//! # Modules
//!
//! - `cli`: Command-line argument parsing and command execution
//! - `ffi`: FFI bindings to the pclsync C library
//! - `wrapper`: Safe Rust wrappers over the FFI bindings
//! - `daemon`: Daemon mode and IPC functionality
//! - `security`: Secure password handling
//! - `utils`: Common utility functions
//! - `error`: Error types for the application
//!
//! # Example
//!
//! ```ignore
//! use console_client::wrapper::PCloudClient;
//! use console_client::ffi::callbacks::{register_status_callback, CallbackConfig};
//!
//! // Set up status callback
//! register_status_callback(|status| {
//!     println!("Status: {}", status.status);
//! });
//!
//! // Initialize client
//! let client = PCloudClient::init()?;
//! ```

pub mod cli;
pub mod crash_reporting;
pub mod daemon;
pub mod error;
pub mod ffi;
pub mod security;
pub mod tui;
pub mod utils;
pub mod wrapper;

// Re-export commonly used types
pub use error::{PCloudError, Result};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_module_accessible() {
        // Verify that error types are accessible
        let _err: Option<PCloudError> = None;
    }

    #[test]
    fn test_modules_compile() {
        // Verify all modules compile correctly
        cli::init();
        // FFI module now has submodules
        let _ = ffi::PSTATUS_READY;
        // Wrapper module exports types
        let _auth_state: wrapper::AuthState = wrapper::AuthState::NotAuthenticated;
        let _crypto_state: wrapper::CryptoState = wrapper::CryptoState::NotSetup;
        daemon::init();
        security::init();
    }

    #[test]
    fn test_ffi_types_accessible() {
        // Verify FFI types are accessible
        let _status: ffi::pstatus_t = Default::default();
        assert_eq!(ffi::status_to_string(ffi::PSTATUS_READY), "Ready");
    }

    #[test]
    fn test_wrapper_types_accessible() {
        // Verify wrapper types are accessible
        use wrapper::{AuthState, CryptoState};

        let auth = AuthState::Authenticated;
        let crypto = CryptoState::Started;

        // Test Clone and PartialEq
        assert_eq!(auth.clone(), AuthState::Authenticated);
        assert_eq!(crypto.clone(), CryptoState::Started);
    }

    #[test]
    fn test_callback_types_accessible() {
        // Verify callback types and functions are accessible
        use ffi::callbacks::{CallbackConfig, CallbackPointers};

        // Test default CallbackPointers
        let pointers = CallbackPointers::default();
        assert!(pointers.status.is_none());
        assert!(pointers.event.is_none());

        // Test CallbackConfig builder
        let _config = CallbackConfig::new();
    }

    #[test]
    fn test_callback_registration_functions() {
        // Verify callback registration functions are accessible
        use ffi::{
            clear_all_callbacks, clear_event_callback, clear_status_callback,
            register_event_callback, register_status_callback,
        };

        // Register a simple callback
        register_status_callback(|_status| {});
        register_event_callback(|_event_type, _event_data| {});

        // Clear callbacks
        clear_status_callback();
        clear_event_callback();
        clear_all_callbacks();
    }

    #[test]
    fn test_trampoline_functions_exist() {
        // Verify trampoline functions are accessible
        use ffi::{
            event_callback_trampoline, fs_start_callback_trampoline,
            notification_callback_trampoline, status_callback_trampoline,
        };

        // These are function pointers - just verify they exist
        let _: unsafe extern "C" fn(*mut ffi::pstatus_t) = status_callback_trampoline;
        let _: unsafe extern "C" fn(ffi::types::psync_eventtype_t, ffi::types::psync_eventdata_t) =
            event_callback_trampoline;
        let _: unsafe extern "C" fn(u32, u32) = notification_callback_trampoline;
        let _: unsafe extern "C" fn() = fs_start_callback_trampoline;
    }

    #[test]
    fn test_overlay_callbacks_accessible() {
        // Verify overlay callbacks are accessible
        use ffi::callbacks::overlay::{
            clear_all_overlay_callbacks, invoke_crypto_start_callback, invoke_crypto_stop_callback,
            register_crypto_start_callback, register_crypto_stop_callback, OverlayCallbackConfig,
        };

        // Register callbacks
        register_crypto_start_callback(|| {});
        register_crypto_stop_callback(|| {});

        // Invoke (they should do nothing since no actual callback registered)
        invoke_crypto_start_callback();
        invoke_crypto_stop_callback();

        // Clear
        clear_all_overlay_callbacks();

        // Test config builder
        let _config = OverlayCallbackConfig::new();
    }

    // Phase 6 tests - CLI argument parsing
    #[test]
    fn test_cli_types_accessible() {
        use cli::{Cli, CommandPrompt, InteractiveCommand};

        // Test Cli default
        let cli = Cli::default();
        assert!(cli.auth_token.is_none());
        assert!(!cli.daemonize);

        // Test InteractiveCommand parsing
        let cmd = InteractiveCommand::parse("startcrypto");
        assert_eq!(cmd, InteractiveCommand::StartCrypto);

        // Test CommandPrompt
        let prompt = CommandPrompt::default();
        assert_eq!(prompt.prompt(), "pcloud> ");
    }

    #[test]
    fn test_cli_argument_validation() {
        use cli::Cli;

        // Valid configuration
        let valid_cli = Cli {
            daemonize: true,
            ..Default::default()
        };
        assert!(valid_cli.validate().is_ok());

        // Invalid: daemon and client mode together
        let invalid_cli = Cli {
            daemonize: true,
            commands_only: true,
            ..Default::default()
        };
        assert!(invalid_cli.validate().is_err());

        // Invalid: logout and unlink together
        let invalid_cli2 = Cli {
            logout: true,
            unlink: true,
            ..Default::default()
        };
        assert!(invalid_cli2.validate().is_err());
    }

    #[test]
    fn test_interactive_commands() {
        use cli::InteractiveCommand;

        // Test all command aliases
        assert_eq!(
            InteractiveCommand::parse("start"),
            InteractiveCommand::StartCrypto
        );
        assert_eq!(
            InteractiveCommand::parse("stop"),
            InteractiveCommand::StopCrypto
        );
        assert_eq!(
            InteractiveCommand::parse("fin"),
            InteractiveCommand::Finalize
        );
        assert_eq!(InteractiveCommand::parse("s"), InteractiveCommand::Status);
        assert_eq!(InteractiveCommand::parse("q"), InteractiveCommand::Quit);
        assert_eq!(InteractiveCommand::parse("?"), InteractiveCommand::Help);

        // Test exit commands
        assert!(InteractiveCommand::Quit.is_exit_command());
        assert!(InteractiveCommand::Finalize.is_exit_command());
        assert!(!InteractiveCommand::Status.is_exit_command());

        // Test crypto commands
        assert!(InteractiveCommand::StartCrypto.is_crypto_command());
        assert!(InteractiveCommand::StopCrypto.is_crypto_command());
        assert!(!InteractiveCommand::Quit.is_crypto_command());
    }

    // Phase 6 tests - Security/Password handling
    #[test]
    fn test_security_types_accessible() {
        use security::SecurePassword;

        // Test SecurePassword creation
        let password = SecurePassword::new("test123".to_string());
        assert_eq!(password.expose(), "test123");
        assert!(!password.is_empty());
        assert_eq!(password.len(), 7);

        // Test debug output is redacted
        let debug_output = format!("{:?}", password);
        assert!(debug_output.contains("REDACTED"));
        assert!(!debug_output.contains("test123"));

        // Test display output is redacted
        let display_output = format!("{}", password);
        assert_eq!(display_output, "[REDACTED]");
    }

    #[test]
    fn test_secure_password_conversions() {
        use security::SecurePassword;

        // From String
        let pw1: SecurePassword = "password".to_string().into();
        assert_eq!(pw1.expose(), "password");

        // From &str
        let pw2: SecurePassword = "password".into();
        assert_eq!(pw2.expose(), "password");

        // Clone and equality
        let pw3 = pw1.clone();
        assert_eq!(pw1, pw3);
    }

    #[test]
    fn test_zeroize_string() {
        use security::zeroize_string;

        let mut s = String::from("secret");
        zeroize_string(&mut s);
        assert!(s.is_empty());
    }
}
