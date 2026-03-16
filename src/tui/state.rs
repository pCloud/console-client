use std::time::Instant;

use ratatui::widgets::ListState;

use crate::ffi::types::{pstatus_t, status_to_string, PSTATUS_LOGIN_REQUIRED};
use crate::wrapper::{AuthState, CryptoState};

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
    Mount,
    Crypto,
    Transfers,
    ActivityLog,
}

impl Panel {
    pub fn next(&self) -> Self {
        match self {
            Panel::Mount => Panel::Crypto,
            Panel::Crypto => Panel::Transfers,
            Panel::Transfers => Panel::ActivityLog,
            Panel::ActivityLog => Panel::Mount,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Panel::Mount => Panel::ActivityLog,
            Panel::Crypto => Panel::Mount,
            Panel::Transfers => Panel::Crypto,
            Panel::ActivityLog => Panel::Transfers,
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
    pub activity_log: Vec<ActivityEntry>,
    pub active_panel: Panel,
    pub should_quit: bool,
    pub log_state: ListState,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub password_stash: Option<secrecy::SecretString>,
    pub status_message: Option<(String, StatusMessageKind)>,
    pub status_message_at: Option<Instant>,
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
            activity_log: Vec::new(),
            active_panel: Panel::ActivityLog,
            should_quit: false,
            log_state: ListState::default(),
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            password_stash: None,
            status_message: None,
            status_message_at: None,
        }
    }

    /// Add an activity log entry, trimming old ones.
    pub fn push_activity(&mut self, entry: ActivityEntry) {
        self.activity_log.push(entry);
        if self.activity_log.len() > MAX_ACTIVITY_LOG {
            self.activity_log.remove(0);
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
}
