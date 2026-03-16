use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::state::{InputMode, Screen, TuiState};
use crate::tui::theme;

pub fn render(frame: &mut Frame, state: &TuiState, area: Rect) {
    let spans = match &state.input_mode {
        InputMode::PasswordPrompt(_) | InputMode::HintPrompt => {
            vec![
                Span::styled(" Enter ", theme::key_hint_style()),
                Span::styled("Submit  ", theme::key_desc_style()),
                Span::styled("Esc ", theme::key_hint_style()),
                Span::styled("Cancel", theme::key_desc_style()),
            ]
        }
        InputMode::AuthMenu => {
            vec![
                Span::styled(" 1 ", theme::key_hint_style()),
                Span::styled("Web Login  ", theme::key_desc_style()),
                Span::styled("2 ", theme::key_hint_style()),
                Span::styled("Auth Token  ", theme::key_desc_style()),
                Span::styled("q ", theme::key_hint_style()),
                Span::styled("Quit", theme::key_desc_style()),
            ]
        }
        InputMode::AuthToken => {
            vec![
                Span::styled(" Enter ", theme::key_hint_style()),
                Span::styled("Submit  ", theme::key_desc_style()),
                Span::styled("Esc ", theme::key_hint_style()),
                Span::styled("Back", theme::key_desc_style()),
            ]
        }
        InputMode::AuthWebWaiting(_) => {
            vec![
                Span::styled(" Esc ", theme::key_hint_style()),
                Span::styled("Cancel  ", theme::key_desc_style()),
                Span::styled("↑↓ ", theme::key_hint_style()),
                Span::styled("Scroll  ", theme::key_desc_style()),
                Span::styled(
                    "Waiting for browser authentication...",
                    theme::muted_text(),
                ),
            ]
        }
        InputMode::UnlinkConfirm => {
            vec![
                Span::styled(" y ", theme::key_hint_style()),
                Span::styled("Confirm  ", theme::key_desc_style()),
                Span::styled("N ", theme::key_hint_style()),
                Span::styled("Cancel  ", theme::key_desc_style()),
                Span::styled("Esc ", theme::key_hint_style()),
                Span::styled("Cancel", theme::key_desc_style()),
            ]
        }
        InputMode::Normal => match state.active_screen {
            Screen::Dashboard => {
                vec![
                    Span::styled(" q ", theme::key_hint_style()),
                    Span::styled("Quit  ", theme::key_desc_style()),
                    Span::styled("1/2/3 ", theme::key_hint_style()),
                    Span::styled("Tabs  ", theme::key_desc_style()),
                    Span::styled("Tab ", theme::key_hint_style()),
                    Span::styled("Switch  ", theme::key_desc_style()),
                    Span::styled("\u{2191}\u{2193} ", theme::key_hint_style()),
                    Span::styled("Scroll  ", theme::key_desc_style()),
                    Span::styled("^L ", theme::key_hint_style()),
                    Span::styled("Crypto  ", theme::key_desc_style()),
                    Span::styled("^U ", theme::key_hint_style()),
                    Span::styled("Unlink", theme::key_desc_style()),
                ]
            }
            Screen::Help | Screen::About => {
                vec![
                    Span::styled(" q ", theme::key_hint_style()),
                    Span::styled("Quit  ", theme::key_desc_style()),
                    Span::styled("1 ", theme::key_hint_style()),
                    Span::styled("Dashboard  ", theme::key_desc_style()),
                    Span::styled("2 ", theme::key_hint_style()),
                    Span::styled("Help  ", theme::key_desc_style()),
                    Span::styled("3 ", theme::key_hint_style()),
                    Span::styled("About", theme::key_desc_style()),
                ]
            }
        },
    };

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
