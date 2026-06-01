//! Command-line argument parsing for pCloud console client.
//!
//! Hierarchical clap tree where every subcommand supports `--help`.
//!
//! # Tree
//!
//! ```text
//! pcloud-cli                          # bare invocation → TUI
//! pcloud-cli help [COMMAND]
//! pcloud-cli auth   login|logout|status|unlink
//! pcloud-cli mount  [PATH] [--token TOKEN]
//! pcloud-cli start  [PATH] [--token TOKEN]
//! pcloud-cli stop
//! pcloud-cli status
//! pcloud-cli crypto start|stop|status
//! pcloud-cli backup add|list|remove|status|stop-device|root-name
//! pcloud-cli tui
//! pcloud-cli doctor
//! ```

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use clap::{Args, Parser, Subcommand};

/// Build a multi-line version string including pclsync info.
fn build_version_string() -> String {
    let version = env!("PCLOUD_VERSION");
    let commit = option_env!("PCLOUD_GIT_COMMIT_SHORT").unwrap_or("unknown");
    let pclsync_ver = option_env!("PSYNC_LIB_VERSION").unwrap_or("unknown");
    let pclsync_commit = option_env!("PCLSYNC_GIT_COMMIT_SHORT").unwrap_or("unknown");
    format!(
        "{} ({})\npclsync {} ({})",
        version, commit, pclsync_ver, pclsync_commit
    )
}

static VERSION_STRING: LazyLock<String> = LazyLock::new(build_version_string);

/// Build the after_long_help text including env vars and build info.
fn build_after_help() -> String {
    let commit = option_env!("PCLOUD_GIT_COMMIT_SHORT").unwrap_or("unknown");
    let pclsync_ver = option_env!("PSYNC_LIB_VERSION").unwrap_or("unknown");
    let pclsync_commit = option_env!("PCLSYNC_GIT_COMMIT_SHORT").unwrap_or("unknown");
    format!(
        "\
ENVIRONMENT VARIABLES:\n\
    PCLOUD_AUTH_TOKEN        Auth token (ephemeral; never saved)\n\
    PCLOUD_AUTH_TOKEN_FILE   Path to file containing auth token\n\
    PCLOUD_CRYPTO_PASS       Crypto password (consumed by `crypto start`)\n\
    PCLOUD_CRYPTO_PASS_FILE  Path to file containing crypto password\n\
    PCLOUD_MOUNTPOINT        Default mountpoint for `mount` / `start` when PATH omitted\n\n\
    Direct env vars take priority over _FILE variants.\n\
    Env-sourced tokens are ephemeral and never saved to the database.\n\
    Secret env vars are cleared from the process after reading.\n\n\
BUILD INFO:\n\
    Console client     {}\n\
    pclsync library    {} ({})",
        commit, pclsync_ver, pclsync_commit
    )
}

static AFTER_HELP: LazyLock<String> = LazyLock::new(build_after_help);

/// pCloud Console Client — mount pCloud storage and manage backups.
///
/// Bare invocation (no subcommand) launches the interactive TUI dashboard.
#[derive(Parser, Debug, Clone, Default)]
#[command(name = "pcloud-cli")]
#[command(version = &**VERSION_STRING, about = "pCloud Console Client")]
#[command(
    long_about = "Mount pCloud storage as a local filesystem and manage backups.\n\n\
        Bare `pcloud-cli` launches the interactive dashboard (TUI). Subcommands \
        provide scriptable access to auth, mount/daemon lifecycle, crypto, status, \
        and backups."
)]
#[command(after_long_help = &**AFTER_HELP)]
pub struct Cli {
    /// Subcommand to execute. If omitted, the interactive TUI is launched.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Top-level subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Manage pCloud authentication (login / logout / status / unlink).
    Auth(AuthArgs),

    /// Mount pCloud as a FUSE filesystem in the foreground.
    ///
    /// Blocks until interrupted (Ctrl+C). For background operation use `start`.
    Mount(MountArgs),

    /// Start the pCloud daemon (mounts the FUSE filesystem in background).
    Start(StartArgs),

    /// Stop the running pCloud daemon (graceful, waits for sync to finish).
    Stop,

    /// Print combined auth / mount / crypto / daemon status.
    Status,

    /// Manage the Crypto folder (unlock / lock / status).
    Crypto(CryptoArgs),

    /// Manage pCloud backups for the current device.
    Backup(BackupArgs),

    /// Launch the interactive TUI dashboard (same as no-argument invocation).
    Tui,

    /// Run dependency and environment diagnostics.
    Doctor,
}

// ============================================================================
// auth
// ============================================================================

/// Arguments for the `auth` subcommand group.
///
/// Bare `pcloud-cli auth` prints the group's help and exits cleanly,
/// matching the behavior of `pcloud-cli auth --help`.
#[derive(Args, Debug, Clone)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub op: Option<AuthOp>,
}

/// Authentication operations.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum AuthOp {
    /// Authenticate with pCloud (interactive web flow or non-interactive token).
    ///
    /// Without `--token`, opens the browser-based login flow and waits for
    /// completion. With `--token`, persists the supplied token to the local
    /// database. Either way, the saved token enables future commands to
    /// auto-start a daemon without re-prompting.
    Login {
        /// Auth token (skips interactive flow and persists the token).
        #[arg(short = 't', long = "token", value_name = "TOKEN")]
        token: Option<String>,
    },

    /// Clear the saved auth token. Keeps local sync data.
    Logout,

    /// Show whether saved credentials exist locally.
    Status,

    /// Destructive: clear the saved token AND all local sync data.
    Unlink {
        /// Skip the interactive confirmation prompt.
        #[arg(long = "yes")]
        yes: bool,
    },
}

// ============================================================================
// mount / start
// ============================================================================

/// Arguments for the `mount` (foreground) subcommand.
#[derive(Args, Debug, Clone)]
pub struct MountArgs {
    /// Mount path. Defaults to `$PCLOUD_MOUNTPOINT` or `~/pCloud`.
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Auth token to use for this session (overrides saved credentials).
    ///
    /// Env-sourced (`PCLOUD_AUTH_TOKEN` / `PCLOUD_AUTH_TOKEN_FILE`) tokens
    /// are read automatically and are never persisted.
    #[arg(short = 't', long = "token", value_name = "TOKEN")]
    pub token: Option<String>,
}

/// Arguments for the `start` (background daemon) subcommand.
#[derive(Args, Debug, Clone)]
pub struct StartArgs {
    /// Mount path. Defaults to `$PCLOUD_MOUNTPOINT` or `~/pCloud`.
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Auth token to use for this session (overrides saved credentials).
    #[arg(short = 't', long = "token", value_name = "TOKEN")]
    pub token: Option<String>,
}

// ============================================================================
// crypto
// ============================================================================

/// Arguments for the `crypto` subcommand group.
///
/// Bare `pcloud-cli crypto` prints the group's help and exits cleanly.
#[derive(Args, Debug, Clone)]
pub struct CryptoArgs {
    #[command(subcommand)]
    pub op: Option<CryptoOp>,
}

/// Crypto folder operations.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum CryptoOp {
    /// Unlock the Crypto folder.
    ///
    /// Reads the password from `$PCLOUD_CRYPTO_PASS[_FILE]` if set, from
    /// `--password-file` if provided, otherwise prompts interactively.
    /// Auto-starts the daemon if saved credentials exist.
    Start {
        /// Read the crypto password from this file (newline-trimmed).
        #[arg(long = "password-file", value_name = "FILE")]
        password_file: Option<PathBuf>,
    },

    /// Lock the Crypto folder.
    Stop,

    /// Show whether the Crypto folder is unlocked.
    Status,
}

// ============================================================================
// backup
// ============================================================================

/// Arguments for the `backup` subcommand group.
///
/// Bare `pcloud-cli backup` prints the group's help and exits cleanly.
#[derive(Args, Debug, Clone)]
pub struct BackupArgs {
    #[command(subcommand)]
    pub op: Option<BackupOp>,
}

/// Individual backup operations.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum BackupOp {
    /// Register a local folder as a pCloud backup destination.
    Add {
        /// Local folder to back up.
        path: PathBuf,
    },
    /// List backups configured for the current device.
    List,
    /// Remove a backup by sync id (does not delete files).
    Remove {
        /// Sync id of the backup to remove.
        id: u32,
    },
    /// Stop all backups on this device.
    StopDevice,
    /// Show backup status; optionally filter to a single backup by id.
    Status {
        /// Sync id to filter on. Omit for a full-device summary.
        id: Option<u32>,
    },
    /// Print the backup root folder name for this device.
    RootName,
}

// ============================================================================
// parsing entry points + shared helpers
// ============================================================================

impl Cli {
    /// Parse arguments from the process command line, exiting on error.
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Parse arguments from the process command line, returning a clap error.
    pub fn try_parse_args() -> Result<Self, clap::Error> {
        Self::try_parse()
    }

    /// Parse arguments from an iterator (useful for tests).
    pub fn parse_from_args<I, T>(args: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::parse_from(args)
    }

    /// Try to parse arguments from an iterator, returning a clap error.
    pub fn try_parse_from_args<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::try_parse_from(args)
    }
}

/// Resolve a mountpoint from a CLI-supplied path, the `PCLOUD_MOUNTPOINT`
/// environment variable, or the `~/pCloud` default.
pub fn resolve_mountpoint(cli_path: Option<&Path>) -> PathBuf {
    if let Some(p) = cli_path {
        return p.to_path_buf();
    }
    if let Ok(env_mp) = std::env::var("PCLOUD_MOUNTPOINT") {
        if !env_mp.is_empty() {
            return PathBuf::from(env_mp);
        }
    }
    default_mountpoint()
}

/// The default mountpoint when no CLI path or env var is set.
pub fn default_mountpoint() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join("pCloud")
    } else {
        PathBuf::from("/tmp/pCloud")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // Top-level parsing
    // ------------------------------------------------------------------------

    #[test]
    fn no_args_yields_no_command() {
        let cli = Cli::parse_from_args(["pcloud-cli"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn tui_subcommand_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "tui"]);
        assert!(matches!(cli.command, Some(Command::Tui)));
    }

    #[test]
    fn doctor_subcommand_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "doctor"]);
        assert!(matches!(cli.command, Some(Command::Doctor)));
    }

    #[test]
    fn stop_subcommand_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "stop"]);
        assert!(matches!(cli.command, Some(Command::Stop)));
    }

    #[test]
    fn status_subcommand_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "status"]);
        assert!(matches!(cli.command, Some(Command::Status)));
    }

    #[test]
    fn unknown_subcommand_fails() {
        let res = Cli::try_parse_from_args(["pcloud-cli", "definitely-not-a-command"]);
        assert!(res.is_err());
    }

    #[test]
    fn legacy_top_level_flags_are_rejected() {
        // The flat -d / -t / -m / -k flags from previous versions must error,
        // since they have been replaced by subcommands.
        for argv in [
            vec!["pcloud-cli", "-d"],
            vec!["pcloud-cli", "-k"],
            vec!["pcloud-cli", "-m", "/tmp/x"],
            vec!["pcloud-cli", "-t", "TOKEN"],
            vec!["pcloud-cli", "--logout"],
            vec!["pcloud-cli", "--unlink"],
            vec!["pcloud-cli", "--doctor"],
            vec!["pcloud-cli", "--non-interactive"],
            vec!["pcloud-cli", "--nosave"],
        ] {
            assert!(
                Cli::try_parse_from_args(argv.iter().copied()).is_err(),
                "expected clap to reject {:?}",
                argv
            );
        }
    }

    // ------------------------------------------------------------------------
    // auth
    // ------------------------------------------------------------------------

    #[test]
    fn auth_login_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "auth", "login"]);
        assert!(matches!(
            cli.command,
            Some(Command::Auth(AuthArgs {
                op: Some(AuthOp::Login { token: None })
            }))
        ));
    }

    #[test]
    fn auth_login_with_token_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "auth", "login", "--token", "abc123"]);
        match cli.command {
            Some(Command::Auth(AuthArgs {
                op: Some(AuthOp::Login { token: Some(t) }),
            })) => assert_eq!(t, "abc123"),
            other => panic!("expected auth login --token, got {:?}", other),
        }
    }

    #[test]
    fn auth_login_short_token_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "auth", "login", "-t", "abc"]);
        match cli.command {
            Some(Command::Auth(AuthArgs {
                op: Some(AuthOp::Login { token: Some(t) }),
            })) => assert_eq!(t, "abc"),
            other => panic!("expected auth login -t, got {:?}", other),
        }
    }

    #[test]
    fn auth_logout_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "auth", "logout"]);
        assert!(matches!(
            cli.command,
            Some(Command::Auth(AuthArgs {
                op: Some(AuthOp::Logout)
            }))
        ));
    }

    #[test]
    fn auth_status_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "auth", "status"]);
        assert!(matches!(
            cli.command,
            Some(Command::Auth(AuthArgs {
                op: Some(AuthOp::Status)
            }))
        ));
    }

    #[test]
    fn auth_unlink_without_yes_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "auth", "unlink"]);
        match cli.command {
            Some(Command::Auth(AuthArgs {
                op: Some(AuthOp::Unlink { yes }),
            })) => assert!(!yes),
            other => panic!("expected auth unlink, got {:?}", other),
        }
    }

    #[test]
    fn auth_unlink_with_yes_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "auth", "unlink", "--yes"]);
        match cli.command {
            Some(Command::Auth(AuthArgs {
                op: Some(AuthOp::Unlink { yes }),
            })) => assert!(yes),
            other => panic!("expected auth unlink --yes, got {:?}", other),
        }
    }

    #[test]
    fn auth_bare_yields_none_op() {
        let cli = Cli::parse_from_args(["pcloud-cli", "auth"]);
        match cli.command {
            Some(Command::Auth(AuthArgs { op: None })) => {}
            other => panic!("expected bare auth → None op, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------------
    // mount / start
    // ------------------------------------------------------------------------

    #[test]
    fn mount_without_path_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "mount"]);
        match cli.command {
            Some(Command::Mount(MountArgs { path, token })) => {
                assert!(path.is_none());
                assert!(token.is_none());
            }
            other => panic!("expected mount, got {:?}", other),
        }
    }

    #[test]
    fn mount_with_path_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "mount", "/mnt/pcloud"]);
        match cli.command {
            Some(Command::Mount(MountArgs { path, .. })) => {
                assert_eq!(path, Some(PathBuf::from("/mnt/pcloud")));
            }
            other => panic!("expected mount /mnt/pcloud, got {:?}", other),
        }
    }

    #[test]
    fn mount_with_token_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "mount", "-t", "TOK"]);
        match cli.command {
            Some(Command::Mount(MountArgs { token, .. })) => {
                assert_eq!(token.as_deref(), Some("TOK"));
            }
            other => panic!("expected mount -t, got {:?}", other),
        }
    }

    #[test]
    fn start_without_path_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "start"]);
        match cli.command {
            Some(Command::Start(StartArgs { path, token })) => {
                assert!(path.is_none());
                assert!(token.is_none());
            }
            other => panic!("expected start, got {:?}", other),
        }
    }

    #[test]
    fn start_with_path_and_token_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "start", "/mnt/pcloud", "--token", "TOK"]);
        match cli.command {
            Some(Command::Start(StartArgs { path, token })) => {
                assert_eq!(path, Some(PathBuf::from("/mnt/pcloud")));
                assert_eq!(token.as_deref(), Some("TOK"));
            }
            other => panic!("expected start, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------------
    // crypto
    // ------------------------------------------------------------------------

    #[test]
    fn crypto_start_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "crypto", "start"]);
        match cli.command {
            Some(Command::Crypto(CryptoArgs {
                op: Some(CryptoOp::Start { password_file }),
            })) => assert!(password_file.is_none()),
            other => panic!("expected crypto start, got {:?}", other),
        }
    }

    #[test]
    fn crypto_start_with_password_file_parses() {
        let cli = Cli::parse_from_args([
            "pcloud-cli",
            "crypto",
            "start",
            "--password-file",
            "/run/secrets/crypto",
        ]);
        match cli.command {
            Some(Command::Crypto(CryptoArgs {
                op: Some(CryptoOp::Start { password_file }),
            })) => {
                assert_eq!(password_file, Some(PathBuf::from("/run/secrets/crypto")));
            }
            other => panic!("expected crypto start --password-file, got {:?}", other),
        }
    }

    #[test]
    fn crypto_stop_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "crypto", "stop"]);
        assert!(matches!(
            cli.command,
            Some(Command::Crypto(CryptoArgs {
                op: Some(CryptoOp::Stop)
            }))
        ));
    }

    #[test]
    fn crypto_status_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "crypto", "status"]);
        assert!(matches!(
            cli.command,
            Some(Command::Crypto(CryptoArgs {
                op: Some(CryptoOp::Status)
            }))
        ));
    }

    #[test]
    fn crypto_bare_yields_none_op() {
        let cli = Cli::parse_from_args(["pcloud-cli", "crypto"]);
        match cli.command {
            Some(Command::Crypto(CryptoArgs { op: None })) => {}
            other => panic!("expected bare crypto → None op, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------------
    // backup (unchanged shape)
    // ------------------------------------------------------------------------

    #[test]
    fn backup_add_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "backup", "add", "/tmp/foo"]);
        match cli.command {
            Some(Command::Backup(BackupArgs {
                op: Some(BackupOp::Add { path }),
            })) => assert_eq!(path, PathBuf::from("/tmp/foo")),
            other => panic!("expected backup add, got {:?}", other),
        }
    }

    #[test]
    fn backup_list_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "backup", "list"]);
        assert!(matches!(
            cli.command,
            Some(Command::Backup(BackupArgs {
                op: Some(BackupOp::List)
            }))
        ));
    }

    #[test]
    fn backup_remove_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "backup", "remove", "5"]);
        match cli.command {
            Some(Command::Backup(BackupArgs {
                op: Some(BackupOp::Remove { id }),
            })) => assert_eq!(id, 5),
            other => panic!("expected backup remove, got {:?}", other),
        }
    }

    #[test]
    fn backup_status_no_id_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "backup", "status"]);
        match cli.command {
            Some(Command::Backup(BackupArgs {
                op: Some(BackupOp::Status { id }),
            })) => assert!(id.is_none()),
            other => panic!("expected backup status, got {:?}", other),
        }
    }

    #[test]
    fn backup_status_with_id_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "backup", "status", "7"]);
        match cli.command {
            Some(Command::Backup(BackupArgs {
                op: Some(BackupOp::Status { id }),
            })) => assert_eq!(id, Some(7)),
            other => panic!("expected backup status 7, got {:?}", other),
        }
    }

    #[test]
    fn backup_stop_device_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "backup", "stop-device"]);
        assert!(matches!(
            cli.command,
            Some(Command::Backup(BackupArgs {
                op: Some(BackupOp::StopDevice)
            }))
        ));
    }

    #[test]
    fn backup_root_name_parses() {
        let cli = Cli::parse_from_args(["pcloud-cli", "backup", "root-name"]);
        assert!(matches!(
            cli.command,
            Some(Command::Backup(BackupArgs {
                op: Some(BackupOp::RootName)
            }))
        ));
    }

    #[test]
    fn backup_bare_yields_none_op() {
        let cli = Cli::parse_from_args(["pcloud-cli", "backup"]);
        match cli.command {
            Some(Command::Backup(BackupArgs { op: None })) => {}
            other => panic!("expected bare backup → None op, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------------
    // resolve_mountpoint
    // ------------------------------------------------------------------------

    #[test]
    fn resolve_mountpoint_prefers_cli_path() {
        let p = PathBuf::from("/custom/path");
        assert_eq!(resolve_mountpoint(Some(&p)), PathBuf::from("/custom/path"));
    }

    #[test]
    fn resolve_mountpoint_uses_env_when_cli_missing() {
        // Use a single combined test so HOME mutation doesn't race other tests.
        let original_env = std::env::var("PCLOUD_MOUNTPOINT").ok();
        // SAFETY: tests run single-threaded with --test-threads=1 in CI; this
        // env mutation is local to this test.
        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var("PCLOUD_MOUNTPOINT", "/from-env");
        }
        assert_eq!(resolve_mountpoint(None), PathBuf::from("/from-env"));

        // Restore prior state.
        #[allow(unused_unsafe)]
        unsafe {
            match original_env {
                Some(v) => std::env::set_var("PCLOUD_MOUNTPOINT", v),
                None => std::env::remove_var("PCLOUD_MOUNTPOINT"),
            }
        }
    }

    #[test]
    fn default_mountpoint_ends_with_pcloud() {
        let p = default_mountpoint();
        assert!(p.to_string_lossy().ends_with("pCloud"));
    }
}
