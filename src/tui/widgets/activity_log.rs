use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;

use crate::tui::state::{Panel, TuiState};
use crate::tui::theme;

pub fn render(frame: &mut Frame, state: &mut TuiState, area: Rect) {
    let border_style = if state.active_panel == Panel::ActivityLog {
        theme::focused_border()
    } else {
        theme::unfocused_border()
    };

    let items: Vec<ListItem> = state
        .activity_log
        .iter()
        .map(|entry| {
            let style = if entry.is_error {
                theme::error_text()
            } else {
                theme::normal_text()
            };

            let line = Line::from(vec![
                Span::styled(entry.timestamp.clone(), theme::muted_text()),
                Span::raw("  "),
                Span::styled(entry.description.clone(), style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Activity Log ", theme::panel_title()))
                .border_style(border_style),
        )
        .highlight_style(theme::highlight_style());

    frame.render_stateful_widget(list, area, &mut state.log_state);
}
