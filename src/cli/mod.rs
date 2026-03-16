//! CLI argument parsing and command execution.
//!
//! This module handles:
//! - Command-line argument parsing using clap
//! - CLI command definitions and execution
//! - User interaction (prompts, output formatting)
//! - Authentication prompts
//!
//! # Submodules
//!
//! - `args`: Clap argument definitions and CLI struct
//! - `commands`: Interactive command parsing and execution
//! - `auth_prompt`: Interactive authentication prompts
//!
//! # Example
//!
//! ```
//! use console_client::cli::{Cli, InteractiveCommand};
//!
//! // Parse CLI arguments
//! // let cli = Cli::parse_args();
//!
//! // Parse interactive commands
//! let cmd = InteractiveCommand::parse("startcrypto");
//! assert_eq!(cmd, InteractiveCommand::StartCrypto);
//! ```

pub mod args;
pub mod auth_prompt;
pub mod commands;

// Re-export main types for convenience
pub use args::Cli;
pub use auth_prompt::{
    print_cli_auth_help, prompt_auth_choice, prompt_confirm, prompt_confirm_by_name, prompt_token,
    AuthChoice,
};
pub use commands::{CommandPrompt, InteractiveCommand};

/// Initialize CLI module.
///
/// This function can be used for any CLI-related initialization
/// that needs to happen at startup.
pub fn init() {
    // Currently no initialization needed
    // Future versions might set up terminal handling, etc.
}
