//! Shared helpers for turning pclsync file-events into human-readable activity
//! log entries.
//!
//! Used both by the daemon (which captures events into its activity ring
//! buffer) and historically by the in-process TUI. Centralizing this keeps the
//! daemon and any client rendering identical descriptions.

use crate::ffi::types::{
    psync_eventdata_t, psync_eventtype_t, PEVENT_FILE_DOWNLOAD_FAILED,
    PEVENT_FILE_DOWNLOAD_FINISHED, PEVENT_FILE_DOWNLOAD_STARTED, PEVENT_FILE_UPLOAD_FAILED,
    PEVENT_FILE_UPLOAD_FINISHED, PEVENT_FILE_UPLOAD_STARTED, PEVENT_FIRST_SHARE_EVENT,
    PEVENT_LOCAL_FILE_DELETED, PEVENT_LOCAL_FOLDER_CREATED, PEVENT_LOCAL_FOLDER_DELETED,
    PEVENT_REMOTE_FILE_DELETED, PEVENT_REMOTE_FOLDER_CREATED, PEVENT_REMOTE_FOLDER_DELETED,
    PEVENT_USEDQUOTA_CHANGED, PEVENT_USERINFO_CHANGED,
};

/// Convert a C library event into a human-readable `(description, is_error)`.
///
/// Returns `None` for metadata events (user info, quota changes, shares) that
/// have no file data and should not appear in the activity log.
///
/// # Safety
///
/// `event_data` must be the union value pclsync passed for `event_type`; for
/// file events its `.file` pointer (and the strings it references) must be
/// valid for the duration of this call. This matches the contract of the
/// pclsync event callback.
pub fn describe_event(
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

/// Current wall-clock time formatted as `HH:MM:SS` (UTC), used to timestamp
/// activity-log entries.
pub fn now_hms() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now % 60;
    let mins = (now / 60) % 60;
    let hours = (now / 3600) % 24;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}
