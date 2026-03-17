use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::theme;

pub fn render(frame: &mut Frame, area: Rect) {
    let shortcuts = [
        ("q", "Quit application"),
        ("1/2/3", "Switch tabs"),
        ("Tab", "Switch panel focus (Dashboard)"),
        ("Up/Down", "Scroll activity log"),
        ("Ctrl+L", "Crypto (auto: Setup/Unlock/Lock)"),
        ("Ctrl+U", "Unlink account"),
    ];

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Support", theme::title_style())),
        Line::from(""),
        Line::from(vec![
            Span::styled("  FAQ               ", theme::muted_text()),
            Span::styled("https://www.pcloud.com/help.html", theme::status_syncing()),
        ]),
        Line::from(vec![
            Span::styled("  Contact           ", theme::muted_text()),
            Span::styled(
                "https://www.pcloud.com/company/contactus.html",
                theme::status_syncing(),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Keyboard Shortcuts", theme::title_style())),
        Line::from(""),
    ];

    for (key, desc) in &shortcuts {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<12}", key), theme::key_hint_style()),
            Span::styled(*desc, theme::normal_text()),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Help ", theme::title_style()))
        .border_style(theme::focused_border());

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
