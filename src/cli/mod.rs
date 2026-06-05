//! CLI argument parsing and authentication prompts.
//!
//! # Submodules
//!
//! - `args`: clap subcommand tree for the hierarchical CLI
//! - `auth_prompt`: interactive authentication prompts used by `auth login`
//!   and other paths that need credentials
//!
//! # Example
//!
//! ```
//! use console_client::cli::Cli;
//!
//! // Parse CLI arguments
//! // let cli = Cli::parse_args();
//! ```

pub mod args;
pub mod auth_prompt;

// Re-export main types for convenience
pub use args::{
    default_mountpoint, resolve_mountpoint, AuthArgs, AuthOp, BackupArgs, BackupOp, Cli, Command,
    CryptoArgs, CryptoOp, MountArgs, ServiceArgs, ServiceOp, StartArgs,
};
pub use auth_prompt::{
    print_cli_auth_help, prompt_auth_choice, prompt_confirm, prompt_confirm_by_name, prompt_token,
    AuthChoice,
};

/// Initialize CLI module.
///
/// Currently a no-op placeholder reserved for future setup work.
pub fn init() {}
