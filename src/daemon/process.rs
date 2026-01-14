
//! Process daemonization functionality.
//!
//! This module provides functionality to daemonize the pCloud client process
//! using the double-fork pattern. It handles:
//! - PID file management
//! - Process backgrounding
//! - Daemon lifecycle management

use std::fs;
use std::path::PathBuf;

use daemonize::Daemonize;

use crate::error::{DaemonError, PCloudError, Result};

/// Configuration for the daemon process.
///
/// This struct holds all configuration options for running the pCloud client
/// as a background daemon.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Path to the PID file.
    ///
    /// The PID file stores the process ID of the running daemon,
    /// allowing client instances to find and communicate with it.
    pub pid_file: PathBuf,

    /// Working directory for the daemon.
    ///
    /// The daemon will change to this directory after forking.
    /// Using a temp directory avoids issues with the original directory
    /// being deleted or unmounted.
    pub working_directory: PathBuf,

    /// User to run as (optional).
    ///
    /// If specified, the daemon will drop privileges to this user
    /// after daemonizing.
    pub user: Option<String>,

    /// Group to run as (optional).
    ///
    /// If specified, the daemon will drop privileges to this group
    /// after daemonizing.
    pub group: Option<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        // Use UID to create user-specific paths
        let uid = unsafe { libc::getuid() };
        Self {
            pid_file: PathBuf::from(format!("/tmp/pcloud-{}.pid", uid)),
            working_directory: std::env::temp_dir(),
            user: None,
            group: None,
        }
    }
}

impl DaemonConfig {
    /// Create a new DaemonConfig with custom PID file path.
    ///
    /// # Arguments
    ///
    /// * `pid_file` - Path where the PID file should be created
    pub fn with_pid_file(pid_file: PathBuf) -> Self {
        Self {
            pid_file,
            ..Default::default()
        }
    }

    /// Get the default socket path for IPC.
    ///
    /// Returns a user-specific socket path in /tmp.
    pub fn socket_path(&self) -> PathBuf {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/pcloud-{}.sock", uid))
    }

    /// Set the user to run as after daemonizing.
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Set the group to run as after daemonizing.
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Set the working directory for the daemon.
    pub fn working_directory(mut self, path: PathBuf) -> Self {
        self.working_directory = path;
        self
    }
}

/// Daemonize the current process.
///
/// This function forks the current process and the parent exits,
/// leaving the child running as a background daemon. The daemon:
/// - Detaches from the controlling terminal
/// - Creates a new session
/// - Changes to the configured working directory
/// - Writes its PID to the PID file
///
/// # Arguments
///
/// * `config` - Configuration for the daemon process
///
/// # Errors
///
/// Returns an error if:
/// - The PID file directory cannot be created
/// - Daemonization fails (fork, setsid, etc.)
/// - The user/group specified doesn't exist
///
/// # Safety
///
/// After this function returns successfully, the calling process
/// is the daemon child. The original parent process has already exited.
pub fn daemonize(config: &DaemonConfig) -> Result<()> {
    // Ensure PID file directory exists
    if let Some(parent) = config.pid_file.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| {
                PCloudError::Daemon(DaemonError::PidFile(format!(
                    "failed to create PID file directory: {}",
                    e
                )))
            })?;
        }
    }

    let mut daemon = Daemonize::new()
        .pid_file(&config.pid_file)
        .chown_pid_file(true)
        .working_directory(&config.working_directory);

    // Optional user/group for privilege dropping
    if let Some(ref user) = config.user {
        daemon = daemon.user(user.as_str());
    }
    if let Some(ref group) = config.group {
        daemon = daemon.group(group.as_str());
    }

    daemon
        .start()
        .map_err(|e| PCloudError::Daemon(DaemonError::DaemonizeFailed(e.to_string())))
}

/// Check if a daemon is already running.
///
/// This function checks if:
/// 1. A PID file exists
/// 2. The PID in the file corresponds to a running process
///
/// # Arguments
///
/// * `config` - Daemon configuration containing PID file path
///
/// # Returns
///
/// `true` if a daemon process is running, `false` otherwise.
pub fn is_daemon_running(config: &DaemonConfig) -> bool {
    if !config.pid_file.exists() {
        return false;
    }

    // Read PID and check if process exists
    if let Ok(pid_str) = fs::read_to_string(&config.pid_file) {
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            // Check if process exists by sending signal 0
            // This doesn't actually send a signal, just checks if the process exists
            // and we have permission to signal it
            unsafe {
                return libc::kill(pid, 0) == 0;
            }
        }
    }

    false
}

/// Get the PID of the running daemon.
///
/// # Arguments
///
/// * `config` - Daemon configuration containing PID file path
///
/// # Returns
///
/// The PID of the running daemon if available, `None` otherwise.
pub fn get_daemon_pid(config: &DaemonConfig) -> Option<i32> {
    fs::read_to_string(&config.pid_file)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

/// Remove the PID file.
///
/// This should be called during daemon shutdown to clean up.
/// Errors are silently ignored since cleanup failure is not critical.
///
/// # Arguments
///
/// * `config` - Daemon configuration containing PID file path
pub fn cleanup_pid_file(config: &DaemonConfig) {
    let _ = fs::remove_file(&config.pid_file);
}

/// Stop the running daemon.
///
/// Sends SIGTERM to the daemon process to request graceful shutdown.
///
/// # Arguments
///
/// * `config` - Daemon configuration containing PID file path
///
/// # Errors
///
/// Returns an error if:
/// - The daemon is not running
/// - Failed to send signal to the daemon
pub fn stop_daemon(config: &DaemonConfig) -> Result<()> {
    let pid = get_daemon_pid(config).ok_or(PCloudError::Daemon(DaemonError::NotRunning))?;

    // Send SIGTERM for graceful shutdown
    let result = unsafe { libc::kill(pid, libc::SIGTERM) };

    if result == 0 {
        Ok(())
    } else {
        let errno = std::io::Error::last_os_error();
        if errno.raw_os_error() == Some(libc::ESRCH) {
            // Process doesn't exist - clean up stale PID file
            cleanup_pid_file(config);
            Err(PCloudError::Daemon(DaemonError::NotRunning))
        } else {
            Err(PCloudError::Daemon(DaemonError::DaemonizeFailed(format!(
                "failed to send signal: {}",
                errno
            ))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        let uid = unsafe { libc::getuid() };

        assert_eq!(
            config.pid_file,
            PathBuf::from(format!("/tmp/pcloud-{}.pid", uid))
        );
        assert!(config.user.is_none());
        assert!(config.group.is_none());
    }

    #[test]
    fn test_daemon_config_with_pid_file() {
        let config = DaemonConfig::with_pid_file(PathBuf::from("/tmp/test.pid"));
        assert_eq!(config.pid_file, PathBuf::from("/tmp/test.pid"));
    }

    #[test]
    fn test_daemon_config_socket_path() {
        let config = DaemonConfig::default();
        let uid = unsafe { libc::getuid() };
        let expected = PathBuf::from(format!("/tmp/pcloud-{}.sock", uid));
        assert_eq!(config.socket_path(), expected);
    }

    #[test]
    fn test_daemon_config_builder_methods() {
        let config = DaemonConfig::default()
            .user("testuser")
            .group("testgroup")
            .working_directory(PathBuf::from("/var/tmp"));

        assert_eq!(config.user, Some("testuser".to_string()));
        assert_eq!(config.group, Some("testgroup".to_string()));
        assert_eq!(config.working_directory, PathBuf::from("/var/tmp"));
    }

    #[test]
    fn test_is_daemon_running_no_pid_file() {
        let config = DaemonConfig::with_pid_file(PathBuf::from("/tmp/nonexistent.pid"));
        assert!(!is_daemon_running(&config));
    }

    #[test]
    fn test_is_daemon_running_with_current_process() {
        // Create a temp PID file with our own PID
        let temp_dir = std::env::temp_dir();
        let pid_file = temp_dir.join("test_daemon.pid");

        let pid = std::process::id();
        let mut file = File::create(&pid_file).unwrap();
        writeln!(file, "{}", pid).unwrap();
        drop(file);

        let config = DaemonConfig::with_pid_file(pid_file.clone());
        assert!(is_daemon_running(&config));

        // Cleanup
        let _ = fs::remove_file(&pid_file);
    }

    #[test]
    fn test_get_daemon_pid() {
        let temp_dir = std::env::temp_dir();
        let pid_file = temp_dir.join("test_daemon_pid.pid");

        // Write a test PID
        let mut file = File::create(&pid_file).unwrap();
        writeln!(file, "12345").unwrap();
        drop(file);

        let config = DaemonConfig::with_pid_file(pid_file.clone());
        assert_eq!(get_daemon_pid(&config), Some(12345));

        // Cleanup
        let _ = fs::remove_file(&pid_file);
    }

    #[test]
    fn test_get_daemon_pid_invalid_content() {
        let temp_dir = std::env::temp_dir();
        let pid_file = temp_dir.join("test_daemon_invalid.pid");

        // Write invalid content
        let mut file = File::create(&pid_file).unwrap();
        writeln!(file, "not_a_number").unwrap();
        drop(file);

        let config = DaemonConfig::with_pid_file(pid_file.clone());
        assert_eq!(get_daemon_pid(&config), None);

        // Cleanup
        let _ = fs::remove_file(&pid_file);
    }

    #[test]
    fn test_cleanup_pid_file() {
        let temp_dir = std::env::temp_dir();
        let pid_file = temp_dir.join("test_cleanup.pid");

        // Create a test file
        File::create(&pid_file).unwrap();
        assert!(pid_file.exists());

        let config = DaemonConfig::with_pid_file(pid_file.clone());
        cleanup_pid_file(&config);

        assert!(!pid_file.exists());
    }
}
