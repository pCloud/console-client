//! Crash reporting integration with Bugsnag.
//!
//! When the `crash-reporting` feature is enabled, this module provides:
//! - Rust panic reporting to Bugsnag
//! - Native crash (SIGSEGV, SIGABRT, SIGBUS, SIGFPE) reporting via minidumps
//! - Queued crash dump upload for failed-at-crash-time uploads
//!
//! When the feature is disabled, all public functions are no-ops.

#[cfg(feature = "crash-reporting")]
mod config;
#[cfg(feature = "crash-reporting")]
mod native;
#[cfg(feature = "crash-reporting")]
mod panic_hook;

/// Initialize crash reporting.
///
/// This should be called as the very first thing in `main()`, before any code
/// that could panic or crash. It:
/// 1. Checks for and uploads any queued crash dumps from previous runs
/// 2. Installs the Rust panic hook for Bugsnag reporting
/// 3. Installs the native signal handler for minidump generation
///
/// When `crash-reporting` is not enabled, this is a no-op.
pub fn init() {
    #[cfg(feature = "crash-reporting")]
    {
        native::report_previous_crash_if_any();
        let client = config::create_client();
        panic_hook::install(client.clone());
        native::install(client);
    }
}

/// Report a non-fatal error to Bugsnag.
///
/// When `crash-reporting` is not enabled, this is a no-op.
pub fn notify_error(_error: &dyn std::error::Error) {
    #[cfg(feature = "crash-reporting")]
    config::with_client(|client| {
        let _ = client
            .notify("Error", &_error.to_string())
            .severity(bugsnag::Severity::Error)
            .send();
    });
}

/// Return the application version string.
///
/// Includes profile suffix: `-dev` for debug builds, `-qa` for QA builds.
pub fn app_version() -> &'static str {
    env!("PCLOUD_VERSION")
}
