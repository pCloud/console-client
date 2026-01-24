//! Interactive authentication prompts.
//!
//! This module provides interactive prompts for authentication when
//! credentials are not provided via command-line arguments.
//!
//! # Example
//!
//! ```ignore
//! use console_client::cli::auth_prompt::{prompt_auth_choice, AuthChoice};
//!
//! match prompt_auth_choice()? {
//!     AuthChoice::WebLogin => handle_web_login(),
//!     AuthChoice::EnterCredentials => handle_manual_login(),
//!     AuthChoice::EnterToken => handle_token_login(),
//!     AuthChoice::ShowCliHelp => print_cli_auth_help(),
//!     AuthChoice::Cancel => return Ok(()),
//! }
//! ```

use std::io::{self, BufRead, Write};

use secrecy::SecretString;

use crate::error::PCloudError;
use crate::security::prompt_for_password;
use crate::Result;

/// User's authentication method choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthChoice {
    /// Web-based login (opens browser)
    WebLogin,
    /// Enter username and password manually
    EnterCredentials,
    /// Enter authentication token directly
    EnterToken,
    /// Show CLI parameter help
    ShowCliHelp,
    /// Cancel authentication
    Cancel,
}

/// Display authentication options and get user choice.
///
/// Shows an interactive menu with authentication options and returns
/// the user's selection.
///
/// # Returns
///
/// The user's choice, or `Cancel` if the user presses 'q' or Ctrl+C.
///
/// # Example
///
/// ```ignore
/// use console_client::cli::auth_prompt::prompt_auth_choice;
///
/// let choice = prompt_auth_choice()?;
/// println!("User chose: {:?}", choice);
/// ```
pub fn prompt_auth_choice() -> Result<AuthChoice> {
    println!();
    println!("Authentication required.");
    println!();
    println!("How would you like to authenticate?");
    println!();
    println!("  [1] Web-based login (opens browser)");
    println!("  [2] Enter username and password");
    println!("  [3] Enter authentication token");
    println!("  [4] Show CLI parameters for scripted auth");
    println!("  [q] Cancel");
    println!();

    loop {
        print!("Choice [1]: ");
        io::stdout().flush().map_err(PCloudError::Io)?;

        let mut input = String::new();
        io::stdin()
            .lock()
            .read_line(&mut input)
            .map_err(PCloudError::Io)?;

        let input = input.trim();

        // Empty input defaults to option 1 (web login)
        if input.is_empty() {
            return Ok(AuthChoice::WebLogin);
        }

        match input {
            "1" => return Ok(AuthChoice::WebLogin),
            "2" => return Ok(AuthChoice::EnterCredentials),
            "3" => return Ok(AuthChoice::EnterToken),
            "4" => return Ok(AuthChoice::ShowCliHelp),
            "q" | "Q" | "quit" | "exit" | "cancel" => return Ok(AuthChoice::Cancel),
            _ => {
                println!("Invalid choice. Please enter 1, 2, 3, 4, or q.");
            }
        }
    }
}

/// Display help for CLI authentication parameters.
///
/// Shows the available command-line options for scripted/automated
/// authentication.
pub fn print_cli_auth_help() {
    println!();
    println!("CLI Authentication Options");
    println!("==========================");
    println!();
    println!("Password authentication:");
    println!("  pcloud -u your@email.com -p");
    println!();
    println!("Token authentication:");
    println!("  pcloud -t YOUR_AUTH_TOKEN");
    println!();
    println!("Custom mount path:");
    println!("  pcloud -m /path/to/mount");
    println!();
    println!("Save credentials for future use:");
    println!("  pcloud -u your@email.com -p -s");
    println!();
    println!("Run as daemon:");
    println!("  pcloud -d");
    println!();
    println!("Default mount path: ~/pCloud");
    println!();
}

/// Prompt for username/email.
///
/// # Returns
///
/// The entered username, or an error if reading fails.
pub fn prompt_username() -> Result<String> {
    print!("Email: ");
    io::stdout().flush().map_err(PCloudError::Io)?;

    let mut username = String::new();
    io::stdin()
        .lock()
        .read_line(&mut username)
        .map_err(PCloudError::Io)?;

    let username = username.trim().to_string();

    if username.is_empty() {
        return Err(PCloudError::InvalidArgument("Email cannot be empty".to_string()));
    }

    Ok(username)
}

/// Prompt for username and password.
///
/// # Returns
///
/// A tuple of (username, password) on success.
///
/// # Example
///
/// ```ignore
/// use console_client::cli::auth_prompt::prompt_credentials;
///
/// let (username, password) = prompt_credentials()?;
/// client.authenticate(&username, &password, save)?;
/// ```
pub fn prompt_credentials() -> Result<(String, SecretString)> {
    println!();

    let username = prompt_username()?;
    let password = prompt_for_password("Password: ").map_err(PCloudError::Io)?;

    Ok((username, password))
}

/// Prompt for authentication token.
///
/// # Returns
///
/// The entered token as a `SecretString`.
pub fn prompt_token() -> Result<SecretString> {
    println!();
    let token = prompt_for_password("Auth token: ").map_err(PCloudError::Io)?;

    if token.expose_secret().is_empty() {
        return Err(PCloudError::InvalidArgument("Auth token cannot be empty".to_string()));
    }

    Ok(token)
}

use secrecy::ExposeSecret;

/// Confirmation prompt for destructive actions.
///
/// # Arguments
///
/// * `message` - The confirmation message to display
///
/// # Returns
///
/// `true` if the user confirms, `false` otherwise.
pub fn prompt_confirm(message: &str) -> Result<bool> {
    print!("{} [y/N]: ", message);
    io::stdout().flush().map_err(PCloudError::Io)?;

    let mut input = String::new();
    io::stdin()
        .lock()
        .read_line(&mut input)
        .map_err(PCloudError::Io)?;

    let input = input.trim().to_lowercase();
    Ok(input == "y" || input == "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_choice_equality() {
        assert_eq!(AuthChoice::WebLogin, AuthChoice::WebLogin);
        assert_ne!(AuthChoice::WebLogin, AuthChoice::Cancel);
    }

    #[test]
    fn test_auth_choice_copy() {
        let choice = AuthChoice::EnterCredentials;
        let copy = choice;
        assert_eq!(choice, copy);
    }

    #[test]
    fn test_auth_choice_debug() {
        let choice = AuthChoice::WebLogin;
        let debug = format!("{:?}", choice);
        assert!(debug.contains("WebLogin"));
    }

    // Note: Interactive prompts can't be easily unit tested
    // without mocking stdin. Integration tests would be more appropriate.
}
