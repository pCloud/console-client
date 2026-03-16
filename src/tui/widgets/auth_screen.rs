use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::state::{InputMode, TuiState};
use crate::tui::theme;
use crate::utils::qrcode::generate_qr_code;

pub fn render(frame: &mut Frame, state: &TuiState) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Fill(1),   // Main content
        Constraint::Length(1), // Footer
    ])
    .split(area);

    match &state.input_mode {
        InputMode::AuthMenu => render_auth_menu(frame, state, chunks[0]),
        InputMode::AuthToken => render_token_input(frame, state, chunks[0]),
        InputMode::AuthWebWaiting(url) => {
            let url = url.clone();
            render_web_waiting(frame, state, chunks[0], &url);
        }
        _ => {}
    }

    // Footer help bar
    crate::tui::widgets::help_bar::render(frame, state, chunks[1]);
}

fn render_auth_menu(frame: &mut Frame, state: &TuiState, area: ratatui::layout::Rect) {
    let version = env!("CARGO_PKG_VERSION");

    let mut lines = vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Authentication Required",
            theme::title_style(),
        )),
        Line::from(""),
        Line::from(Span::styled("Select a login method:", theme::normal_text())),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [", theme::muted_text()),
            Span::styled("1", theme::key_hint_style()),
            Span::styled("] Web Login (opens browser)", theme::normal_text()),
        ]),
        Line::from(vec![
            Span::styled("  [", theme::muted_text()),
            Span::styled("2", theme::key_hint_style()),
            Span::styled("] Auth Token", theme::normal_text()),
        ]),
        Line::from(""),
        Line::from(Span::styled("Press 1 or 2 to select", theme::muted_text())),
    ];

    // Show status message if any
    if let Some((ref msg, ref kind)) = state.status_message {
        let style = match kind {
            crate::tui::state::StatusMessageKind::Success => theme::success_text(),
            crate::tui::state::StatusMessageKind::Error => theme::error_text(),
        };
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(msg.as_str(), style)));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " pCloud Console Client ",
            theme::title_style(),
        ))
        .title_bottom(
            Line::from(Span::styled(format!(" v{} ", version), theme::muted_text()))
                .alignment(Alignment::Right),
        )
        .border_style(theme::focused_border());

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_token_input(frame: &mut Frame, state: &TuiState, area: ratatui::layout::Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled("Enter Auth Token", theme::title_style())),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Token: ", theme::muted_text()),
            Span::styled(state.input_buffer.clone(), theme::normal_text()),
            Span::styled("_", theme::normal_text()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Paste your token and press Enter",
            theme::muted_text(),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " pCloud Console Client ",
            theme::title_style(),
        ))
        .border_style(theme::focused_border());

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_web_waiting(
    frame: &mut Frame,
    state: &TuiState,
    area: ratatui::layout::Rect,
    url: &str,
) {
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled("Web Login", theme::title_style())),
        Line::from(""),
        Line::from(Span::styled(
            "Open this URL in your browser:",
            theme::normal_text(),
        )),
        Line::from(""),
        Line::from(Span::styled(url, theme::status_syncing())),
        Line::from(""),
    ];

    // Try to add QR code
    if let Ok(qr) = generate_qr_code(url) {
        for qr_line in qr.lines() {
            lines.push(Line::from(Span::raw(qr_line.to_string())));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "Waiting for authentication...",
        theme::muted_text(),
    )));

    // Show status message if any
    if let Some((ref msg, ref kind)) = state.status_message {
        let style = match kind {
            crate::tui::state::StatusMessageKind::Success => theme::success_text(),
            crate::tui::state::StatusMessageKind::Error => theme::error_text(),
        };
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(msg.as_str(), style)));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " pCloud Console Client ",
            theme::title_style(),
        ))
        .border_style(theme::focused_border());

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false })
        .scroll((state.scroll_offset, 0));
    frame.render_widget(paragraph, area);
}
