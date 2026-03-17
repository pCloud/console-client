use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, LineGauge};
use ratatui::Frame;

use crate::tui::state::{Panel, TuiState};
use crate::tui::theme;
use crate::tui::widgets::header::format_speed;

fn transfer_gauge(
    bytes_done: u64,
    bytes_total: u64,
    files: u32,
    speed: u64,
    label_prefix: &str,
    color: Color,
) -> LineGauge<'static> {
    let ratio = if bytes_total > 0 {
        (bytes_done as f64 / bytes_total as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let pct = (ratio * 100.0) as u64;
    let label = format!(
        "{} {} file(s)  {}%  {}",
        label_prefix,
        files,
        pct,
        format_speed(speed)
    );

    LineGauge::default()
        .ratio(ratio)
        .label(label)
        .filled_style(Style::default().fg(color))
        .unfilled_style(Style::default().fg(theme::MUTED))
}

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

    if inner.height < 2 {
        return;
    }

    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);

    let dl_gauge = transfer_gauge(
        state.status.bytes_downloaded,
        state.status.bytes_to_download,
        state.status.files_to_download,
        state.status.download_speed as u64,
        " ↓ Downloading",
        Color::Blue,
    );
    frame.render_widget(dl_gauge, rows[0]);

    let ul_gauge = transfer_gauge(
        state.status.bytes_uploaded,
        state.status.bytes_to_upload,
        state.status.files_to_upload,
        state.status.upload_speed as u64,
        " ↑ Uploading  ",
        Color::Green,
    );
    frame.render_widget(ul_gauge, rows[1]);
}
