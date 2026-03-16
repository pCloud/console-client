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
    println!("  [2] Enter authentication token");
    println!("  [3] Show CLI parameters for scripted auth");
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
            "2" => return Ok(AuthChoice::EnterToken),
            "3" => return Ok(AuthChoice::ShowCliHelp),
            "q" | "Q" | "quit" | "exit" | "cancel" => return Ok(AuthChoice::Cancel),
            _ => {
                println!("Invalid choice. Please enter 1, 2, 3, or q.");
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
    println!("Token authentication:");
    println!("  pcloud -t YOUR_AUTH_TOKEN");
    println!();
    println!("Environment variables (for containers/automation):");
    println!("  PCLOUD_AUTH_TOKEN=<token> pcloud");
    println!("  PCLOUD_AUTH_TOKEN_FILE=/run/secrets/token pcloud");
    println!("  PCLOUD_CRYPTO_PASS=<password> pcloud");
    println!("  PCLOUD_CRYPTO_PASS_FILE=/run/secrets/crypto pcloud");
    println!("  PCLOUD_MOUNTPOINT=/mnt/pcloud pcloud");
    println!();
    println!("Custom mount path:");
    println!("  pcloud -m /path/to/mount");
    println!();
    println!("Don't save credentials:");
    println!("  pcloud --nosave");
    println!();
    println!("Run as daemon:");
    println!("  pcloud -d");
    println!();
    println!("Log out (clear saved credentials):");
    println!("  pcloud --logout");
    println!();
    println!("Unlink account (clear all local data):");
    println!("  pcloud --unlink");
    println!();
    println!("Default mount path: ~/pCloud");
    println!();
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
        return Err(PCloudError::InvalidArgument(
            "Auth token cannot be empty".to_string(),
        ));
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

/// High-friction confirmation prompt requiring the user to type a directory name.
///
/// Used for destructive operations where a simple y/N is insufficient.
/// Reads input character-by-character in raw terminal mode so that
/// pressing Esc immediately cancels the operation.
///
/// # Returns
///
/// `true` only if the user types `dir_name` exactly and presses Enter.
/// `false` if the user presses Esc or submits non-matching input.
pub fn prompt_confirm_by_name(dir_name: &str) -> Result<bool> {
    print!("Type \"{}\" to confirm (Esc to cancel): ", dir_name);
    io::stdout().flush().map_err(PCloudError::Io)?;

    let input = read_line_raw().map_err(PCloudError::Io)?;

    match input {
        None => {
            // Esc was pressed
            println!();
            Ok(false)
        }
        Some(s) => Ok(s == dir_name),
    }
}

/// Read a line from stdin in raw terminal mode.
///
/// Returns `None` if Esc is pressed, `Some(input)` on Enter.
/// Handles backspace for basic line editing.
fn read_line_raw() -> io::Result<Option<String>> {
    use std::os::unix::io::AsRawFd;

    let stdin_fd = io::stdin().as_raw_fd();

    // Save original terminal settings
    let mut orig_termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    if unsafe { libc::tcgetattr(stdin_fd, orig_termios.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let orig_termios = unsafe { orig_termios.assume_init() };

    // Enter raw mode: disable canonical mode and echo
    let mut raw = orig_termios;
    raw.c_lflag &= !(libc::ICANON | libc::ECHO);
    raw.c_cc[libc::VMIN] = 1;
    raw.c_cc[libc::VTIME] = 0;
    if unsafe { libc::tcsetattr(stdin_fd, libc::TCSANOW, &raw) } != 0 {
        return Err(io::Error::last_os_error());
    }

    let result = read_line_raw_loop();

    // Restore original terminal settings (always, even on error)
    unsafe { libc::tcsetattr(stdin_fd, libc::TCSANOW, &orig_termios) };

    result
}

/// Inner loop for raw line reading. Separated so terminal restore always runs.
fn read_line_raw_loop() -> io::Result<Option<String>> {
    use std::io::Read;

    let mut buf = [0u8; 1];
    let mut input = String::new();
    let stdin = io::stdin();

    loop {
        stdin.lock().read_exact(&mut buf)?;
        match buf[0] {
            // Esc
            0x1b => return Ok(None),
            // Enter
            b'\n' | b'\r' => {
                println!();
                return Ok(Some(input));
            }
            // Backspace / DEL
            0x7f | 0x08 => {
                if input.pop().is_some() {
                    // Move cursor back, overwrite with space, move back again
                    print!("\x08 \x08");
                    io::stdout().flush()?;
                }
            }
            // Ctrl-C
            0x03 => return Ok(None),
            // Regular printable character
            c if c >= 0x20 => {
                input.push(c as char);
                print!("{}", c as char);
                io::stdout().flush()?;
            }
            // Ignore other control characters
            _ => {}
        }
    }
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
        let choice = AuthChoice::EnterToken;
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
