use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, LineGauge};
use ratatui::Frame;

use crate::tui::state::{Panel, TuiState};
use crate::tui::theme;
use crate::tui::widgets::header::format_speed;

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

    // Download line
    let dl_ratio = if state.status.bytes_to_download > 0 {
        state.status.bytes_downloaded as f64 / state.status.bytes_to_download as f64
    } else {
        0.0
    };
    let dl_pct = (dl_ratio * 100.0) as u64;
    let dl_files = state.status.files_to_download;
    let dl_speed = format_speed(state.status.download_speed as u64);

    let dl_label = format!(" DL {} files  {}%  {}", dl_files, dl_pct, dl_speed);
    let dl_gauge = LineGauge::default()
        .ratio(dl_ratio.min(1.0))
        .label(dl_label)
        .filled_style(Style::default().fg(ratatui::style::Color::Blue))
        .unfilled_style(Style::default().fg(theme::MUTED));
    frame.render_widget(dl_gauge, rows[0]);

    // Upload line
    let ul_ratio = if state.status.bytes_to_upload > 0 {
        state.status.bytes_uploaded as f64 / state.status.bytes_to_upload as f64
    } else {
        0.0
    };
    let ul_pct = (ul_ratio * 100.0) as u64;
    let ul_files = state.status.files_to_upload;
    let ul_speed = format_speed(state.status.upload_speed as u64);

    let ul_label = format!(" UL {} files  {}%  {}", ul_files, ul_pct, ul_speed);
    let ul_gauge = LineGauge::default()
        .ratio(ul_ratio.min(1.0))
        .label(ul_label)
        .filled_style(Style::default().fg(ratatui::style::Color::Green))
        .unfilled_style(Style::default().fg(theme::MUTED));
    frame.render_widget(ul_gauge, rows[1]);
}
