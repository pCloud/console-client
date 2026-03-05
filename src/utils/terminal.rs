//! Terminal formatting utilities.
//!
//! This module provides utilities for rich terminal output including:
//! - ASCII box drawing around text
//! - Status indicators with symbols
//! - Terminal size detection
//!
//! # Example
//!
//! ```
//! use console_client::utils::terminal::{print_boxed, print_status, StatusIndicator};
//!
//! // Print text in a box
//! print_boxed(&["Open this URL:", "", "https://example.com"]);
//!
//! // Print status messages
//! print_status(StatusIndicator::Info, "Initializing...");
//! print_status(StatusIndicator::Success, "Done!");
//! ```

use std::io::{self, Write};

/// Status indicator types for terminal output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusIndicator {
    /// Informational message `[*]`
    Info,
    /// Success message `[+]` (or checkmark if unicode supported)
    Success,
    /// Error message `[-]` (or X if unicode supported)
    Error,
    /// Warning message `[!]`
    Warning,
    /// Progress/waiting message `[~]`
    Progress,
}

impl StatusIndicator {
    /// Get the ASCII representation of the indicator.
    pub fn as_ascii(&self) -> &'static str {
        match self {
            StatusIndicator::Info => "[*]",
            StatusIndicator::Success => "[+]",
            StatusIndicator::Error => "[-]",
            StatusIndicator::Warning => "[!]",
            StatusIndicator::Progress => "[~]",
        }
    }

    /// Get the Unicode representation of the indicator (if supported).
    pub fn as_unicode(&self) -> &'static str {
        match self {
            StatusIndicator::Info => "[*]",
            StatusIndicator::Success => "[\u{2713}]", // checkmark
            StatusIndicator::Error => "[\u{2717}]",   // X mark
            StatusIndicator::Warning => "[!]",
            StatusIndicator::Progress => "[~]",
        }
    }

    /// Get the indicator string based on terminal capabilities.
    pub fn as_str(&self) -> &'static str {
        if supports_unicode() {
            self.as_unicode()
        } else {
            self.as_ascii()
        }
    }
}

/// Check if the terminal likely supports Unicode.
fn supports_unicode() -> bool {
    // Check LANG environment variable
    if let Ok(lang) = std::env::var("LANG") {
        if lang.to_lowercase().contains("utf") {
            return true;
        }
    }

    // Check LC_ALL
    if let Ok(lc_all) = std::env::var("LC_ALL") {
        if lc_all.to_lowercase().contains("utf") {
            return true;
        }
    }

    // Check LC_CTYPE
    if let Ok(lc_ctype) = std::env::var("LC_CTYPE") {
        if lc_ctype.to_lowercase().contains("utf") {
            return true;
        }
    }

    // Default to ASCII for safety
    false
}

/// Print a status message with an indicator.
///
/// # Arguments
///
/// * `indicator` - The type of status indicator
/// * `message` - The message to print
///
/// # Example
///
/// ```
/// use console_client::utils::terminal::{print_status, StatusIndicator};
///
/// print_status(StatusIndicator::Info, "Starting sync...");
/// print_status(StatusIndicator::Success, "Sync complete!");
/// ```
pub fn print_status(indicator: StatusIndicator, message: &str) {
    println!("{} {}", indicator.as_str(), message);
}

/// Print a status message to stderr.
pub fn eprint_status(indicator: StatusIndicator, message: &str) {
    eprintln!("{} {}", indicator.as_str(), message);
}

/// Get the terminal width, with a fallback default.
fn terminal_width() -> usize {
    // Try to get terminal size from environment or system
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(width) = cols.parse::<usize>() {
            return width;
        }
    }

    // Try using ioctl on Unix
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let stdout = io::stdout();
        let fd = stdout.as_raw_fd();

        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        let result = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut size) };

        if result == 0 && size.ws_col > 0 {
            return size.ws_col as usize;
        }
    }

    // Default fallback
    80
}

/// Print text inside an ASCII box.
///
/// Creates a nicely formatted box around the provided lines of text.
///
/// # Arguments
///
/// * `lines` - The lines of text to display inside the box
///
/// # Example
///
/// ```
/// use console_client::utils::terminal::print_boxed;
///
/// print_boxed(&[
///     "Open this URL in your browser:",
///     "",
///     "https://my.pcloud.com/webview/...",
/// ]);
/// ```
///
/// Output:
/// ```text
/// +--------------------------------------+
/// |  Open this URL in your browser:     |
/// |                                      |
/// |  https://my.pcloud.com/webview/...  |
/// +--------------------------------------+
/// ```
pub fn print_boxed(lines: &[&str]) {
    let output = format_boxed(lines);
    print!("{}", output);
    let _ = io::stdout().flush();
}

/// Format text inside an ASCII box (returns the formatted string).
///
/// # Arguments
///
/// * `lines` - The lines of text to display inside the box
///
/// # Returns
///
/// A formatted string containing the box with content.
pub fn format_boxed(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let term_width = terminal_width();
    let max_box_width = term_width.saturating_sub(4); // Leave some margin

    // Find the longest line
    let max_line_len = lines.iter().map(|s| s.chars().count()).max().unwrap_or(0);

    // Calculate box width (content + padding)
    let content_width = max_line_len.min(max_box_width);
    let box_inner_width = content_width + 4; // 2 chars padding on each side

    // Use Unicode box drawing characters if supported
    let (tl, tr, bl, br, h, v) = if supports_unicode() {
        (
            '\u{256D}', '\u{256E}', '\u{2570}', '\u{256F}', '\u{2500}', '\u{2502}',
        )
    } else {
        ('+', '+', '+', '+', '-', '|')
    };

    let mut output = String::new();

    // Top border
    output.push(tl);
    for _ in 0..box_inner_width {
        output.push(h);
    }
    output.push(tr);
    output.push('\n');

    // Content lines
    for line in lines {
        let display_line = if line.chars().count() > content_width {
            // Truncate long lines
            let mut truncated: String = line.chars().take(content_width - 3).collect();
            truncated.push_str("...");
            truncated
        } else {
            line.to_string()
        };

        let padding = content_width - display_line.chars().count();
        output.push(v);
        output.push_str("  "); // Left padding
        output.push_str(&display_line);
        for _ in 0..padding {
            output.push(' ');
        }
        output.push_str("  "); // Right padding
        output.push(v);
        output.push('\n');
    }

    // Bottom border
    output.push(bl);
    for _ in 0..box_inner_width {
        output.push(h);
    }
    output.push(br);
    output.push('\n');

    output
}

/// Print a centered line of text.
pub fn print_centered(text: &str) {
    let term_width = terminal_width();
    let text_len = text.chars().count();

    if text_len >= term_width {
        println!("{}", text);
        return;
    }

    let padding = (term_width - text_len) / 2;
    let spaces: String = std::iter::repeat(' ').take(padding).collect();
    println!("{}{}", spaces, text);
}

/// Print a horizontal line.
pub fn print_horizontal_line() {
    let term_width = terminal_width();
    let line_char = if supports_unicode() { '\u{2500}' } else { '-' };
    let line: String = std::iter::repeat(line_char).take(term_width).collect();
    println!("{}", line);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_indicator_ascii() {
        assert_eq!(StatusIndicator::Info.as_ascii(), "[*]");
        assert_eq!(StatusIndicator::Success.as_ascii(), "[+]");
        assert_eq!(StatusIndicator::Error.as_ascii(), "[-]");
        assert_eq!(StatusIndicator::Warning.as_ascii(), "[!]");
        assert_eq!(StatusIndicator::Progress.as_ascii(), "[~]");
    }

    #[test]
    fn test_format_boxed_empty() {
        let result = format_boxed(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_boxed_single_line() {
        let result = format_boxed(&["Hello"]);
        assert!(result.contains("Hello"));
        // Should have top and bottom borders
        assert!(result.lines().count() >= 3);
    }

    #[test]
    fn test_format_boxed_multiple_lines() {
        let result = format_boxed(&["Line 1", "Line 2", "Line 3"]);
        assert!(result.contains("Line 1"));
        assert!(result.contains("Line 2"));
        assert!(result.contains("Line 3"));
    }

    #[test]
    fn test_status_indicator_equality() {
        assert_eq!(StatusIndicator::Info, StatusIndicator::Info);
        assert_ne!(StatusIndicator::Info, StatusIndicator::Error);
    }

    #[test]
    fn test_status_indicator_copy() {
        let indicator = StatusIndicator::Success;
        let copy = indicator;
        assert_eq!(indicator, copy);
    }
}
