//! Raw FFI function declarations for the pclsync C library.
//!
//! This module contains manual declarations of the pclsync C library functions.
//! All functions are unsafe and should be wrapped in safe Rust functions before use.
//!
//! # Safety
//!
//! All functions in this module are `unsafe extern "C"` functions. Callers must:
//! - Ensure the library has been initialized before calling most functions
//! - Pass valid pointers where required
//! - Properly handle returned pointers (checking for null, freeing memory)
//! - Not call these functions from multiple threads without synchronization
//!   (unless the specific function is documented as thread-safe)
//!
//! # Thread Safety
//!
//! The pclsync library uses internal threading. Callbacks are called from a dedicated
//! callback thread and are guaranteed not to overlap. However, most functions should
//! only be called from the main application thread.
//!
//! # Memory Management
//!
//! Functions that return pointers to allocated memory typically require the caller
//! to free the memory using `psync_free()` or the standard `free()` function.
//! Check the documentation of each function for specific requirements.

use std::os::raw::{c_char, c_int, c_uint, c_ulong};

use super::types::*;

// ============================================================================
// Initialization and Lifecycle
// ============================================================================

extern "C" {
    /// Set the path to the database file.
    ///
    /// This function should be called before `psync_init()`.
    /// If not called, an appropriate default location will be used.
    ///
    /// Special values:
    /// - `:memory:` - creates an in-memory database (not persisted)
    /// - Empty string - creates database in a temporary file
    ///
    /// # Safety
    ///
    /// `databasepath` must be a valid null-terminated C string or NULL.
    /// The library makes its own copy of the path.
    pub fn psync_set_database_path(databasepath: *const c_char);

    /// Set a custom memory allocator.
    ///
    /// Must be called before `psync_init()` if used at all.
    /// If a custom allocator is provided, its `free` function must be used
    /// to free any memory returned by the library.
    ///
    /// # Safety
    ///
    /// All function pointers must be valid and remain valid for the lifetime
    /// of the library.
    pub fn psync_set_alloc(
        malloc_call: psync_malloc_t,
        realloc_call: psync_realloc_t,
        free_call: psync_free_t,
    );

    /// Set the software name string passed to the server.
    ///
    /// Should be called before `psync_start_sync()`.
    /// The library does NOT make a copy - pass a static string or ensure
    /// the string lives for the duration of the program.
    ///
    /// # Safety
    ///
    /// `str` must be a valid null-terminated C string that remains valid
    /// for the lifetime of the library.
    pub fn psync_set_software_string(str: *const c_char);

    /// Initialize the pclsync library.
    ///
    /// This must be called before any other library functions (except
    /// `psync_set_database_path`, `psync_set_alloc`, and `psync_set_software_string`).
    ///
    /// No network or local scan operations are initiated by this call.
    /// Call `psync_start_sync()` to start those operations.
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on failure (call `psync_get_last_error()` for details)
    ///
    /// # Safety
    ///
    /// - Must only be called once
    /// - Must be called before most other library functions
    pub fn psync_init() -> c_int;

    /// Start the sync process.
    ///
    /// This initiates network connections and local file scanning.
    /// Callbacks are called from a dedicated callback thread.
    ///
    /// After the first run, expect an immediate status callback with
    /// `PSTATUS_LOGIN_REQUIRED` if no credentials are saved.
    ///
    /// # Arguments
    ///
    /// * `status_callback` - Called when sync status changes (can be NULL)
    /// * `event_callback` - Called for file/folder events (can be NULL)
    ///
    /// # Safety
    ///
    /// - Callbacks must be safe to call from any thread
    /// - Callbacks must not block for extended periods
    /// - `psync_init()` must have been called first
    pub fn psync_start_sync(
        status_callback: pstatus_change_callback_t,
        event_callback: pevent_callback_t,
    );

    /// Set the notification callback.
    ///
    /// Should be called before `psync_start_sync()` if notifications are needed.
    ///
    /// # Arguments
    ///
    /// * `notification_callback` - Called when new notifications arrive (can be NULL)
    /// * `thumbsize` - Thumbnail size in "WxH" format (e.g., "64x64"), or NULL for no thumbs
    ///
    /// # Safety
    ///
    /// - `thumbsize` must be a valid null-terminated C string or NULL
    /// - The callback must be safe to call from any thread
    pub fn psync_set_notification_callback(
        notification_callback: pnotification_callback_t,
        thumbsize: *const c_char,
    );

    /// Get the list of notifications.
    ///
    /// # Returns
    ///
    /// Pointer to notification list, or NULL on error.
    /// The returned pointer must be freed with `psync_free()`.
    ///
    /// # Safety
    ///
    /// The returned pointer must be freed by the caller.
    pub fn psync_get_notifications() -> *mut psync_notification_list_t;

    /// Mark notifications as read up to the given ID.
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on error
    pub fn psync_mark_notificaitons_read(notificationid: u32) -> c_int;

    /// Download the directory structure (blocking).
    ///
    /// This downloads the remote directory structure into the local state.
    /// Should be called after `psync_init()` but can be called instead of
    /// or before `psync_start_sync()`.
    ///
    /// # Returns
    ///
    /// One of the `PSTATUS_*` constants:
    /// - `PSTATUS_READY` on success
    /// - `PSTATUS_OFFLINE` if no network
    /// - Login-related status if authentication needed
    ///
    /// # Safety
    ///
    /// - `psync_init()` must have been called first
    /// - This function blocks until complete
    pub fn psync_download_state() -> u32;

    /// Destroy the pclsync library and free resources.
    ///
    /// This should be called before application exit.
    /// Returns relatively quickly regardless of pending operations.
    ///
    /// # Safety
    ///
    /// - Should only be called once
    /// - No library functions should be called after this
    pub fn psync_destroy();
}

// ============================================================================
// Status Functions
// ============================================================================

extern "C" {
    /// Get the current sync status.
    ///
    /// # Arguments
    ///
    /// * `status` - Pointer to status struct to fill
    ///
    /// # Safety
    ///
    /// `status` must be a valid pointer to a `pstatus_t` struct.
    pub fn psync_get_status(status: *mut pstatus_t);

    /// Get the last error code for the current thread.
    ///
    /// # Returns
    ///
    /// One of the `PERROR_*` constants, or 0 if no error.
    pub fn psync_get_last_error() -> u32;

    /// Pause sync operations.
    ///
    /// Sync is stopped but local and remote directories are still monitored.
    /// Status updates continue to be received.
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on error
    pub fn psync_pause() -> c_int;

    /// Stop all sync operations.
    ///
    /// All network and local scan operations stop.
    /// Only a status update with `PSTATUS_STOPPED` is sent.
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on error
    pub fn psync_stop() -> c_int;

    /// Resume sync operations.
    ///
    /// Restarts operations from both paused and stopped states.
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on error
    pub fn psync_resume() -> c_int;

    /// Force a rescan of local files.
    ///
    /// This is normally not needed but can be useful as a user-triggered option.
    pub fn psync_run_localscan();

    /// Notify the library of a network change.
    ///
    /// Call this when the network connection changes (e.g., WiFi access point change).
    /// Safe to call frequently.
    pub fn psync_network_exception();
}

// ============================================================================
// Authentication Functions
// ============================================================================

extern "C" {
    /// Get the current username.
    ///
    /// # Returns
    ///
    /// Pointer to username string, or NULL if not logged in.
    /// The returned pointer must be freed with `psync_free()`.
    ///
    /// # Safety
    ///
    /// The returned pointer must be freed by the caller.
    pub fn psync_get_username() -> *mut c_char;

    /// Set username and password for login.
    ///
    /// Use this for initial login or when `PSTATUS_BAD_LOGIN_DATA` is received.
    ///
    /// Note: If the username doesn't match the previously logged-in user,
    /// `PSTATUS_USER_MISMATCH` will be generated. Call `psync_unlink()` first
    /// to switch users.
    ///
    /// # Arguments
    ///
    /// * `username` - User's email address
    /// * `password` - User's password
    /// * `save` - If non-zero, save credentials for future sessions
    ///
    /// # Safety
    ///
    /// `username` and `password` must be valid null-terminated C strings.
    pub fn psync_set_user_pass(username: *const c_char, password: *const c_char, save: c_int);

    /// Set password only (when username is already known).
    ///
    /// Use this when `PSTATUS_BAD_LOGIN_DATA` is received to update just the password.
    ///
    /// # Arguments
    ///
    /// * `password` - User's password
    /// * `save` - If non-zero, save credentials for future sessions
    ///
    /// # Safety
    ///
    /// `password` must be a valid null-terminated C string.
    pub fn psync_set_pass(password: *const c_char, save: c_int);

    /// Set authentication token for login.
    ///
    /// Alternative to username/password login using an auth token.
    ///
    /// # Arguments
    ///
    /// * `auth` - Authentication token
    /// * `save` - If non-zero, save for future sessions
    ///
    /// # Safety
    ///
    /// `auth` must be a valid null-terminated C string.
    pub fn psync_set_auth(auth: *const c_char, save: c_int);

    /// Log out the current user.
    ///
    /// Clears credentials but keeps the local database.
    pub fn psync_logout();

    /// Unlink the current user.
    ///
    /// Clears credentials and all synced data.
    /// Required before logging in as a different user.
    pub fn psync_unlink();

    /// Get the authentication string for the current user.
    ///
    /// # Returns
    ///
    /// Pointer to auth string, or NULL if not logged in.
    /// Do NOT free the returned pointer.
    ///
    /// # Safety
    ///
    /// The returned pointer is managed by the library and must not be freed.
    pub fn psync_get_auth_string() -> *const c_char;

    /// Register a new user account.
    ///
    /// # Arguments
    ///
    /// * `email` - User's email address (will be username)
    /// * `password` - Chosen password
    /// * `termsaccepted` - Must be non-zero if user accepted terms
    /// * `err` - If non-NULL and function fails, set to error message (must be freed)
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on network error
    /// - Positive API error code on other errors
    ///
    /// # Safety
    ///
    /// - `email` and `password` must be valid null-terminated C strings
    /// - If `err` is non-NULL and return value is non-zero, `*err` must be freed
    pub fn psync_register(
        email: *const c_char,
        password: *const c_char,
        termsaccepted: c_int,
        err: *mut *mut c_char,
    ) -> c_int;

    /// Send email verification mail.
    ///
    /// # Arguments
    ///
    /// * `err` - If non-NULL and function fails, set to error message (must be freed)
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on network error
    /// - Positive API error code on other errors
    pub fn psync_verify_email(err: *mut *mut c_char) -> c_int;

    /// Send password reset email.
    ///
    /// # Arguments
    ///
    /// * `email` - User's email address
    /// * `err` - If non-NULL and function fails, set to error message (must be freed)
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on network error
    /// - Positive API error code on other errors
    ///
    /// # Safety
    ///
    /// `email` must be a valid null-terminated C string.
    pub fn psync_lost_password(email: *const c_char, err: *mut *mut c_char) -> c_int;

    /// Change the user's password.
    ///
    /// # Arguments
    ///
    /// * `currentpass` - Current password
    /// * `newpass` - New password
    /// * `err` - If non-NULL and function fails, set to error message (must be freed)
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on network error
    /// - Positive API error code on other errors
    ///
    /// # Safety
    ///
    /// `currentpass` and `newpass` must be valid null-terminated C strings.
    pub fn psync_change_password(
        currentpass: *const c_char,
        newpass: *const c_char,
        err: *mut *mut c_char,
    ) -> c_int;
}

// ============================================================================
// Web Login Functions
// ============================================================================

extern "C" {
    /// Get a request ID for web-based login.
    ///
    /// This initiates a web login session and returns a request ID that can be
    /// used to construct the login URL and to poll for authentication completion.
    ///
    /// # Arguments
    ///
    /// * `req_id` - On success, set to point to the request ID string (must be freed)
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on network error
    /// - Positive API error code on other errors
    ///
    /// # Safety
    ///
    /// `req_id` must be a valid pointer. On success, `*req_id` must be freed with `psync_free()`.
    pub fn get_login_req_id(req_id: *mut *mut c_char) -> c_int;

    /// Wait for authentication token after web login.
    ///
    /// Blocks until the user completes authentication in the browser or timeout occurs.
    /// On success, the auth token is automatically set in the library.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The request ID obtained from `get_login_req_id()`
    ///
    /// # Returns
    ///
    /// - `0` on success (token auto-set)
    /// - `-1` on network error
    /// - Positive API error code on other errors (e.g., timeout)
    ///
    /// # Safety
    ///
    /// `request_id` must be a valid null-terminated C string.
    pub fn wait_auth_token(request_id: *const c_char) -> c_int;

    /// Get the machine name (hostname).
    ///
    /// Uses platform-specific methods to get a human-readable machine name.
    /// On Linux, this typically reads from /etc/hostname or uses gethostname().
    ///
    /// # Returns
    ///
    /// A psync_strdup'd string containing the machine name, or NULL on error.
    /// The returned pointer must be freed with `psync_free()`.
    ///
    /// # Safety
    ///
    /// The returned pointer must be freed by the caller.
    pub fn get_machine_name() -> *mut c_char;
}

// ============================================================================
// Sync Folder Management
// ============================================================================

extern "C" {
    /// Add a sync relationship by remote path.
    ///
    /// # Arguments
    ///
    /// * `localpath` - Local folder path
    /// * `remotepath` - Remote folder path (starts with "/")
    /// * `synctype` - One of `PSYNC_DOWNLOAD_ONLY`, `PSYNC_UPLOAD_ONLY`, or `PSYNC_FULL`
    ///
    /// # Returns
    ///
    /// - Sync ID on success
    /// - `-1` on error (call `psync_get_last_error()` for details)
    ///
    /// # Safety
    ///
    /// `localpath` and `remotepath` must be valid null-terminated C strings.
    pub fn psync_add_sync_by_path(
        localpath: *const c_char,
        remotepath: *const c_char,
        synctype: psync_synctype_t,
    ) -> psync_syncid_t;

    /// Add a sync relationship by remote folder ID.
    ///
    /// # Arguments
    ///
    /// * `localpath` - Local folder path
    /// * `folderid` - Remote folder ID (0 for root)
    /// * `synctype` - One of `PSYNC_DOWNLOAD_ONLY`, `PSYNC_UPLOAD_ONLY`, or `PSYNC_FULL`
    ///
    /// # Returns
    ///
    /// - Sync ID on success
    /// - `-1` on error
    ///
    /// # Safety
    ///
    /// `localpath` must be a valid null-terminated C string.
    pub fn psync_add_sync_by_folderid(
        localpath: *const c_char,
        folderid: psync_folderid_t,
        synctype: psync_synctype_t,
    ) -> psync_syncid_t;

    /// Add a sync relationship with delayed creation.
    ///
    /// Can be called right after `psync_init()`, even before login.
    /// The sync will be created when the user logs in and state is downloaded.
    /// If the remote path doesn't exist, it will be created if possible.
    ///
    /// # Arguments
    ///
    /// * `localpath` - Local folder path
    /// * `remotepath` - Remote folder path
    /// * `synctype` - Sync type
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on error (only if localpath is invalid)
    ///
    /// # Safety
    ///
    /// `localpath` and `remotepath` must be valid null-terminated C strings.
    pub fn psync_add_sync_by_path_delayed(
        localpath: *const c_char,
        remotepath: *const c_char,
        synctype: psync_synctype_t,
    ) -> c_int;

    /// Change the sync type for an existing sync.
    ///
    /// # Arguments
    ///
    /// * `syncid` - ID of the sync to modify
    /// * `synctype` - New sync type
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on error
    pub fn psync_change_synctype(syncid: psync_syncid_t, synctype: psync_synctype_t) -> c_int;

    /// Delete a sync relationship.
    ///
    /// No files or folders are deleted - only the sync relationship.
    ///
    /// # Arguments
    ///
    /// * `syncid` - ID of the sync to delete
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on error (e.g., invalid sync ID)
    pub fn psync_delete_sync(syncid: psync_syncid_t) -> c_int;

    /// Get the list of all sync relationships.
    ///
    /// # Returns
    ///
    /// Pointer to sync list, or NULL on error.
    /// The returned pointer must be freed with `psync_free()`.
    ///
    /// # Safety
    ///
    /// The returned pointer must be freed by the caller.
    pub fn psync_get_sync_list() -> *mut psync_folder_list_t;

    /// Get suggested folders for syncing.
    ///
    /// # Returns
    ///
    /// Pointer to suggestions list.
    /// The returned pointer must be freed with `psync_free()`.
    pub fn psync_get_sync_suggestions() -> *mut psuggested_folders_t;
}

// ============================================================================
// Folder Listing Functions
// ============================================================================

extern "C" {
    /// List contents of a local folder.
    ///
    /// # Arguments
    ///
    /// * `localpath` - Path to local folder
    /// * `listtype` - One of `PLIST_FILES`, `PLIST_FOLDERS`, or `PLIST_ALL`
    ///
    /// # Returns
    ///
    /// Pointer to folder list, or NULL on error.
    /// The returned pointer must be freed with a single `free()` call.
    ///
    /// # Safety
    ///
    /// `localpath` must be a valid null-terminated C string.
    pub fn psync_list_local_folder_by_path(
        localpath: *const c_char,
        listtype: psync_listtype_t,
    ) -> *mut pfolder_list_t;

    /// List contents of a remote folder by path.
    ///
    /// # Arguments
    ///
    /// * `remotepath` - Remote path (starts with "/")
    /// * `listtype` - One of `PLIST_FILES`, `PLIST_FOLDERS`, or `PLIST_ALL`
    ///
    /// # Returns
    ///
    /// Pointer to folder list, or NULL on error.
    /// The returned pointer must be freed with a single `free()` call.
    ///
    /// # Safety
    ///
    /// `remotepath` must be a valid null-terminated C string.
    pub fn psync_list_remote_folder_by_path(
        remotepath: *const c_char,
        listtype: psync_listtype_t,
    ) -> *mut pfolder_list_t;

    /// List contents of a remote folder by ID.
    ///
    /// # Arguments
    ///
    /// * `folderid` - Remote folder ID (0 for root)
    /// * `listtype` - One of `PLIST_FILES`, `PLIST_FOLDERS`, or `PLIST_ALL`
    ///
    /// # Returns
    ///
    /// Pointer to folder list, or NULL on error.
    /// The returned pointer must be freed with a single `free()` call.
    pub fn psync_list_remote_folder_by_folderid(
        folderid: psync_folderid_t,
        listtype: psync_listtype_t,
    ) -> *mut pfolder_list_t;

    /// Get information about a remote path.
    ///
    /// # Arguments
    ///
    /// * `remotepath` - Remote path
    ///
    /// # Returns
    ///
    /// Pointer to entry info, or NULL if not found.
    /// The returned pointer must be freed with `psync_free()`.
    ///
    /// # Safety
    ///
    /// `remotepath` must be a valid null-terminated C string.
    pub fn psync_stat_path(remotepath: *const c_char) -> *mut pentry_t;

    /// Check if a name should be ignored during sync.
    ///
    /// # Arguments
    ///
    /// * `name` - File or folder name to check
    ///
    /// # Returns
    ///
    /// - `1` if the name should be ignored
    /// - `0` otherwise
    ///
    /// # Safety
    ///
    /// `name` must be a valid null-terminated C string.
    pub fn psync_is_name_to_ignore(name: *const c_char) -> c_int;

    /// Create a remote folder by path.
    ///
    /// # Arguments
    ///
    /// * `path` - Full remote path to create
    /// * `err` - If non-NULL and fails, set to error message
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on network error
    /// - Positive API error code on other errors
    ///
    /// # Safety
    ///
    /// `path` must be a valid null-terminated C string.
    pub fn psync_create_remote_folder_by_path(path: *const c_char, err: *mut *mut c_char) -> c_int;

    /// Create a remote folder in a parent folder.
    ///
    /// # Arguments
    ///
    /// * `parentfolderid` - Parent folder ID
    /// * `name` - Name of new folder
    /// * `err` - If non-NULL and fails, set to error message
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on network error
    /// - Positive API error code on other errors
    ///
    /// # Safety
    ///
    /// `name` must be a valid null-terminated C string.
    pub fn psync_create_remote_folder(
        parentfolderid: psync_folderid_t,
        name: *const c_char,
        err: *mut *mut c_char,
    ) -> c_int;
}

// ============================================================================
// Settings Functions
// ============================================================================

extern "C" {
    /// Get a boolean setting.
    ///
    /// # Arguments
    ///
    /// * `settingname` - Name of the setting
    ///
    /// # Returns
    ///
    /// Setting value, or 0 on error.
    ///
    /// # Safety
    ///
    /// `settingname` must be a valid null-terminated C string.
    pub fn psync_get_bool_setting(settingname: *const c_char) -> c_int;

    /// Set a boolean setting.
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on error (invalid setting name or type mismatch)
    ///
    /// # Safety
    ///
    /// `settingname` must be a valid null-terminated C string.
    pub fn psync_set_bool_setting(settingname: *const c_char, value: c_int) -> c_int;

    /// Get an integer setting.
    ///
    /// # Safety
    ///
    /// `settingname` must be a valid null-terminated C string.
    pub fn psync_get_int_setting(settingname: *const c_char) -> i64;

    /// Set an integer setting.
    ///
    /// # Safety
    ///
    /// `settingname` must be a valid null-terminated C string.
    pub fn psync_set_int_setting(settingname: *const c_char, value: i64) -> c_int;

    /// Get an unsigned integer setting.
    ///
    /// # Safety
    ///
    /// `settingname` must be a valid null-terminated C string.
    pub fn psync_get_uint_setting(settingname: *const c_char) -> u64;

    /// Set an unsigned integer setting.
    ///
    /// # Safety
    ///
    /// `settingname` must be a valid null-terminated C string.
    pub fn psync_set_uint_setting(settingname: *const c_char, value: u64) -> c_int;

    /// Get a string setting.
    ///
    /// # Returns
    ///
    /// Setting value (do not free), or empty string on error.
    ///
    /// # Safety
    ///
    /// - `settingname` must be a valid null-terminated C string
    /// - The returned pointer must not be freed and is only valid until
    ///   the setting is changed
    pub fn psync_get_string_setting(settingname: *const c_char) -> *const c_char;

    /// Set a string setting.
    ///
    /// # Safety
    ///
    /// `settingname` and `value` must be valid null-terminated C strings.
    pub fn psync_set_string_setting(settingname: *const c_char, value: *const c_char) -> c_int;
}

// ============================================================================
// Value Functions (User-Defined Key-Value Storage)
// ============================================================================

extern "C" {
    /// Check if a value exists.
    ///
    /// # Safety
    ///
    /// `valuename` must be a valid null-terminated C string.
    pub fn psync_has_value(valuename: *const c_char) -> c_int;

    /// Get a boolean value.
    ///
    /// # Safety
    ///
    /// `valuename` must be a valid null-terminated C string.
    pub fn psync_get_bool_value(valuename: *const c_char) -> c_int;

    /// Set a boolean value.
    ///
    /// # Safety
    ///
    /// `valuename` must be a valid null-terminated C string.
    pub fn psync_set_bool_value(valuename: *const c_char, value: c_int);

    /// Get an integer value.
    ///
    /// # Safety
    ///
    /// `valuename` must be a valid null-terminated C string.
    pub fn psync_get_int_value(valuename: *const c_char) -> i64;

    /// Set an integer value.
    ///
    /// # Safety
    ///
    /// `valuename` must be a valid null-terminated C string.
    pub fn psync_set_int_value(valuename: *const c_char, value: i64);

    /// Get an unsigned integer value.
    ///
    /// # Safety
    ///
    /// `valuename` must be a valid null-terminated C string.
    pub fn psync_get_uint_value(valuename: *const c_char) -> u64;

    /// Set an unsigned integer value.
    ///
    /// # Safety
    ///
    /// `valuename` must be a valid null-terminated C string.
    pub fn psync_set_uint_value(valuename: *const c_char, value: u64);

    /// Get a string value.
    ///
    /// # Returns
    ///
    /// String value (must be freed), or NULL if not found.
    ///
    /// # Safety
    ///
    /// - `valuename` must be a valid null-terminated C string
    /// - The returned pointer must be freed by the caller
    pub fn psync_get_string_value(valuename: *const c_char) -> *mut c_char;

    /// Set a string value.
    ///
    /// # Safety
    ///
    /// `valuename` and `value` must be valid null-terminated C strings.
    pub fn psync_set_string_value(valuename: *const c_char, value: *const c_char);
}

// ============================================================================
// Sharing Functions
// ============================================================================

extern "C" {
    /// List incoming or outgoing share requests.
    ///
    /// # Arguments
    ///
    /// * `incoming` - If non-zero, list incoming requests; otherwise outgoing
    ///
    /// # Returns
    ///
    /// Pointer to share request list.
    /// The returned pointer must be freed with `psync_free()`.
    pub fn psync_list_sharerequests(incoming: c_int) -> *mut psync_sharerequest_list_t;

    /// List established shares.
    ///
    /// # Arguments
    ///
    /// * `incoming` - If non-zero, list incoming shares; otherwise outgoing
    ///
    /// # Returns
    ///
    /// Pointer to share list.
    /// The returned pointer must be freed with `psync_free()`.
    pub fn psync_list_shares(incoming: c_int) -> *mut psync_share_list_t;

    /// Share a folder with another user.
    ///
    /// # Arguments
    ///
    /// * `folderid` - Folder to share
    /// * `name` - Display name for the share
    /// * `mail` - Email address of user to share with
    /// * `message` - Optional message to include
    /// * `permissions` - Bitwise OR of `PSYNC_PERM_*` constants
    /// * `err` - If non-NULL and fails, set to error message
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - `-1` on network error
    /// - Positive API error code on other errors
    ///
    /// # Safety
    ///
    /// All string parameters must be valid null-terminated C strings.
    pub fn psync_share_folder(
        folderid: psync_folderid_t,
        name: *const c_char,
        mail: *const c_char,
        message: *const c_char,
        permissions: u32,
        err: *mut *mut c_char,
    ) -> c_int;

    /// Cancel an outgoing share request.
    ///
    /// # Safety
    ///
    /// If `err` is non-NULL and function fails, `*err` must be freed.
    pub fn psync_cancel_share_request(
        requestid: psync_sharerequestid_t,
        err: *mut *mut c_char,
    ) -> c_int;

    /// Decline an incoming share request.
    ///
    /// # Safety
    ///
    /// If `err` is non-NULL and function fails, `*err` must be freed.
    pub fn psync_decline_share_request(
        requestid: psync_sharerequestid_t,
        err: *mut *mut c_char,
    ) -> c_int;

    /// Accept an incoming share request.
    ///
    /// # Arguments
    ///
    /// * `requestid` - Share request ID
    /// * `tofolderid` - Parent folder to place share in
    /// * `name` - Name for the shared folder (NULL to use original name)
    /// * `err` - Error message pointer
    ///
    /// # Safety
    ///
    /// `name` must be a valid null-terminated C string or NULL.
    pub fn psync_accept_share_request(
        requestid: psync_sharerequestid_t,
        tofolderid: psync_folderid_t,
        name: *const c_char,
        err: *mut *mut c_char,
    ) -> c_int;

    /// Remove an established share.
    ///
    /// Can be called by either the sharing or receiving user.
    pub fn psync_remove_share(shareid: psync_shareid_t, err: *mut *mut c_char) -> c_int;

    /// Modify permissions on an established share.
    pub fn psync_modify_share(
        shareid: psync_shareid_t,
        permissions: u32,
        err: *mut *mut c_char,
    ) -> c_int;
}

// ============================================================================
// Filesystem (FUSE) Functions
// ============================================================================

extern "C" {
    /// Start the virtual filesystem.
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - Non-zero on error
    pub fn psync_fs_start() -> c_int;

    /// Check if the filesystem is started.
    ///
    /// # Returns
    ///
    /// - `1` if started
    /// - `0` otherwise
    pub fn psync_fs_isstarted() -> c_int;

    /// Stop the virtual filesystem.
    pub fn psync_fs_stop();

    /// Get the filesystem mount point.
    ///
    /// # Returns
    ///
    /// Mount point path, or NULL if not mounted.
    /// The returned pointer must be freed by the caller.
    pub fn psync_fs_getmountpoint() -> *mut c_char;

    /// Register a callback to be called when the filesystem starts.
    ///
    /// # Safety
    ///
    /// The callback must be safe to call from any thread.
    pub fn psync_fs_register_start_callback(callback: psync_generic_callback_t);

    /// Get the full path for a folder ID on the filesystem.
    ///
    /// # Returns
    ///
    /// Full path including mount point, or NULL if not found or not mounted.
    /// The returned pointer must be freed by the caller.
    pub fn psync_fs_get_path_by_folderid(folderid: psync_folderid_t) -> *mut c_char;
}

// ============================================================================
// Crypto Functions
// ============================================================================

extern "C" {
    /// Set up crypto for the account.
    ///
    /// Creates encryption keys with the given password.
    ///
    /// # Arguments
    ///
    /// * `password` - Crypto password
    /// * `hint` - Password hint (optional)
    ///
    /// # Returns
    ///
    /// One of `PSYNC_CRYPTO_SETUP_*` constants.
    ///
    /// # Safety
    ///
    /// `password` and `hint` must be valid null-terminated C strings.
    pub fn psync_crypto_setup(password: *const c_char, hint: *const c_char) -> c_int;

    /// Get the crypto password hint.
    ///
    /// # Arguments
    ///
    /// * `hint` - On success, set to point to hint string (must be freed)
    ///
    /// # Returns
    ///
    /// One of `PSYNC_CRYPTO_HINT_*` constants.
    /// On success, `*hint` must be freed.
    ///
    /// # Safety
    ///
    /// `hint` must be a valid pointer.
    pub fn psync_crypto_get_hint(hint: *mut *mut c_char) -> c_int;

    /// Start crypto (unlock encryption).
    ///
    /// # Arguments
    ///
    /// * `password` - Crypto password
    ///
    /// # Returns
    ///
    /// One of `PSYNC_CRYPTO_START_*` constants.
    ///
    /// # Safety
    ///
    /// `password` must be a valid null-terminated C string.
    pub fn psync_crypto_start(password: *const c_char) -> c_int;

    /// Stop crypto (lock encryption).
    ///
    /// # Returns
    ///
    /// One of `PSYNC_CRYPTO_STOP_*` constants.
    pub fn psync_crypto_stop() -> c_int;

    /// Check if crypto is currently started.
    ///
    /// # Returns
    ///
    /// - `1` if started
    /// - `0` otherwise
    pub fn psync_crypto_isstarted() -> c_int;

    /// Create an encrypted folder.
    ///
    /// # Arguments
    ///
    /// * `folderid` - Parent folder ID
    /// * `name` - Name for the new folder
    /// * `err` - On error, set to point to error string (do not free)
    /// * `newfolderid` - On success, set to new folder ID (can be NULL)
    ///
    /// # Returns
    ///
    /// - `0` on success
    /// - Negative for local errors
    /// - Positive for API errors
    ///
    /// # Safety
    ///
    /// `name` must be a valid null-terminated C string.
    /// `err` is set to a static string and must not be freed.
    pub fn psync_crypto_mkdir(
        folderid: psync_folderid_t,
        name: *const c_char,
        err: *mut *const c_char,
        newfolderid: *mut psync_folderid_t,
    ) -> c_int;

    /// Check if crypto has been set up for this account.
    ///
    /// # Returns
    ///
    /// - `1` if set up
    /// - `0` otherwise
    pub fn psync_crypto_issetup() -> c_int;

    /// Check if the user has an active crypto subscription.
    ///
    /// # Returns
    ///
    /// - `1` if has subscription
    /// - `0` otherwise
    pub fn psync_crypto_hassubscription() -> c_int;

    /// Check if the crypto service is expired.
    ///
    /// # Returns
    ///
    /// - `1` if expired
    /// - `0` if not expired or never set up (eligible for trial)
    pub fn psync_crypto_isexpired() -> c_int;

    /// Get the crypto expiration timestamp.
    ///
    /// # Returns
    ///
    /// Unix timestamp of expiration, or 0 if never set up.
    pub fn psync_crypto_expires() -> libc::time_t;

    /// Reset crypto (delete all encrypted data).
    ///
    /// Sends a confirmation email to the user.
    ///
    /// # Returns
    ///
    /// One of `PSYNC_CRYPTO_RESET_*` constants.
    pub fn psync_crypto_reset() -> c_int;

    /// Get the ID of the first encrypted folder.
    ///
    /// # Returns
    ///
    /// Folder ID, or `PSYNC_CRYPTO_INVALID_FOLDERID` if none found.
    pub fn psync_crypto_folderid() -> psync_folderid_t;

    /// Get all encrypted folder IDs.
    ///
    /// # Returns
    ///
    /// Array of folder IDs terminated by `PSYNC_CRYPTO_INVALID_FOLDERID`.
    /// The returned pointer must be freed by the caller.
    pub fn psync_crypto_folderids() -> *mut psync_folderid_t;
}

// ============================================================================
// Password Quality Functions
// ============================================================================

extern "C" {
    /// Estimate password quality.
    ///
    /// # Returns
    ///
    /// - `0` - weak
    /// - `1` - moderate
    /// - `2` - strong
    ///
    /// # Safety
    ///
    /// `password` must be a valid null-terminated C string.
    pub fn psync_password_quality(password: *const c_char) -> c_int;

    /// Estimate password quality with finer granularity.
    ///
    /// # Returns
    ///
    /// - `0-9999` - weak
    /// - `10000-19999` - moderate
    /// - `20000-29999` - strong
    ///
    /// Divide by 10000 to get the same result as `psync_password_quality()`.
    ///
    /// # Safety
    ///
    /// `password` must be a valid null-terminated C string.
    pub fn psync_password_quality10000(password: *const c_char) -> c_int;
}

// ============================================================================
// Update Functions
// ============================================================================

extern "C" {
    /// Check for new version (string version).
    ///
    /// # Arguments
    ///
    /// * `os` - OS identifier (e.g., "LINUX64")
    /// * `currentversion` - Current version string (e.g., "3.0.0")
    ///
    /// # Returns
    ///
    /// Version info, or NULL if no update available.
    /// The returned pointer must be freed with `psync_free()`.
    ///
    /// # Safety
    ///
    /// All string parameters must be valid null-terminated C strings.
    pub fn psync_check_new_version_str(
        os: *const c_char,
        currentversion: *const c_char,
    ) -> *mut psync_new_version_t;

    /// Check for new version (numeric version).
    ///
    /// Version format: `a*10000 + b*100 + c` for version "a.b.c"
    ///
    /// # Safety
    ///
    /// `os` must be a valid null-terminated C string.
    pub fn psync_check_new_version(
        os: *const c_char,
        currentversion: c_ulong,
    ) -> *mut psync_new_version_t;

    /// Check and download new version.
    ///
    /// Same as `psync_check_new_version_str` but also downloads the update.
    /// The download path is stored in the returned struct's `localpath` field.
    ///
    /// # Safety
    ///
    /// All string parameters must be valid null-terminated C strings.
    pub fn psync_check_new_version_download_str(
        os: *const c_char,
        currentversion: *const c_char,
    ) -> *mut psync_new_version_t;

    /// Check and download new version (numeric).
    ///
    /// # Safety
    ///
    /// `os` must be a valid null-terminated C string.
    pub fn psync_check_new_version_download(
        os: *const c_char,
        currentversion: c_ulong,
    ) -> *mut psync_new_version_t;

    /// Run the update installer.
    ///
    /// On success, this function does not return (the process is replaced).
    ///
    /// # Safety
    ///
    /// `ver` must point to a valid version struct.
    pub fn psync_run_new_version(ver: *mut psync_new_version_t);
}

// ============================================================================
// Memory Management
// ============================================================================

extern "C" {
    /// Allocate memory using the library's allocator.
    pub fn psync_malloc(size: usize) -> *mut std::ffi::c_void;

    /// Reallocate memory using the library's allocator.
    pub fn psync_realloc(ptr: *mut std::ffi::c_void, size: usize) -> *mut std::ffi::c_void;

    /// Free memory allocated by the library.
    ///
    /// Use this to free any memory returned by pclsync functions
    /// (unless documented otherwise).
    pub fn psync_free(ptr: *mut std::ffi::c_void);
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests verify that the FFI declarations compile correctly.
    // Actual functionality tests require the library to be initialized.

    #[test]
    fn test_function_pointers_exist() {
        // Just verify that the function symbols are declared
        let _: unsafe extern "C" fn() -> c_int = psync_init;
        let _: unsafe extern "C" fn() = psync_destroy;
        let _: unsafe extern "C" fn(*mut pstatus_t) = psync_get_status;
    }
}
