//! CLI integration tests for the hierarchical pcloud-cli.
//!
//! These tests verify the binary's argument parsing, help/version output,
//! and rejection of the legacy flat-flag interface that the 3.x preview
//! releases shipped with.

use assert_cmd::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;

/// Spawn the compiled `pcloud-cli` binary.
fn pcloud_cmd() -> Command {
    cargo_bin_cmd!("pcloud-cli")
}

// ============================================================================
// --help / --version
// ============================================================================

#[test]
fn help_short_flag_succeeds() {
    let mut cmd = pcloud_cmd();
    cmd.arg("-h")
        .assert()
        .success()
        .stdout(predicate::str::contains("pCloud Console Client"))
        .stdout(predicate::str::contains("Usage"));
}

#[test]
fn help_long_flag_shows_subcommands() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("auth"))
        .stdout(predicate::str::contains("mount"))
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("stop"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("crypto"))
        .stdout(predicate::str::contains("backup"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("tui"));
}

#[test]
fn version_short_flag_succeeds() {
    let mut cmd = pcloud_cmd();
    cmd.arg("-V").assert().success();
}

#[test]
fn version_long_flag_mentions_binary_name() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("pcloud-cli"));
}

// ============================================================================
// Legacy flat flags must error
// ============================================================================

#[test]
fn legacy_daemon_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.arg("-d").assert().failure();
}

#[test]
fn legacy_client_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.arg("-k").assert().failure();
}

#[test]
fn legacy_mountpoint_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-m", "/tmp/pcloud"]).assert().failure();
}

#[test]
fn legacy_token_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.args(["-t", "abc"]).assert().failure();
}

#[test]
fn legacy_commands_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.arg("-o").assert().failure();
}

#[test]
fn legacy_crypto_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.arg("-c").assert().failure();
}

#[test]
fn legacy_doctor_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--doctor").assert().failure();
}

#[test]
fn legacy_logout_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--logout").assert().failure();
}

#[test]
fn legacy_unlink_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--unlink").assert().failure();
}

#[test]
fn legacy_nosave_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--nosave").assert().failure();
}

#[test]
fn legacy_non_interactive_flag_fails() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--non-interactive").assert().failure();
}

// ============================================================================
// auth subcommand
// ============================================================================

#[test]
fn auth_help_lists_operations() {
    let mut cmd = pcloud_cmd();
    cmd.args(["auth", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("login"))
        .stdout(predicate::str::contains("logout"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("unlink"));
}

#[test]
fn bare_auth_prints_help_to_stdout_and_exits_zero() {
    let mut cmd = pcloud_cmd();
    cmd.arg("auth")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage: pcloud-cli auth"))
        .stdout(predicate::str::contains("login"))
        .stdout(predicate::str::contains("unlink"));
}

#[test]
fn auth_login_help_documents_token_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["auth", "login", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--token"));
}

#[test]
fn auth_unlink_help_documents_yes_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["auth", "unlink", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--yes"));
}

// ============================================================================
// mount / start subcommands
// ============================================================================

#[test]
fn mount_help_documents_path_and_token() {
    let mut cmd = pcloud_cmd();
    cmd.args(["mount", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATH"))
        .stdout(predicate::str::contains("--token"));
}

#[test]
fn start_help_documents_path_and_token() {
    let mut cmd = pcloud_cmd();
    cmd.args(["start", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATH"))
        .stdout(predicate::str::contains("--token"));
}

// ============================================================================
// crypto subcommand
// ============================================================================

#[test]
fn crypto_help_lists_operations() {
    let mut cmd = pcloud_cmd();
    cmd.args(["crypto", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("stop"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn bare_crypto_prints_help_to_stdout_and_exits_zero() {
    let mut cmd = pcloud_cmd();
    cmd.arg("crypto")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage: pcloud-cli crypto"));
}

#[test]
fn crypto_start_help_documents_password_file_flag() {
    let mut cmd = pcloud_cmd();
    cmd.args(["crypto", "start", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--password-file"));
}

// ============================================================================
// backup subcommand
// ============================================================================

#[test]
fn backup_help_lists_all_operations() {
    let mut cmd = pcloud_cmd();
    cmd.args(["backup", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("add"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("remove"))
        .stdout(predicate::str::contains("stop-device"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("root-name"));
}

#[test]
fn bare_backup_prints_help_to_stdout_and_exits_zero() {
    let mut cmd = pcloud_cmd();
    cmd.arg("backup")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage: pcloud-cli backup"))
        .stdout(predicate::str::contains("add"))
        .stdout(predicate::str::contains("stop-device"));
}

#[test]
fn backup_add_help_documents_path_arg() {
    let mut cmd = pcloud_cmd();
    cmd.args(["backup", "add", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATH"));
}

#[test]
fn backup_add_without_path_fails() {
    let mut cmd = pcloud_cmd();
    cmd.args(["backup", "add"]).assert().failure();
}

#[test]
fn backup_add_with_no_daemon_and_no_creds_errors() {
    use tempfile::TempDir;

    // Throwaway HOME / XDG so we don't trip over real saved credentials.
    let home_dir = TempDir::new().expect("tempdir");
    let mut cmd = pcloud_cmd();
    cmd.env("HOME", home_dir.path())
        .env("XDG_DATA_HOME", home_dir.path().join("xdg-data"))
        .env("XDG_CONFIG_HOME", home_dir.path().join("xdg-config"))
        .env("XDG_CACHE_HOME", home_dir.path().join("xdg-cache"))
        .args(["backup", "add", "/tmp/some-path"])
        .assert()
        .failure();
}

// ============================================================================
// Error message quality
// ============================================================================

#[test]
fn unknown_subcommand_errors() {
    let mut cmd = pcloud_cmd();
    cmd.arg("definitely-not-a-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn unknown_flag_errors() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--unknown-flag")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn token_without_value_errors() {
    let mut cmd = pcloud_cmd();
    cmd.args(["auth", "login", "--token"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires").or(predicate::str::contains("value")));
}

// ============================================================================
// stop / status / doctor / tui
// ============================================================================

#[test]
fn stop_help_succeeds() {
    let mut cmd = pcloud_cmd();
    cmd.args(["stop", "--help"]).assert().success();
}

#[test]
fn status_help_succeeds() {
    let mut cmd = pcloud_cmd();
    cmd.args(["status", "--help"]).assert().success();
}

#[test]
fn doctor_help_succeeds() {
    let mut cmd = pcloud_cmd();
    cmd.args(["doctor", "--help"]).assert().success();
}

#[test]
fn tui_help_succeeds() {
    let mut cmd = pcloud_cmd();
    cmd.args(["tui", "--help"]).assert().success();
}
