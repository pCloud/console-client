//! Command-line argument parsing for pCloud console client.
//!
//! This module provides CLI argument parsing using clap with derive macros.
//!
//! # CLI Interface
//!
//! ```text
//! -t <token>       Authentication token
//! -c               Prompt for crypto password
//! -d               Daemonize (background)
//! -o               Commands mode (interactive)
//! -m <path>        Mountpoint
//! -k               Commands only (talk to existing daemon)
//! --logout         Clear saved credentials
//! --unlink         Clear all local data
//! --nosave         Don't save credentials
//! ```

use std::path::PathBuf;
use std::sync::LazyLock;

use clap::Parser;

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

/// Build the after_long_help text including build info.
fn build_after_help() -> String {
    let commit = option_env!("PCLOUD_GIT_COMMIT_SHORT").unwrap_or("unknown");
    let pclsync_ver = option_env!("PSYNC_LIB_VERSION").unwrap_or("unknown");
    let pclsync_commit = option_env!("PCLSYNC_GIT_COMMIT_SHORT").unwrap_or("unknown");
    format!(
        "\
ENVIRONMENT VARIABLES:\n\
    PCLOUD_AUTH_TOKEN       Auth token (alternative to -t)\n\
    PCLOUD_AUTH_TOKEN_FILE  Path to file containing auth token\n\
    PCLOUD_CRYPTO_PASS     Crypto password (auto-enables crypto)\n\
    PCLOUD_CRYPTO_PASS_FILE Path to file containing crypto password\n\
    PCLOUD_MOUNTPOINT      Mountpoint path (alternative to -m)\n\n\
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

/// pCloud Console Client - Mount pCloud storage as a local filesystem.
///
/// This client allows you to access your pCloud storage through a FUSE
/// filesystem mount, with support for encrypted folders (Crypto) and
/// background daemon operation.
///
/// # Authentication
///
/// If no credentials are provided, an interactive prompt will offer:
/// - Web-based login (opens browser with QR code)
/// - Auth token entry
#[derive(Parser, Debug, Clone, Default)]
#[command(name = "pcloud")]
#[command(version = &**VERSION_STRING, about = "pCloud Console Client")]
#[command(long_about = "Mount pCloud storage as a local filesystem.\n\n\
    This client allows you to access your pCloud storage through a FUSE \
    filesystem mount, with support for encrypted folders (Crypto) and \
    background daemon operation.\n\n\
    If no credentials are provided, an interactive authentication prompt \
    will be displayed offering web-based login or auth token input.")]
#[command(after_long_help = &**AFTER_HELP)]
pub struct Cli {
    /// Use authentication token directly
    ///
    /// Bypasses interactive authentication.
    /// The token can be obtained from pCloud account settings.
    /// Can also be set via PCLOUD_AUTH_TOKEN or PCLOUD_AUTH_TOKEN_FILE env vars.
    #[arg(short = 't', long = "token")]
    pub auth_token: Option<String>,

    /// Prompt for crypto password (interactive)
    ///
    /// Can also be set via PCLOUD_CRYPTO_PASS or PCLOUD_CRYPTO_PASS_FILE
    /// env vars, which auto-enable crypto without this flag.
    #[arg(short = 'c', long = "crypto")]
    pub crypto_prompt: bool,

    /// Run as daemon (background process)
    #[arg(short = 'd', long = "daemon")]
    pub daemonize: bool,

    /// Enable interactive commands mode
    #[arg(short = 'o', long = "commands")]
    pub commands_mode: bool,

    /// Mountpoint for FUSE filesystem
    ///
    /// Defaults to ~/pCloud if not specified.
    /// Can also be set via the PCLOUD_MOUNTPOINT env var.
    #[arg(short = 'm', long = "mountpoint")]
    pub mountpoint: Option<PathBuf>,

    /// Send commands to running daemon (client mode)
    #[arg(short = 'k', long = "client")]
    pub commands_only: bool,

    /// Launch the TUI dashboard instead of CLI mode
    ///
    /// Displays a terminal user interface with real-time status,
    /// transfer progress, crypto controls, and an activity log.
    #[arg(long = "tui")]
    pub tui: bool,

    /// Do not save credentials between sessions
    ///
    /// By default, credentials are saved for automatic login on next run.
    /// Use this flag to prevent saving credentials.
    #[arg(long = "nosave")]
    pub nosave: bool,

    /// Log out and clear saved credentials
    ///
    /// Removes saved auth token from the local database but keeps
    /// any synced data intact. The client exits after logging out.
    #[arg(long = "logout")]
    pub logout: bool,

    /// Unlink account and clear all local data
    ///
    /// Removes saved credentials AND all local sync data.
    /// This is destructive and cannot be undone. The client exits
    /// after unlinking.
    #[arg(long = "unlink")]
    pub unlink: bool,
}

impl Cli {
    /// Parse arguments from command line.
    ///
    /// This is a convenience wrapper around `clap::Parser::parse()`.
    ///
    /// # Panics
    ///
    /// Will exit the process with an error message if required arguments
    /// are missing or invalid.
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Try to parse arguments from command line, returning an error on failure.
    ///
    /// Unlike `parse_args()`, this method returns a Result instead of
    /// exiting the process on error.
    pub fn try_parse_args() -> Result<Self, clap::Error> {
        Self::try_parse()
    }

    /// Parse arguments from an iterator (useful for testing).
    ///
    /// # Arguments
    ///
    /// * `args` - Iterator of string arguments (including program name as first element)
    pub fn parse_from_args<I, T>(args: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::parse_from(args)
    }

    /// Try to parse arguments from an iterator, returning an error on failure.
    pub fn try_parse_from_args<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::try_parse_from(args)
    }

    /// Validate argument combinations.
    ///
    /// Some argument combinations are mutually exclusive or require
    /// other arguments to be present. This method checks for these
    /// conflicts and returns an error if any are found.
    ///
    /// # Errors
    ///
    /// Returns an error string describing the conflict if validation fails.
    ///
    /// # Example
    ///
    /// ```
    /// use console_client::cli::Cli;
    ///
    /// let cli = Cli {
    ///     daemonize: true,
    ///     commands_only: true,  // Conflict!
    ///     ..Default::default()
    /// };
    /// assert!(cli.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        // TUI mode conflicts with daemon, client, and commands modes
        if self.tui && self.daemonize {
            return Err("Cannot use both --tui and --daemon. \
                TUI mode runs in the foreground."
                .to_string());
        }
        if self.tui && self.commands_only {
            return Err("Cannot use both --tui and --client. \
                TUI mode runs its own interface."
                .to_string());
        }
        if self.tui && self.commands_mode {
            return Err("Cannot use both --tui and --commands. \
                TUI mode provides its own interactive interface."
                .to_string());
        }

        // Can't use both -d (daemon) and -k (client/commands_only)
        if self.daemonize && self.commands_only {
            return Err("Cannot use both --daemon and --client mode. \
                Use --daemon to start a new background service, \
                or --client to connect to an existing daemon."
                .to_string());
        }

        // --logout and --unlink are mutually exclusive
        if self.logout && self.unlink {
            return Err("Cannot use both --logout and --unlink. \
                Use --logout to clear credentials only, \
                or --unlink to clear all local data."
                .to_string());
        }

        // --logout and --unlink are standalone operations
        if self.logout || self.unlink {
            let flag = if self.logout { "--logout" } else { "--unlink" };

            if self.daemonize {
                return Err(format!(
                    "{} cannot be combined with --daemon. \
                    Run {} as a standalone operation.",
                    flag, flag
                ));
            }
            if self.commands_only {
                return Err(format!(
                    "{} cannot be combined with --client. \
                    Run {} as a standalone operation.",
                    flag, flag
                ));
            }
            if self.auth_token.is_some() {
                return Err(format!(
                    "{} cannot be combined with --token. \
                    Run {} as a standalone operation.",
                    flag, flag
                ));
            }
            if self.crypto_prompt {
                return Err(format!(
                    "{} cannot be combined with --crypto. \
                    Run {} as a standalone operation.",
                    flag, flag
                ));
            }
        }

        Ok(())
    }

    /// Check if this is a "client only" invocation.
    ///
    /// Client mode connects to an existing daemon to send commands
    /// rather than starting a new pCloud session.
    pub fn is_client_mode(&self) -> bool {
        self.commands_only
    }

    /// Check if crypto functionality is requested.
    pub fn wants_crypto(&self) -> bool {
        self.crypto_prompt
    }

    /// Check if interactive mode is requested.
    ///
    /// Interactive mode allows the user to send commands to the running
    /// client (e.g., startcrypto, stopcrypto, finalize, quit).
    pub fn wants_interactive(&self) -> bool {
        self.commands_mode
    }

    /// Get the mountpoint, applying fallbacks if not specified.
    ///
    /// Priority: CLI `-m` > `PCLOUD_MOUNTPOINT` env var > ~/pCloud default.
    pub fn get_mountpoint(&self) -> PathBuf {
        if let Some(ref mp) = self.mountpoint {
            return mp.clone();
        }
        if let Ok(env_mp) = std::env::var("PCLOUD_MOUNTPOINT") {
            if !env_mp.is_empty() {
                return PathBuf::from(env_mp);
            }
        }
        Self::default_mountpoint()
    }

    /// Get the default mountpoint path.
    ///
    /// Returns ~/pCloud if HOME is set, otherwise /tmp/pCloud.
    pub fn default_mountpoint() -> PathBuf {
        if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join("pCloud")
        } else {
            PathBuf::from("/tmp/pCloud")
        }
    }

    /// Check if credentials should be saved for future sessions.
    ///
    /// Returns `true` by default (save credentials). Returns `false` only
    /// when `--nosave` is explicitly specified.
    pub fn should_save_credentials(&self) -> bool {
        !self.nosave
    }

    /// Check if this is a logout operation.
    pub fn is_logout(&self) -> bool {
        self.logout
    }

    /// Check if this is an unlink operation.
    pub fn is_unlink(&self) -> bool {
        self.unlink
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_no_args() {
        let cli = Cli::parse_from_args(["pcloud"]);
        assert!(cli.auth_token.is_none());
        assert!(!cli.daemonize);
    }

    #[test]
    fn test_parse_token_flag() {
        let cli = Cli::parse_from_args(["pcloud", "-t", "my-auth-token"]);
        assert_eq!(cli.auth_token, Some("my-auth-token".to_string()));
    }

    #[test]
    fn test_parse_long_flags() {
        let cli = Cli::parse_from_args([
            "pcloud",
            "--token",
            "my-token",
            "--daemon",
            "--mountpoint",
            "/home/user/cloud",
        ]);
        assert_eq!(cli.auth_token, Some("my-token".to_string()));
        assert!(cli.daemonize);
        assert_eq!(cli.mountpoint, Some(PathBuf::from("/home/user/cloud")));
    }

    #[test]
    fn test_parse_all_flags() {
        let cli = Cli::parse_from_args([
            "pcloud",
            "-t",
            "token",
            "-c",
            "-d",
            "-o",
            "-m",
            "/mnt/pcloud",
        ]);
        assert_eq!(cli.auth_token, Some("token".to_string()));
        assert!(cli.crypto_prompt);
        assert!(cli.daemonize);
        assert!(cli.commands_mode);
        assert_eq!(cli.mountpoint, Some(PathBuf::from("/mnt/pcloud")));
    }

    #[test]
    fn test_conflicting_daemon_and_client() {
        let cli = Cli {
            daemonize: true,
            commands_only: true,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("daemon"));
    }

    #[test]
    fn test_logout_and_unlink_conflict() {
        let cli = Cli {
            logout: true,
            unlink: true,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--logout"));
    }

    #[test]
    fn test_logout_conflicts_with_daemon() {
        let cli = Cli {
            logout: true,
            daemonize: true,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--logout"));
    }

    #[test]
    fn test_unlink_conflicts_with_client() {
        let cli = Cli {
            unlink: true,
            commands_only: true,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--unlink"));
    }

    #[test]
    fn test_logout_conflicts_with_token() {
        let cli = Cli {
            logout: true,
            auth_token: Some("token".to_string()),
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--logout"));
    }

    #[test]
    fn test_unlink_conflicts_with_crypto() {
        let cli = Cli {
            unlink: true,
            crypto_prompt: true,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--unlink"));
    }

    #[test]
    fn test_logout_standalone_valid() {
        let cli = Cli {
            logout: true,
            ..Default::default()
        };
        assert!(cli.validate().is_ok());
        assert!(cli.is_logout());
    }

    #[test]
    fn test_unlink_standalone_valid() {
        let cli = Cli {
            unlink: true,
            ..Default::default()
        };
        assert!(cli.validate().is_ok());
        assert!(cli.is_unlink());
    }

    #[test]
    fn test_client_mode_flag() {
        let cli = Cli {
            commands_only: true,
            ..Default::default()
        };
        assert!(cli.is_client_mode());
    }

    #[test]
    fn test_wants_crypto() {
        let cli1 = Cli {
            crypto_prompt: true,
            ..Default::default()
        };
        assert!(cli1.wants_crypto());

        let cli2 = Cli::default();
        assert!(!cli2.wants_crypto());
    }

    #[test]
    fn test_default_mountpoint() {
        let cli = Cli::default();
        let mountpoint = cli.get_mountpoint();
        // Should end with pCloud (not pCloudDrive anymore)
        assert!(mountpoint.to_string_lossy().ends_with("pCloud"));
    }

    #[test]
    fn test_custom_mountpoint() {
        let cli = Cli {
            mountpoint: Some(PathBuf::from("/custom/path")),
            ..Default::default()
        };
        assert_eq!(cli.get_mountpoint(), PathBuf::from("/custom/path"));
    }

    #[test]
    fn test_env_mountpoint() {
        // CLI flag takes priority over env var
        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var("PCLOUD_MOUNTPOINT", "/env/path");
        }
        let cli = Cli {
            mountpoint: Some(PathBuf::from("/cli/path")),
            ..Default::default()
        };
        assert_eq!(cli.get_mountpoint(), PathBuf::from("/cli/path"));

        // Env var used when no CLI flag
        let cli = Cli::default();
        assert_eq!(cli.get_mountpoint(), PathBuf::from("/env/path"));

        // Clean up
        #[allow(unused_unsafe)]
        unsafe {
            std::env::remove_var("PCLOUD_MOUNTPOINT");
        }
    }

    #[test]
    fn test_default_values() {
        let cli = Cli::default();
        assert!(cli.auth_token.is_none());
        assert!(!cli.crypto_prompt);
        assert!(!cli.daemonize);
        assert!(!cli.commands_mode);
        assert!(cli.mountpoint.is_none());
        assert!(!cli.commands_only);
        assert!(!cli.tui);
        assert!(!cli.nosave);
        assert!(!cli.logout);
        assert!(!cli.unlink);
        assert!(cli.should_save_credentials());
    }

    #[test]
    fn test_should_save_credentials_default() {
        let cli = Cli::default();
        assert!(cli.should_save_credentials());
    }

    #[test]
    fn test_should_save_credentials_nosave() {
        let cli = Cli {
            nosave: true,
            ..Default::default()
        };
        assert!(!cli.should_save_credentials());
    }

    #[test]
    fn test_parse_nosave_flag() {
        let cli = Cli::parse_from_args(["pcloud", "--nosave"]);
        assert!(cli.nosave);
        assert!(!cli.should_save_credentials());
    }

    #[test]
    fn test_parse_logout_flag() {
        let cli = Cli::parse_from_args(["pcloud", "--logout"]);
        assert!(cli.logout);
        assert!(cli.is_logout());
    }

    #[test]
    fn test_parse_unlink_flag() {
        let cli = Cli::parse_from_args(["pcloud", "--unlink"]);
        assert!(cli.unlink);
        assert!(cli.is_unlink());
    }

    #[test]
    fn test_valid_daemon_with_mountpoint() {
        let cli = Cli {
            daemonize: true,
            mountpoint: Some(PathBuf::from("/mnt/pcloud")),
            ..Default::default()
        };
        assert!(cli.validate().is_ok());
    }
}
