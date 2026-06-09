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

/// Hidden CLI argument used to launch the crash reporter subprocess.
///
/// When this is the first argument, `main()` should call [`run_monitor`]
/// instead of the normal application flow.
#[cfg(feature = "crash-reporting")]
pub const CRASH_MONITOR_ARG: &str = native::CRASH_MONITOR_ARG;

/// Check whether the process was launched as a crash monitor subprocess.
///
/// Returns `Some((socket_name, dump_dir))` if `--crash-monitor` was passed,
/// `None` otherwise. This must be called **before** [`init`] and before
/// clap argument parsing.
pub fn check_monitor_args() -> Option<(String, String)> {
    #[cfg(feature = "crash-reporting")]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.len() >= 4 && args[1] == native::CRASH_MONITOR_ARG {
            return Some((args[2].clone(), args[3].clone()));
        }
    }
    None
}

/// Run as a crash monitor subprocess.
///
/// This is the entry point for the child process spawned by [`init`].
/// It runs the minidumper server until the parent disconnects or crashes.
/// This function does **not** return under normal operation.
pub fn run_monitor(_socket_name: &str, _dump_dir: &str) {
    #[cfg(feature = "crash-reporting")]
    native::run_monitor(_socket_name, _dump_dir);
}

/// Initialize crash reporting.
///
/// This should be called as the very first thing in `main()`, before any code
/// that could panic or crash. It:
/// 1. Checks for and uploads any queued crash dumps from previous runs
/// 2. Installs the Rust panic hook for Bugsnag reporting
///
/// It deliberately does **not** install the native (signal) crash handler:
/// that is fork-sensitive and must be set up in the process that actually runs
/// the pclsync engine, after any daemonization fork. Call
/// [`install_native_handler`] from those processes once they are past the
/// fork. The Rust panic hook installed here, by contrast, survives `fork`
/// unchanged and so covers every process.
///
/// When `crash-reporting` is not enabled, this is a no-op.
pub fn init() {
    #[cfg(feature = "crash-reporting")]
    {
        native::report_previous_crash_if_any();
        let client = config::create_client();
        panic_hook::install(client);
    }
}

/// Install the native (signal) crash handler for this process.
///
/// Spawns the out-of-process crash reporter and attaches handlers for SIGSEGV,
/// SIGABRT, SIGBUS and SIGFPE so native crashes (including those originating in
/// the pclsync C library) are captured as minidumps and uploaded to Bugsnag.
///
/// This **must** be called from the process that should be monitored, *after*
/// any `fork`/daemonization. In particular the background daemon
/// (`pcloud-cli start`) double-forks via the `daemonize` crate, which reparents
/// the daemon away from the process that called [`init`]; installing the native
/// handler before that fork would leave the reporter unable to `ptrace` the
/// daemon (the `PR_SET_PTRACER` declaration and the reporter's parent link are
/// both lost across the fork). It is therefore installed by the long-running
/// engine entry points — foreground `mount`, and the daemon/foreground body of
/// `start` once it is past `daemonize()`.
///
/// Idempotent and a no-op when `crash-reporting` is not enabled.
pub fn install_native_handler() {
    #[cfg(feature = "crash-reporting")]
    native::install();
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
