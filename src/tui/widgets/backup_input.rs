use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::state::{InputMode, TuiState};
use crate::tui::theme;

/// Modal popup that collects the local folder path for a new backup.
pub fn render(frame: &mut Frame, state: &TuiState) {
    if state.input_mode != InputMode::BackupAdd {
        return;
    }

    let area = centered_rect(60, 7, frame.area());
    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Folder path: ", theme::muted_text()),
            Span::styled(state.input_buffer.clone(), theme::normal_text()),
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
        .title(Span::styled(" Add Backup ", theme::title_style()))
        .border_style(theme::focused_border());

    frame.render_widget(Paragraph::new(lines).block(block), area);
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
