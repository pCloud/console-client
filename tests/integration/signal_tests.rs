//! Signal handling integration tests for pCloud console-client.
//!
//! These tests verify that the application responds correctly to
//! Unix signals (SIGINT, SIGTERM) by spawning it as a subprocess
//! and sending signals to it.

use std::process::Stdio;
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;

/// Spawn the pcloud binary as a child process.
///
/// Uses `-m` with a temp path so it enters foreground mode.
/// The process will attempt `psync_init()` which may block,
/// giving us a window to test signal handling.
fn spawn_pcloud(mountpoint: &str) -> std::process::Child {
    std::process::Command::cargo_bin("pcloud-cli")
        .expect("Failed to find pcloud-cli binary")
        .args(["-m", mountpoint])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn pcloud process")
}

/// Wait for a child process to exit within a timeout.
///
/// Returns `Some(ExitStatus)` if the process exited, or `None` if it
/// did not exit within the timeout (the process is killed in that case).
fn wait_for_exit(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Option<std::process::ExitStatus> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

#[test]
fn test_sigint_terminates_process() {
    let dir = tempfile::tempdir().unwrap();
    let mountpoint = dir.path().join("sigint-test");

    let mut child = spawn_pcloud(mountpoint.to_str().unwrap());
    let pid = child.id() as i32;

    // Give the process time to start and register signal handlers
    std::thread::sleep(Duration::from_millis(500));

    // Send SIGINT (equivalent to Ctrl+C)
    let kill_result = unsafe { libc::kill(pid, libc::SIGINT) };
    assert_eq!(kill_result, 0, "Failed to send SIGINT to process {pid}");

    // Process should exit within 5 seconds
    let status = wait_for_exit(&mut child, Duration::from_secs(5));
    assert!(
        status.is_some(),
        "Process did not exit within 5 seconds after SIGINT"
    );
}

#[test]
fn test_sigterm_terminates_process() {
    let dir = tempfile::tempdir().unwrap();
    let mountpoint = dir.path().join("sigterm-test");

    let mut child = spawn_pcloud(mountpoint.to_str().unwrap());
    let pid = child.id() as i32;

    // Give the process time to start
    std::thread::sleep(Duration::from_millis(500));

    // Send SIGTERM
    let kill_result = unsafe { libc::kill(pid, libc::SIGTERM) };
    assert_eq!(kill_result, 0, "Failed to send SIGTERM to process {pid}");

    // Process should exit within 5 seconds
    let status = wait_for_exit(&mut child, Duration::from_secs(5));
    assert!(
        status.is_some(),
        "Process did not exit within 5 seconds after SIGTERM"
    );
}

#[test]
fn test_double_sigint_force_terminates() {
    let dir = tempfile::tempdir().unwrap();
    let mountpoint = dir.path().join("double-sigint-test");

    let mut child = spawn_pcloud(mountpoint.to_str().unwrap());
    let pid = child.id() as i32;

    // Give the process time to start
    std::thread::sleep(Duration::from_millis(500));

    // Send first SIGINT
    unsafe { libc::kill(pid, libc::SIGINT) };

    // Brief pause, then send second SIGINT
    std::thread::sleep(Duration::from_millis(200));
    unsafe { libc::kill(pid, libc::SIGINT) };

    // Process should exit within 5 seconds
    let status = wait_for_exit(&mut child, Duration::from_secs(5));
    assert!(
        status.is_some(),
        "Process did not exit within 5 seconds after double SIGINT"
    );
}
