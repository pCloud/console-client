//! Mountpoint preparation utilities.
//!
//! Handles stale FUSE mount detection and recovery when preparing
//! a directory for use as a FUSE mountpoint.

use std::path::Path;
use std::process::Command;

use crate::cli::prompt_confirm;
use crate::error::{FilesystemError, PCloudError};

/// Check whether an OS error code indicates a stale FUSE mount.
///
/// A stale mount produces `ENOTCONN` (transport endpoint not connected)
/// or `EIO` (I/O error) when you `stat()` it.
fn is_stale_mount_error(raw_os_error: i32) -> bool {
    raw_os_error == libc::ENOTCONN || raw_os_error == libc::EIO
}

/// Attempt to unmount a stale FUSE mountpoint.
///
/// Uses `fusermount -u` on Linux and `umount` on macOS.
fn attempt_fuse_unmount(path: &Path) -> Result<(), PCloudError> {
    let path_str = path.to_string_lossy();

    #[cfg(target_os = "linux")]
    let output = Command::new("fusermount")
        .args(["-u", &path_str])
        .output()
        .map_err(|e| FilesystemError::MountPoint(format!("Failed to run fusermount: {e}")))?;

    #[cfg(target_os = "macos")]
    let output = Command::new("umount")
        .arg(&*path_str)
        .output()
        .map_err(|e| FilesystemError::MountPoint(format!("Failed to run umount: {e}")))?;

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return Err(PCloudError::NotSupported(
        "Automatic unmount is not supported on this platform".to_string(),
    ));

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FilesystemError::MountPoint(format!(
            "Unmount failed: {}\n\
             Try manually: fusermount -u {} (or: sudo umount -l {})",
            stderr.trim(),
            path_str,
            path_str,
        ))
        .into());
    }

    Ok(())
}

/// Ensure the mountpoint path is ready for use.
///
/// This function handles three cases:
/// 1. **Path doesn't exist** — creates it with `create_dir_all`
/// 2. **Path exists and is a directory** — returns Ok
/// 3. **Path is a stale FUSE mount** (`ENOTCONN`/`EIO`) — prompts the user
///    for permission to unmount, then recreates if needed
///
/// # Safety concern
///
/// The mountpoint could be a live mount from another process. This function
/// never auto-unmounts — it always asks the user first.
pub fn ensure_mountpoint(path: &Path) -> Result<(), PCloudError> {
    match std::fs::metadata(path) {
        Ok(meta) => {
            if !meta.is_dir() {
                return Err(FilesystemError::NotADirectory(path.to_path_buf()).into());
            }
            Ok(())
        }
        Err(e) => {
            if let Some(raw) = e.raw_os_error() {
                if is_stale_mount_error(raw) {
                    return handle_stale_mount(path);
                }
            }

            if e.kind() == std::io::ErrorKind::NotFound {
                std::fs::create_dir_all(path).map_err(PCloudError::Io)?;
                return Ok(());
            }

            Err(PCloudError::Io(e))
        }
    }
}

/// Handle a detected stale FUSE mount by prompting the user.
fn handle_stale_mount(path: &Path) -> Result<(), PCloudError> {
    eprintln!();
    eprintln!(
        "Warning: {} appears to be a stale FUSE mount.",
        path.display()
    );
    eprintln!("A previous pCloud process may have crashed without unmounting.");
    eprintln!();

    let confirmed = prompt_confirm("Attempt to unmount it?")?;
    if !confirmed {
        return Err(FilesystemError::MountPoint(format!(
            "Stale mount at {} must be removed before continuing.\n\
             Run manually: fusermount -u {} (or: sudo umount -l {})",
            path.display(),
            path.display(),
            path.display(),
        ))
        .into());
    }

    attempt_fuse_unmount(path)?;

    // After unmount, the directory may or may not still exist
    if !path.exists() {
        std::fs::create_dir_all(path).map_err(PCloudError::Io)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_existing_directory_passthrough() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("mountpoint");
        fs::create_dir(&dir).unwrap();

        assert!(ensure_mountpoint(&dir).is_ok());
    }

    #[test]
    fn test_missing_directory_creation() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("deep/nested/mountpoint");

        assert!(ensure_mountpoint(&dir).is_ok());
        assert!(dir.is_dir());
    }

    #[test]
    fn test_not_a_directory_error() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("regular_file");
        fs::write(&file, "content").unwrap();

        let err = ensure_mountpoint(&file).unwrap_err();
        assert!(err.to_string().contains("Not a directory"));
    }

    #[test]
    fn test_is_stale_mount_error_enotconn() {
        assert!(is_stale_mount_error(libc::ENOTCONN));
    }

    #[test]
    fn test_is_stale_mount_error_eio() {
        assert!(is_stale_mount_error(libc::EIO));
    }

    #[test]
    fn test_is_stale_mount_error_other() {
        assert!(!is_stale_mount_error(libc::ENOENT));
        assert!(!is_stale_mount_error(libc::EACCES));
        assert!(!is_stale_mount_error(0));
    }
}
