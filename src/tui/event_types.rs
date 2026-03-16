use ratatui::crossterm::event::KeyEvent;

use super::state::StatusSnapshot;

/// Events that the TUI event loop processes.
#[allow(dead_code)]
pub enum TuiEvent {
    /// Keyboard input
    Key(KeyEvent),
    /// Status update from C library callback
    StatusUpdate(StatusSnapshot),
    /// File event from C library callback
    FileEvent { description: String, is_error: bool },
    /// Filesystem mounted notification
    FsMounted,
    /// Periodic tick for polling client state
    Tick,
    /// Result from background web auth thread
    WebAuthResult(std::result::Result<(), String>),
    /// Quit signal
    Quit,
}
