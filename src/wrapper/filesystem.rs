//! Filesystem operations for PCloudClient.
//!
//! This module provides FUSE filesystem mounting and unmounting operations,
//! as well as sync folder management for the pCloud client.
//!
//! # FUSE Filesystem
//!
//! The pCloud client provides a FUSE-based virtual filesystem that allows
//! accessing pCloud files as if they were local files. The filesystem must
//! be mounted to a directory before use.
//!
//! # Example
//!
//! ```ignore
//! use console_client::wrapper::PCloudClient;
//!
//! let client = PCloudClient::init()?;
//! let mut guard = client.lock().unwrap();
//!
//! // Mount the filesystem
//! guard.mount_filesystem("/home/user/pCloud")?;
//!
//! // Check if mounted
//! if guard.is_fs_mounted() {
//!     println!("Mounted at: {:?}", guard.mountpoint());
//! }
//!
//! // Unmount when done
//! guard.unmount_filesystem()?;
//! ```
//!
//! # Sync Folders
//!
//! Sync folders allow bidirectional or one-way synchronization between
//! local folders and remote pCloud folders.
//!
//! ```ignore
//! use console_client::wrapper::{PCloudClient, SyncType};
//!
//! let client = PCloudClient::init()?;
//! let mut guard = client.lock().unwrap();
//!
//! // Add a full sync folder
//! let sync_id = guard.add_sync_by_path(
//!     "/home/user/Documents",
//!     "/Documents",
//!     SyncType::Full
//! )?;
//!
//! // List all syncs
//! let syncs = guard.list_syncs()?;
//! for sync in syncs {
//!     println!("Sync {}: {} <-> {}", sync.id, sync.local_path.display(), sync.remote_path);
//! }
//!
//! // Remove a sync
//! guard.remove_sync(sync_id)?;
//! ```

use std::path::{Path, PathBuf};

use crate::error::{FilesystemError, PCloudError, Result};
use crate::ffi::raw;
use crate::ffi::types::{
    psync_folder_list_t, psync_syncid_t, psync_synctype_t, PSYNC_DOWNLOAD_ONLY, PSYNC_FULL,
    PSYNC_UPLOAD_ONLY,
};
use crate::utils::cstring::{from_cstr_and_free, try_to_cstring};

use super::client::PCloudClient;

/// Type of synchronization for a sync folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncType {
    /// Only download files from pCloud to local folder
    DownloadOnly,
    /// Only upload files from local folder to pCloud
    UploadOnly,
    /// Full bidirectional synchronization
    Full,
}

impl SyncType {
    /// Convert to the C library's sync type constant.
    pub fn to_raw(self) -> psync_synctype_t {
        match self {
            SyncType::DownloadOnly => PSYNC_DOWNLOAD_ONLY,
            SyncType::UploadOnly => PSYNC_UPLOAD_ONLY,
            SyncType::Full => PSYNC_FULL,
        }
    }

    /// Create from the C library's sync type constant.
    pub fn from_raw(raw: psync_synctype_t) -> Option<Self> {
        match raw {
            PSYNC_DOWNLOAD_ONLY => Some(SyncType::DownloadOnly),
            PSYNC_UPLOAD_ONLY => Some(SyncType::UploadOnly),
            PSYNC_FULL => Some(SyncType::Full),
            _ => None,
        }
    }
}

impl std::fmt::Display for SyncType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncType::DownloadOnly => write!(f, "download-only"),
            SyncType::UploadOnly => write!(f, "upload-only"),
            SyncType::Full => write!(f, "full"),
        }
    }
}

/// Information about a sync folder.
#[derive(Debug, Clone)]
pub struct SyncFolder {
    /// Unique identifier for this sync
    pub id: u32,
    /// Local folder path
    pub local_path: PathBuf,
    /// Remote folder path (e.g., "/Documents")
    pub remote_path: String,
    /// Type of synchronization
    pub sync_type: SyncType,
}

impl PCloudClient {
    // ========================================================================
    // Filesystem Mount Operations
    // ========================================================================

    /// Set the filesystem root path without starting the FUSE filesystem.
    ///
    /// This should be called before `start_sync()`, which will mount the
    /// filesystem automatically at the configured path.
    ///
    /// # Arguments
    ///
    /// * `mountpoint` - Path where the filesystem will be mounted.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not valid UTF-8.
    pub fn set_fs_root(&mut self, mountpoint: impl AsRef<Path>) -> Result<()> {
        let path = mountpoint.as_ref();
        let path_str = path
            .to_str()
            .ok_or(PCloudError::Filesystem(FilesystemError::InvalidPath))?;
        let c_path = try_to_cstring(path_str)?;
        let key = try_to_cstring("fsroot")?;

        // Safety: psync_set_string_value is safe to call with valid C strings.
        // The C library makes its own copy of the value.
        unsafe {
            raw::psync_set_string_value(key.as_ptr(), c_path.as_ptr());
        }

        Ok(())
    }

    /// Mount the pCloud filesystem at the specified mountpoint.
    ///
    /// This starts the FUSE virtual filesystem, allowing pCloud files to be
    /// accessed as local files at the given path.
    ///
    /// # Arguments
    ///
    /// * `mountpoint` - Path where the filesystem should be mounted.
    ///                  Must be an existing directory.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The filesystem is already mounted (`FilesystemError::AlreadyMounted`)
    /// - The mountpoint doesn't exist (`FilesystemError::MountpointNotFound`)
    /// - The mountpoint is not a directory (`FilesystemError::NotADirectory`)
    /// - The path is not valid UTF-8 (`FilesystemError::InvalidPath`)
    /// - The FUSE mount fails (`FilesystemError::MountFailed`)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Create mountpoint directory first
    /// std::fs::create_dir_all("/home/user/pCloud")?;
    ///
    /// // Mount the filesystem
    /// client.mount_filesystem("/home/user/pCloud")?;
    /// ```
    ///
    /// # Note
    ///
    /// The C library must be initialized and authenticated before mounting.
    pub fn mount_filesystem(&mut self, mountpoint: impl AsRef<Path>) -> Result<()> {
        let path = mountpoint.as_ref();

        // Check if already mounted (from our cached state)
        if self.fs_mounted {
            return Err(PCloudError::Filesystem(FilesystemError::AlreadyMounted));
        }

        // Also check the C library's state
        if self.is_fs_mounted() {
            // Update our cached state and return error
            self.refresh_mount_state();
            return Err(PCloudError::Filesystem(FilesystemError::AlreadyMounted));
        }

        // Validate mountpoint exists
        if !path.exists() {
            return Err(PCloudError::Filesystem(
                FilesystemError::MountpointNotFound(path.to_path_buf()),
            ));
        }

        // Validate mountpoint is a directory
        if !path.is_dir() {
            return Err(PCloudError::Filesystem(FilesystemError::NotADirectory(
                path.to_path_buf(),
            )));
        }

        // Convert path to C string
        let path_str = path
            .to_str()
            .ok_or(PCloudError::Filesystem(FilesystemError::InvalidPath))?;
        let c_path = try_to_cstring(path_str)?;

        // Set the filesystem root in the C library
        // Safety: psync_set_string_value is safe to call with valid C strings
        // The C library makes its own copy of the value
        let key = try_to_cstring("fsroot")?;
        unsafe {
            raw::psync_set_string_value(key.as_ptr(), c_path.as_ptr());
        }

        // Start the filesystem
        // Safety: psync_fs_start is safe to call after initialization
        let result = unsafe { raw::psync_fs_start() };

        if result != 0 {
            return Err(PCloudError::Filesystem(FilesystemError::MountFailed(
                result,
            )));
        }

        // Update cached state
        self.fs_mounted = true;
        self.mountpoint = Some(path.to_path_buf());

        Ok(())
    }

    /// Unmount the pCloud filesystem.
    ///
    /// This stops the FUSE virtual filesystem. Any file operations in progress
    /// will be completed or aborted.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The filesystem is not mounted (`FilesystemError::NotMounted`)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Unmount when done
    /// client.unmount_filesystem()?;
    /// ```
    ///
    /// # Note
    ///
    /// This is automatically called when the `PCloudClient` is dropped,
    /// so explicit unmounting is only needed for controlled shutdown.
    pub fn unmount_filesystem(&mut self) -> Result<()> {
        // Check if mounted (from our cached state)
        if !self.fs_mounted && !self.is_fs_mounted() {
            return Err(PCloudError::Filesystem(FilesystemError::NotMounted));
        }

        // Stop the filesystem
        // Safety: psync_fs_stop is safe to call if filesystem was started
        unsafe {
            raw::psync_fs_stop();
        }

        // Update cached state
        self.fs_mounted = false;
        self.mountpoint = None;

        Ok(())
    }

    /// Check if the filesystem is currently mounted.
    ///
    /// This queries the C library directly for the current state,
    /// bypassing the cached state.
    ///
    /// # Returns
    ///
    /// `true` if the FUSE filesystem is mounted, `false` otherwise.
    pub fn is_fs_mounted(&self) -> bool {
        // Safety: psync_fs_isstarted is safe to call anytime
        unsafe { raw::psync_fs_isstarted() == 1 }
    }

    /// Get the current filesystem mountpoint from the C library.
    ///
    /// This queries the C library directly for the current mountpoint,
    /// which may differ from the cached state if the filesystem was
    /// mounted/unmounted externally.
    ///
    /// # Returns
    ///
    /// - `Some(PathBuf)` with the mountpoint path if mounted
    /// - `None` if not mounted or on error
    pub fn get_fs_mountpoint(&self) -> Option<PathBuf> {
        // Safety: psync_fs_getmountpoint returns a string that must be freed
        let ptr = unsafe { raw::psync_fs_getmountpoint() };

        if ptr.is_null() {
            return None;
        }

        // Safety: from_cstr_and_free handles null check and frees memory
        unsafe { from_cstr_and_free(ptr, |p| raw::psync_free(p)) }.map(PathBuf::from)
    }

    /// Refresh filesystem state from the C library.
    ///
    /// Updates the cached `fs_mounted` and `mountpoint` fields to match
    /// the C library's actual state. This is useful if the filesystem
    /// state may have changed outside of Rust (e.g., via callbacks).
    pub fn refresh_fs_state(&mut self) {
        self.fs_mounted = self.is_fs_mounted();

        if self.fs_mounted {
            self.mountpoint = self.get_fs_mountpoint();
        } else {
            self.mountpoint = None;
        }
    }

    /// Get the full filesystem path for a remote folder ID.
    ///
    /// This returns the full local path (including mountpoint) for a
    /// remote folder, allowing direct file system access to pCloud folders.
    ///
    /// # Arguments
    ///
    /// * `folder_id` - The remote folder ID (0 for root)
    ///
    /// # Returns
    ///
    /// - `Some(PathBuf)` with the full path if the filesystem is mounted
    /// - `None` if not mounted or the folder doesn't exist
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Get path to root folder
    /// if let Some(path) = client.get_fs_path_by_folder_id(0) {
    ///     println!("Root folder is at: {}", path.display());
    /// }
    /// ```
    pub fn get_fs_path_by_folder_id(&self, folder_id: u64) -> Option<PathBuf> {
        // Safety: psync_fs_get_path_by_folderid returns a string that must be freed
        let ptr = unsafe { raw::psync_fs_get_path_by_folderid(folder_id) };

        if ptr.is_null() {
            return None;
        }

        // Safety: from_cstr_and_free handles null check and frees memory
        unsafe { from_cstr_and_free(ptr, |p| raw::psync_free(p)) }.map(PathBuf::from)
    }

    // ========================================================================
    // Sync Folder Management
    // ========================================================================

    /// Add a sync folder by remote path.
    ///
    /// Creates a synchronization relationship between a local folder and
    /// a remote pCloud folder.
    ///
    /// # Arguments
    ///
    /// * `local_path` - Path to the local folder to sync
    /// * `remote_path` - Remote pCloud path (must start with "/")
    /// * `sync_type` - Type of synchronization
    ///
    /// # Returns
    ///
    /// - `Ok(i64)` with the sync ID on success
    /// - `Err` on failure
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Full sync between local and remote folders
    /// let sync_id = client.add_sync_by_path(
    ///     "/home/user/Documents",
    ///     "/Documents",
    ///     SyncType::Full
    /// )?;
    ///
    /// // Download-only sync
    /// let sync_id = client.add_sync_by_path(
    ///     "/home/user/Downloads",
    ///     "/Shared",
    ///     SyncType::DownloadOnly
    /// )?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Local folder doesn't exist
    /// - Remote folder doesn't exist
    /// - Folder is already being synced
    /// - Parent or subfolder is already syncing
    pub fn add_sync_by_path(
        &mut self,
        local_path: impl AsRef<Path>,
        remote_path: &str,
        sync_type: SyncType,
    ) -> Result<u32> {
        let local = local_path.as_ref();

        // Validate local path exists
        if !local.exists() {
            return Err(PCloudError::Filesystem(
                FilesystemError::LocalFolderNotFound(local.display().to_string()),
            ));
        }

        let local_str = local
            .to_str()
            .ok_or(PCloudError::Filesystem(FilesystemError::InvalidPath))?;
        let c_local = try_to_cstring(local_str)?;
        let c_remote = try_to_cstring(remote_path)?;

        // Safety: psync_add_sync_by_path is safe with valid C strings
        let result = unsafe {
            raw::psync_add_sync_by_path(c_local.as_ptr(), c_remote.as_ptr(), sync_type.to_raw())
        };

        // -1 in C becomes u32::MAX when returned as unsigned
        if result == u32::MAX {
            // Get the specific error
            let error_code = self.get_last_error();
            return Err(PCloudError::Filesystem(FilesystemError::from_code(
                error_code,
            )));
        }

        Ok(result)
    }

    /// Add a sync folder by remote folder ID.
    ///
    /// Creates a synchronization relationship between a local folder and
    /// a remote pCloud folder identified by its ID.
    ///
    /// # Arguments
    ///
    /// * `local_path` - Path to the local folder to sync
    /// * `folder_id` - Remote folder ID (0 for root)
    /// * `sync_type` - Type of synchronization
    ///
    /// # Returns
    ///
    /// - `Ok(i64)` with the sync ID on success
    /// - `Err` on failure
    pub fn add_sync_by_folder_id(
        &mut self,
        local_path: impl AsRef<Path>,
        folder_id: u64,
        sync_type: SyncType,
    ) -> Result<u32> {
        let local = local_path.as_ref();

        // Validate local path exists
        if !local.exists() {
            return Err(PCloudError::Filesystem(
                FilesystemError::LocalFolderNotFound(local.display().to_string()),
            ));
        }

        let local_str = local
            .to_str()
            .ok_or(PCloudError::Filesystem(FilesystemError::InvalidPath))?;
        let c_local = try_to_cstring(local_str)?;

        // Safety: psync_add_sync_by_folderid is safe with valid C strings
        let result = unsafe {
            raw::psync_add_sync_by_folderid(c_local.as_ptr(), folder_id, sync_type.to_raw())
        };

        // -1 in C becomes u32::MAX when returned as unsigned
        if result == u32::MAX {
            let error_code = self.get_last_error();
            return Err(PCloudError::Filesystem(FilesystemError::from_code(
                error_code,
            )));
        }

        Ok(result)
    }

    /// Add a sync folder with delayed creation.
    ///
    /// This can be called right after `init()`, even before login.
    /// The sync will be created when the user logs in and state is downloaded.
    /// If the remote path doesn't exist, it will be created if possible.
    ///
    /// # Arguments
    ///
    /// * `local_path` - Path to the local folder to sync
    /// * `remote_path` - Remote pCloud path (will be created if needed)
    /// * `sync_type` - Type of synchronization
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err` on failure (only if local path is invalid)
    pub fn add_sync_delayed(
        &mut self,
        local_path: impl AsRef<Path>,
        remote_path: &str,
        sync_type: SyncType,
    ) -> Result<()> {
        let local = local_path.as_ref();

        let local_str = local
            .to_str()
            .ok_or(PCloudError::Filesystem(FilesystemError::InvalidPath))?;
        let c_local = try_to_cstring(local_str)?;
        let c_remote = try_to_cstring(remote_path)?;

        // Safety: psync_add_sync_by_path_delayed is safe with valid C strings
        let result = unsafe {
            raw::psync_add_sync_by_path_delayed(
                c_local.as_ptr(),
                c_remote.as_ptr(),
                sync_type.to_raw(),
            )
        };

        if result == -1 {
            return Err(PCloudError::Filesystem(
                FilesystemError::LocalFolderNotFound(local.display().to_string()),
            ));
        }

        Ok(())
    }

    /// Change the sync type for an existing sync.
    ///
    /// # Arguments
    ///
    /// * `sync_id` - ID of the sync to modify
    /// * `sync_type` - New synchronization type
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(FilesystemError::InvalidSyncId)` if the sync ID is invalid
    pub fn change_sync_type(&mut self, sync_id: u32, sync_type: SyncType) -> Result<()> {
        // Safety: psync_change_synctype is safe with valid sync_id
        let result = unsafe { raw::psync_change_synctype(sync_id, sync_type.to_raw()) };

        if result == -1 {
            return Err(PCloudError::Filesystem(FilesystemError::InvalidSyncId));
        }

        Ok(())
    }

    /// Remove a sync folder.
    ///
    /// This removes the synchronization relationship but does NOT delete
    /// any files or folders on either side.
    ///
    /// # Arguments
    ///
    /// * `sync_id` - ID of the sync to remove
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(FilesystemError::InvalidSyncId)` if the sync ID is invalid
    pub fn remove_sync(&mut self, sync_id: u32) -> Result<()> {
        // Safety: psync_delete_sync is safe with valid sync_id
        let result = unsafe { raw::psync_delete_sync(sync_id) };

        if result == -1 {
            return Err(PCloudError::Filesystem(FilesystemError::InvalidSyncId));
        }

        Ok(())
    }

    /// List all sync folders.
    ///
    /// # Returns
    ///
    /// A vector of `SyncFolder` structs describing each active sync.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let syncs = client.list_syncs()?;
    /// for sync in syncs {
    ///     println!(
    ///         "Sync {}: {} <-> {} ({})",
    ///         sync.id,
    ///         sync.local_path.display(),
    ///         sync.remote_path,
    ///         sync.sync_type
    ///     );
    /// }
    /// ```
    pub fn list_syncs(&self) -> Result<Vec<SyncFolder>> {
        // Safety: psync_get_sync_list returns a pointer that must be freed
        let list_ptr = unsafe { raw::psync_get_sync_list() };

        if list_ptr.is_null() {
            return Ok(Vec::new());
        }

        // Safety: We checked for null, and the list is valid
        let list: &psync_folder_list_t = unsafe { &*list_ptr };
        let mut syncs = Vec::with_capacity(list.foldercnt as usize);

        // Read each folder from the list
        for i in 0..list.foldercnt {
            // Safety: folders array has foldercnt elements
            let folder = unsafe { &*list.folders.as_ptr().add(i as usize) };

            // Get local path
            let local_path = if !folder.localpath.is_null() {
                unsafe {
                    std::ffi::CStr::from_ptr(folder.localpath)
                        .to_str()
                        .ok()
                        .map(PathBuf::from)
                }
            } else {
                None
            };

            // Get remote path
            let remote_path = if !folder.remotepath.is_null() {
                unsafe {
                    std::ffi::CStr::from_ptr(folder.remotepath)
                        .to_str()
                        .ok()
                        .map(String::from)
                }
            } else {
                None
            };

            // Only add if we have both paths
            if let (Some(local), Some(remote)) = (local_path, remote_path) {
                syncs.push(SyncFolder {
                    id: folder.syncid,
                    local_path: local,
                    remote_path: remote,
                    sync_type: SyncType::from_raw(folder.synctype).unwrap_or(SyncType::Full),
                });
            }
        }

        // Free the list
        // Safety: list_ptr was allocated by the C library
        unsafe {
            raw::psync_free(list_ptr as *mut std::ffi::c_void);
        }

        Ok(syncs)
    }

    /// Create a remote folder by path.
    ///
    /// Creates a new folder in pCloud at the specified path.
    ///
    /// # Arguments
    ///
    /// * `path` - Full remote path to create (e.g., "/Documents/NewFolder")
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err` on failure
    pub fn create_remote_folder(&mut self, path: &str) -> Result<()> {
        let c_path = try_to_cstring(path)?;
        let mut err_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();

        // Safety: psync_create_remote_folder_by_path is safe with valid C strings
        let result =
            unsafe { raw::psync_create_remote_folder_by_path(c_path.as_ptr(), &mut err_ptr) };

        if result == 0 {
            Ok(())
        } else {
            // Get error message if available
            let error_msg = if !err_ptr.is_null() {
                unsafe { from_cstr_and_free(err_ptr, |p| raw::psync_free(p)) }
                    .unwrap_or_else(|| format!("Error code: {}", result))
            } else {
                format!("Error code: {}", result)
            };

            Err(PCloudError::Filesystem(FilesystemError::MountPoint(
                error_msg,
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_type_conversion() {
        assert_eq!(SyncType::DownloadOnly.to_raw(), PSYNC_DOWNLOAD_ONLY);
        assert_eq!(SyncType::UploadOnly.to_raw(), PSYNC_UPLOAD_ONLY);
        assert_eq!(SyncType::Full.to_raw(), PSYNC_FULL);

        assert_eq!(
            SyncType::from_raw(PSYNC_DOWNLOAD_ONLY),
            Some(SyncType::DownloadOnly)
        );
        assert_eq!(
            SyncType::from_raw(PSYNC_UPLOAD_ONLY),
            Some(SyncType::UploadOnly)
        );
        assert_eq!(SyncType::from_raw(PSYNC_FULL), Some(SyncType::Full));
        assert_eq!(SyncType::from_raw(999), None);
    }

    #[test]
    fn test_sync_type_display() {
        assert_eq!(format!("{}", SyncType::DownloadOnly), "download-only");
        assert_eq!(format!("{}", SyncType::UploadOnly), "upload-only");
        assert_eq!(format!("{}", SyncType::Full), "full");
    }

    #[test]
    fn test_sync_folder_debug() {
        let sync = SyncFolder {
            id: 1,
            local_path: PathBuf::from("/home/user/test"),
            remote_path: "/test".to_string(),
            sync_type: SyncType::Full,
        };
        let debug = format!("{:?}", sync);
        assert!(debug.contains("SyncFolder"));
        assert!(debug.contains("id: 1"));
    }

    #[test]
    fn test_sync_folder_clone() {
        let sync = SyncFolder {
            id: 42,
            local_path: PathBuf::from("/home/user/docs"),
            remote_path: "/Documents".to_string(),
            sync_type: SyncType::DownloadOnly,
        };
        let cloned = sync.clone();
        assert_eq!(cloned.id, 42);
        assert_eq!(cloned.local_path, PathBuf::from("/home/user/docs"));
        assert_eq!(cloned.remote_path, "/Documents");
        assert_eq!(cloned.sync_type, SyncType::DownloadOnly);
    }
}
