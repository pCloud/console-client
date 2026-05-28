//! Backup operations for PCloudClient.
//!
//! This module wraps the pclsync backup API (`psync_create_backup`,
//! `psync_delete_backup`, `psync_stop_device`, `get_backup_root_name`,
//! `psync_get_syncs_bytype`, `psync_register_backup_events_callback`) in
//! safe Rust methods on [`PCloudClient`].
//!
//! Unlike regular sync folders, backups are created via a dedicated
//! `psync_create_backup` entry point and surfaced via
//! `psync_get_syncs_bytype("7")`. They are **not** valid inputs to
//! `psync_add_sync_by_path`, so [`super::SyncType`] is intentionally NOT
//! widened to include a `Backup` variant.
//!
//! # Example
//!
//! ```ignore
//! use console_client::wrapper::{PCloudClient, BackupId};
//! use std::path::Path;
//!
//! let client = PCloudClient::init()?;
//! let mut guard = client.lock().unwrap();
//!
//! let id = guard.create_backup(Path::new("/home/user/Documents"))?;
//! for b in guard.list_backups()? {
//!     println!("[{}] {} -> {}", b.sync_id, b.local_path.display(), b.remote_path);
//! }
//! guard.delete_backup(id)?;
//! ```

use std::collections::VecDeque;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{BackupError, FfiError, PCloudError, Result};
use crate::ffi::callbacks;
use crate::ffi::raw;
use crate::ffi::types::{
    backup_event_to_string, psync_eventdata_t, psync_eventtype_t, psync_folder_list_t,
    PSYNC_BACKUPS,
};
use crate::utils::cstring::{from_cstr_and_free, try_to_cstring};

use super::client::PCloudClient;

/// Maximum number of backup events held in the in-memory ring buffer.
const EVENT_RING_CAP: usize = 32;

/// Strongly typed wrapper around a backup sync id.
///
/// Backups reuse `psync_syncid_t` (a `u32`) but live in a separate code
/// path from regular syncs; the newtype prevents accidental mixing.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
pub struct BackupId(pub u32);

impl std::fmt::Display for BackupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Information about a single backup registered with pCloud.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    /// Sync id of this backup.
    pub sync_id: u32,
    /// Local folder path being backed up.
    pub local_path: PathBuf,
    /// Remote pCloud path the backup is stored under.
    pub remote_path: String,
    /// Remote folder id (0 when unknown).
    pub folder_id: u64,
}

/// A backup-related event reported by pclsync.
///
/// The C event-data union is intentionally not decoded yet — `path` and
/// `sync_id` are best-effort populated only when the trampoline has enough
/// safe context. The `timestamp` is a Unix epoch second when the event was
/// observed by the Rust trampoline, not when the C library generated it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEvent {
    /// Raw event code (see `PEVENT_BACKUP_*` constants).
    pub kind: u32,
    /// Human-readable event kind.
    pub kind_str: String,
    /// Optional path associated with the event.
    pub path: Option<String>,
    /// Optional sync id associated with the event.
    pub sync_id: Option<u32>,
    /// Unix epoch seconds when the trampoline observed the event.
    pub timestamp: u64,
}

/// Status summary for backups on the current device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupStatusInfo {
    /// Backup device name (from `get_backup_root_name`).
    pub device_name: String,
    /// Backups visible right now (filtered if a sync id was supplied).
    pub backups: Vec<BackupInfo>,
    /// Snapshot of the most recent backup events seen by this process.
    pub recent_events: Vec<BackupEvent>,
}

/// In-memory ring buffer of the most recent backup events.
static BACKUP_EVENT_RING: Mutex<VecDeque<BackupEvent>> = Mutex::new(VecDeque::new());

/// Ensure the backup-events callback is registered exactly once per process.
static BACKUP_EVENT_REGISTER: Once = Once::new();

fn push_event(ev: BackupEvent) {
    if let Ok(mut guard) = BACKUP_EVENT_RING.lock() {
        while guard.len() >= EVENT_RING_CAP {
            guard.pop_front();
        }
        guard.push_back(ev);
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Build a [`BackupEvent`] from a raw `pevent_callback_t` invocation.
///
/// We treat the union as opaque on purpose: backup event-data layouts are
/// not all exercised in this codebase yet, and decoding them unsafely from
/// a callback thread is easy to get wrong. Once we have end-to-end tests
/// we can enrich this with `path`/`sync_id` extraction.
fn make_event(kind: psync_eventtype_t, _data: psync_eventdata_t) -> BackupEvent {
    BackupEvent {
        kind,
        kind_str: backup_event_to_string(kind).to_string(),
        path: None,
        sync_id: None,
        timestamp: now_unix_secs(),
    }
}

/// Take a snapshot of the recent backup events ring buffer.
pub fn recent_backup_events() -> Vec<BackupEvent> {
    BACKUP_EVENT_RING
        .lock()
        .map(|guard| guard.iter().cloned().collect())
        .unwrap_or_default()
}

/// Idempotently register the backup-events callback with the C library.
///
/// Safe to call from anywhere; subsequent calls are no-ops thanks to
/// [`std::sync::Once`].
///
/// # Note
///
/// `psync_register_backup_events_callback` is declared in `psynclib.h` but
/// its implementation is currently missing from the upstream pclsync C
/// source. We still install the Rust-side closure into
/// `ffi::callbacks::BACKUP_EVENT_CALLBACK` so the trampoline is wired and
/// ready: if pclsync ever ships the symbol, restoring the FFI call site
/// is a one-line change. Until then `recent_backup_events()` will return
/// an empty snapshot in production.
pub fn ensure_backup_events_registered() {
    BACKUP_EVENT_REGISTER.call_once(|| {
        callbacks::register_backup_event_callback(|kind, data| {
            push_event(make_event(kind, data));
        });
        // TODO: re-enable once pclsync ships psync_register_backup_events_callback
        // (declared at pclsync/psynclib.h:1557 but currently undefined). When
        // restored, the call below installs the trampoline.
        //
        //     unsafe {
        //         raw::psync_register_backup_events_callback(Some(
        //             callbacks::backup_event_callback_trampoline,
        //         ));
        //     }
    });
}

/// SAFETY: helper that copies a C string into Rust if non-null.
unsafe fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok().map(String::from)
}

/// SAFETY: helper that copies a C string into a `PathBuf` if non-null.
unsafe fn cstr_to_pathbuf(ptr: *const c_char) -> Option<PathBuf> {
    cstr_to_string(ptr).map(PathBuf::from)
}

/// SAFETY: helper that takes an `**err` out-parameter populated by pclsync,
/// converts it to an owned `String`, and frees the C memory.
unsafe fn take_err(ptr: *mut c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    from_cstr_and_free(ptr, |p| raw::psync_free(p))
}

impl PCloudClient {
    // ========================================================================
    // Backup CRUD
    // ========================================================================

    /// Create a new backup for the given local path.
    ///
    /// The backup is created synchronously via `psync_create_backup`; the
    /// new sync id is resolved post-hoc by listing backups and matching on
    /// `local_path`, since the C entry point does not provide an out-param.
    pub fn create_backup(&self, local_path: &Path) -> Result<BackupId> {
        let local_str = local_path.to_str().ok_or_else(|| {
            PCloudError::InvalidArgument(format!(
                "Backup path is not valid UTF-8: {}",
                local_path.display()
            ))
        })?;
        let c_path = try_to_cstring(local_str)?;

        let mut err_ptr: *mut c_char = std::ptr::null_mut();
        // Safety:
        // - `c_path.as_ptr()` is a valid null-terminated C string for the
        //   duration of this call; the cast to `*mut c_char` is fine because
        //   the library does not mutate it.
        // - `err_ptr` is a valid out-pointer.
        let code = unsafe {
            raw::psync_create_backup(c_path.as_ptr() as *mut c_char, &mut err_ptr as *mut _)
        };

        let err_message = unsafe { take_err(err_ptr) };

        if code != 0 {
            return Err(PCloudError::Backup(BackupError::from_create_code(
                code,
                err_message,
            )));
        }

        // Resolve the new sync id by listing backups and matching on path.
        let backups = self.list_backups()?;
        let canonical = local_path.canonicalize().ok();
        let id_opt = backups.iter().find(|b| {
            if b.local_path == local_path {
                return true;
            }
            if let Some(ref c) = canonical {
                if b.local_path == *c {
                    return true;
                }
                if let Ok(other) = b.local_path.canonicalize() {
                    return &other == c;
                }
            }
            false
        });

        match id_opt {
            Some(b) => Ok(BackupId(b.sync_id)),
            None => Err(PCloudError::Backup(BackupError::IdResolutionFailed)),
        }
    }

    /// Delete a backup by sync id.
    pub fn delete_backup(&self, id: BackupId) -> Result<()> {
        let mut err_ptr: *mut c_char = std::ptr::null_mut();
        // Safety: id.0 is a u32 sync id; err_ptr is a valid out-pointer.
        let code = unsafe { raw::psync_delete_backup(id.0, &mut err_ptr as *mut _) };
        let err_message = unsafe { take_err(err_ptr) };

        if code != 0 {
            return Err(PCloudError::Backup(BackupError::from_delete_code(
                code, err_message, id.0,
            )));
        }
        Ok(())
    }

    /// Stop all backups on a device.
    ///
    /// `folder_id == None` defaults to `0`, which the C library interprets
    /// as "this device".
    pub fn stop_device(&self, folder_id: Option<u64>) -> Result<()> {
        let folder_id = folder_id.unwrap_or(0);
        let mut err_ptr: *mut c_char = std::ptr::null_mut();
        // Safety: void return; err_ptr is a valid out-pointer.
        unsafe { raw::psync_stop_device(folder_id, &mut err_ptr as *mut _) };
        let err_message = unsafe { take_err(err_ptr) };

        match err_message {
            Some(msg) if !msg.is_empty() => {
                Err(PCloudError::Backup(BackupError::Failed(msg)))
            }
            _ => Ok(()),
        }
    }

    /// List all backups configured for the current device.
    ///
    /// Uses `psync_get_syncs_bytype("7")` (PSYNC_BACKUPS) and defensively
    /// re-filters the returned rows by `synctype` in case the C side ever
    /// returns mixed types.
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        let filter = try_to_cstring("7")?;
        // Safety: `filter` outlives the call; the return is either NULL or a
        // psync_folder_list_t* that must be freed with psync_free.
        let list_ptr = unsafe { raw::psync_get_syncs_bytype(filter.as_ptr()) };

        if list_ptr.is_null() {
            return Ok(Vec::new());
        }

        // Safety: We checked for null.
        let list: &psync_folder_list_t = unsafe { &*list_ptr };
        let mut out = Vec::with_capacity(list.foldercnt as usize);

        for i in 0..list.foldercnt {
            // Safety: folders has foldercnt entries (flexible-array member).
            let folder = unsafe { &*list.folders.as_ptr().add(i as usize) };
            if folder.synctype != PSYNC_BACKUPS {
                continue;
            }
            let local_path = match unsafe { cstr_to_pathbuf(folder.localpath) } {
                Some(p) => p,
                None => continue,
            };
            let remote_path =
                unsafe { cstr_to_string(folder.remotepath) }.unwrap_or_default();
            out.push(BackupInfo {
                sync_id: folder.syncid,
                local_path,
                remote_path,
                folder_id: folder.folderid,
            });
        }

        // Safety: list_ptr was allocated by the C library.
        unsafe {
            raw::psync_free(list_ptr as *mut std::ffi::c_void);
        }

        Ok(out)
    }

    /// Get the backup root folder name for this device.
    pub fn backup_root_name(&self) -> Result<String> {
        // Safety: get_backup_root_name returns either NULL or a psync-allocated
        // string the caller must free.
        let ptr = unsafe { raw::get_backup_root_name() };
        if ptr.is_null() {
            return Err(PCloudError::Ffi(FfiError::NullPointer {
                context: "get_backup_root_name",
            }));
        }
        // Safety: ptr is non-null and psync-owned.
        unsafe { from_cstr_and_free(ptr, |p| raw::psync_free(p)) }.ok_or_else(|| {
            PCloudError::Ffi(FfiError::CError {
                code: -1,
                message: "backup root name was not valid UTF-8".to_string(),
            })
        })
    }

    /// Build a [`BackupStatusInfo`] summary, optionally filtered to one backup.
    pub fn backup_status(&self, id: Option<BackupId>) -> Result<BackupStatusInfo> {
        let mut backups = self.list_backups()?;
        if let Some(BackupId(want)) = id {
            backups.retain(|b| b.sync_id == want);
        }

        // Best-effort device name — fall back to a placeholder so a missing
        // root name doesn't surface as a hard error from `backup status`.
        let device_name = self
            .backup_root_name()
            .unwrap_or_else(|_| "(unknown)".to_string());

        Ok(BackupStatusInfo {
            device_name,
            backups,
            recent_events: recent_backup_events(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_id_display() {
        let id = BackupId(42);
        assert_eq!(id.to_string(), "42");
    }

    #[test]
    fn test_backup_id_serde_roundtrip() {
        let id = BackupId(7);
        let bytes = bincode::serialize(&id).expect("serialize");
        let back: BackupId = bincode::deserialize(&bytes).expect("deserialize");
        assert_eq!(id, back);
    }

    #[test]
    fn test_backup_info_serde_roundtrip() {
        let info = BackupInfo {
            sync_id: 11,
            local_path: PathBuf::from("/home/user/docs"),
            remote_path: "/Backups/host/docs".to_string(),
            folder_id: 99,
        };
        let bytes = bincode::serialize(&info).expect("serialize");
        let back: BackupInfo = bincode::deserialize(&bytes).expect("deserialize");
        assert_eq!(back.sync_id, 11);
        assert_eq!(back.local_path, PathBuf::from("/home/user/docs"));
        assert_eq!(back.remote_path, "/Backups/host/docs");
        assert_eq!(back.folder_id, 99);
    }

    #[test]
    fn test_backup_event_serde_roundtrip() {
        let ev = BackupEvent {
            kind: 401,
            kind_str: "backup-stopped".to_string(),
            path: Some("/x".to_string()),
            sync_id: Some(5),
            timestamp: 1_700_000_000,
        };
        let bytes = bincode::serialize(&ev).expect("serialize");
        let back: BackupEvent = bincode::deserialize(&bytes).expect("deserialize");
        assert_eq!(back.kind, 401);
        assert_eq!(back.kind_str, "backup-stopped");
        assert_eq!(back.path, Some("/x".to_string()));
        assert_eq!(back.sync_id, Some(5));
        assert_eq!(back.timestamp, 1_700_000_000);
    }

    #[test]
    fn test_recent_backup_events_initially_empty_or_buffered() {
        // We can't assert "empty" because other tests in the suite may push
        // events; just verify the call is safe and returns a Vec.
        let _ = recent_backup_events();
    }

    #[test]
    fn test_push_event_caps_ring_buffer() {
        // Drain whatever is there first by overflowing past the cap.
        for i in 0..(EVENT_RING_CAP * 2) {
            push_event(BackupEvent {
                kind: 401,
                kind_str: "backup-stopped".to_string(),
                path: None,
                sync_id: Some(i as u32),
                timestamp: now_unix_secs(),
            });
        }
        let snapshot = recent_backup_events();
        assert!(snapshot.len() <= EVENT_RING_CAP);
    }
}
