use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::state::{AboutFocus, TuiState};
use crate::tui::theme;

pub fn render(frame: &mut Frame, state: &TuiState, area: Rect) {
    let version = env!("PCLOUD_VERSION");
    let client_commit = option_env!("PCLOUD_GIT_COMMIT_SHORT").unwrap_or("unknown");
    let pclsync_ver = option_env!("PSYNC_LIB_VERSION").unwrap_or("unknown");
    let pclsync_commit = option_env!("PCLSYNC_GIT_COMMIT_SHORT").unwrap_or("unknown");

    let focus = &state.about_focus;

    let client_build_style = focusable_style(focus, &AboutFocus::ClientBuild);
    let pclsync_build_style = focusable_style(focus, &AboutFocus::PclsyncBuild);
    let license_link_style = focusable_style(focus, &AboutFocus::LicenseLink);

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
            Span::styled(client_commit, client_build_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  pclsync Library   ", theme::muted_text()),
            Span::styled(format!("v{}", pclsync_ver), theme::normal_text()),
        ]),
        Line::from(vec![
            Span::styled("  Build             ", theme::muted_text()),
            Span::styled(pclsync_commit, pclsync_build_style),
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
            Span::styled("BSD-3-Clause, details ", theme::normal_text()),
            Span::styled("here", license_link_style),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " About \u{2500}\u{2500} pCloud CLI ",
            theme::title_style(),
        ))
        .border_style(theme::focused_border());

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Return the style for a focusable element: highlighted if focused, underlined link otherwise.
fn focusable_style(
    current_focus: &Option<AboutFocus>,
    target: &AboutFocus,
) -> ratatui::style::Style {
    if current_focus.as_ref() == Some(target) {
        theme::status_syncing().add_modifier(Modifier::UNDERLINED | Modifier::REVERSED)
    } else {
        theme::status_syncing().add_modifier(Modifier::UNDERLINED)
    }
}
