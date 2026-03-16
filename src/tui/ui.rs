use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Clear;
use ratatui::Frame;

use super::app::App;
use super::state::{InputMode, Screen};
use super::widgets;

/// Top-level render function.
pub fn render(frame: &mut Frame, app: &mut App) {
    let state = &mut app.state;

    // One-shot clear: wipe stale cells after a layout-incompatible screen
    // transition (e.g. auth QR code → dashboard).
    if state.needs_clear {
        frame.render_widget(Clear, frame.area());
        state.needs_clear = false;
    }

    // Check if we need to show auth screen
    if matches!(
        state.input_mode,
        InputMode::AuthMenu | InputMode::AuthToken | InputMode::AuthWebWaiting(_)
    ) {
        widgets::auth_screen::render(frame, state);
        return;
    }

    // Top-level layout: tab bar + content + help bar
    let outer = Layout::vertical([
        Constraint::Length(1), // Tab bar
        Constraint::Fill(1),  // Content area
        Constraint::Length(1), // Help bar
    ])
    .split(frame.area());

    widgets::tab_bar::render(frame, state, outer[0]);

    match state.active_screen {
        Screen::Dashboard => render_dashboard(frame, app, outer[1]),
        Screen::Help => widgets::help_screen::render(frame, outer[1]),
        Screen::About => widgets::about_screen::render(frame, outer[1]),
    }

    widgets::help_bar::render(frame, &app.state, outer[2]);

    // Render modal overlays on top of everything
    if matches!(
        app.state.input_mode,
        InputMode::PasswordPrompt(_) | InputMode::HintPrompt
    ) {
        widgets::password_input::render(frame, &app.state);
    }

    if app.state.input_mode == InputMode::UnlinkConfirm {
        widgets::unlink_confirm::render(frame, &app.state);
    }
}

/// Render the dashboard content area.
fn render_dashboard(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let state = &mut app.state;

    let chunks = Layout::vertical([
        Constraint::Length(4), // Header (2 content lines + 2 borders)
        Constraint::Length(3), // Mount
        Constraint::Length(3), // Crypto
        Constraint::Length(4), // Transfers
        Constraint::Fill(1),  // Activity log
    ])
    .split(area);

    widgets::header::render(frame, state, chunks[0]);
    widgets::mount_panel::render(frame, state, chunks[1]);
    widgets::crypto_panel::render(frame, state, chunks[2]);
    widgets::transfer::render(frame, state, chunks[3]);
    widgets::activity_log::render(frame, state, chunks[4]);
}
