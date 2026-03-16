use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::state::{Panel, TuiState};
use crate::tui::theme;

pub fn render(frame: &mut Frame, state: &TuiState, area: Rect) {
    let border_style = if state.active_panel == Panel::Mount {
        theme::focused_border()
    } else {
        theme::unfocused_border()
    };

    let (icon, icon_style, text) = if state.fs_mounted {
        let mp = state.mountpoint.as_deref().unwrap_or("unknown");
        ("V", theme::success_text(), format!("Mounted at {}", mp))
    } else {
        ("X", theme::error_text(), "Not mounted".to_string())
    };

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(icon, icon_style),
        Span::raw(" "),
        Span::styled(text, theme::normal_text()),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Mount ", theme::panel_title()))
        .border_style(border_style);

    let paragraph = Paragraph::new(vec![line]).block(block);
    frame.render_widget(paragraph, area);
}
