use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

use super::app::App;
use super::state::InputMode;
use super::widgets;

/// Top-level render function.
pub fn render(frame: &mut Frame, app: &mut App) {
    let state = &mut app.state;

    // Check if we need to show auth screen
    if matches!(
        state.input_mode,
        InputMode::AuthMenu | InputMode::AuthToken | InputMode::AuthWebWaiting(_)
    ) {
        widgets::auth_screen::render(frame, state);
        return;
    }

    // Dashboard layout
    let chunks = Layout::vertical([
        Constraint::Length(4), // Header (2 content lines + 2 borders)
        Constraint::Length(3), // Mount
        Constraint::Length(3), // Crypto
        Constraint::Length(4), // Transfers
        Constraint::Fill(1),   // Activity log
        Constraint::Length(1), // Footer
    ])
    .split(frame.area());

    widgets::header::render(frame, state, chunks[0]);
    widgets::mount_panel::render(frame, state, chunks[1]);
    widgets::crypto_panel::render(frame, state, chunks[2]);
    widgets::transfer::render(frame, state, chunks[3]);
    widgets::activity_log::render(frame, state, chunks[4]);
    widgets::help_bar::render(frame, state, chunks[5]);

    // Render password overlay if in password/hint mode
    if matches!(
        state.input_mode,
        InputMode::PasswordPrompt(_) | InputMode::HintPrompt
    ) {
        widgets::password_input::render(frame, state);
    }
}
