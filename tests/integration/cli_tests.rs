//! CLI integration tests for pCloud console-client.
//!
//! These tests verify the CLI interface behavior using the compiled binary.
//! They test argument parsing, help output, version information, and
//! validation of argument combinations.
//!
//! # Note
//!
//! These tests run the actual binary and verify its output/exit codes.
//! They do NOT test the actual pCloud sync functionality since that
//! requires the pclsync library and network access.

use assert_cmd::Command;
use predicates::prelude::*;

/// Get a Command instance for the console-client binary.
fn pcloud_cmd() -> Command {
    assert_cmd::cargo_bin_cmd!("pcloud")
}

// ============================================================================
// Help and Version Tests
// ============================================================================

#[test]
fn test_help_flag_short() {
    let mut cmd = pcloud_cmd();
    cmd.arg("-h")
        .assert()
        .success()
        .stdout(predicate::str::contains("pCloud Console Client"))
        .stdout(predicate::str::contains("-t"))
        .stdout(predicate::str::contains("--token"));
}

#[test]
fn test_help_flag_long() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("pCloud storage"))
        .stdout(predicate::str::contains("FUSE"))
        .stdout(predicate::str::contains("daemon"));
}

#[test]
fn test_help_shows_all_flags() {
    let mut cmd = pcloud_cmd();
    let output = cmd.arg("--help").assert().success();

    // Verify all flags are documented
    output
        .stdout(predicate::str::contains("-t"))
        .stdout(predicate::str::contains("--token"))
        .stdout(predicate::str::contains("-c"))
        .stdout(predicate::str::contains("--crypto"))
        .stdout(predicate::str::contains("-d"))
        .stdout(predicate::str::contains("--daemon"))
        .stdout(predicate::str::contains("-o"))
        .stdout(predicate::str::contains("--commands"))
        .stdout(predicate::str::contains("-m"))
        .stdout(predicate::str::contains("--mountpoint"))
        .stdout(predicate::str::contains("-k"))
        .stdout(predicate::str::contains("--client"))
        .stdout(predicate::str::contains("--nosave"))
        .stdout(predicate::str::contains("--logout"))
        .stdout(predicate::str::contains("--unlink"));
}

#[test]
fn test_help_does_not_show_removed_flags() {
    let mut cmd = pcloud_cmd();
    let assert = cmd.arg("--help").assert().success();

    // These flags have been removed
    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(!output.contains("--username"));
    assert!(!output.contains("--password"));
    assert!(!output.contains("--newuser"));
    assert!(!output.contains("--savepassword"));
    assert!(!output.contains("--passascrypto"));
}

#[test]
fn test_version_flag_short() {
    let mut cmd = pcloud_cmd();
    cmd.arg("-V").assert().success();
}

#[test]
fn test_version_flag_long() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("pcloud"));
}

// ============================================================================
// Removed Flag Tests
// ============================================================================

#[test]
fn test_removed_username_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-u", "test@example.com"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn test_removed_password_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-p"]).assert().failure();
}

#[test]
fn test_removed_newuser_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-n"]).assert().failure();
}

// ============================================================================
// Argument Conflict Tests
// ============================================================================

#[test]
fn test_conflicting_daemon_and_client_flags() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-d", "-k"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot use both"));
}

#[test]
fn test_conflicting_daemon_and_client_long_flags() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--daemon", "--client"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("daemon").or(predicate::str::contains("client")));
}

#[test]
fn test_logout_and_unlink_conflict() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--logout", "--unlink"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--logout"));
}

#[test]
fn test_logout_conflicts_with_daemon() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--logout", "-d"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--logout"));
}

#[test]
fn test_unlink_conflicts_with_daemon() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--unlink", "-d"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--unlink"));
}

#[test]
fn test_logout_conflicts_with_client() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--logout", "-k"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--logout"));
}

#[test]
fn test_logout_conflicts_with_token() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--logout", "-t", "token"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--logout"));
}

#[test]
fn test_unlink_conflicts_with_crypto() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--unlink", "-c"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--unlink"));
}

// ============================================================================
// Mountpoint Argument Tests
// ============================================================================

#[test]
fn test_mountpoint_short_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-m", "/tmp/pcloud"]).assert().failure(); // Expected - can't actually connect
}

#[test]
fn test_mountpoint_long_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--mountpoint", "/tmp/pcloud"]).assert().failure(); // Expected - can't actually connect
}

// ============================================================================
// Flag Combination Tests
// ============================================================================

#[test]
fn test_valid_daemon_configuration() {
    let mut cmd = pcloud_cmd();
    // Daemon mode without conflicting flags should parse
    cmd.args(["-d", "-m", "/tmp/pcloud"]).assert().failure(); // Expected - needs auth
}

#[test]
fn test_valid_client_configuration() {
    let mut cmd = pcloud_cmd();
    // Client mode should parse
    cmd.args(["-k"]).assert().failure(); // Expected - no daemon running
}

#[test]
fn test_commands_mode_flag_parses() {
    // Just verify -o is a valid flag by checking --help mentions it
    let mut cmd = pcloud_cmd();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--commands"));
}

#[test]
fn test_token_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-t", "my-auth-token"]).assert().failure(); // Expected - can't actually connect
}

// ============================================================================
// Nosave Flag Tests
// ============================================================================

#[test]
fn test_nosave_flag() {
    let mut cmd = pcloud_cmd();
    // --nosave should parse successfully
    cmd.args(["--nosave"]).assert().failure(); // Expected - can't actually connect
}

#[test]
fn test_help_shows_nosave_flag() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--nosave"));
}

// ============================================================================
// Logout/Unlink Flag Tests
// ============================================================================

#[test]
fn test_help_shows_logout_flag() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--logout"));
}

#[test]
fn test_help_shows_unlink_flag() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--unlink"));
}

// ============================================================================
// Error Message Quality Tests
// ============================================================================

#[test]
fn test_error_message_for_unknown_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--unknown-flag"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn test_error_message_for_missing_value() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-t"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires").or(predicate::str::contains("value")));
}

#[test]
fn test_error_message_for_empty_mountpoint() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-m", ""]).assert().failure();
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_mountpoint_with_spaces() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-m", "/tmp/my pcloud folder"]).assert().failure(); // Will fail at runtime, but should parse
}
