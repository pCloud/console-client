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
|-- mod.rs              # Entry point: run(daemon, cli), terminal + event loop
|-- app.rs              # App struct: key handling, IPC actions, tick()/poll_*()
|-- state.rs            # TuiState, enums (Screen, Panel, InputMode)
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
    |-- backups_screen.rs  # Backups tab: device root + selectable backups list
    |-- backup_input.rs    # Modal popup for entering a new backup's folder path
    |-- backup_confirm.rs  # Modal popup for remove / stop-device confirmation
    |-- help_screen.rs  # Static help/shortcuts reference page
    +-- about_screen.rs # Version info and links page
```

### Data Flow

The TUI is a **pure IPC client** of the daemon. It owns no pclsync engine and
registers no C callbacks. All state is fetched from, and all actions are sent
to, the daemon over the Unix socket via `DaemonClient`.

```
1-second tick ──> App.tick() ──> poll_status()   (DaemonCommand::StatusFull)
                                  poll_activity() (DaemonCommand::ActivitySince { cursor })
                                                |
Crossterm key events ─────────────────> App.handle_key()
                                                |  (actions send IPC commands:
                                                |   Pause/Stop/Resume/StartCrypto/
                                                |   SetupCrypto/AuthBeginWeb/SetAuthToken;
                                                |   each follows up with an immediate
                                                |   poll_status() refresh)
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

`App.send()` wraps every `DaemonClient::send_command(...)`. On an IPC failure it
sets `state.daemon_unavailable = true` (surfacing a "Daemon unavailable — press
Ctrl+R to restart" message) and returns `None` so callers degrade gracefully.
Connectivity is re-established automatically on the next successful command.
`activity_cursor` tracks the highest activity sequence id already pulled so
`ActivitySince` only returns new entries.

### Threading Model

- **Main thread**: Owns the terminal, runs the event loop, renders UI, and
  performs all IPC round-trips (`StatusFull` / `ActivitySince` on each tick,
  action commands on key presses). There are no other threads in the TUI.
- **No C callbacks**: The TUI does not register status/event/fs callbacks and
  has no `mpsc` channel. The live event stream lives in the daemon, which feeds
  a bounded `ActivityLog` ring buffer that the TUI polls via `ActivitySince`.
- **Web auth**: Started by sending `DaemonCommand::AuthBeginWeb`; the daemon
  runs the browser-login wait on its own thread. The TUI displays the returned
  URL/QR (`DaemonResponse::AuthWeb`) and detects success by polling `StatusFull`.

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
- `PasswordPrompt`, `HintPrompt`, `UnlinkConfirm`, `BackupAdd`, `BackupRemoveConfirm`, and `BackupStopDeviceConfirm` render as modal overlays on top of the active screen.
- When transitioning from a full-screen auth view back to the dashboard, set `state.needs_clear = true` to wipe stale cell artifacts.

## Screens and Tabs

The UI has four top-level screens, switchable via number keys:

| Key | Screen | Content |
|-----|--------|---------|
| `1` | Dashboard | Live sync status, panels, activity log |
| `2` | Backups | Device backup root + add/remove/stop-device |
| `3` | Help | Keyboard shortcuts, support links |
| `4` | About | Version info, build hashes, license |

The `tab_bar` widget renders the tab selector. The `help_bar` widget adapts its content based on both `InputMode` and `Screen`.

## Keyboard Shortcuts (Current)

### Dashboard (Normal mode)
| Key | Action |
|-----|--------|
| `q` / `Q` | Quit |
| `Ctrl+C` | Quit |
| `1` / `2` / `3` / `4` | Switch screen |
| `Tab` / `Shift+Tab` | Cycle panel focus |
| `Up` / `k` | Scroll activity log up |
| `Down` / `j` | Scroll activity log down |
| `Home` / `g` | Jump to log top |
| `End` / `G` | Jump to log bottom |
| `Ctrl+L` | Crypto action (auto-selects Setup/Unlock/Lock) |
| `Ctrl+P` | Pause / resume sync transfers |
| `Ctrl+T` | Stop / resume sync |
| `Ctrl+U` | Unlink account (shows confirmation) |
| `Ctrl+R` | Restart daemon (when daemon is unavailable) |

### Auth screens
| Key | Action |
|-----|--------|
| `1` | Web login |
| `2` | Token input |
| `Enter` | Submit token |
| `Esc` | Back / cancel |
| `Up/Down` | Scroll QR code view |

### Backups screen
| Key | Action |
|-----|--------|
| `Up` / `Down` (`k`/`j`) | Select a backup |
| `a` | Add a folder to back up (path input modal) |
| `d` / `Delete` | Remove the selected backup (confirm) |
| `S` | Stop all backups on this device (confirm) |
| `r` | Refresh the backups list |

### Modal prompts
| Key | Action |
|-----|--------|
| `Enter` | Submit password/hint/backup path |
| `Esc` | Cancel |
| `y` / `N` | Confirm/cancel unlink, backup remove, stop-device |

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

## Daemon (IPC) Integration

The TUI does **not** touch the pclsync C library directly. It owns no engine,
registers no callbacks, and makes no FFI calls. All interaction goes through the
daemon over the Unix socket via `DaemonClient::send_command(...)`.

1. **State polling** (`App.tick()`, once per second):
   - `DaemonCommand::StatusFull` -> `DaemonResponse::StatusFull(Box<DashboardSnapshot>)`.
     `apply_snapshot()` copies status, auth/crypto state, mount state, account
     email/quota/location, and crypto folder path into `TuiState`, and drives
     auth-screen transitions when the engine's login state changes.
   - `DaemonCommand::ActivitySince { cursor }` -> `DaemonResponse::Activity { entries, cursor }`.
     New `ActivityEntry` items (each carries a `seq`) are appended to the log and
     `activity_cursor` advances.

2. **Actions** (from `app.rs` key handlers): `Pause`, `Stop`, `Resume`,
   `StartCrypto`, `SetupCrypto`, `AuthBeginWeb`, `SetAuthToken`, plus the backup
   commands. Each action issues an immediate `poll_status()` so the dashboard
   reflects the result without waiting for the next tick.

3. **Daemon-side ownership**: The daemon registers the real status/event
   callbacks, runs `psync_get_status` on demand, and maintains the `ActivityLog`
   ring buffer. The shared snapshot types (`DashboardSnapshot`, `StatusSnapshot`,
   `SyncEngineState`, `ActivityEntry`) live in `src/wrapper/status.rs` and are
   serialized over the bincode IPC protocol.

### Daemon-unavailable handling

When an IPC command fails, `App.send()` flags `state.daemon_unavailable`. The
dashboard then shows a "Daemon unavailable — press Ctrl+R to restart" message.
`Ctrl+R` (`restart_daemon()`) calls
`daemon::process::spawn_background_daemon(/* allow_unauthenticated = */ true)`
to bring a fresh daemon back up; the next successful poll clears the flag.

## Terminal Lifecycle

```
ratatui::init()          -- enters raw mode, switches to alternate screen
  set_hook(restore)      -- panic hook restores terminal on crash
  loop { draw, poll }    -- main event loop
ratatui::restore()       -- exits raw mode, returns to normal screen
```

If the process crashes without calling `restore()`, the terminal will be left in raw mode. The panic hook handles this for panics, but `SIGKILL` or `abort()` cannot be caught.
