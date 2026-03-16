use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::ffi::types::{is_error_status, is_syncing, PSTATUS_READY};
use crate::tui::state::TuiState;
use crate::tui::theme;

pub fn render(frame: &mut Frame, state: &TuiState, area: Rect) {
    let status_style = if state.status.status == PSTATUS_READY {
        theme::status_ready()
    } else if is_error_status(state.status.status) {
        theme::status_error()
    } else if is_syncing(state.status.status) {
        theme::status_syncing()
    } else {
        theme::normal_text()
    };

    let email_str = state.account_email.as_deref().unwrap_or("--");
    let storage_str = if state.quota_total > 0 {
        let pct = (state.quota_used as f64 / state.quota_total as f64 * 100.0) as u64;
        format!(
            "{} / {} ({}%)",
            format_bytes(state.quota_used),
            format_bytes(state.quota_total),
            pct
        )
    } else {
        "--".to_string()
    };

    // Status message line (if any)
    let status_msg_span = if let Some((ref msg, ref kind)) = state.status_message {
        let style = match kind {
            crate::tui::state::StatusMessageKind::Success => theme::success_text(),
            crate::tui::state::StatusMessageKind::Error => theme::error_text(),
        };
        Span::styled(format!("  {}", msg), style)
    } else {
        Span::raw("")
    };

    let line1 = Line::from(vec![
        Span::styled("  Status: ", theme::muted_text()),
        Span::styled(&state.status.status_str, status_style),
        Span::raw("          "),
        Span::styled("Account: ", theme::muted_text()),
        Span::styled(email_str, theme::normal_text()),
        status_msg_span,
    ]);

    let line2 = Line::from(vec![
        Span::raw("                          "),
        Span::styled("Storage: ", theme::muted_text()),
        Span::styled(storage_str, theme::normal_text()),
    ]);

    let version = env!("CARGO_PKG_VERSION");
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " pCloud Console Client ",
            theme::title_style(),
        ))
        .title_alignment(Alignment::Left)
        .border_style(theme::focused_border())
        .title_bottom(
            Line::from(vec![Span::styled(
                format!(" v{} ", version),
                theme::muted_text(),
            )])
            .alignment(Alignment::Right),
        );

    let paragraph = Paragraph::new(vec![line1, line2]).block(block);
    frame.render_widget(paragraph, area);
}

/// Format bytes into human-readable form.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format speed in bytes/sec into human-readable form.
pub fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec == 0 {
        return "0 B/s".to_string();
    }
    format!("{}/s", format_bytes(bytes_per_sec))
}
