//! Daemon integration tests for pCloud console-client.
//!
//! These tests verify the daemon process management functionality,
//! including PID file handling, daemon detection, and configuration.
//!
//! # Note
//!
//! These tests use temporary directories to avoid interfering with
//! any real daemon processes that might be running.

use std::fs::{self, File};
use std::path::PathBuf;
use tempfile::tempdir;

use console_client::daemon::{cleanup_pid_file, get_daemon_pid, is_daemon_running, DaemonConfig};

// ============================================================================
// DaemonConfig Tests
// ============================================================================

#[test]
fn test_daemon_config_default() {
    let config = DaemonConfig::default();

    // PID file should be in /tmp with user-specific name
    let pid_str = config.pid_file.to_string_lossy();
    assert!(pid_str.starts_with("/tmp/pcloud-"));
    assert!(pid_str.ends_with(".pid"));

    // Working directory should be set
    assert!(!config.working_directory.as_os_str().is_empty());

    // User and group should be None by default
    assert!(config.user.is_none());
    assert!(config.group.is_none());
}

#[test]
fn test_daemon_config_with_pid_file() {
    let custom_path = PathBuf::from("/tmp/custom-pcloud.pid");
    let config = DaemonConfig::with_pid_file(custom_path.clone());

    assert_eq!(config.pid_file, custom_path);
}

#[test]
fn test_daemon_config_socket_path() {
    let config = DaemonConfig::default();
    let socket_path = config.socket_path();

    // Socket path should be in /tmp with user-specific name
    let socket_str = socket_path.to_string_lossy();
    assert!(socket_str.starts_with("/tmp/pcloud-"));
    assert!(socket_str.ends_with(".sock"));
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
fn test_daemon_config_debug() {
    let config = DaemonConfig::default();
    let debug_output = format!("{:?}", config);

    // Should contain relevant field names
    assert!(debug_output.contains("pid_file"));
    assert!(debug_output.contains("working_directory"));
}

// ============================================================================
// PID File Detection Tests
// ============================================================================

#[test]
fn test_daemon_not_running_without_pid_file() {
    let dir = tempdir().unwrap();
    let config = DaemonConfig::with_pid_file(dir.path().join("nonexistent.pid"));

    assert!(!is_daemon_running(&config));
    assert!(get_daemon_pid(&config).is_none());
}

#[test]
fn test_daemon_not_running_with_stale_pid() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("stale.pid");

    // Write a PID that almost certainly doesn't exist (very high number)
    // Using 4194304 which is above typical PID_MAX_LIMIT on most systems
    fs::write(&pid_path, "4194304").unwrap();

    let config = DaemonConfig::with_pid_file(pid_path);

    // Should detect that the process doesn't exist
    assert!(!is_daemon_running(&config));
}

#[test]
fn test_daemon_running_with_current_process() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("current.pid");

    // Write our own PID - we're definitely running
    let our_pid = std::process::id();
    fs::write(&pid_path, our_pid.to_string()).unwrap();

    let config = DaemonConfig::with_pid_file(pid_path.clone());

    // Should detect that our process exists
    assert!(is_daemon_running(&config));
    assert_eq!(get_daemon_pid(&config), Some(our_pid as i32));

    // Cleanup
    let _ = fs::remove_file(&pid_path);
}

#[test]
fn test_get_daemon_pid_valid() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("valid.pid");

    fs::write(&pid_path, "12345\n").unwrap();

    let config = DaemonConfig::with_pid_file(pid_path);
    assert_eq!(get_daemon_pid(&config), Some(12345));
}

#[test]
fn test_get_daemon_pid_with_whitespace() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("whitespace.pid");

    fs::write(&pid_path, "  54321  \n").unwrap();

    let config = DaemonConfig::with_pid_file(pid_path);
    assert_eq!(get_daemon_pid(&config), Some(54321));
}

#[test]
fn test_get_daemon_pid_invalid_content() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("invalid.pid");

    // Write non-numeric content
    fs::write(&pid_path, "not_a_number").unwrap();

    let config = DaemonConfig::with_pid_file(pid_path);
    assert_eq!(get_daemon_pid(&config), None);
}

#[test]
fn test_get_daemon_pid_empty_file() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("empty.pid");

    File::create(&pid_path).unwrap();

    let config = DaemonConfig::with_pid_file(pid_path);
    assert_eq!(get_daemon_pid(&config), None);
}

#[test]
fn test_get_daemon_pid_negative_number() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("negative.pid");

    fs::write(&pid_path, "-1").unwrap();

    let config = DaemonConfig::with_pid_file(pid_path);
    // Negative numbers might parse - depends on implementation
    // The important thing is it doesn't crash
    let _pid = get_daemon_pid(&config);
}

// ============================================================================
// PID File Cleanup Tests
// ============================================================================

#[test]
fn test_cleanup_pid_file_removes_file() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("cleanup.pid");

    // Create the file
    File::create(&pid_path).unwrap();
    assert!(pid_path.exists());

    let config = DaemonConfig::with_pid_file(pid_path.clone());
    cleanup_pid_file(&config);

    assert!(!pid_path.exists());
}

#[test]
fn test_cleanup_pid_file_nonexistent_no_error() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("nonexistent.pid");

    // File doesn't exist - cleanup should not error
    let config = DaemonConfig::with_pid_file(pid_path.clone());
    cleanup_pid_file(&config); // Should not panic

    assert!(!pid_path.exists());
}

// ============================================================================
// Socket Path Tests
// ============================================================================

#[test]
fn test_socket_path_is_user_specific() {
    let config1 = DaemonConfig::default();
    let config2 = DaemonConfig::default();

    // Same user should get same socket path
    assert_eq!(config1.socket_path(), config2.socket_path());
}

#[test]
fn test_socket_path_format() {
    let config = DaemonConfig::default();
    let socket_path = config.socket_path();
    let path_str = socket_path.to_string_lossy();

    // Should match the expected format
    assert!(path_str.contains("pcloud-"));
    assert!(path_str.ends_with(".sock"));
}

// ============================================================================
// Configuration Consistency Tests
// ============================================================================

#[test]
fn test_pid_and_socket_share_uid() {
    let config = DaemonConfig::default();

    let pid_str = config.pid_file.to_string_lossy();
    let sock_path = config.socket_path();
    let sock_str = sock_path.to_string_lossy();

    // Extract UID from both paths - they should match
    // Format: /tmp/pcloud-{uid}.pid and /tmp/pcloud-{uid}.sock
    let pid_uid: Vec<&str> = pid_str.split("pcloud-").collect();
    let sock_uid: Vec<&str> = sock_str.split("pcloud-").collect();

    if pid_uid.len() > 1 && sock_uid.len() > 1 {
        let pid_uid = pid_uid[1].trim_end_matches(".pid");
        let sock_uid = sock_uid[1].trim_end_matches(".sock");
        assert_eq!(pid_uid, sock_uid);
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_pid_file_with_special_characters() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("test-daemon_v2.pid");

    fs::write(&pid_path, "1234").unwrap();

    let config = DaemonConfig::with_pid_file(pid_path);
    assert_eq!(get_daemon_pid(&config), Some(1234));
}

#[test]
fn test_pid_file_in_nested_directory() {
    let dir = tempdir().unwrap();
    let nested_dir = dir.path().join("nested").join("dir");
    fs::create_dir_all(&nested_dir).unwrap();

    let pid_path = nested_dir.join("daemon.pid");
    fs::write(&pid_path, "9999").unwrap();

    let config = DaemonConfig::with_pid_file(pid_path);
    assert_eq!(get_daemon_pid(&config), Some(9999));
}

#[test]
fn test_large_pid_number() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("large.pid");

    // Maximum PID on Linux is typically 4194304 (2^22)
    fs::write(&pid_path, "4194303").unwrap();

    let config = DaemonConfig::with_pid_file(pid_path);
    assert_eq!(get_daemon_pid(&config), Some(4194303));
}

#[test]
fn test_concurrent_pid_file_access() {
    use std::sync::Arc;
    use std::thread;

    let dir = tempdir().unwrap();
    let pid_path = Arc::new(dir.path().join("concurrent.pid"));

    // Write initial PID
    fs::write(pid_path.as_ref(), "1111").unwrap();

    let config = DaemonConfig::with_pid_file(pid_path.as_ref().to_path_buf());

    // Multiple threads reading simultaneously
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let cfg = config.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = get_daemon_pid(&cfg);
                    let _ = is_daemon_running(&cfg);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}
