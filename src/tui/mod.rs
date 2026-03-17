use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event};

use crate::cli::Cli;
use crate::ffi::callbacks::{
    register_event_callback, register_fs_start_callback, register_status_callback,
};
use crate::ffi::types::{
    pstatus_t, psync_eventdata_t, psync_eventtype_t, PEVENT_FILE_DOWNLOAD_FAILED,
    PEVENT_FILE_DOWNLOAD_FINISHED, PEVENT_FILE_DOWNLOAD_STARTED, PEVENT_FILE_UPLOAD_FAILED,
    PEVENT_FILE_UPLOAD_FINISHED, PEVENT_FILE_UPLOAD_STARTED, PEVENT_LOCAL_FILE_DELETED,
    PEVENT_LOCAL_FOLDER_CREATED, PEVENT_LOCAL_FOLDER_DELETED, PEVENT_REMOTE_FILE_DELETED,
    PEVENT_REMOTE_FOLDER_CREATED, PEVENT_REMOTE_FOLDER_DELETED, PEVENT_USERINFO_CHANGED,
    PEVENT_USEDQUOTA_CHANGED, PEVENT_FIRST_SHARE_EVENT,
};
use crate::wrapper::PCloudClient;
use crate::Result;

mod app;
mod event_types;
mod state;
mod theme;
mod ui;
mod widgets;

use app::App;
use event_types::TuiEvent;
use state::StatusSnapshot;

/// Run the TUI dashboard.
///
/// This takes over the terminal, registers callbacks, starts sync,
/// and runs the event loop until the user quits.
pub fn run(client: Arc<Mutex<PCloudClient>>, _cli: &Cli) -> Result<()> {
    // Set up the event channel
    let (tx, rx) = mpsc::channel::<TuiEvent>();

    // Register C library callbacks that send events over the channel
    let status_tx = tx.clone();
    register_status_callback(move |status: &pstatus_t| {
        let snapshot = StatusSnapshot::from_pstatus(status);
        let _ = status_tx.send(TuiEvent::StatusUpdate(snapshot));
    });

    let event_tx = tx.clone();
    register_event_callback(
        move |event_type: psync_eventtype_t, event_data: psync_eventdata_t| {
            if let Some((description, is_error)) = describe_event(event_type, event_data) {
                let _ = event_tx.send(TuiEvent::FileEvent {
                    description,
                    is_error,
                });
            }
        },
    );

    let fs_tx = tx.clone();
    register_fs_start_callback(move || {
        let _ = fs_tx.send(TuiEvent::FsMounted);
    });

    // Start sync -- the C library handles auth + mount internally
    {
        let mut guard = client
            .lock()
            .map_err(|_| crate::error::PCloudError::Config("Lock failed".into()))?;
        guard.start_sync(
            Some(crate::ffi::callbacks::status_callback_trampoline),
            Some(crate::ffi::callbacks::event_callback_trampoline),
        );
    }

    // Initialize terminal
    let mut terminal = ratatui::init();

    // Set up panic hook for terminal restoration
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        original_hook(info);
    }));

    // Create app state
    let mut app = App::new(client.clone());

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
                app.handle_key(key);
            }
        }

        // Drain mpsc channel
        while let Ok(tui_event) = rx.try_recv() {
            app.handle_event(tui_event);
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

/// Convert a C library event into a human-readable description.
///
/// Returns `None` for metadata events (user info, quota changes, shares)
/// that have no file data and should not appear in the activity log.
fn describe_event(
    event_type: psync_eventtype_t,
    event_data: psync_eventdata_t,
) -> Option<(String, bool)> {
    // Metadata events: no file data pointer, skip them
    match event_type {
        PEVENT_USERINFO_CHANGED | PEVENT_USEDQUOTA_CHANGED => return None,
        e if e >= PEVENT_FIRST_SHARE_EVENT => return None,
        _ => {}
    }

    let path = unsafe {
        let file_ptr = event_data.file;
        if !file_ptr.is_null() {
            let local = (*file_ptr).localpath;
            if !local.is_null() {
                let c_str = std::ffi::CStr::from_ptr(local);
                c_str.to_string_lossy().into_owned()
            } else {
                let name = (*file_ptr).name;
                if !name.is_null() {
                    let c_str = std::ffi::CStr::from_ptr(name);
                    c_str.to_string_lossy().into_owned()
                } else {
                    "unknown".to_string()
                }
            }
        } else {
            // Unknown event type with no file data -- skip it
            return None;
        }
    };

    let (prefix, is_error) = match event_type {
        PEVENT_FILE_DOWNLOAD_STARTED => ("Downloading", false),
        PEVENT_FILE_DOWNLOAD_FINISHED => ("Downloaded", false),
        PEVENT_FILE_DOWNLOAD_FAILED => ("Download failed", true),
        PEVENT_FILE_UPLOAD_STARTED => ("Uploading", false),
        PEVENT_FILE_UPLOAD_FINISHED => ("Uploaded", false),
        PEVENT_FILE_UPLOAD_FAILED => ("Upload failed", true),
        PEVENT_LOCAL_FOLDER_CREATED => ("Folder created", false),
        PEVENT_REMOTE_FOLDER_CREATED => ("Remote folder created", false),
        PEVENT_LOCAL_FOLDER_DELETED => ("Folder deleted", false),
        PEVENT_REMOTE_FOLDER_DELETED => ("Remote folder deleted", false),
        PEVENT_LOCAL_FILE_DELETED => ("File deleted", false),
        PEVENT_REMOTE_FILE_DELETED => ("Remote file deleted", false),
        _ => ("Event", false),
    };

    Some((format!("{}: {}", prefix, path), is_error))
}
