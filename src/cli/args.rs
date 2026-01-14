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
#[derive(Parser, Debug, Clone)]
#[command(name = "pcloud")]
#[command(version, about = "pCloud Console Client")]
#[command(long_about = "Mount pCloud storage as a local filesystem.\n\n\
    This client allows you to access your pCloud storage through a FUSE \
    filesystem mount, with support for encrypted folders (Crypto) and \
    background daemon operation.")]
pub struct Cli {
    /// Username/email for pCloud account (required)
    #[arg(short = 'u', long = "username", required = true)]
    pub username: String,

    /// Prompt for password (interactive)
    #[arg(short = 'p', long = "password")]
    pub password_prompt: bool,

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
    #[arg(short = 'm', long = "mountpoint")]
    pub mountpoint: Option<PathBuf>,

    /// Send commands to running daemon (client mode)
    #[arg(short = 'k', long = "client")]
    pub commands_only: bool,

    /// Register new user account
    #[arg(short = 'n', long = "newuser")]
    pub newuser: bool,

    /// Save password for automatic login
    #[arg(short = 's', long = "savepassword")]
    pub save_password: bool,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            username: String::new(),
            password_prompt: false,
            crypto_prompt: false,
            use_password_as_crypto: false,
            daemonize: false,
            commands_mode: false,
            mountpoint: None,
            commands_only: false,
            newuser: false,
            save_password: false,
        }
    }
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
    ///     username: "test@example.com".to_string(),
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

        // -k (commands_only) typically requires -o (commands_mode) to be useful
        // but we allow it without warning as the user may have other intentions

        // -n (newuser) with -s (save_password) is valid - save after registration

        // -d (daemon) without -m (mountpoint) is suspicious but may be valid
        // for some use cases, so we don't enforce it

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
    /// based on the platform conventions.
    pub fn get_mountpoint(&self) -> PathBuf {
        self.mountpoint.clone().unwrap_or_else(|| {
            // Default mountpoint follows pCloud conventions
            if let Some(home) = std::env::var_os("HOME") {
                PathBuf::from(home).join("pCloudDrive")
            } else {
                PathBuf::from("/tmp/pCloudDrive")
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_args() {
        let cli = Cli::parse_from_args(["pcloud", "-u", "test@example.com"]);
        assert_eq!(cli.username, "test@example.com");
        assert!(!cli.password_prompt);
        assert!(!cli.daemonize);
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
        assert_eq!(cli.username, "test@example.com");
        assert!(cli.password_prompt);
        assert!(cli.crypto_prompt);
        assert!(cli.daemonize);
        assert!(cli.commands_mode);
        assert_eq!(cli.mountpoint, Some(PathBuf::from("/mnt/pcloud")));
        assert!(cli.newuser);
        assert!(cli.save_password);
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
        assert_eq!(cli.username, "user@test.com");
        assert!(cli.password_prompt);
        assert!(cli.daemonize);
        assert_eq!(cli.mountpoint, Some(PathBuf::from("/home/user/cloud")));
    }

    #[test]
    fn test_conflicting_daemon_and_client() {
        let cli = Cli {
            username: "test@test.com".to_string(),
            daemonize: true,
            commands_only: true,
            ..Default::default()
        };
        let result = cli.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("daemon"));
    }

    #[test]
    fn test_passascrypto_requires_password() {
        let cli = Cli {
            username: "test@test.com".to_string(),
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
            username: "test@test.com".to_string(),
            use_password_as_crypto: true,
            password_prompt: true,
            ..Default::default()
        };
        assert!(cli.validate().is_ok());
    }

    #[test]
    fn test_crypto_and_passascrypto_conflict() {
        let cli = Cli {
            username: "test@test.com".to_string(),
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
    fn test_valid_daemon_with_mountpoint() {
        let cli = Cli {
            username: "test@test.com".to_string(),
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
            username: "test@test.com".to_string(),
            commands_only: true,
            ..Default::default()
        };
        assert!(cli.is_client_mode());
    }

    #[test]
    fn test_wants_crypto() {
        let cli1 = Cli {
            username: "test@test.com".to_string(),
            crypto_prompt: true,
            ..Default::default()
        };
        assert!(cli1.wants_crypto());

        let cli2 = Cli {
            username: "test@test.com".to_string(),
            use_password_as_crypto: true,
            password_prompt: true,
            ..Default::default()
        };
        assert!(cli2.wants_crypto());

        let cli3 = Cli {
            username: "test@test.com".to_string(),
            ..Default::default()
        };
        assert!(!cli3.wants_crypto());
    }

    #[test]
    fn test_default_mountpoint() {
        let cli = Cli {
            username: "test@test.com".to_string(),
            ..Default::default()
        };
        let mountpoint = cli.get_mountpoint();
        // Should end with pCloudDrive
        assert!(mountpoint.to_string_lossy().ends_with("pCloudDrive"));
    }

    #[test]
    fn test_custom_mountpoint() {
        let cli = Cli {
            username: "test@test.com".to_string(),
            mountpoint: Some(PathBuf::from("/custom/path")),
            ..Default::default()
        };
        assert_eq!(cli.get_mountpoint(), PathBuf::from("/custom/path"));
    }

    #[test]
    fn test_missing_username_fails() {
        let result = Cli::try_parse_from_args(["pcloud"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_values() {
        let cli = Cli::default();
        assert!(cli.username.is_empty());
        assert!(!cli.password_prompt);
        assert!(!cli.crypto_prompt);
        assert!(!cli.use_password_as_crypto);
        assert!(!cli.daemonize);
        assert!(!cli.commands_mode);
        assert!(cli.mountpoint.is_none());
        assert!(!cli.commands_only);
        assert!(!cli.newuser);
        assert!(!cli.save_password);
    }
}
