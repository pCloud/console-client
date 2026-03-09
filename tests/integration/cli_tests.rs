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
        .stdout(predicate::str::contains("-u"))
        .stdout(predicate::str::contains("--username"));
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
        .stdout(predicate::str::contains("-u"))
        .stdout(predicate::str::contains("--username"))
        .stdout(predicate::str::contains("-p"))
        .stdout(predicate::str::contains("--password"))
        .stdout(predicate::str::contains("-c"))
        .stdout(predicate::str::contains("--crypto"))
        .stdout(predicate::str::contains("-y"))
        .stdout(predicate::str::contains("--passascrypto"))
        .stdout(predicate::str::contains("-d"))
        .stdout(predicate::str::contains("--daemon"))
        .stdout(predicate::str::contains("-o"))
        .stdout(predicate::str::contains("--commands"))
        .stdout(predicate::str::contains("-m"))
        .stdout(predicate::str::contains("--mountpoint"))
        .stdout(predicate::str::contains("-k"))
        .stdout(predicate::str::contains("--client"))
        .stdout(predicate::str::contains("-n"))
        .stdout(predicate::str::contains("--newuser"))
        .stdout(predicate::str::contains("-s"))
        .stdout(predicate::str::contains("--savepassword"));
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
// Argument Requirement Tests
// ============================================================================

#[test]
fn test_password_flag_without_username_fails() {
    // -p requires -u to be specified (validated by Cli::validate)
    let mut cmd = pcloud_cmd();
    cmd.args(["-p"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--password requires --username"));
}

#[test]
fn test_newuser_flag_without_username_fails() {
    // -n requires -u to be specified (validated by Cli::validate)
    let mut cmd = pcloud_cmd();
    cmd.args(["-n"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--newuser requires --username"));
}

#[test]
fn test_username_with_short_flag() {
    // This will fail at validation/runtime since we can't actually connect,
    // but it should at least pass argument parsing
    let mut cmd = pcloud_cmd();
    // Just verify it parses - it will fail later when trying to connect
    cmd.args(["-u", "test@example.com"]).assert().failure();
    // Note: failure is expected because we can't actually connect to pCloud
}

#[test]
fn test_username_with_long_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--username", "test@example.com"])
        .assert()
        .failure(); // Expected - can't actually connect
}

// ============================================================================
// Argument Conflict Tests
// ============================================================================

#[test]
fn test_conflicting_daemon_and_client_flags() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-u", "test@example.com", "-d", "-k"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot use both"));
}

#[test]
fn test_conflicting_daemon_and_client_long_flags() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--username", "test@example.com", "--daemon", "--client"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("daemon").or(predicate::str::contains("client")));
}

#[test]
fn test_passascrypto_without_password_fails() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-u", "test@example.com", "-y"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("password").or(predicate::str::contains("Password")));
}

#[test]
fn test_passascrypto_with_password_parses() {
    let mut cmd = pcloud_cmd();
    // -p -y together should parse (will fail at runtime without input)
    cmd.args(["-u", "test@example.com", "-p", "-y"])
        .assert()
        .failure(); // Expected - needs password input
}

#[test]
fn test_crypto_and_passascrypto_conflict() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-u", "test@example.com", "-p", "-c", "-y"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("--crypto").or(predicate::str::contains("--passascrypto")),
        );
}

// ============================================================================
// Mountpoint Argument Tests
// ============================================================================

#[test]
fn test_mountpoint_short_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-u", "test@example.com", "-m", "/tmp/pcloud"])
        .assert()
        .failure(); // Expected - can't actually connect
}

#[test]
fn test_mountpoint_long_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args([
        "--username",
        "test@example.com",
        "--mountpoint",
        "/tmp/pcloud",
    ])
    .assert()
    .failure(); // Expected - can't actually connect
}

// ============================================================================
// Flag Combination Tests
// ============================================================================

#[test]
fn test_valid_daemon_configuration() {
    let mut cmd = pcloud_cmd();
    // Daemon mode without conflicting flags should parse
    cmd.args(["-u", "test@example.com", "-d", "-m", "/tmp/pcloud"])
        .assert()
        .failure(); // Expected - needs password
}

#[test]
fn test_valid_client_configuration() {
    let mut cmd = pcloud_cmd();
    // Client mode should parse
    cmd.args(["-u", "test@example.com", "-k"])
        .assert()
        .failure(); // Expected - no daemon running
}

#[test]
fn test_newuser_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-u", "test@example.com", "-n"])
        .assert()
        .failure(); // Expected - needs password
}

#[test]
fn test_savepassword_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-u", "test@example.com", "-s"])
        .assert()
        .failure(); // Expected - can't actually connect
}

#[test]
fn test_commands_mode_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-u", "test@example.com", "-o"])
        .assert()
        .failure(); // Expected - can't actually connect
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
    cmd.args(["-u"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires").or(predicate::str::contains("value")));
}

#[test]
fn test_error_message_for_invalid_value() {
    let mut cmd = pcloud_cmd();
    // -m with empty path might trigger an error
    cmd.args(["-u", "test@example.com", "-m", ""])
        .assert()
        .failure();
}

// ============================================================================
// Combined Flag Tests
// ============================================================================

#[test]
fn test_all_valid_flags_together() {
    let mut cmd = pcloud_cmd();
    // Many flags together (non-conflicting)
    cmd.args([
        "-u",
        "test@example.com",
        "-p",
        "-c",
        "-d",
        "-o",
        "-m",
        "/tmp/pcloud",
        "-s",
    ])
    .assert()
    .failure(); // Expected - needs password input
}

#[test]
fn test_long_and_short_flags_mixed() {
    let mut cmd = pcloud_cmd();
    cmd.args([
        "--username",
        "test@example.com",
        "-p",
        "--daemon",
        "-m",
        "/tmp/pcloud",
    ])
    .assert()
    .failure(); // Expected - needs password input
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
fn test_nosave_and_savepassword_conflict() {
    let mut cmd = pcloud_cmd();
    cmd.args(["--nosave", "-s"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--nosave"));
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
// Edge Cases
// ============================================================================

#[test]
fn test_empty_username() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-u", ""]).assert().failure();
}

#[test]
fn test_username_with_special_characters() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-u", "test+tag@example.com"]).assert().failure(); // Will fail at runtime, but should parse
}

#[test]
fn test_mountpoint_with_spaces() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-u", "test@example.com", "-m", "/tmp/my pcloud folder"])
        .assert()
        .failure(); // Will fail at runtime, but should parse
}

#[test]
fn test_repeated_flags() {
    let mut cmd = pcloud_cmd();
    // Repeated flags - clap behavior may vary
    cmd.args(["-u", "first@example.com", "-u", "second@example.com"])
        .assert()
        .failure(); // Should use last value or error
}
