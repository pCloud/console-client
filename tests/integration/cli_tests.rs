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

// ============================================================================
// shell completions
// ============================================================================

#[test]
fn completions_supports_bash_zsh_fish() {
    for shell in ["bash", "zsh", "fish"] {
        let mut cmd = pcloud_cmd();
        cmd.args(["completions", shell])
            .assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }
}

#[test]
fn completions_rejects_unsupported_shells() {
    // The app targets Linux/macOS only; PowerShell and Elvish are not offered.
    for shell in ["powershell", "elvish"] {
        let mut cmd = pcloud_cmd();
        cmd.args(["completions", shell]).assert().failure();
    }
}

#[test]
fn completions_bash_emits_dynamic_callback_script() {
    // The bash script must register a completion function and call back into the
    // hidden `__complete` subcommand (the cobra-style dynamic integration).
    let mut cmd = pcloud_cmd();
    cmd.args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete -F"))
        .stdout(predicate::str::contains("__complete"))
        .stdout(predicate::str::contains("COMPREPLY"));
}

/// Regression guard for the `clap_complete` `unstable-dynamic` engine: the hidden
/// `__complete` callback must keep emitting `value<TAB>description` lines, since
/// the bash completion's descriptions depend on it. A breaking upgrade to the
/// unstable API would surface here rather than only in manual shell testing.
#[test]
fn complete_callback_emits_descriptions_for_subcommands() {
    let mut cmd = pcloud_cmd();
    let output = cmd
        .args(["__complete", "--index", "1", "--", "pcloud-cli", ""])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf-8 completion output");

    // Every top-level subcommand should appear with its tab-separated description.
    for (value, description) in [
        ("backup", "Manage pCloud backups for the current device"),
        ("crypto", "Manage the Crypto folder"),
        ("mount", "Mount pCloud as a FUSE filesystem"),
    ] {
        let line = stdout
            .lines()
            .find(|l| l.starts_with(&format!("{value}\t")))
            .unwrap_or_else(|| panic!("missing completion candidate for {value:?}\n{stdout}"));
        assert!(
            line.contains(description),
            "candidate {value:?} missing description {description:?}: {line:?}"
        );
    }
}

#[test]
fn complete_callback_honors_value_hints() {
    use std::fs;

    // A directory the path-typed args should surface, and a file they should not
    // (directory hints only offer directories).
    let dir = tempfile::tempdir().expect("tempdir");
    fs::create_dir(dir.path().join("zzz_sentinel_dir")).expect("mkdir sentinel");
    fs::write(dir.path().join("zzz_sentinel_file"), b"x").expect("write file");

    // `backup add <PATH>` (DirPath) → the sentinel directory shows up.
    let mut cmd = pcloud_cmd();
    cmd.current_dir(dir.path())
        .args([
            "__complete",
            "--index",
            "3",
            "--",
            "pcloud-cli",
            "backup",
            "add",
            "",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("zzz_sentinel_dir"))
        .stdout(predicate::str::contains("zzz_sentinel_file").not());

    // `backup remove <ID>` (numeric, ValueHint::Other) → no filesystem listing.
    let mut cmd = pcloud_cmd();
    cmd.current_dir(dir.path())
        .args([
            "__complete",
            "--index",
            "3",
            "--",
            "pcloud-cli",
            "backup",
            "remove",
            "",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("zzz_sentinel").not());
}

// ============================================================================
// service (boot / login autostart)
// ============================================================================

#[test]
fn top_level_help_lists_service() {
    let mut cmd = pcloud_cmd();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("service"));
}

#[test]
fn bare_service_prints_help_to_stdout_and_exits_zero() {
    let mut cmd = pcloud_cmd();
    cmd.arg("service")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage: pcloud-cli service"))
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("uninstall"))
        .stdout(predicate::str::contains("restart"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn service_help_lists_all_operations() {
    let mut cmd = pcloud_cmd();
    cmd.args(["service", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("uninstall"))
        .stdout(predicate::str::contains("restart"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn service_install_help_documents_flags() {
    let mut cmd = pcloud_cmd();
    cmd.args(["service", "install", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATH"))
        .stdout(predicate::str::contains("--user"))
        .stdout(predicate::str::contains("--system"))
        .stdout(predicate::str::contains("--boot"))
        .stdout(predicate::str::contains("--no-start"));
}

#[test]
fn service_install_user_and_system_conflict() {
    let mut cmd = pcloud_cmd();
    cmd.args(["service", "install", "--user", "--system"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("cannot be used with")
                .or(predicate::str::contains("conflict")),
        );
}

#[test]
fn service_status_user_and_system_conflict() {
    let mut cmd = pcloud_cmd();
    cmd.args(["service", "status", "--user", "--system"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("cannot be used with")
                .or(predicate::str::contains("conflict")),
        );
}
