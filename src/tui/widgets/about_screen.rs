use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::theme;

pub fn render(frame: &mut Frame, area: Rect) {
    let version = env!("PCLOUD_VERSION");
    let client_commit = option_env!("PCLOUD_GIT_COMMIT_SHORT").unwrap_or("unknown");
    let pclsync_ver = option_env!("PSYNC_LIB_VERSION").unwrap_or("unknown");
    let pclsync_commit = option_env!("PCLSYNC_GIT_COMMIT_SHORT").unwrap_or("unknown");

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  \u{00a9} 2026 pCloud Ltd. All rights reserved.",
            theme::normal_text(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Console Client    ", theme::muted_text()),
            Span::styled(format!("v{}", version), theme::normal_text()),
        ]),
        Line::from(vec![
            Span::styled("  Build             ", theme::muted_text()),
            Span::styled(client_commit, theme::status_syncing()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  pclsync Library   ", theme::muted_text()),
            Span::styled(format!("v{}", pclsync_ver), theme::normal_text()),
        ]),
        Line::from(vec![
            Span::styled("  Build             ", theme::muted_text()),
            Span::styled(pclsync_commit, theme::status_syncing()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Website           ", theme::muted_text()),
            Span::styled("https://www.pcloud.com", theme::status_syncing()),
        ]),
        Line::from(vec![
            Span::styled("  GitHub            ", theme::muted_text()),
            Span::styled(
                "https://github.com/pCloud/console-client",
                theme::status_syncing(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  License           ", theme::muted_text()),
            Span::styled("BSD-3-Clause", theme::normal_text()),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " About \u{2500}\u{2500} pCloud Console Client ",
            theme::title_style(),
        ))
        .border_style(theme::focused_border());

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
