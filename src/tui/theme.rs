use ratatui::style::{Color, Modifier, Style};

// Base colors
pub const PRIMARY: Color = Color::Cyan;
pub const SUCCESS: Color = Color::Green;
pub const ERROR: Color = Color::Red;
pub const WARNING: Color = Color::Yellow;
pub const MUTED: Color = Color::DarkGray;
pub const TEXT: Color = Color::White;
pub const HIGHLIGHT_BG: Color = Color::DarkGray;

// Styles
pub fn title_style() -> Style {
    Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)
}

pub fn status_ready() -> Style {
    Style::default().fg(SUCCESS).add_modifier(Modifier::BOLD)
}

pub fn status_syncing() -> Style {
    Style::default()
        .fg(Color::Blue)
        .add_modifier(Modifier::BOLD)
}

pub fn status_error() -> Style {
    Style::default().fg(ERROR).add_modifier(Modifier::BOLD)
}

#[allow(dead_code)]
pub fn status_warning() -> Style {
    Style::default().fg(WARNING)
}

pub fn normal_text() -> Style {
    Style::default().fg(TEXT)
}

pub fn muted_text() -> Style {
    Style::default().fg(MUTED)
}

pub fn highlight_style() -> Style {
    Style::default()
        .bg(HIGHLIGHT_BG)
        .add_modifier(Modifier::BOLD)
}

pub fn success_text() -> Style {
    Style::default().fg(SUCCESS)
}

pub fn error_text() -> Style {
    Style::default().fg(ERROR)
}

pub fn key_hint_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

pub fn key_desc_style() -> Style {
    Style::default().fg(MUTED)
}

pub fn focused_border() -> Style {
    Style::default().fg(PRIMARY)
}

pub fn unfocused_border() -> Style {
    Style::default().fg(MUTED)
}

pub fn panel_title() -> Style {
    Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
}
