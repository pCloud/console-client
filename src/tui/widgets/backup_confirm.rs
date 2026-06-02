use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::state::{InputMode, TuiState};
use crate::tui::theme;

/// Confirmation modal for the destructive backup actions: removing the
/// selected backup, or stopping all backups on this device.
pub fn render(frame: &mut Frame, state: &TuiState) {
    let (title, lines) = match state.input_mode {
        InputMode::BackupRemoveConfirm => {
            let detail = match state.selected_backup() {
                Some(b) => format!("  Backup {} - {}", b.sync_id, b.local_path.display()),
                None => "  (no backup selected)".to_string(),
            };
            (
                " Remove Backup ",
                vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Stop backing up this folder?",
                        theme::normal_text(),
                    )),
                    Line::from(Span::styled(detail, theme::muted_text())),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Existing backed-up files are kept.",
                        theme::muted_text(),
                    )),
                    Line::from(""),
                    confirm_prompt(),
                ],
            )
        }
        InputMode::BackupStopDeviceConfirm => (
            " Stop Device Backups ",
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Stop ALL backups on this device?",
                    theme::error_text(),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Every backup for this device will be stopped.",
                    theme::normal_text(),
                )),
                Line::from(""),
                confirm_prompt(),
            ],
        ),
        _ => return,
    };

    let height = lines.len() as u16 + 2;
    let area = centered_rect(55, height, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, theme::error_text()))
        .border_style(theme::status_error());

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn confirm_prompt() -> Line<'static> {
    Line::from(vec![
        Span::styled("  Continue? (", theme::muted_text()),
        Span::styled("y", theme::key_hint_style()),
        Span::styled("/", theme::muted_text()),
        Span::styled("N", theme::key_hint_style()),
        Span::styled(")", theme::muted_text()),
    ])
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_width = (area.width as u32 * percent_x as u32 / 100).min(area.width as u32) as u16;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    Rect::new(
        area.x + x,
        area.y + y,
        popup_width.min(area.width),
        height.min(area.height),
    )
}
