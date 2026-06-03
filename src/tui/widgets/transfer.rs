use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, LineGauge, Paragraph};
use ratatui::Frame;

use crate::tui::state::{Panel, TuiState};
use crate::tui::theme;
use crate::tui::widgets::header::{format_bytes, format_speed};
use crate::tui::widgets::util;

// Width of the label column ("Download: ") used to left-align values, matching
// the other dashboard panels.
const LABEL_WIDTH: usize = 13;

pub fn render(frame: &mut Frame, state: &TuiState, area: Rect) {
    let border_style = if state.active_panel == Panel::Transfers {
        theme::focused_border()
    } else {
        theme::unfocused_border()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Transfers ", theme::panel_title()))
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    frame.render_widget(
        Paragraph::new(transfer_line(
            "Download:",
            "\u{2193}",
            Color::Blue,
            state.status.bytes_downloaded,
            state.status.bytes_to_download,
            state.status.files_to_download,
            state.status.download_speed as u64,
        )),
        rows[0],
    );
    frame.render_widget(
        transfer_gauge(
            state.status.bytes_downloaded,
            state.status.bytes_to_download,
            Color::Blue,
        ),
        rows[1],
    );

    frame.render_widget(
        Paragraph::new(transfer_line(
            "Upload:",
            "\u{2191}",
            Color::Green,
            state.status.bytes_uploaded,
            state.status.bytes_to_upload,
            state.status.files_to_upload,
            state.status.upload_speed as u64,
        )),
        rows[2],
    );
    frame.render_widget(
        transfer_gauge(
            state.status.bytes_uploaded,
            state.status.bytes_to_upload,
            Color::Green,
        ),
        rows[3],
    );
}

fn transfer_line(
    label: &str,
    icon: &'static str,
    icon_color: Color,
    bytes_done: u64,
    bytes_total: u64,
    files: u32,
    speed: u64,
) -> Line<'static> {
    let pct = if bytes_total > 0 {
        ((bytes_done as f64 / bytes_total as f64).clamp(0.0, 1.0) * 100.0) as u64
    } else {
        0
    };
    let value = format!("{} file(s)  {}%  {}", files, pct, format_speed(speed));

    Line::from(vec![
        label_span(label),
        Span::styled(icon, Style::default().fg(icon_color)),
        Span::raw(" "),
        Span::styled(value, theme::normal_text()),
    ])
}

fn transfer_gauge(bytes_done: u64, bytes_total: u64, color: Color) -> LineGauge<'static> {
    let ratio = if bytes_total > 0 {
        (bytes_done as f64 / bytes_total as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let label_text = if bytes_total > 0 {
        format!(
            "{} / {} ",
            format_bytes(bytes_done),
            format_bytes(bytes_total)
        )
    } else {
        "— ".to_string()
    };

    LineGauge::default()
        .ratio(ratio)
        .label(Line::from(Span::styled(label_text, theme::muted_text())))
        .filled_style(Style::default().fg(color))
        .unfilled_style(Style::default().fg(theme::MUTED))
}

fn label_span(label: &str) -> Span<'static> {
    util::label_span(label, LABEL_WIDTH)
}
