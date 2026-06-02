use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::tui::state::{StatusMessageKind, TuiState};
use crate::tui::theme;

/// Render the Backups screen: a device-root header above a selectable list of
/// the backups configured for this device.
pub fn render(frame: &mut Frame, state: &mut TuiState, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(2), // Device / root-name header
        Constraint::Fill(1),   // Backups list
    ])
    .split(area);

    render_header(frame, state, chunks[0]);
    render_list(frame, state, chunks[1]);
}

fn render_header(frame: &mut Frame, state: &TuiState, area: Rect) {
    let root = state
        .backup_root_name
        .clone()
        .unwrap_or_else(|| "(unknown)".to_string());
    let root_line = Line::from(vec![
        Span::raw("  "),
        Span::styled("Backup root: ", theme::muted_text()),
        Span::styled(root, theme::normal_text()),
        Span::raw("   "),
        Span::styled(
            format!("{} backup(s)", state.backups.len()),
            theme::muted_text(),
        ),
    ]);

    // Second header row doubles as the transient status-message line, since
    // the dashboard header (which normally shows it) is not rendered here.
    let status_line = match &state.status_message {
        Some((msg, kind)) => {
            let style = match kind {
                StatusMessageKind::Success => theme::success_text(),
                StatusMessageKind::Error => theme::error_text(),
            };
            Line::from(Span::styled(format!("  {}", msg), style))
        }
        None => Line::from(""),
    };

    frame.render_widget(Paragraph::new(vec![root_line, status_line]), area);
}

fn render_list(frame: &mut Frame, state: &mut TuiState, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Backups ", theme::panel_title()))
        .border_style(theme::focused_border());

    if let Some(err) = &state.backup_error {
        let p = Paragraph::new(Line::from(Span::styled(
            format!("  Failed to load backups: {}", err),
            theme::error_text(),
        )))
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    if state.backups.is_empty() {
        let p = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No backups configured.",
                theme::muted_text(),
            )),
            Line::from(vec![
                Span::styled("  Press ", theme::muted_text()),
                Span::styled("a", theme::key_hint_style()),
                Span::styled(" to add a folder to back up.", theme::muted_text()),
            ]),
        ])
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = state
        .backups
        .iter()
        .map(|b| {
            let line = Line::from(vec![
                Span::styled(format!("{:<6}", b.sync_id), theme::muted_text()),
                Span::styled(b.local_path.display().to_string(), theme::normal_text()),
                Span::styled("  ->  ", theme::muted_text()),
                Span::styled(b.remote_path.clone(), theme::status_syncing()),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(theme::highlight_style());

    frame.render_stateful_widget(list, area, &mut state.backup_list_state);
}
