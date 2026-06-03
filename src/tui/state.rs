use std::collections::VecDeque;
use std::time::Instant;

use ratatui::widgets::ListState;

use crate::ffi::types::{
    is_error_status, pstatus_t, status_to_string, PSTATUS_LOGIN_REQUIRED, PSTATUS_PAUSED,
    PSTATUS_STOPPED,
};
use crate::wrapper::{AuthState, CryptoState};

/// High-level state of the sync engine, derived from `pstatus_t.status`.
///
/// Used by the dashboard key handler and the help bar to decide which of
/// pause/resume/stop is meaningful at any given moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncEngineState {
    /// Sync is actively running (downloading, uploading, scanning, ready, ...).
    Running,
    /// `psync_pause()` was called — monitoring continues but transfers are halted.
    Paused,
    /// `psync_stop()` was called — no network or local scans until resumed.
    Stopped,
    /// Engine is unavailable (login required / error state). Chords are inert.
    Inactive,
}

/// Copy of pstatus_t fields that is Clone + Send.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct StatusSnapshot {
    pub status: u32,
    pub status_str: String,
    pub files_to_download: u32,
    pub files_downloading: u32,
    pub files_to_upload: u32,
    pub files_uploading: u32,
    pub download_speed: u32,
    pub upload_speed: u32,
    pub bytes_to_download: u64,
    pub bytes_downloaded: u64,
    pub bytes_to_upload: u64,
    pub bytes_uploaded: u64,
    pub remote_is_full: bool,
    pub local_is_full: bool,
}

impl StatusSnapshot {
    pub fn from_pstatus(s: &pstatus_t) -> Self {
        Self {
            status: s.status,
            status_str: status_to_string(s.status).to_string(),
            files_to_download: s.filestodownload,
            files_downloading: s.filesdownloading,
            files_to_upload: s.filestoupload,
            files_uploading: s.filesuploading,
            download_speed: s.downloadspeed,
            upload_speed: s.uploadspeed,
            bytes_to_download: s.bytestodownload,
            bytes_downloaded: s.bytesdownloaded,
            bytes_to_upload: s.bytestoupload,
            bytes_uploaded: s.bytesuploaded,
            remote_is_full: s.remoteisfull != 0,
            local_is_full: s.localisfull != 0,
        }
    }
}

impl Default for StatusSnapshot {
    fn default() -> Self {
        Self {
            status: PSTATUS_LOGIN_REQUIRED,
            status_str: "Connecting...".to_string(),
            files_to_download: 0,
            files_downloading: 0,
            files_to_upload: 0,
            files_uploading: 0,
            download_speed: 0u32,
            upload_speed: 0u32,
            bytes_to_download: 0,
            bytes_downloaded: 0,
            bytes_to_upload: 0,
            bytes_uploaded: 0,
            remote_is_full: false,
            local_is_full: false,
        }
    }
}

/// A single entry in the activity log.
#[derive(Clone, Debug)]
pub struct ActivityEntry {
    pub timestamp: String,
    pub description: String,
    pub is_error: bool,
}

/// Top-level screen / tab.
#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    Dashboard,
    Help,
    About,
}

/// Which panel currently has focus.
#[derive(Clone, Debug, PartialEq)]
pub enum Panel {
    Crypto,
    Transfers,
    ActivityLog,
}

impl Panel {
    pub fn next(&self) -> Self {
        match self {
            Panel::Crypto => Panel::Transfers,
            Panel::Transfers => Panel::ActivityLog,
            Panel::ActivityLog => Panel::Crypto,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Panel::Crypto => Panel::ActivityLog,
            Panel::Transfers => Panel::Crypto,
            Panel::ActivityLog => Panel::Transfers,
        }
    }
}

/// Which element has focus on the About screen.
#[derive(Clone, Debug, PartialEq)]
pub enum AboutFocus {
    ClientBuild,
    PclsyncBuild,
    LicenseLink,
}

impl AboutFocus {
    pub fn next(&self) -> Self {
        match self {
            AboutFocus::ClientBuild => AboutFocus::PclsyncBuild,
            AboutFocus::PclsyncBuild => AboutFocus::LicenseLink,
            AboutFocus::LicenseLink => AboutFocus::ClientBuild,
        }
    }
}

/// What the user is currently doing input-wise.
#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    /// Normal dashboard navigation
    Normal,
    /// Auth method selection screen (press 1 or 2)
    AuthMenu,
    /// Typing/pasting auth token
    AuthToken,
    /// Waiting for web auth completion (holds the login URL)
    AuthWebWaiting(String),
    /// Collecting password for a crypto operation
    PasswordPrompt(CryptoAction),
    /// Collecting hint after password for crypto setup
    HintPrompt,
    /// Confirming account unlink (destructive)
    UnlinkConfirm,
}

/// Which crypto action we're collecting a password for.
#[derive(Clone, Debug, PartialEq)]
pub enum CryptoAction {
    Unlock,
    Setup,
}

/// Kind of transient status message.
#[derive(Clone, Debug, PartialEq)]
pub enum StatusMessageKind {
    Success,
    Error,
}

/// Maximum number of activity log entries to keep.
const MAX_ACTIVITY_LOG: usize = 100;

/// The full TUI state.
pub struct TuiState {
    pub active_screen: Screen,
    pub status: StatusSnapshot,
    pub auth_state: AuthState,
    pub crypto_state: CryptoState,
    pub fs_mounted: bool,
    pub mountpoint: Option<String>,
    pub account_email: Option<String>,
    pub quota_used: u64,
    pub quota_total: u64,
    pub account_location: Option<String>,
    pub crypto_folder_path: Option<String>,
    pub activity_log: VecDeque<ActivityEntry>,
    pub active_panel: Panel,
    pub should_quit: bool,
    pub log_state: ListState,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub password_stash: Option<secrecy::SecretString>,
    pub status_message: Option<(String, StatusMessageKind)>,
    pub status_message_at: Option<Instant>,
    pub about_focus: Option<AboutFocus>,
    /// Vertical scroll offset for screens with overflow (e.g. QR code).
    pub scroll_offset: u16,
    /// One-shot flag: clear the frame buffer on the next render cycle.
    /// Set when switching between screens that have incompatible layouts
    /// (e.g. auth QR code → dashboard) to prevent stale cell artifacts.
    pub needs_clear: bool,
}

impl TuiState {
    pub fn new() -> Self {
        Self {
            active_screen: Screen::Dashboard,
            status: StatusSnapshot::default(),
            auth_state: AuthState::NotAuthenticated,
            crypto_state: CryptoState::NotSetup,
            fs_mounted: false,
            mountpoint: None,
            account_email: None,
            quota_used: 0,
            quota_total: 0,
            account_location: None,
            crypto_folder_path: None,
            activity_log: VecDeque::new(),
            active_panel: Panel::Crypto,
            should_quit: false,
            log_state: ListState::default(),
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            password_stash: None,
            about_focus: None,
            status_message: None,
            status_message_at: None,
            scroll_offset: 0,
            needs_clear: false,
        }
    }

    /// Add an activity log entry, trimming old ones.
    pub fn push_activity(&mut self, entry: ActivityEntry) {
        self.activity_log.push_back(entry);
        if self.activity_log.len() > MAX_ACTIVITY_LOG {
            self.activity_log.pop_front();
        }
        // Auto-scroll to bottom
        let len = self.activity_log.len();
        if len > 0 {
            self.log_state.select(Some(len - 1));
        }
    }

    /// Set a transient status message that auto-clears after 5s.
    pub fn set_status_message(&mut self, msg: String, kind: StatusMessageKind) {
        self.status_message = Some((msg, kind));
        self.status_message_at = Some(Instant::now());
    }

    /// Clear expired status messages.
    pub fn clear_expired_status_message(&mut self) {
        if let Some(at) = self.status_message_at {
            if at.elapsed() > std::time::Duration::from_secs(5) {
                self.status_message = None;
                self.status_message_at = None;
            }
        }
    }

    /// Map the raw `pstatus_t.status` code to the high-level engine state
    /// that drives the dashboard's pause/resume/stop chords.
    pub fn sync_engine_state(&self) -> SyncEngineState {
        match self.status.status {
            PSTATUS_PAUSED => SyncEngineState::Paused,
            PSTATUS_STOPPED => SyncEngineState::Stopped,
            code if is_error_status(code) => SyncEngineState::Inactive,
            _ => SyncEngineState::Running,
        }
    }

    /// Optimistically set the cached status code (and derived label) so the
    /// dashboard reflects a user-triggered action before the next C-side
    /// status callback arrives. The next `TuiEvent::StatusUpdate` overwrites
    /// this with the authoritative value from pclsync.
    pub fn set_status_code(&mut self, code: u32) {
        self.status.status = code;
        self.status.status_str = status_to_string(code).to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::types::{
        PSTATUS_BAD_LOGIN_DATA, PSTATUS_DOWNLOADINGANDUPLOADING, PSTATUS_READY, PSTATUS_SCANNING,
    };

    #[test]
    fn sync_engine_state_maps_known_codes() {
        let mut state = TuiState::new();

        for (code, expected) in [
            (PSTATUS_READY, SyncEngineState::Running),
            (PSTATUS_DOWNLOADINGANDUPLOADING, SyncEngineState::Running),
            (PSTATUS_SCANNING, SyncEngineState::Running),
            (PSTATUS_PAUSED, SyncEngineState::Paused),
            (PSTATUS_STOPPED, SyncEngineState::Stopped),
            (PSTATUS_LOGIN_REQUIRED, SyncEngineState::Inactive),
            (PSTATUS_BAD_LOGIN_DATA, SyncEngineState::Inactive),
        ] {
            state.set_status_code(code);
            assert_eq!(state.sync_engine_state(), expected, "code {code}");
        }
    }

    #[test]
    fn set_status_code_updates_label() {
        let mut state = TuiState::new();
        state.set_status_code(PSTATUS_PAUSED);
        assert_eq!(state.status.status, PSTATUS_PAUSED);
        assert_eq!(state.status.status_str, "Paused");
    }
}
