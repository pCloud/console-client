use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::state::{Screen, TuiState};
use crate::tui::theme;

pub fn render(frame: &mut Frame, state: &TuiState, area: Rect) {
    let tabs = [
        ("1", "Dashboard", Screen::Dashboard),
        ("2", "Help", Screen::Help),
        ("3", "About", Screen::About),
    ];

    let mut spans = Vec::new();
    spans.push(Span::raw(" "));

    for (key, label, screen) in &tabs {
        let is_active = state.active_screen == *screen;
        if is_active {
            spans.push(Span::styled(
                format!(" {} {} ", key, label),
                theme::title_style(),
            ));
        } else {
            spans.push(Span::styled(format!(" {} ", key), theme::key_hint_style()));
            spans.push(Span::styled(format!("{} ", label), theme::muted_text()));
        }
        spans.push(Span::raw(" "));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
