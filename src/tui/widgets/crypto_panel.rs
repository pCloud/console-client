use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::tui::state::{Panel, TuiState};
use crate::tui::theme;
use crate::tui::widgets::util;
use crate::wrapper::CryptoState;

// Width of the label column (e.g. "Status: ") used to left-align the value.
const LABEL_WIDTH: usize = 13;

pub fn render(frame: &mut Frame, state: &TuiState, area: Rect) {
    let border_style = if state.active_panel == Panel::Crypto {
        theme::focused_border()
    } else {
        theme::unfocused_border()
    };

    let (icon, icon_style, status_text) = match &state.crypto_state {
        CryptoState::NotSetup => ("o", theme::muted_text(), "Not set up".to_string()),
        CryptoState::SetupComplete | CryptoState::Stopped => {
            ("\u{1F512}", theme::status_warning(), "Locked".to_string())
        }
        CryptoState::Started => {
            let text = match &state.crypto_folder_path {
                Some(path) => format!("Unlocked - {}", path),
                None => "Unlocked".to_string(),
            };
            ("\u{1F513}", theme::success_text(), text)
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

    // Compute display widths (in terminal cells) so emoji-width icons don't
    // throw off the right-aligned button group.
    let status_len = 2 + LABEL_WIDTH + icon.width() + 1 + status_text.as_str().width();
    let buttons_len: usize = buttons.iter().map(|s| s.content.as_ref().width()).sum();
    let padding = if area.width as usize > status_len + buttons_len + 2 {
        area.width as usize - status_len - buttons_len - 2
    } else {
        1
    };

    let mut spans = vec![
        util::label_span("Status:", LABEL_WIDTH),
        Span::styled(icon, icon_style),
        Span::raw(" "),
        Span::styled(&status_text, theme::normal_text()),
        Span::raw(" ".repeat(padding)),
    ];
    spans.extend(buttons);

    let line = Line::from(spans);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Crypto folder ", theme::panel_title()))
        .border_style(border_style);

    let paragraph = Paragraph::new(vec![line]).block(block);
    frame.render_widget(paragraph, area);
}
