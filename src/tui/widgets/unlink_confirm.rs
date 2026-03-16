use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::state::{InputMode, TuiState};
use crate::tui::theme;

pub fn render(frame: &mut Frame, state: &TuiState) {
    if state.input_mode != InputMode::UnlinkConfirm {
        return;
    }

    let area = centered_rect(50, 11, frame.area());
    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  This will clear ALL local data:",
            theme::error_text(),
        )),
        Line::from(Span::styled(
            "  - Saved credentials",
            theme::normal_text(),
        )),
        Line::from(Span::styled("  - Sync database", theme::normal_text())),
        Line::from(Span::styled("  - All cached data", theme::normal_text())),
        Line::from(""),
        Line::from(Span::styled(
            "  This cannot be undone!",
            theme::error_text(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Continue? (", theme::muted_text()),
            Span::styled("y", theme::key_hint_style()),
            Span::styled("/", theme::muted_text()),
            Span::styled("N", theme::key_hint_style()),
            Span::styled(")", theme::muted_text()),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Unlink Account ", theme::error_text()))
        .border_style(theme::status_error());

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

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
