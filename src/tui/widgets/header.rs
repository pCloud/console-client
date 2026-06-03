use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::ffi::types::{status_icon, status_kind, StatusKind};
use crate::tui::state::TuiState;
use crate::tui::theme;
use crate::tui::widgets::util;

// Width of the widest label ("Data region: ") used to left-align values.
const LABEL_WIDTH: usize = 13;

pub fn render(frame: &mut Frame, state: &TuiState, area: Rect) {
    let status_style = match status_kind(state.status.status) {
        StatusKind::Ready => theme::status_ready(),
        StatusKind::InProgress => theme::status_syncing(),
        StatusKind::NeedsAction => theme::status_warning(),
        StatusKind::Idle => theme::normal_text(),
    };

    let email_str = state.account_email.as_deref().unwrap_or("--");
    let region_str = state.account_location.as_deref().unwrap_or("--");
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

    let status_msg_span = if let Some((ref msg, ref kind)) = state.status_message {
        let style = match kind {
            crate::tui::state::StatusMessageKind::Success => theme::success_text(),
            crate::tui::state::StatusMessageKind::Error => theme::error_text(),
        };
        Span::styled(format!("  {}", msg), style)
    } else {
        Span::raw("")
    };

    let status_line = Line::from(vec![
        label_span("Status:"),
        Span::styled(status_icon(state.status.status), status_style),
        Span::raw(" "),
        Span::styled(&state.status.status_str, status_style),
        status_msg_span,
    ]);

    let account_line = Line::from(vec![
        label_span("Account:"),
        Span::styled("\u{1F464}", theme::normal_text()),
        Span::raw(" "),
        Span::styled(email_str, theme::normal_text()),
    ]);

    let region_line = Line::from(vec![
        label_span("Data region:"),
        Span::styled(region_str, theme::normal_text()),
    ]);

    let storage_line = Line::from(vec![
        label_span("Storage:"),
        Span::styled("\u{1F4BE}", theme::normal_text()),
        Span::raw(" "),
        Span::styled(storage_str, theme::normal_text()),
    ]);

    let (mount_icon, mount_icon_style, mount_text) = if state.fs_mounted {
        let mp = state.mountpoint.as_deref().unwrap_or("unknown");
        (
            "\u{2705}",
            theme::success_text(),
            format!("Mounted at {}", mp),
        )
    } else {
        (
            "\u{2298}",
            theme::status_warning(),
            "Not mounted".to_string(),
        )
    };

    let mount_line = Line::from(vec![
        label_span("Mount:"),
        Span::styled(mount_icon, mount_icon_style),
        Span::raw(" "),
        Span::styled(mount_text, theme::normal_text()),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" pCloud ", theme::title_style()))
        .title_alignment(Alignment::Left)
        .border_style(theme::focused_border());

    let paragraph = Paragraph::new(vec![
        status_line,
        account_line,
        region_line,
        storage_line,
        mount_line,
    ])
    .block(block);
    frame.render_widget(paragraph, area);
}

fn label_span(label: &str) -> Span<'static> {
    util::label_span(label, LABEL_WIDTH)
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
