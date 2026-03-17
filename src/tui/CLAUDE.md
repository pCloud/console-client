# TUI Module Guide

## Library

The TUI is built with **Ratatui v0.29** (`ratatui = "0.29"` in Cargo.toml).

Ratatui re-exports `crossterm` as `ratatui::crossterm`, so there is no separate `crossterm` dependency. Always import crossterm types through ratatui:

```rust
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
```

### Documentation

- Ratatui docs: https://docs.rs/ratatui/0.29.0/ratatui/
- Ratatui website with tutorials and recipes: https://ratatui.rs
- Widget gallery and examples: https://github.com/ratatui/ratatui/tree/main/examples
- Crossterm (via ratatui re-export): https://docs.rs/crossterm/0.28.1/crossterm/

When asking Claude for help, use the `context7` MCP tool with library ID `ratatui/ratatui` to fetch up-to-date API docs and examples.

### Key Ratatui Concepts

- **Immediate-mode rendering**: Every frame redraws the entire UI. There is no retained widget tree. The `draw()` closure receives a `Frame` and renders widgets into it.
- **Layout**: `Layout::vertical([Constraint::Length(n), Constraint::Fill(1), ...])` splits a `Rect` into chunks. `Length` is fixed rows, `Fill` expands.
- **Widgets**: Stateless structs that implement `Widget`. Call `frame.render_widget(widget, area)`. Stateful widgets use `frame.render_stateful_widget(widget, area, &mut state)`.
- **Spans and Lines**: Text is built from `Span` (styled text fragment) -> `Line` (row of spans) -> `Paragraph` (multi-line block of text).

## Architecture

```
src/tui/
|-- mod.rs              # Entry point: run(), callback registration, event loop
|-- app.rs              # App struct: key handling, crypto/auth operations, tick()
|-- state.rs            # TuiState, StatusSnapshot, enums (Screen, Panel, InputMode)
|-- event_types.rs      # TuiEvent enum (channel messages)
|-- theme.rs            # Color constants and Style factory functions
|-- ui.rs               # Top-level render dispatch (screen routing, overlays)
+-- widgets/            # One file per visual component
    |-- mod.rs          # Module declarations
    |-- tab_bar.rs      # Screen tab selector (1/2/3)
    |-- header.rs       # Status, account, storage bar (+ format_bytes/format_speed)
    |-- mount_panel.rs  # Filesystem mount status
    |-- crypto_panel.rs # Crypto lock/unlock status with action button
    |-- transfer.rs     # Download/upload LineGauge progress bars
    |-- activity_log.rs # Scrollable file event list (stateful List widget)
    |-- help_bar.rs     # Context-sensitive keyboard shortcuts footer
    |-- auth_screen.rs  # Full-screen auth flow (menu, token input, web/QR wait)
    |-- password_input.rs  # Modal popup for crypto password/hint entry
    |-- unlink_confirm.rs  # Modal popup for destructive unlink confirmation
    |-- help_screen.rs  # Static help/shortcuts reference page
    +-- about_screen.rs # Version info and links page
```

### Data Flow

```
C library callbacks ──(mpsc channel)──> mod.rs event loop ──> App.handle_event()
                                                |
Crossterm key events ─────────────────> App.handle_key()
                                                |
1-second timer ───────────────────────> App.tick()  (polls PCloudClient state)
                                                |
                                          TuiState (mutated)
                                                |
                                          ui::render(frame, app)
                                                |
                          ┌─────────────────────┼──────────────────────┐
                    auth screens          dashboard panels        modal overlays
                  (auth_screen.rs)     (header, mount, crypto,  (password_input,
                                       transfer, activity_log)   unlink_confirm)
```

### Threading Model

- **Main thread**: Owns the terminal, runs the event loop, renders UI.
- **C library threads**: Fire callbacks (status, event, fs_start) from internal pclsync threads. These callbacks send `TuiEvent` messages over an `mpsc::channel`. The main loop drains the channel with `try_recv()`.
- **Web auth thread**: A `std::thread::spawn` waits for browser-based login to complete. Auth success is detected via `tick()` polling, not via the channel.

All `PCloudClient` access goes through `Arc<Mutex<PCloudClient>>`. Keep lock scopes minimal to avoid blocking callbacks.

## State Machine

`InputMode` drives both key handling dispatch and UI rendering:

```
                          ┌──────────────────────┐
                          |       Normal         |  (dashboard navigation)
                          └──────┬───────┬───────┘
          (status: needs auth)   |       |   (Ctrl+L)         (Ctrl+U)
                                 v       v                        v
                         ┌──────────┐  ┌──────────────┐   ┌──────────────┐
                         | AuthMenu |  | PasswordPrompt|   | UnlinkConfirm|
                         └──┬───┬──┘  └──────┬───────┘   └──────────────┘
                    (1)     |   | (2)     (Enter, Setup)
                            v   v               v
               ┌────────────┐  ┌──────────┐  ┌──────────┐
               |AuthWebWait |  | AuthToken|  | HintPrompt|
               └────────────┘  └──────────┘  └──────────┘
```

- Auth screens (`AuthMenu`, `AuthToken`, `AuthWebWaiting`) take over the full screen; the dashboard is not rendered.
- `PasswordPrompt`, `HintPrompt`, and `UnlinkConfirm` render as modal overlays on top of the dashboard.
- When transitioning from a full-screen auth view back to the dashboard, set `state.needs_clear = true` to wipe stale cell artifacts.

## Screens and Tabs

The UI has three top-level screens, switchable via number keys:

| Key | Screen | Content |
|-----|--------|---------|
| `1` | Dashboard | Live sync status, panels, activity log |
| `2` | Help | Keyboard shortcuts, support links |
| `3` | About | Version info, build hashes, license |

The `tab_bar` widget renders the tab selector. The `help_bar` widget adapts its content based on both `InputMode` and `Screen`.

## Keyboard Shortcuts (Current)

### Dashboard (Normal mode)
| Key | Action |
|-----|--------|
| `q` / `Q` | Quit |
| `Ctrl+C` | Quit |
| `1` / `2` / `3` | Switch screen |
| `Tab` / `Shift+Tab` | Cycle panel focus |
| `Up` / `k` | Scroll activity log up |
| `Down` / `j` | Scroll activity log down |
| `Home` / `g` | Jump to log top |
| `End` / `G` | Jump to log bottom |
| `Ctrl+L` | Crypto action (auto-selects Setup/Unlock/Lock) |
| `Ctrl+U` | Unlink account (shows confirmation) |

### Auth screens
| Key | Action |
|-----|--------|
| `1` | Web login |
| `2` | Token input |
| `Enter` | Submit token |
| `Esc` | Back / cancel |
| `Up/Down` | Scroll QR code view |

### Modal prompts
| Key | Action |
|-----|--------|
| `Enter` | Submit password/hint |
| `Esc` | Cancel |
| `y` / `N` | Confirm/cancel unlink |

## How to Add a New Widget

1. Create `src/tui/widgets/your_widget.rs`:

```rust
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::tui::state::TuiState;
use crate::tui::theme;

pub fn render(frame: &mut Frame, state: &TuiState, area: Rect) {
    // Build widget using ratatui primitives
    // Use theme::* for consistent styling
    frame.render_widget(your_widget, area);
}
```

2. Register it in `src/tui/widgets/mod.rs`:

```rust
pub mod your_widget;
```

3. Call it from `ui.rs` (in `render()` or `render_dashboard()`), passing the appropriate layout chunk.

### Widget Conventions

- Each widget file exports a single `pub fn render(...)` function.
- Widgets receive `&TuiState` (read-only) or `&mut TuiState` (only `activity_log` needs mutable access for `ListState`).
- Use `theme::focused_border()` / `theme::unfocused_border()` for panels that participate in Tab-cycling.
- Use `theme::key_hint_style()` for shortcut key labels and `theme::key_desc_style()` for their descriptions.
- Modal overlays should render `Clear` first to erase the background, then the popup content. See `password_input.rs` and `unlink_confirm.rs` for the `centered_rect()` helper pattern.

### Available Ratatui Widgets (used in this codebase)

| Widget | Used in | Purpose |
|--------|---------|---------|
| `Paragraph` | Most widgets | Multi-line styled text |
| `Block` | Panels, screens | Borders and titles |
| `List` + `ListState` | `activity_log` | Scrollable, selectable list |
| `LineGauge` | `transfer` | Horizontal progress bar with label |
| `Clear` | `ui`, overlays | Wipe area before redraw |

### Additional Ratatui Widgets (available but not yet used)

These are available from `ratatui::widgets::*` and may be useful for future features:

- `Table` + `TableState` -- tabular data with column headers and row selection
- `BarChart` -- vertical or horizontal bar charts
- `Sparkline` -- compact inline line chart (good for speed history)
- `Gauge` -- filled percentage bar (alternative to `LineGauge`)
- `Tabs` -- styled tab selector (alternative to our manual `tab_bar`)
- `Scrollbar` -- visual scrollbar indicator
- `Canvas` -- freeform drawing (lines, circles, etc.)
- `Chart` -- full line/scatter chart with axes

See the full widget list at https://docs.rs/ratatui/0.29.0/ratatui/widgets/index.html

## Theme System

All colors and styles are centralized in `theme.rs`. Never hardcode colors in widget files.

| Function | Use for |
|----------|---------|
| `title_style()` | Section titles, active tabs |
| `normal_text()` | Default content text |
| `muted_text()` | Labels, secondary info |
| `success_text()` | Success messages |
| `error_text()` | Error messages, warnings |
| `key_hint_style()` | Keyboard shortcut keys (yellow bold) |
| `key_desc_style()` | Shortcut descriptions |
| `status_ready()` | "Ready" status |
| `status_syncing()` | Active sync, links, URLs |
| `status_error()` | Error status |
| `focused_border()` | Active panel border |
| `unfocused_border()` | Inactive panel border |
| `panel_title()` | Panel title text |
| `highlight_style()` | Selected list item |

## Render Artifacts (needs_clear)

Ratatui uses a double-buffer diff to only update changed cells. When switching between layouts with incompatible geometry (e.g., full-screen auth with a QR code to a compact dashboard), leftover cells from the old layout may persist as visual artifacts.

**Fix**: Set `state.needs_clear = true` before the transition. The next `ui::render()` call will render `Clear` over the entire frame area, forcing a full redraw.

Currently applied when:
- Auth screen transitions back to dashboard (on successful auth)
- Web auth waiting completes

## Security Rules

- Passwords go through `state.input_buffer` and are zeroized with `crate::security::zeroize_string()` immediately after use.
- The `password_stash` field uses `secrecy::SecretString` for the crypto setup flow (password must survive across the password -> hint prompt transition).
- Password display uses `"*".repeat(len)` masking in `password_input.rs`.
- Never log, print, or store raw password strings beyond their immediate use scope.

## C Library Integration

The TUI interacts with the pclsync C library through:

1. **Callbacks** (registered in `mod.rs`):
   - `register_status_callback` -- sync status changes (fires from C thread)
   - `register_event_callback` -- file download/upload/delete events (fires from C thread)
   - `register_fs_start_callback` -- filesystem mounted notification

2. **Direct FFI** (in `app.rs`):
   - `raw::psync_get_uint_value` -- reads quota values from the C library's settings DB
   - `PCloudClient` methods via the wrapper layer (auth, crypto, mount state)

3. **Trampoline pattern**: C callbacks invoke `extern "C"` trampoline functions which retrieve stored Rust closures from global `Mutex` storage. The closures send `TuiEvent` messages over the mpsc channel. This decouples C threads from the main render thread.

## Terminal Lifecycle

```
ratatui::init()          -- enters raw mode, switches to alternate screen
  set_hook(restore)      -- panic hook restores terminal on crash
  loop { draw, poll }    -- main event loop
ratatui::restore()       -- exits raw mode, returns to normal screen
```

If the process crashes without calling `restore()`, the terminal will be left in raw mode. The panic hook handles this for panics, but `SIGKILL` or `abort()` cannot be caught.
