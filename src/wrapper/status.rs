//! Serializable status / activity types shared between the daemon, the IPC
//! layer, and the TUI.
//!
//! These types are the bridge that lets the TUI run as a pure IPC client: the
//! daemon owns the pclsync engine and captures its live status/event stream
//! into these structs, and the TUI renders them after fetching over IPC. They
//! are `Serialize`/`Deserialize` for that reason (same pattern as
//! [`super::backup::BackupInfo`]).

use serde::{Deserialize, Serialize};

use crate::ffi::types::{
    is_error_status, pstatus_t, status_to_string, PSTATUS_LOGIN_REQUIRED, PSTATUS_PAUSED,
    PSTATUS_STOPPED,
};

use super::client::{AuthState, CryptoState};

/// High-level state of the sync engine, derived from `pstatus_t.status`.
///
/// Used by the dashboard key handler and the help bar to decide which of
/// pause/resume/stop is meaningful at any given moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

impl SyncEngineState {
    /// Map a raw `pstatus_t.status` code to the high-level engine state that
    /// drives the dashboard's pause/resume/stop chords.
    pub fn from_status_code(code: u32) -> Self {
        match code {
            PSTATUS_PAUSED => SyncEngineState::Paused,
            PSTATUS_STOPPED => SyncEngineState::Stopped,
            c if is_error_status(c) => SyncEngineState::Inactive,
            _ => SyncEngineState::Running,
        }
    }
}

/// Copy of `pstatus_t` fields that is `Clone + Send + Serialize`.
#[derive(Clone, Debug, Serialize, Deserialize)]
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

    /// The high-level engine state derived from this snapshot's status code.
    pub fn sync_engine_state(&self) -> SyncEngineState {
        SyncEngineState::from_status_code(self.status)
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
///
/// `seq` is a monotonically increasing id assigned by the daemon's activity
/// ring buffer so IPC clients can fetch incrementally (`ActivitySince`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// Monotonic sequence id (0 for locally-generated entries that never went
    /// through the daemon ring buffer).
    pub seq: u64,
    pub timestamp: String,
    pub description: String,
    pub is_error: bool,
}

/// A full snapshot of everything the dashboard renders, fetched in one IPC
/// round-trip (`DaemonCommand::StatusFull`).
///
/// Mirrors exactly the fields the TUI's `tick()` populates so the remote tick
/// can copy them straight into `TuiState`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardSnapshot {
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
}
