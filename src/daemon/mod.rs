//! Daemon mode and IPC functionality.
//!
//! This module provides:
//! - Process daemonization (double-fork pattern)
//! - Signal handling (SIGTERM, SIGHUP, SIGINT)
//! - PID file management
//! - Unix domain socket IPC server
//! - IPC client for sending commands to daemon
//!
//! # Architecture
//!
//! When running in daemon mode (`-d` flag):
//! 1. Process daemonizes using double-fork via the `daemonize` crate
//! 2. Creates PID file for tracking at `/tmp/pcloud-cli-<uid>.pid`
//! 3. Sets up signal handlers for graceful shutdown
//! 4. Opens Unix domain socket at `/tmp/pcloud-cli-<uid>.sock` for IPC
//! 5. Listens for commands from client instances
//!
//! When running in commands-only mode (`-k` flag):
//! 1. Connects to existing daemon via Unix socket
//! 2. Sends command and waits for response
//! 3. Displays result and exits
//!
//! # Example
//!
//! ```ignore
//! use console_client::daemon::{DaemonConfig, daemonize, is_daemon_running, setup_daemon_signals};
//!
//! let config = DaemonConfig::default();
//!
//! // Check if daemon is already running
//! if is_daemon_running(&config) {
//!     eprintln!("Daemon is already running");
//!     return;
//! }
//!
//! // Daemonize the process
//! daemonize(&config)?;
//!
//! // Set up signal handlers
//! setup_daemon_signals()?;
//!
//! // Main daemon loop
//! while !is_shutdown_requested() {
//!     // Do work...
//! }
//! ```
//!
//! # IPC Example
//!
//! ```ignore
//! use console_client::daemon::{DaemonClient, DaemonCommand, DaemonResponse, DaemonConfig};
//!
//! let config = DaemonConfig::default();
//!
//! // Connect to running daemon
//! let client = DaemonClient::new(config.socket_path());
//!
//! // Send a command
//! match client.send_command(DaemonCommand::Status)? {
//!     DaemonResponse::Status { authenticated, crypto_started, mounted, mountpoint } => {
//!         println!("Authenticated: {}", authenticated);
//!         println!("Crypto started: {}", crypto_started);
//!         println!("Mounted: {}", mounted);
//!     }
//!     _ => {}
//! }
//! ```
//!
//! # Submodules
//!
//! - `process`: Daemonization logic, PID file management
//! - `signals`: Signal handling for graceful shutdown
//! - `ipc`: Unix domain socket IPC server and client

pub mod ipc;
pub mod process;
pub mod signals;

// Re-export commonly used items from process module
pub use process::{
    cleanup_pid_file, daemonize, get_daemon_pid, is_daemon_running, stop_daemon, DaemonConfig,
};

// Re-export commonly used items from signals module
pub use signals::{
    is_reload_requested, is_shutdown_requested, request_shutdown, setup_daemon_signals,
};

// Re-export commonly used items from ipc module
pub use ipc::{DaemonClient, DaemonCommand, DaemonResponse, DaemonServer};

/// Initialize the daemon module.
///
/// This is a placeholder that verifies the module is correctly loaded.
/// No actual initialization is required.
pub fn init() {
    // Module is ready to use
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_config_accessible() {
        let config = DaemonConfig::default();
        assert!(!config.pid_file.as_os_str().is_empty());
    }

    #[test]
    fn test_exports_work() {
        // Verify all exports are accessible
        let _: fn(&DaemonConfig) -> crate::Result<()> = daemonize;
        let _: fn(&DaemonConfig) -> bool = is_daemon_running;
        let _: fn(&DaemonConfig) -> Option<i32> = get_daemon_pid;
        let _: fn(&DaemonConfig) = cleanup_pid_file;
        let _: fn(&DaemonConfig) -> crate::Result<()> = stop_daemon;
        let _: fn() -> crate::Result<()> = setup_daemon_signals;
        let _: fn() -> bool = is_shutdown_requested;
        let _: fn() -> bool = is_reload_requested;
        let _: fn() = request_shutdown;
    }

    #[test]
    fn test_ipc_exports_work() {
        // Verify IPC exports are accessible
        let _cmd = DaemonCommand::Ping;
        let _resp = DaemonResponse::Pong;

        // DaemonClient and DaemonServer are struct types
        // Just verify they're accessible
        let _: fn(std::path::PathBuf) -> DaemonClient = |p| DaemonClient::new(p);
    }

    #[test]
    fn test_init_does_not_panic() {
        init();
    }
}
