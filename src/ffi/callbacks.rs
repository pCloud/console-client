//! Callback trampolines and safe callback registration for pclsync C library.
//!
//! This module provides:
//! - Safe Rust callback registration for C library callbacks
//! - Trampoline functions that can be passed to C code
//! - Panic guards to prevent unwinding across FFI boundaries
//!
//! # Architecture
//!
//! The pclsync C library uses function pointer callbacks for notifications:
//! - Status change callback: Called when sync status changes
//! - Event callback: Called for file/folder events (download, upload, etc.)
//! - Notification callback: Called when new notifications arrive
//! - Generic callback: Simple void callback (e.g., for filesystem start)
//!
//! Since C callbacks cannot capture state, we use the "trampoline pattern":
//! 1. Store Rust closures in global static storage
//! 2. Provide extern "C" trampoline functions that C code can call
//! 3. The trampolines retrieve the stored closure and invoke it
//!
//! # Thread Safety
//!
//! - Callbacks may be called from any thread by the C library
//! - All callback storage is protected by Mutex
//! - Callbacks must be `Send + 'static`
//! - The C library guarantees callbacks don't overlap
//!
//! # Panic Safety
//!
//! Panics in callbacks called from C code cause undefined behavior.
//! All trampoline functions use `catch_unwind` to prevent panics from
//! unwinding across the FFI boundary.
//!
//! # Example
//!
//! ```ignore
//! use console_client::ffi::callbacks::{register_status_callback, status_callback_trampoline};
//! use console_client::ffi::raw;
//!
//! // Register a Rust closure as the status callback
//! register_status_callback(|status| {
//!     println!("Status changed: {}", status.status);
//! });
//!
//! // Pass the trampoline to C code
//! unsafe {
//!     raw::psync_start_sync(
//!         Some(status_callback_trampoline),
//!         None,
//!     );
//! }
//! ```

use std::panic;
use std::sync::Mutex;

use crate::ffi::types::{
    pevent_callback_t, pstatus_change_callback_t, pstatus_t, psync_eventdata_t, psync_eventtype_t,
    psync_generic_callback_t,
};

// ============================================================================
// Type Aliases for Callback Functions
// ============================================================================

/// Type alias for status change callback functions.
///
/// The callback receives a reference to the current status struct.
/// The status struct is only valid for the duration of the callback.
pub type StatusCallbackFn = Box<dyn Fn(&pstatus_t) + Send + 'static>;

/// Type alias for event callback functions.
///
/// The callback receives:
/// - `event_type`: The type of event (PEVENT_* constants)
/// - `event_data`: Union containing event-specific data
///
/// The event data pointers are only valid for the duration of the callback.
/// If you need to store the data, make copies of the strings.
pub type EventCallbackFn = Box<dyn Fn(psync_eventtype_t, psync_eventdata_t) + Send + 'static>;

/// Type alias for notification callback functions.
///
/// The callback receives:
/// - `notification_count`: Total number of notifications
/// - `new_notification_count`: Number of new (unread) notifications
pub type NotificationCallbackFn = Box<dyn Fn(u32, u32) + Send + 'static>;

/// Type alias for generic (void) callback functions.
///
/// Used for simple notifications like filesystem start.
pub type GenericCallbackFn = Box<dyn Fn() + Send + 'static>;

// ============================================================================
// Global Callback Storage
// ============================================================================

/// Global storage for the status change callback.
///
/// Protected by Mutex for thread-safe access from C callback thread.
static STATUS_CALLBACK: Mutex<Option<StatusCallbackFn>> = Mutex::new(None);

/// Global storage for the event callback.
static EVENT_CALLBACK: Mutex<Option<EventCallbackFn>> = Mutex::new(None);

/// Global storage for the notification callback.
static NOTIFICATION_CALLBACK: Mutex<Option<NotificationCallbackFn>> = Mutex::new(None);

/// Global storage for the filesystem start callback.
static FS_START_CALLBACK: Mutex<Option<GenericCallbackFn>> = Mutex::new(None);

// ============================================================================
// Trampoline Functions (extern "C")
// ============================================================================

/// Status change callback trampoline.
///
/// This function is called from the C library when the sync status changes.
/// It retrieves the registered Rust callback and invokes it.
///
/// # Safety
///
/// This function is called from C code and must:
/// - Not panic (uses catch_unwind)
/// - Handle null status pointer gracefully
/// - Not hold the callback mutex for long
///
/// The status pointer is valid for the duration of this call and must not
/// be stored or used after the callback returns.
#[no_mangle]
pub unsafe extern "C" fn status_callback_trampoline(status: *mut pstatus_t) {
    // Catch any panics to prevent unwinding across FFI boundary
    let _ = panic::catch_unwind(|| {
        // Check for null pointer
        if status.is_null() {
            return;
        }

        // Try to acquire the callback mutex
        if let Ok(guard) = STATUS_CALLBACK.lock() {
            if let Some(ref callback) = *guard {
                // Safety: We checked status is not null above
                callback(&*status);
            }
        }
        // If mutex is poisoned or callback is None, silently do nothing
    });
}

/// Event callback trampoline.
///
/// This function is called from the C library for file/folder events.
/// It retrieves the registered Rust callback and invokes it.
///
/// # Safety
///
/// This function is called from C code and must:
/// - Not panic (uses catch_unwind)
/// - Handle the event data union carefully
/// - Not store pointers from event_data beyond callback duration
#[no_mangle]
pub unsafe extern "C" fn event_callback_trampoline(
    event_type: psync_eventtype_t,
    event_data: psync_eventdata_t,
) {
    let _ = panic::catch_unwind(|| {
        if let Ok(guard) = EVENT_CALLBACK.lock() {
            if let Some(ref callback) = *guard {
                callback(event_type, event_data);
            }
        }
    });
}

/// Notification callback trampoline.
///
/// This function is called from the C library when notifications arrive.
///
/// # Safety
///
/// This function is called from C code and must not panic.
#[no_mangle]
pub unsafe extern "C" fn notification_callback_trampoline(
    notification_count: u32,
    new_notification_count: u32,
) {
    let _ = panic::catch_unwind(|| {
        if let Ok(guard) = NOTIFICATION_CALLBACK.lock() {
            if let Some(ref callback) = *guard {
                callback(notification_count, new_notification_count);
            }
        }
    });
}

/// Generic callback trampoline for filesystem start.
///
/// This function is called from the C library when the filesystem starts.
///
/// # Safety
///
/// This function is called from C code and must not panic.
#[no_mangle]
pub unsafe extern "C" fn fs_start_callback_trampoline() {
    let _ = panic::catch_unwind(|| {
        if let Ok(guard) = FS_START_CALLBACK.lock() {
            if let Some(ref callback) = *guard {
                callback();
            }
        }
    });
}

// ============================================================================
// Callback Registration Functions
// ============================================================================

/// Register a status change callback.
///
/// The callback will be invoked whenever the sync status changes.
/// The previous callback (if any) is replaced.
///
/// # Arguments
///
/// * `callback` - The callback function to register. Must be `Send + 'static`.
///
/// # Thread Safety
///
/// This function is thread-safe. The callback will be called from the
/// pclsync callback thread.
///
/// # Example
///
/// ```ignore
/// register_status_callback(|status| {
///     println!("Status: {}", status.status);
///     println!("Files to download: {}", status.filestodownload);
/// });
/// ```
pub fn register_status_callback<F>(callback: F)
where
    F: Fn(&pstatus_t) + Send + 'static,
{
    let mut guard = STATUS_CALLBACK
        .lock()
        .expect("STATUS_CALLBACK mutex poisoned");
    *guard = Some(Box::new(callback));
}

/// Register an event callback.
///
/// The callback will be invoked for file/folder events (downloads, uploads, etc.).
/// The previous callback (if any) is replaced.
///
/// # Arguments
///
/// * `callback` - The callback function to register. Must be `Send + 'static`.
///
/// # Event Types
///
/// The event type is one of the `PEVENT_*` constants from `ffi::types`.
/// Common events include:
/// - `PEVENT_FILE_DOWNLOAD_STARTED`
/// - `PEVENT_FILE_DOWNLOAD_FINISHED`
/// - `PEVENT_FILE_UPLOAD_STARTED`
/// - `PEVENT_FILE_UPLOAD_FINISHED`
/// - `PEVENT_LOCAL_FOLDER_CREATED`
/// - etc.
///
/// # Example
///
/// ```ignore
/// use console_client::ffi::types::*;
///
/// register_event_callback(|event_type, event_data| {
///     match event_type {
///         PEVENT_FILE_DOWNLOAD_FINISHED => {
///             // event_data.file contains file info
///             println!("Download finished!");
///         }
///         _ => {}
///     }
/// });
/// ```
pub fn register_event_callback<F>(callback: F)
where
    F: Fn(psync_eventtype_t, psync_eventdata_t) + Send + 'static,
{
    let mut guard = EVENT_CALLBACK
        .lock()
        .expect("EVENT_CALLBACK mutex poisoned");
    *guard = Some(Box::new(callback));
}

/// Register a notification callback.
///
/// The callback will be invoked when new notifications arrive.
/// The previous callback (if any) is replaced.
///
/// # Arguments
///
/// * `callback` - The callback function to register. Must be `Send + 'static`.
///
/// # Example
///
/// ```ignore
/// register_notification_callback(|total, new| {
///     if new > 0 {
///         println!("You have {} new notifications!", new);
///     }
/// });
/// ```
pub fn register_notification_callback<F>(callback: F)
where
    F: Fn(u32, u32) + Send + 'static,
{
    let mut guard = NOTIFICATION_CALLBACK
        .lock()
        .expect("NOTIFICATION_CALLBACK mutex poisoned");
    *guard = Some(Box::new(callback));
}

/// Register a filesystem start callback.
///
/// The callback will be invoked when the virtual filesystem starts.
/// The previous callback (if any) is replaced.
///
/// # Arguments
///
/// * `callback` - The callback function to register. Must be `Send + 'static`.
///
/// # Example
///
/// ```ignore
/// register_fs_start_callback(|| {
///     println!("Filesystem is now mounted!");
/// });
/// ```
pub fn register_fs_start_callback<F>(callback: F)
where
    F: Fn() + Send + 'static,
{
    let mut guard = FS_START_CALLBACK
        .lock()
        .expect("FS_START_CALLBACK mutex poisoned");
    *guard = Some(Box::new(callback));
}

// ============================================================================
// Callback Clearing Functions
// ============================================================================

/// Clear the registered status callback.
///
/// After calling this, status changes will not trigger any callback.
pub fn clear_status_callback() {
    let mut guard = STATUS_CALLBACK
        .lock()
        .expect("STATUS_CALLBACK mutex poisoned");
    *guard = None;
}

/// Clear the registered event callback.
///
/// After calling this, events will not trigger any callback.
pub fn clear_event_callback() {
    let mut guard = EVENT_CALLBACK
        .lock()
        .expect("EVENT_CALLBACK mutex poisoned");
    *guard = None;
}

/// Clear the registered notification callback.
///
/// After calling this, notifications will not trigger any callback.
pub fn clear_notification_callback() {
    let mut guard = NOTIFICATION_CALLBACK
        .lock()
        .expect("NOTIFICATION_CALLBACK mutex poisoned");
    *guard = None;
}

/// Clear the registered filesystem start callback.
pub fn clear_fs_start_callback() {
    let mut guard = FS_START_CALLBACK
        .lock()
        .expect("FS_START_CALLBACK mutex poisoned");
    *guard = None;
}

/// Clear all registered callbacks.
///
/// This is useful for cleanup or when reinitializing the library.
pub fn clear_all_callbacks() {
    clear_status_callback();
    clear_event_callback();
    clear_notification_callback();
    clear_fs_start_callback();
}

// ============================================================================
// Callback Configuration
// ============================================================================

/// Configuration builder for all callbacks.
///
/// This struct provides a convenient way to configure multiple callbacks
/// and then register them all at once.
///
/// # Example
///
/// ```ignore
/// let config = CallbackConfig::new()
///     .with_status_callback(|status| {
///         println!("Status: {}", status.status);
///     })
///     .with_event_callback(|event, data| {
///         println!("Event: {}", event);
///     });
///
/// let pointers = config.register();
///
/// // Pass pointers to C library
/// unsafe {
///     raw::psync_start_sync(pointers.status, pointers.event);
/// }
/// ```
#[derive(Default)]
pub struct CallbackConfig {
    status_callback: Option<StatusCallbackFn>,
    event_callback: Option<EventCallbackFn>,
    notification_callback: Option<NotificationCallbackFn>,
    fs_start_callback: Option<GenericCallbackFn>,
}

impl CallbackConfig {
    /// Create a new empty callback configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the status change callback.
    pub fn with_status_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&pstatus_t) + Send + 'static,
    {
        self.status_callback = Some(Box::new(callback));
        self
    }

    /// Set the event callback.
    pub fn with_event_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(psync_eventtype_t, psync_eventdata_t) + Send + 'static,
    {
        self.event_callback = Some(Box::new(callback));
        self
    }

    /// Set the notification callback.
    pub fn with_notification_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(u32, u32) + Send + 'static,
    {
        self.notification_callback = Some(Box::new(callback));
        self
    }

    /// Set the filesystem start callback.
    pub fn with_fs_start_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + 'static,
    {
        self.fs_start_callback = Some(Box::new(callback));
        self
    }

    /// Register all configured callbacks and return the C function pointers.
    ///
    /// This consumes the configuration and stores the callbacks in global storage.
    /// The returned `CallbackPointers` struct contains the C function pointers
    /// that can be passed to pclsync functions.
    ///
    /// # Note
    ///
    /// If a callback was not configured (None), the corresponding pointer
    /// in `CallbackPointers` will be `None`, which is typically valid to pass
    /// to pclsync (meaning no callback).
    pub fn register(self) -> CallbackPointers {
        // Register status callback if provided
        if let Some(callback) = self.status_callback {
            let mut guard = STATUS_CALLBACK
                .lock()
                .expect("STATUS_CALLBACK mutex poisoned");
            *guard = Some(callback);
        }

        // Register event callback if provided
        if let Some(callback) = self.event_callback {
            let mut guard = EVENT_CALLBACK
                .lock()
                .expect("EVENT_CALLBACK mutex poisoned");
            *guard = Some(callback);
        }

        // Register notification callback if provided
        if let Some(callback) = self.notification_callback {
            let mut guard = NOTIFICATION_CALLBACK
                .lock()
                .expect("NOTIFICATION_CALLBACK mutex poisoned");
            *guard = Some(callback);
        }

        // Register fs_start callback if provided
        if let Some(callback) = self.fs_start_callback {
            let mut guard = FS_START_CALLBACK
                .lock()
                .expect("FS_START_CALLBACK mutex poisoned");
            *guard = Some(callback);
        }

        // Return the C function pointers
        CallbackPointers {
            status: Some(status_callback_trampoline),
            event: Some(event_callback_trampoline),
            notification: Some(notification_callback_trampoline),
            fs_start: Some(fs_start_callback_trampoline),
        }
    }
}

/// Raw C function pointers to pass to pclsync library functions.
///
/// These pointers are the extern "C" trampoline functions that can be
/// passed directly to pclsync functions like `psync_start_sync`.
#[derive(Debug, Clone, Copy)]
pub struct CallbackPointers {
    /// Status change callback function pointer.
    pub status: pstatus_change_callback_t,

    /// Event callback function pointer.
    pub event: pevent_callback_t,

    /// Notification callback function pointer.
    pub notification: Option<unsafe extern "C" fn(u32, u32)>,

    /// Filesystem start callback function pointer.
    pub fs_start: psync_generic_callback_t,
}

impl Default for CallbackPointers {
    fn default() -> Self {
        Self {
            status: None,
            event: None,
            notification: None,
            fs_start: None,
        }
    }
}

// ============================================================================
// Application-Level Overlay Callbacks
// ============================================================================

/// Application-level overlay callbacks for interactive mode.
///
/// These are not C library callbacks, but Rust callbacks used by the
/// application for the interactive command loop (startcrypto, stopcrypto,
/// finalize, quit commands).
pub mod overlay {
    use std::sync::Mutex;

    /// Type alias for crypto start callback.
    /// Called when crypto is successfully started (unlocked).
    pub type CryptoStartCallbackFn = Box<dyn Fn() + Send + 'static>;

    /// Type alias for crypto stop callback.
    /// Called when crypto is successfully stopped (locked).
    pub type CryptoStopCallbackFn = Box<dyn Fn() + Send + 'static>;

    /// Type alias for finalize callback.
    /// Called when the finalize command is executed.
    pub type FinalizeCallbackFn = Box<dyn Fn() + Send + 'static>;

    /// Type alias for list callback.
    /// Called when the list command is executed.
    pub type ListCallbackFn = Box<dyn Fn() + Send + 'static>;

    /// Global storage for overlay callbacks.
    static CRYPTO_START_CALLBACK: Mutex<Option<CryptoStartCallbackFn>> = Mutex::new(None);
    static CRYPTO_STOP_CALLBACK: Mutex<Option<CryptoStopCallbackFn>> = Mutex::new(None);
    static FINALIZE_CALLBACK: Mutex<Option<FinalizeCallbackFn>> = Mutex::new(None);
    static LIST_CALLBACK: Mutex<Option<ListCallbackFn>> = Mutex::new(None);

    /// Register a callback for crypto start events.
    pub fn register_crypto_start_callback<F>(callback: F)
    where
        F: Fn() + Send + 'static,
    {
        let mut guard = CRYPTO_START_CALLBACK
            .lock()
            .expect("CRYPTO_START_CALLBACK mutex poisoned");
        *guard = Some(Box::new(callback));
    }

    /// Register a callback for crypto stop events.
    pub fn register_crypto_stop_callback<F>(callback: F)
    where
        F: Fn() + Send + 'static,
    {
        let mut guard = CRYPTO_STOP_CALLBACK
            .lock()
            .expect("CRYPTO_STOP_CALLBACK mutex poisoned");
        *guard = Some(Box::new(callback));
    }

    /// Register a callback for finalize events.
    pub fn register_finalize_callback<F>(callback: F)
    where
        F: Fn() + Send + 'static,
    {
        let mut guard = FINALIZE_CALLBACK
            .lock()
            .expect("FINALIZE_CALLBACK mutex poisoned");
        *guard = Some(Box::new(callback));
    }

    /// Register a callback for list events.
    pub fn register_list_callback<F>(callback: F)
    where
        F: Fn() + Send + 'static,
    {
        let mut guard = LIST_CALLBACK.lock().expect("LIST_CALLBACK mutex poisoned");
        *guard = Some(Box::new(callback));
    }

    /// Invoke the crypto start callback (if registered).
    pub fn invoke_crypto_start_callback() {
        if let Ok(guard) = CRYPTO_START_CALLBACK.lock() {
            if let Some(ref callback) = *guard {
                callback();
            }
        }
    }

    /// Invoke the crypto stop callback (if registered).
    pub fn invoke_crypto_stop_callback() {
        if let Ok(guard) = CRYPTO_STOP_CALLBACK.lock() {
            if let Some(ref callback) = *guard {
                callback();
            }
        }
    }

    /// Invoke the finalize callback (if registered).
    pub fn invoke_finalize_callback() {
        if let Ok(guard) = FINALIZE_CALLBACK.lock() {
            if let Some(ref callback) = *guard {
                callback();
            }
        }
    }

    /// Invoke the list callback (if registered).
    pub fn invoke_list_callback() {
        if let Ok(guard) = LIST_CALLBACK.lock() {
            if let Some(ref callback) = *guard {
                callback();
            }
        }
    }

    /// Clear all overlay callbacks.
    pub fn clear_all_overlay_callbacks() {
        if let Ok(mut guard) = CRYPTO_START_CALLBACK.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = CRYPTO_STOP_CALLBACK.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = FINALIZE_CALLBACK.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = LIST_CALLBACK.lock() {
            *guard = None;
        }
    }

    /// Configuration builder for overlay callbacks.
    #[derive(Default)]
    pub struct OverlayCallbackConfig {
        crypto_start: Option<CryptoStartCallbackFn>,
        crypto_stop: Option<CryptoStopCallbackFn>,
        finalize: Option<FinalizeCallbackFn>,
        list: Option<ListCallbackFn>,
    }

    impl OverlayCallbackConfig {
        /// Create a new empty overlay callback configuration.
        pub fn new() -> Self {
            Self::default()
        }

        /// Set the crypto start callback.
        pub fn with_crypto_start_callback<F>(mut self, callback: F) -> Self
        where
            F: Fn() + Send + 'static,
        {
            self.crypto_start = Some(Box::new(callback));
            self
        }

        /// Set the crypto stop callback.
        pub fn with_crypto_stop_callback<F>(mut self, callback: F) -> Self
        where
            F: Fn() + Send + 'static,
        {
            self.crypto_stop = Some(Box::new(callback));
            self
        }

        /// Set the finalize callback.
        pub fn with_finalize_callback<F>(mut self, callback: F) -> Self
        where
            F: Fn() + Send + 'static,
        {
            self.finalize = Some(Box::new(callback));
            self
        }

        /// Set the list callback.
        pub fn with_list_callback<F>(mut self, callback: F) -> Self
        where
            F: Fn() + Send + 'static,
        {
            self.list = Some(Box::new(callback));
            self
        }

        /// Register all configured overlay callbacks.
        pub fn register(self) {
            if let Some(callback) = self.crypto_start {
                let mut guard = CRYPTO_START_CALLBACK
                    .lock()
                    .expect("CRYPTO_START_CALLBACK mutex poisoned");
                *guard = Some(callback);
            }
            if let Some(callback) = self.crypto_stop {
                let mut guard = CRYPTO_STOP_CALLBACK
                    .lock()
                    .expect("CRYPTO_STOP_CALLBACK mutex poisoned");
                *guard = Some(callback);
            }
            if let Some(callback) = self.finalize {
                let mut guard = FINALIZE_CALLBACK
                    .lock()
                    .expect("FINALIZE_CALLBACK mutex poisoned");
                *guard = Some(callback);
            }
            if let Some(callback) = self.list {
                let mut guard = LIST_CALLBACK.lock().expect("LIST_CALLBACK mutex poisoned");
                *guard = Some(callback);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        #[test]
        fn test_overlay_callback_registration_and_invocation() {
            let called = Arc::new(AtomicBool::new(false));
            let called_clone = Arc::clone(&called);

            register_crypto_start_callback(move || {
                called_clone.store(true, Ordering::SeqCst);
            });

            invoke_crypto_start_callback();

            assert!(called.load(Ordering::SeqCst));

            // Clean up
            clear_all_overlay_callbacks();
        }

        #[test]
        fn test_overlay_config_builder() {
            let start_called = Arc::new(AtomicBool::new(false));
            let stop_called = Arc::new(AtomicBool::new(false));

            let start_clone = Arc::clone(&start_called);
            let stop_clone = Arc::clone(&stop_called);

            OverlayCallbackConfig::new()
                .with_crypto_start_callback(move || {
                    start_clone.store(true, Ordering::SeqCst);
                })
                .with_crypto_stop_callback(move || {
                    stop_clone.store(true, Ordering::SeqCst);
                })
                .register();

            invoke_crypto_start_callback();
            invoke_crypto_stop_callback();

            assert!(start_called.load(Ordering::SeqCst));
            assert!(stop_called.load(Ordering::SeqCst));

            // Clean up
            clear_all_overlay_callbacks();
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_status_callback_registration() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);

        register_status_callback(move |_status| {
            called_clone.store(true, Ordering::SeqCst);
        });

        // Verify callback is stored
        {
            let guard = STATUS_CALLBACK.lock().unwrap();
            assert!(guard.is_some());
        }

        // Clean up
        clear_status_callback();

        // Verify callback is cleared
        {
            let guard = STATUS_CALLBACK.lock().unwrap();
            assert!(guard.is_none());
        }
    }

    #[test]
    fn test_event_callback_registration() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);

        register_event_callback(move |_event_type, _event_data| {
            called_clone.store(true, Ordering::SeqCst);
        });

        // Verify callback is stored
        {
            let guard = EVENT_CALLBACK.lock().unwrap();
            assert!(guard.is_some());
        }

        // Clean up
        clear_event_callback();

        // Verify callback is cleared
        {
            let guard = EVENT_CALLBACK.lock().unwrap();
            assert!(guard.is_none());
        }
    }

    #[test]
    fn test_notification_callback_registration() {
        let count = Arc::new(AtomicU32::new(0));
        let count_clone = Arc::clone(&count);

        register_notification_callback(move |total, _new| {
            count_clone.store(total, Ordering::SeqCst);
        });

        // Verify callback is stored
        {
            let guard = NOTIFICATION_CALLBACK.lock().unwrap();
            assert!(guard.is_some());
        }

        // Clean up
        clear_notification_callback();
    }

    #[test]
    fn test_clear_all_callbacks() {
        // Register all callbacks
        register_status_callback(|_| {});
        register_event_callback(|_, _| {});
        register_notification_callback(|_, _| {});
        register_fs_start_callback(|| {});

        // Verify all are registered
        assert!(STATUS_CALLBACK.lock().unwrap().is_some());
        assert!(EVENT_CALLBACK.lock().unwrap().is_some());
        assert!(NOTIFICATION_CALLBACK.lock().unwrap().is_some());
        assert!(FS_START_CALLBACK.lock().unwrap().is_some());

        // Clear all
        clear_all_callbacks();

        // Verify all are cleared
        assert!(STATUS_CALLBACK.lock().unwrap().is_none());
        assert!(EVENT_CALLBACK.lock().unwrap().is_none());
        assert!(NOTIFICATION_CALLBACK.lock().unwrap().is_none());
        assert!(FS_START_CALLBACK.lock().unwrap().is_none());
    }

    #[test]
    fn test_callback_config_builder() {
        let status_called = Arc::new(AtomicBool::new(false));
        let event_called = Arc::new(AtomicBool::new(false));

        let status_clone = Arc::clone(&status_called);
        let event_clone = Arc::clone(&event_called);

        let config = CallbackConfig::new()
            .with_status_callback(move |_| {
                status_clone.store(true, Ordering::SeqCst);
            })
            .with_event_callback(move |_, _| {
                event_clone.store(true, Ordering::SeqCst);
            });

        let pointers = config.register();

        // Verify pointers are set
        assert!(pointers.status.is_some());
        assert!(pointers.event.is_some());

        // Clean up
        clear_all_callbacks();
    }

    #[test]
    fn test_status_trampoline_null_pointer() {
        // Should not panic when called with null
        unsafe {
            status_callback_trampoline(std::ptr::null_mut());
        }
    }

    #[test]
    fn test_status_trampoline_invokes_callback() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);

        register_status_callback(move |status| {
            // Verify we received valid data
            assert!(status.status <= 14); // Max status value
            called_clone.store(true, Ordering::SeqCst);
        });

        // Create a test status
        let mut status = pstatus_t::default();

        // Invoke the trampoline
        unsafe {
            status_callback_trampoline(&mut status);
        }

        assert!(called.load(Ordering::SeqCst));

        // Clean up
        clear_status_callback();
    }

    #[test]
    fn test_callback_panic_safety() {
        // Register a callback that panics
        register_status_callback(|_| {
            panic!("This panic should be caught!");
        });

        // Create a test status
        let mut status = pstatus_t::default();

        // Invoke the trampoline - should not panic the test
        unsafe {
            status_callback_trampoline(&mut status);
        }

        // If we got here, the panic was caught
        // Clean up
        clear_status_callback();
    }

    #[test]
    fn test_callback_pointers_default() {
        let pointers = CallbackPointers::default();
        assert!(pointers.status.is_none());
        assert!(pointers.event.is_none());
        assert!(pointers.notification.is_none());
        assert!(pointers.fs_start.is_none());
    }
}
