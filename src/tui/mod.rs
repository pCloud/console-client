use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::cli::Cli;
use crate::daemon::DaemonClient;
use crate::Result;

mod app;
mod state;
mod theme;
mod ui;
mod widgets;

use app::App;

/// Run the TUI dashboard as a pure IPC client of the daemon.
///
/// The TUI owns no pclsync engine: it renders state fetched over IPC and sends
/// actions to the daemon. It assumes a daemon is already reachable on
/// `daemon`'s socket (the caller ensures one is running). The render loop polls
/// the daemon once per `tick_rate`; key-triggered actions refresh immediately.
pub fn run(daemon: DaemonClient, _cli: &Cli) -> Result<()> {
    // Initialize terminal
    let mut terminal = ratatui::init();

    // Set up panic hook for terminal restoration
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        original_hook(info);
    }));

    // Create app state and pull an initial snapshot before the first draw.
    let mut app = App::new(daemon);
    app.tick();

    // Main event loop
    let tick_rate = Duration::from_secs(1);
    let mut last_tick = Instant::now();

    loop {
        // Draw
        terminal
            .draw(|frame| ui::render(frame, &mut app))
            .map_err(crate::error::PCloudError::Io)?;

        // Poll for crossterm events
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout).map_err(crate::error::PCloudError::Io)? {
            if let Event::Key(key) = event::read().map_err(crate::error::PCloudError::Io)? {
                // Only act on key presses: terminals with keyboard-enhancement
                // support also emit Release (and Repeat) events, which would
                // otherwise fire a chord like Ctrl+T twice.
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }

        // Tick
        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
        }

        if app.should_quit() {
            break;
        }
    }

    // Restore terminal
    ratatui::restore();

    Ok(())
}
