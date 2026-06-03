use ratatui::text::Span;

use crate::tui::theme;

/// Render a left-padded, muted label span: `"  <label>"` padded so the value
/// column starts at `width + 2` cells. Callers own their column width so
/// panels stay decoupled.
pub fn label_span(label: &str, width: usize) -> Span<'static> {
    Span::styled(
        format!("  {:<width$}", label, width = width),
        theme::muted_text(),
    )
}
