use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::state::{Panel, TuiState};
use crate::tui::theme;
use crate::wrapper::CryptoState;

pub fn render(frame: &mut Frame, state: &TuiState, area: Rect) {
    let border_style = if state.active_panel == Panel::Crypto {
        theme::focused_border()
    } else {
        theme::unfocused_border()
    };

    let (icon, icon_style, status_text) = match &state.crypto_state {
        CryptoState::NotSetup => ("o", theme::muted_text(), "Not set up".to_string()),
        CryptoState::SetupComplete | CryptoState::Stopped => {
            ("X", theme::error_text(), "Locked".to_string())
        }
        CryptoState::Started => {
            let text = match &state.crypto_folder_path {
                Some(path) => format!("Unlocked - {}", path),
                None => "Unlocked".to_string(),
            };
            ("V", theme::success_text(), text)
        }
        CryptoState::Failed(_) => ("!", theme::error_text(), "Error".to_string()),
    };

    // Build action buttons based on state
    let mut buttons: Vec<Span> = Vec::new();
    match &state.crypto_state {
        CryptoState::Started => {
            buttons.push(Span::styled(" [", theme::muted_text()));
            buttons.push(Span::styled("Ctrl+L", theme::key_hint_style()));
            buttons.push(Span::styled(" Lock] ", theme::muted_text()));
        }
        CryptoState::SetupComplete | CryptoState::Stopped => {
            buttons.push(Span::styled(" [", theme::muted_text()));
            buttons.push(Span::styled("Ctrl+L", theme::key_hint_style()));
            buttons.push(Span::styled(" Unlock] ", theme::muted_text()));
        }
        CryptoState::NotSetup => {
            buttons.push(Span::styled(" [", theme::muted_text()));
            buttons.push(Span::styled("Ctrl+L", theme::key_hint_style()));
            buttons.push(Span::styled(" Setup] ", theme::muted_text()));
        }
        CryptoState::Failed(_) => {
            buttons.push(Span::styled(" [", theme::muted_text()));
            buttons.push(Span::styled("Ctrl+L", theme::key_hint_style()));
            buttons.push(Span::styled(" Setup] ", theme::muted_text()));
        }
    }

    // Calculate padding to right-align buttons
    let status_len = 4 + status_text.len(); // "  X " + status_text
    let buttons_len: usize = buttons.iter().map(|s| s.content.len()).sum();
    let padding = if area.width as usize > status_len + buttons_len + 2 {
        area.width as usize - status_len - buttons_len - 2
    } else {
        1
    };

    let mut spans = vec![
        Span::raw("  "),
        Span::styled(icon, icon_style),
        Span::raw(" "),
        Span::styled(&status_text, theme::normal_text()),
        Span::raw(" ".repeat(padding)),
    ];
    spans.extend(buttons);

    let line = Line::from(spans);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Crypto folder", theme::panel_title()))
        .border_style(border_style);

    let paragraph = Paragraph::new(vec![line]).block(block);
    frame.render_widget(paragraph, area);
}
