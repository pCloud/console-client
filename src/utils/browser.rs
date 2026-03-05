//! Browser opening utility.
//!
//! This module provides utilities for opening URLs in the user's default
//! web browser, with support for detecting graphical environments.
//!
//! # Example
//!
//! ```ignore
//! use console_client::utils::browser::{open_url, has_display};
//!
//! if has_display() {
//!     match open_url("https://example.com") {
//!         Ok(true) => println!("Browser opened!"),
//!         Ok(false) => println!("Could not open browser"),
//!         Err(e) => eprintln!("Error: {}", e),
//!     }
//! } else {
//!     println!("No display available - please copy the URL manually");
//! }
//! ```

use std::process::Command;

/// Check if a graphical display is available.
///
/// This checks for common environment variables that indicate a graphical
/// session is running (X11, Wayland, macOS, etc.).
///
/// # Returns
///
/// `true` if a graphical display appears to be available.
///
/// # Example
///
/// ```
/// use console_client::utils::browser::has_display;
///
/// if has_display() {
///     println!("GUI available");
/// } else {
///     println!("Running in text-only mode (SSH, console, etc.)");
/// }
/// ```
pub fn has_display() -> bool {
    // Check for X11 display
    if std::env::var("DISPLAY").is_ok() {
        return true;
    }

    // Check for Wayland
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return true;
    }

    // Check for macOS (always has a display when logged in via GUI)
    #[cfg(target_os = "macos")]
    {
        // On macOS, check if we're in a GUI session
        if std::env::var("TERM_PROGRAM").is_ok() {
            return true;
        }
    }

    // Check for Windows (always has a display in normal circumstances)
    #[cfg(target_os = "windows")]
    {
        return true;
    }

    // Check for generic GUI session
    if std::env::var("XDG_SESSION_TYPE")
        .map(|v| v != "tty")
        .unwrap_or(false)
    {
        return true;
    }

    // Check for SSH session (implies no local display, but DISPLAY might be forwarded)
    if std::env::var("SSH_CONNECTION").is_ok() && std::env::var("DISPLAY").is_err() {
        return false;
    }

    false
}

/// Try to open a URL in the default browser.
///
/// Uses platform-specific commands:
/// - Linux: `xdg-open`
/// - macOS: `open`
/// - Windows: `start`
///
/// # Arguments
///
/// * `url` - The URL to open
///
/// # Returns
///
/// - `Ok(true)` if the browser was opened successfully
/// - `Ok(false)` if no suitable command was found
/// - `Err` if an error occurred during execution
///
/// # Example
///
/// ```ignore
/// use console_client::utils::browser::open_url;
///
/// match open_url("https://pcloud.com") {
///     Ok(true) => println!("Browser opened successfully"),
///     Ok(false) => println!("Could not find a browser to open"),
///     Err(e) => eprintln!("Error opening browser: {}", e),
/// }
/// ```
pub fn open_url(url: &str) -> Result<bool, std::io::Error> {
    // Validate URL (basic check)
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "URL must start with http:// or https://",
        ));
    }

    #[cfg(target_os = "linux")]
    {
        // Try xdg-open first (most common)
        if try_open_with("xdg-open", &[url])? {
            return Ok(true);
        }

        // Try other common browsers/openers as fallback
        for cmd in &[
            "x-www-browser",
            "gnome-open",
            "kde-open",
            "firefox",
            "chromium",
            "google-chrome",
        ] {
            if try_open_with(cmd, &[url])? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    #[cfg(target_os = "macos")]
    {
        try_open_with("open", &[url])
    }

    #[cfg(target_os = "windows")]
    {
        try_open_with("cmd", &["/c", "start", "", url])
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        // Unsupported platform
        Ok(false)
    }
}

/// Try to open a URL with a specific command.
fn try_open_with(cmd: &str, args: &[&str]) -> Result<bool, std::io::Error> {
    match Command::new(cmd).args(args).spawn() {
        Ok(mut child) => {
            // Don't wait for the browser process to complete
            // Just check if it started successfully
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Command not found - not an error, just return false
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

/// Open URL with explicit browser preference.
///
/// This allows specifying a preferred browser command.
///
/// # Arguments
///
/// * `url` - The URL to open
/// * `browser` - The browser command to use (e.g., "firefox", "chrome")
pub fn open_url_with(url: &str, browser: &str) -> Result<bool, std::io::Error> {
    try_open_with(browser, &[url])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_display_returns_bool() {
        // Just verify it returns a bool without panicking
        let _result = has_display();
    }

    #[test]
    fn test_open_url_invalid_url() {
        let result = open_url("not-a-url");
        assert!(result.is_err());
    }

    #[test]
    fn test_open_url_ftp_url() {
        let result = open_url("ftp://example.com");
        assert!(result.is_err());
    }

    // Note: We can't really test actual browser opening in unit tests
    // as it depends on the system's browser configuration.
}
