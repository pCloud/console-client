use std::time::Instant;

use ratatui::widgets::ListState;

use crate::ffi::types::status_to_string;
use crate::wrapper::{AuthState, BackupInfo, CryptoState};
// Re-exported so existing `crate::tui::state::*` references keep resolving.
pub use crate::wrapper::{StatusSnapshot, SyncEngineState};

/// Top-level screen / tab.
#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    Dashboard,
    Backups,
    Help,
    About,
}

/// Which panel currently has focus.
#[derive(Clone, Debug, PartialEq)]
pub enum Panel {
    Crypto,
    Transfers,
}

impl Panel {
    pub fn next(&self) -> Self {
        match self {
            Panel::Crypto => Panel::Transfers,
            Panel::Transfers => Panel::Crypto,
        }
    }

    pub fn prev(&self) -> Self {
        // Only two panels, so prev mirrors next.
        self.next()
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
    /// Typing the local folder path for a new backup
    BackupAdd,
    /// Confirming removal of the selected backup
    BackupRemoveConfirm,
    /// Confirming stopping ALL backups on this device (destructive)
    BackupStopDeviceConfirm,
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
    pub active_panel: Panel,
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub password_stash: Option<secrecy::SecretString>,
    pub status_message: Option<(String, StatusMessageKind)>,
    pub status_message_at: Option<Instant>,
    pub about_focus: Option<AboutFocus>,
    /// Backups configured for the current device (Backups screen).
    pub backups: Vec<BackupInfo>,
    /// Device backup root folder name (shown on the Backups screen).
    pub backup_root_name: Option<String>,
    /// Selection state for the backups list.
    pub backup_list_state: ListState,
    /// Last error encountered while loading backups, shown in-panel.
    pub backup_error: Option<String>,
    /// Vertical scroll offset for screens with overflow (e.g. QR code).
    pub scroll_offset: u16,
    /// One-shot flag: clear the frame buffer on the next render cycle.
    /// Set when switching between screens that have incompatible layouts
    /// (e.g. auth QR code → dashboard) to prevent stale cell artifacts.
    pub needs_clear: bool,
    /// Set when an IPC call to the daemon fails; cleared on the next success.
    /// Drives the "daemon unavailable" hint and the Ctrl+R restart affordance.
    pub daemon_unavailable: bool,
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
            active_panel: Panel::Crypto,
            should_quit: false,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            password_stash: None,
            about_focus: None,
            backups: Vec::new(),
            backup_root_name: None,
            backup_list_state: ListState::default(),
            backup_error: None,
            status_message: None,
            status_message_at: None,
            scroll_offset: 0,
            needs_clear: false,
            daemon_unavailable: false,
        }
    }

    /// The currently selected backup on the Backups screen, if any.
    pub fn selected_backup(&self) -> Option<&BackupInfo> {
        self.backup_list_state
            .selected()
            .and_then(|i| self.backups.get(i))
    }

    /// Clamp the backups selection to the current list length.
    ///
    /// Keeps a valid selection when the list shrinks/grows, and selects the
    /// first row when the list becomes non-empty but nothing was selected.
    pub fn clamp_backup_selection(&mut self) {
        let len = self.backups.len();
        if len == 0 {
            self.backup_list_state.select(None);
        } else {
            let idx = self.backup_list_state.selected().unwrap_or(0).min(len - 1);
            self.backup_list_state.select(Some(idx));
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
        self.status.sync_engine_state()
    }

    /// Optimistically set the cached status code (and derived label) so the
    /// dashboard reflects a user-triggered action before the next status poll
    /// arrives. The next `StatusFull` snapshot from the daemon overwrites this
    /// with the authoritative value from pclsync.
    pub fn set_status_code(&mut self, code: u32) {
        self.status.status = code;
        self.status.status_str = status_to_string(code).to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::types::{
        PSTATUS_BAD_LOGIN_DATA, PSTATUS_DOWNLOADINGANDUPLOADING, PSTATUS_LOGIN_REQUIRED,
        PSTATUS_PAUSED, PSTATUS_READY, PSTATUS_SCANNING, PSTATUS_STOPPED,
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
