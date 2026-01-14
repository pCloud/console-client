//! Signal handling for daemon mode.
//!
//! This module provides signal handlers for the daemon process:
//! - SIGTERM: Graceful shutdown
//! - SIGINT: Graceful shutdown (Ctrl+C)
//! - SIGHUP: Configuration reload (logged, no action)
//!
//! Signal handlers must be async-signal-safe, meaning they cannot:
//! - Allocate memory
//! - Use locks
//! - Call non-reentrant functions
//!
//! We use atomic booleans to safely communicate between signal handlers
//! and the main daemon loop.

use std::sync::atomic::{AtomicBool, Ordering};

use nix::sys::signal::{self, SigHandler, Signal};

use crate::error::{DaemonError, PCloudError, Result};

/// Atomic flag indicating a shutdown has been requested.
///
/// Set by SIGTERM or SIGINT signal handlers.
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Atomic flag indicating a reload has been requested.
///
/// Set by SIGHUP signal handler. The daemon can check this flag
/// and reload configuration if supported.
static RELOAD_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Set up signal handlers for daemon mode.
///
/// Installs handlers for:
/// - SIGTERM: Requests graceful shutdown
/// - SIGINT: Requests graceful shutdown
/// - SIGHUP: Requests configuration reload
///
/// # Errors
///
/// Returns an error if signal handlers cannot be installed.
///
/// # Safety
///
/// This function installs C signal handlers using `nix::sys::signal::signal`.
/// The handlers are async-signal-safe and only modify atomic booleans.
pub fn setup_daemon_signals() -> Result<()> {
    unsafe {
        // SIGTERM - graceful shutdown request
        signal::signal(Signal::SIGTERM, SigHandler::Handler(handle_sigterm)).map_err(|e| {
            PCloudError::Daemon(DaemonError::Signal(format!(
                "failed to set SIGTERM handler: {}",
                e
            )))
        })?;

        // SIGHUP - reload configuration
        signal::signal(Signal::SIGHUP, SigHandler::Handler(handle_sighup)).map_err(|e| {
            PCloudError::Daemon(DaemonError::Signal(format!(
                "failed to set SIGHUP handler: {}",
                e
            )))
        })?;

        // SIGINT - same as SIGTERM (for when attached to terminal during testing)
        signal::signal(Signal::SIGINT, SigHandler::Handler(handle_sigterm)).map_err(|e| {
            PCloudError::Daemon(DaemonError::Signal(format!(
                "failed to set SIGINT handler: {}",
                e
            )))
        })?;
    }

    Ok(())
}

/// Signal handler for SIGTERM and SIGINT.
///
/// Sets the shutdown requested flag. This handler is async-signal-safe
/// because it only performs an atomic store.
///
/// # Safety
///
/// This is a C signal handler called from signal context.
/// It must only use async-signal-safe operations.
extern "C" fn handle_sigterm(_: libc::c_int) {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

/// Signal handler for SIGHUP.
///
/// Sets the reload requested flag. This handler is async-signal-safe
/// because it only performs an atomic store.
///
/// # Safety
///
/// This is a C signal handler called from signal context.
/// It must only use async-signal-safe operations.
extern "C" fn handle_sighup(_: libc::c_int) {
    RELOAD_REQUESTED.store(true, Ordering::SeqCst);
}

/// Check if a shutdown has been requested.
///
/// Returns `true` if SIGTERM or SIGINT has been received.
/// The daemon main loop should check this periodically and
/// initiate graceful shutdown when it returns `true`.
pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

/// Check and clear the reload requested flag.
///
/// Returns `true` if SIGHUP has been received since the last check.
/// The flag is cleared after reading, so subsequent calls will return
/// `false` until another SIGHUP is received.
///
/// This allows the daemon to detect reload requests and take action
/// without missing signals that arrive while processing.
pub fn is_reload_requested() -> bool {
    RELOAD_REQUESTED.swap(false, Ordering::SeqCst)
}

/// Request shutdown programmatically.
///
/// This can be used to trigger a shutdown from code without
/// relying on signals (e.g., from an IPC command).
pub fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

/// Reset all signal flags.
///
/// This is primarily useful for testing. In normal operation,
/// signal flags should not be reset.
#[cfg(test)]
pub fn reset_signal_flags() {
    SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);
    RELOAD_REQUESTED.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_flag_default() {
        reset_signal_flags();
        assert!(!is_shutdown_requested());
    }

    #[test]
    fn test_shutdown_flag_set() {
        reset_signal_flags();
        request_shutdown();
        assert!(is_shutdown_requested());
        reset_signal_flags();
    }

    #[test]
    fn test_reload_flag_clears_on_read() {
        reset_signal_flags();

        // Simulate SIGHUP
        RELOAD_REQUESTED.store(true, Ordering::SeqCst);

        // First read should return true
        assert!(is_reload_requested());

        // Second read should return false (flag was cleared)
        assert!(!is_reload_requested());

        reset_signal_flags();
    }

    #[test]
    fn test_request_shutdown() {
        reset_signal_flags();
        assert!(!is_shutdown_requested());

        request_shutdown();
        assert!(is_shutdown_requested());

        reset_signal_flags();
    }

    #[test]
    fn test_signal_handlers_are_valid() {
        // Verify the handlers have the correct signature
        let _: extern "C" fn(libc::c_int) = handle_sigterm;
        let _: extern "C" fn(libc::c_int) = handle_sighup;
    }

    // Note: We don't test actual signal delivery here because:
    // 1. It would affect the test process
    // 2. Signal handling in tests can interfere with the test runner
    // Integration tests should verify signal handling in a subprocess
}
