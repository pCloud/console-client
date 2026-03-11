//! Command-line argument parsing for pCloud console client.
//!
//! This module provides CLI argument parsing using clap with derive macros,
//! maintaining full compatibility with the original C++ implementation's flags.
//!
//! # Original CLI Interface
//!
//! ```text
//! -u <username>    Username (required)
//! -p               Prompt for password
//! -c               Prompt for crypto password
//! -y               Use password as crypto password
//! -d               Daemonize (background)
//! -o               Commands mode (interactive)
//! -m <path>        Mountpoint
//! -k               Commands only (talk to existing daemon)
//! -n               New user registration
//! -s               Save password
//! ```

use std::path::PathBuf;

use clap::Parser;

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
/// - Manual username/password entry
/// - Auth token entry
#[derive(Parser, Debug, Clone, Default)]
#[command(name = "pcloud")]
#[command(version = env!("PCLOUD_VERSION"), about = "pCloud Console Client")]
#[command(long_about = "Mount pCloud storage as a local filesystem.\n\n\
    This client allows you to access your pCloud storage through a FUSE \
    filesystem mount, with support for encrypted folders (Crypto) and \
    background daemon operation.\n\n\
    If no credentials are provided, an interactive authentication prompt \
    will be displayed offering web-based login, manual credentials entry, \
    or auth token input.")]
pub struct Cli {
    /// Username/email for pCloud account
    ///
    /// Required for password authentication (-p) and new user registration (-n).
    /// Optional when using token auth (-t) or web login.
    #[arg(short = 'u', long = "username")]
    pub username: Option<String>,

    /// Prompt for password (interactive)
    ///
    /// Requires -u/--username to be specified.
    #[arg(short = 'p', long = "password")]
    pub password_prompt: bool,

    /// Use authentication token directly
    ///
    /// Bypasses username/password authentication.
    /// The token can be obtained from pCloud account settings.
    #[arg(short = 't', long = "token")]
    pub auth_token: Option<String>,

    /// Prompt for crypto password (interactive)
    #[arg(short = 'c', long = "crypto")]
    pub crypto_prompt: bool,

    /// Use login password as crypto password
    #[arg(short = 'y', long = "passascrypto")]
    pub use_password_as_crypto: bool,

    /// Run as daemon (background process)
    #[arg(short = 'd', long = "daemon")]
    pub daemonize: bool,

    /// Enable interactive commands mode
    #[arg(short = 'o', long = "commands")]
    pub commands_mode: bool,

    /// Mountpoint for FUSE filesystem
    ///
    /// Defaults to ~/pCloud if not specified.
    #[arg(short = 'm', long = "mountpoint")]
    pub mountpoint: Option<PathBuf>,

    /// Send commands to running daemon (client mode)
    #[arg(short = 'k', long = "client")]
    pub commands_only: bool,

    /// Register new user account
    #[arg(short = 'n', long = "newuser")]
    pub newuser: bool,

    /// Save credentials for automatic login
    ///
    /// This is the default behavior. Kept for backward compatibility.
    #[arg(short = 's', long = "savepassword")]
    pub save_password: bool,

    /// Do not save credentials between sessions
    ///
    /// By default, credentials are saved for automatic login on next run.
    /// Use this flag to prevent saving credentials.
    #[arg(long = "nosave")]
    pub nosave: bool,
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
    ///     username: Some("test@example.com".to_string()),
    ///     daemonize: true,
    ///     commands_only: true,  // Conflict!
    ///     ..Default::default()
    /// };
    /// assert!(cli.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        // Can't use both -d (daemon) and -k (client/commands_only)
        // -d starts a new daemon, -k connects to existing daemon
        if self.daemonize && self.commands_only {
            return Err("Cannot use both --daemon and --client mode. \
                Use --daemon to start a new background service, \
                or --client to connect to an existing daemon."
                .to_string());
        }

        // Username must not be empty
        if let Some(ref username) = self.username {
            if username.trim().is_empty() {
                return Err("--username value must not be empty. \
                    Please provide a valid email address."
                    .to_string());
            }
        }

        // -p (password_prompt) requires -u (username)
        if self.password_prompt && self.username.is_none() {
            return Err("--password requires --username. \
                Please provide a username with -u/--username."
                .to_string());
        }

        // -n (newuser) requires -u (username) and -p (password)
        if self.newuser && self.username.is_none() {
            return Err("--newuser requires --username. \
                Please provide a username with -u/--username."
                .to_string());
        }

        // -y (use_password_as_crypto) requires -p (password_prompt)
        // We need a password to use it as the crypto password
        if self.use_password_as_crypto && !self.password_prompt {
            return Err("--passascrypto requires --password. \
                You must provide a password to use it as the crypto password."
                .to_string());
        }

        // -c (crypto_prompt) and -y (use_password_as_crypto) are mutually exclusive
        // Either prompt separately for crypto password OR use login password
        if self.crypto_prompt && self.use_password_as_crypto {
            return Err("Cannot use both --crypto and --passascrypto. \
                Use --crypto to prompt for a separate crypto password, \
                or --passascrypto to use the login password."
                .to_string());
        }

        // -t (auth_token) conflicts with -p (password_prompt)
        // Can't use both token and password authentication
        if self.auth_token.is_some() && self.password_prompt {
            return Err("Cannot use both --token and --password. \
                Use --token for token authentication, \
                or --password for username/password authentication."
                .to_string());
        }

        // -o (commands_mode) in foreground mode requires authentication credentials
        // Without -p or -t, the binary would attempt to initialize pclsync and connect
        // before entering the command loop, which requires pre-specified credentials
        if self.commands_mode && !self.commands_only && !self.password_prompt && self.auth_token.is_none() {
            return Err("--commands requires authentication credentials. \
                Use --password (-p) or --token (-t) to provide credentials."
                .to_string());
        }

        // --nosave and -s (savepassword) are mutually exclusive
        if self.nosave && self.save_password {
            return Err("Cannot use both --nosave and --savepassword. \
                Use --nosave to prevent saving credentials, \
                or --savepassword to explicitly save them (default behavior)."
                .to_string());
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
    ///
    /// Returns true if either crypto password prompt or use-password-as-crypto
    /// is enabled.
    pub fn wants_crypto(&self) -> bool {
        self.crypto_prompt || self.use_password_as_crypto
    }

    /// Check if interactive mode is requested.
    ///
    /// Interactive mode allows the user to send commands to the running
    /// client (e.g., startcrypto, stopcrypto, finalize, quit).
    pub fn wants_interactive(&self) -> bool {
        self.commands_mode
    }

    /// Get the mountpoint, applying default if not specified.
    ///
    /// If no mountpoint is specified, returns the default mountpoint
    /// which is ~/pCloud.
    pub fn get_mountpoint(&self) -> PathBuf {
        self.mountpoint
            .clone()
            .unwrap_or_else(Self::default_mountpoint)
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

    /// Check if any authentication credentials are provided via CLI.
    ///
    /// Returns `true` if username+password, token, or other auth methods
    /// are specified via command-line arguments.
    pub fn has_cli_credentials(&self) -> bool {
        self.password_prompt || self.auth_token.is_some()
    }

    /// Check if interactive authentication is needed.
    ///
    /// Returns `true` if no credentials are provided via CLI and
    /// we need to prompt the user for authentication method.
    pub fn needs_interactive_auth(&self) -> bool {
        !self.has_cli_credentials() && !self.commands_only
    }

    /// Check if credentials should be saved for future sessions.
    ///
    /// Returns `true` by default (save credentials). Returns `false` only
    /// when `--nosave` is explicitly specified.
    pub fn should_save_credentials(&self) -> bool {
        !self.nosave
    }

    /// Get the username, if provided.
    pub fn get_username(&self) -> Option<&str> {
        self.username.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_args() {
        let cli = Cli::parse_from_args(["pcloud", "-u", "test@example.com"]);
        assert_eq!(cli.username, Some("test@example.com".to_string()));
        assert!(!cli.password_prompt);
        assert!(!cli.daemonize);
    }

    #[test]
    fn test_parse_no_args() {
        // Username is now optional
        let cli = Cli::parse_from_args(["pcloud"]);
        assert!(cli.username.is_none());
    }

    #[test]
    fn test_parse_all_flags() {
        let cli = Cli::parse_from_args([
            "pcloud",
            "-u",
            "test@example.com",
            "-p",
            "-c",
            "-d",
            "-o",
            "-m",
            "/mnt/pcloud",
            "-n",
            "-s",
        ]);
        assert_eq!(cli.username, Some("test@example.com".to_string()));
        assert!(cli.password_prompt);
        assert!(cli.crypto_prompt);
        assert!(cli.daemonize);
        assert!(cli.commands_mode);
        assert_eq!(cli.mountpoint, Some(PathBuf::from("/mnt/pcloud")));
        assert!(cli.newuser);
        assert!(cli.save_password);
    }

    #[test]
    fn test_parse_token_flag() {
        let cli = Cli::parse_from_args(["pcloud", "-t", "my-auth-token"]);
        assert_eq!(cli.auth_token, Some("my-auth-token".to_string()));
        assert!(cli.username.is_none());
    }

    #[test]
    fn test_parse_long_flags() {
        let cli = Cli::parse_from_args([
            "pcloud",
            "--username",
            "user@test.com",
            "--password",
            "--daemon",
            "--mountpoint",
            "/home/user/cloud",
        ]);
        assert_eq!(cli.username, Some("user@test.com".to_string()));
        assert!(cli.password_prompt);
        assert!(cli.daemonize);
        assert_eq!(cli.mountpoint, Some(PathBuf::from("/home/user/cloud")));
    }

    #[test]
    fn test_conflicting_daemon_and_client() {
        let cli = Cli {
            username: Some("test@test.com".to_string()),
            daemonize: true,
            commands_only: true,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("daemon"));
    }

    #[test]
    fn test_password_requires_username() {
        let cli = Cli {
            password_prompt: true,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("username"));
    }

    #[test]
    fn test_passascrypto_requires_password() {
        let cli = Cli {
            username: Some("test@test.com".to_string()),
            use_password_as_crypto: true,
            password_prompt: false,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("password"));
    }

    #[test]
    fn test_passascrypto_with_password_valid() {
        let cli = Cli {
            username: Some("test@test.com".to_string()),
            use_password_as_crypto: true,
            password_prompt: true,
            ..Default::default()
        };
        assert!(cli.validate().is_ok());
    }

    #[test]
    fn test_crypto_and_passascrypto_conflict() {
        let cli = Cli {
            username: Some("test@test.com".to_string()),
            crypto_prompt: true,
            use_password_as_crypto: true,
            password_prompt: true,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--crypto"));
    }

    #[test]
    fn test_token_and_password_conflict() {
        let cli = Cli {
            username: Some("test@test.com".to_string()),
            auth_token: Some("token".to_string()),
            password_prompt: true,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--token"));
    }

    #[test]
    fn test_valid_daemon_with_mountpoint() {
        let cli = Cli {
            username: Some("test@test.com".to_string()),
            password_prompt: true,
            daemonize: true,
            mountpoint: Some(PathBuf::from("/mnt/pcloud")),
            ..Default::default()
        };
        assert!(cli.validate().is_ok());
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

        let cli2 = Cli {
            username: Some("test@test.com".to_string()),
            use_password_as_crypto: true,
            password_prompt: true,
            ..Default::default()
        };
        assert!(cli2.wants_crypto());

        let cli3 = Cli::default();
        assert!(!cli3.wants_crypto());
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
    fn test_default_values() {
        let cli = Cli::default();
        assert!(cli.username.is_none());
        assert!(!cli.password_prompt);
        assert!(cli.auth_token.is_none());
        assert!(!cli.crypto_prompt);
        assert!(!cli.use_password_as_crypto);
        assert!(!cli.daemonize);
        assert!(!cli.commands_mode);
        assert!(cli.mountpoint.is_none());
        assert!(!cli.commands_only);
        assert!(!cli.newuser);
        assert!(!cli.save_password);
        assert!(!cli.nosave);
        assert!(cli.should_save_credentials());
    }

    #[test]
    fn test_has_cli_credentials() {
        let cli1 = Cli {
            password_prompt: true,
            username: Some("test".to_string()),
            ..Default::default()
        };
        assert!(cli1.has_cli_credentials());

        let cli2 = Cli {
            auth_token: Some("token".to_string()),
            ..Default::default()
        };
        assert!(cli2.has_cli_credentials());

        let cli3 = Cli::default();
        assert!(!cli3.has_cli_credentials());
    }

    #[test]
    fn test_needs_interactive_auth() {
        // No credentials = needs interactive
        let cli1 = Cli::default();
        assert!(cli1.needs_interactive_auth());

        // Has password = doesn't need interactive
        let cli2 = Cli {
            password_prompt: true,
            username: Some("test".to_string()),
            ..Default::default()
        };
        assert!(!cli2.needs_interactive_auth());

        // Has token = doesn't need interactive
        let cli3 = Cli {
            auth_token: Some("token".to_string()),
            ..Default::default()
        };
        assert!(!cli3.needs_interactive_auth());

        // Client mode = doesn't need interactive
        let cli4 = Cli {
            commands_only: true,
            ..Default::default()
        };
        assert!(!cli4.needs_interactive_auth());
    }

    #[test]
    fn test_get_username() {
        let cli1 = Cli {
            username: Some("test@example.com".to_string()),
            ..Default::default()
        };
        assert_eq!(cli1.get_username(), Some("test@example.com"));

        let cli2 = Cli::default();
        assert_eq!(cli2.get_username(), None);
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
    fn test_nosave_and_savepassword_conflict() {
        let cli = Cli {
            nosave: true,
            save_password: true,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--nosave"));
    }

    #[test]
    fn test_parse_nosave_flag() {
        let cli = Cli::parse_from_args(["pcloud", "--nosave"]);
        assert!(cli.nosave);
        assert!(!cli.should_save_credentials());
    }
}
