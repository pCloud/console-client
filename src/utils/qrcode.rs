//! QR code generation for terminal display.
//!
//! This module provides utilities for generating and displaying QR codes
//! in the terminal using ASCII/Unicode block characters.
//!
//! # Example
//!
//! ```ignore
//! use console_client::utils::qrcode::{generate_qr_code, can_display_qr};
//!
//! if can_display_qr() {
//!     if let Ok(qr) = generate_qr_code("https://example.com") {
//!         println!("{}", qr);
//!     }
//! }
//! ```

use std::io;

/// Check if the terminal can likely display QR codes.
///
/// This checks:
/// - Terminal width (needs at least 40 columns for a reasonable QR code)
/// - Unicode support (needed for block characters)
///
/// # Returns
///
/// `true` if the terminal appears capable of displaying QR codes.
pub fn can_display_qr() -> bool {
    // Check terminal size
    let (cols, rows) = get_terminal_size();

    // Need at least 40 columns for a reasonable QR code display
    // and at least 20 rows
    if cols < 40 || rows < 20 {
        return false;
    }

    // Check Unicode support
    if !supports_unicode() {
        return false;
    }

    // Check if we're running in a terminal at all
    is_tty()
}

/// Get terminal size.
fn get_terminal_size() -> (usize, usize) {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let stdout = io::stdout();
        let fd = stdout.as_raw_fd();

        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        let result = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut size) };

        if result == 0 && size.ws_col > 0 && size.ws_row > 0 {
            return (size.ws_col as usize, size.ws_row as usize);
        }
    }

    // Default fallback
    (80, 24)
}

/// Check if the terminal likely supports Unicode.
fn supports_unicode() -> bool {
    for var in &["LANG", "LC_ALL", "LC_CTYPE"] {
        if let Ok(value) = std::env::var(var) {
            if value.to_lowercase().contains("utf") {
                return true;
            }
        }
    }
    false
}

/// Check if stdout is a TTY.
fn is_tty() -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let stdout = io::stdout();
        let fd = stdout.as_raw_fd();
        unsafe { libc::isatty(fd) == 1 }
    }

    #[cfg(not(unix))]
    {
        true // Assume TTY on non-Unix
    }
}

/// Generate a QR code as ASCII art for terminal display.
///
/// This uses the `qrcode` crate to generate a QR code and renders it
/// using Unicode block characters for compact display.
///
/// # Arguments
///
/// * `data` - The data to encode in the QR code (typically a URL)
///
/// # Returns
///
/// A `Result` containing the QR code as a string, or an error.
///
/// # Example
///
/// ```ignore
/// use console_client::utils::qrcode::generate_qr_code;
///
/// let qr = generate_qr_code("https://pcloud.com")?;
/// println!("{}", qr);
/// ```
pub fn generate_qr_code(data: &str) -> Result<String, QrCodeError> {
    use qrcode::render::unicode::Dense1x2;
    use qrcode::QrCode;

    let code =
        QrCode::new(data.as_bytes()).map_err(|e| QrCodeError::EncodingError(e.to_string()))?;

    let image = code
        .render::<Dense1x2>()
        .dark_color(Dense1x2::Light)
        .light_color(Dense1x2::Dark)
        .quiet_zone(true)
        .module_dimensions(1, 1)
        .build();

    Ok(image)
}

/// Generate a QR code with ASCII-only characters (fallback).
///
/// Uses `#` for dark modules and ` ` for light modules.
/// Less compact but works on any terminal.
pub fn generate_qr_code_ascii(data: &str) -> Result<String, QrCodeError> {
    use qrcode::QrCode;

    let code =
        QrCode::new(data.as_bytes()).map_err(|e| QrCodeError::EncodingError(e.to_string()))?;

    let width = code.width();
    let mut result = String::new();

    // Add quiet zone
    let quiet_zone = 2;
    let total_width = width + quiet_zone * 2;

    // Top quiet zone
    for _ in 0..quiet_zone {
        for _ in 0..total_width {
            result.push_str("  ");
        }
        result.push('\n');
    }

    // QR code content
    for y in 0..width {
        // Left quiet zone
        for _ in 0..quiet_zone {
            result.push_str("  ");
        }

        for x in 0..width {
            let is_dark = code[(x, y)] == qrcode::Color::Dark;
            if is_dark {
                result.push_str("\u{2588}\u{2588}"); // Full block
            } else {
                result.push_str("  ");
            }
        }

        // Right quiet zone
        for _ in 0..quiet_zone {
            result.push_str("  ");
        }

        result.push('\n');
    }

    // Bottom quiet zone
    for _ in 0..quiet_zone {
        for _ in 0..total_width {
            result.push_str("  ");
        }
        result.push('\n');
    }

    Ok(result)
}

/// Errors that can occur during QR code generation.
#[derive(Debug)]
pub enum QrCodeError {
    /// Data could not be encoded into a QR code
    EncodingError(String),
    /// QR code is too large for the terminal
    TooLarge,
    /// Terminal doesn't support QR code display
    NotSupported,
}

impl std::fmt::Display for QrCodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QrCodeError::EncodingError(msg) => write!(f, "QR encoding error: {}", msg),
            QrCodeError::TooLarge => write!(f, "QR code too large for terminal"),
            QrCodeError::NotSupported => write!(f, "Terminal doesn't support QR code display"),
        }
    }
}

impl std::error::Error for QrCodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_qr_code() {
        let result = generate_qr_code("https://pcloud.com");
        assert!(result.is_ok());
        let qr = result.unwrap();
        assert!(!qr.is_empty());
    }

    #[test]
    fn test_generate_qr_code_long_url() {
        let long_url = "https://my.pcloud.com/webview/authentication?view=login&request_id=12345678&device=test";
        let result = generate_qr_code(long_url);
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_qr_code_ascii() {
        let result = generate_qr_code_ascii("test");
        assert!(result.is_ok());
        let qr = result.unwrap();
        assert!(!qr.is_empty());
    }

    #[test]
    fn test_qr_code_error_display() {
        let err = QrCodeError::EncodingError("test error".to_string());
        assert!(err.to_string().contains("test error"));

        let err = QrCodeError::TooLarge;
        assert!(err.to_string().contains("too large"));

        let err = QrCodeError::NotSupported;
        assert!(err.to_string().contains("support"));
    }
}
