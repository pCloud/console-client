use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::state::{CryptoAction, InputMode, TuiState};
use crate::tui::theme;

pub fn render(frame: &mut Frame, state: &TuiState) {
    let (title, is_password) = match &state.input_mode {
        InputMode::PasswordPrompt(CryptoAction::Unlock) => ("Enter Crypto Password", true),
        InputMode::PasswordPrompt(CryptoAction::Setup) => ("Set Crypto Password", true),
        InputMode::HintPrompt => ("Enter Password Hint", false),
        _ => return,
    };

    let area = centered_rect(50, 7, frame.area());

    // Clear the area first
    frame.render_widget(Clear, area);

    let display_text = if is_password {
        "*".repeat(state.input_buffer.len())
    } else {
        state.input_buffer.clone()
    };

    let label = if is_password { "Password" } else { "Hint" };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("  {}: ", label), theme::muted_text()),
            Span::styled(display_text, theme::normal_text()),
            Span::styled("_", theme::normal_text()), // cursor
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Enter to submit, Escape to cancel",
            theme::muted_text(),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(format!(" {} ", title), theme::title_style()))
        .border_style(theme::focused_border());

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Helper to create a centered rectangle.
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_width = (area.width as u32 * percent_x as u32 / 100).min(area.width as u32) as u16;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    Rect::new(
        area.x + x,
        area.y + y,
        popup_width.min(area.width),
        height.min(area.height),
    )
}
