//! Backup operations for PCloudClient.
//!
//! This module wraps the pclsync backup API (`psync_create_backup`,
//! `psync_delete_backup`, `psync_stop_device`, `get_backup_root_name`,
//! `psync_get_syncs_bytype`) in safe Rust methods on [`PCloudClient`].
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

use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{BackupError, FfiError, PCloudError, Result};
use crate::ffi::raw;
use crate::ffi::types::{psync_folder_list_t, PSYNC_BACKUPS};
use crate::utils::cstring::{from_cstr_and_free, try_to_cstring};

use super::client::PCloudClient;

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

/// Status summary for backups on the current device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupStatusInfo {
    /// Backup device name (from `get_backup_root_name`).
    pub device_name: String,
    /// Backups visible right now (filtered if a sync id was supplied).
    pub backups: Vec<BackupInfo>,
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
                code,
                err_message,
                id.0,
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
            Some(msg) if !msg.is_empty() => Err(PCloudError::Backup(BackupError::Failed(msg))),
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
            let folder = unsafe { &*list.folders.as_ptr().add(i) };
            if folder.synctype != PSYNC_BACKUPS {
                continue;
            }
            let local_path = match unsafe { cstr_to_pathbuf(folder.localpath) } {
                Some(p) => p,
                None => continue,
            };
            let remote_path = unsafe { cstr_to_string(folder.remotepath) }.unwrap_or_default();
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
}
