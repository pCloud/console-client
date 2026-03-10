//! Native crash handling via minidump generation and upload.
//!
//! Uses an out-of-process model: the main binary re-execs itself with a hidden
//! `--crash-monitor` flag to spawn a dedicated crash reporter process. That
//! process runs a `minidumper::Server` that listens for crash notifications
//! from the `crash_handler::CrashHandler` attached to the main process.
//! On crash, the reporter writes a minidump and uploads it to Bugsnag.
//!
//! The separate process is required on Linux because `minidump-writer` uses
//! `ptrace` to inspect the crashing thread, and the kernel does not allow
//! `ptrace` between threads in the same process.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use bugsnag::Bugsnag;

/// Bugsnag minidump upload endpoint.
const BUGSNAG_MINIDUMP_URL: &str = "https://notify.bugsnag.com/minidump";

/// Hidden CLI flag used to launch the crash monitor subprocess.
pub const CRASH_MONITOR_ARG: &str = "--crash-monitor";

/// Returns the directory used to queue crash dumps for deferred upload.
fn crash_dump_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("pcloud")
        .join("crashes")
}

/// Upload any minidump files left over from a previous crash where the
/// upload failed at crash time.
pub fn report_previous_crash_if_any() {
    let dir = crash_dump_dir();
    if !dir.exists() {
        return;
    }

    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("dmp") {
            match fs::read(&path) {
                Ok(contents) if !contents.is_empty() => {
                    if upload_minidump(&contents).is_ok() {
                        let _ = fs::remove_file(&path);
                    }
                }
                _ => {
                    // Remove empty or unreadable dump files
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}

/// Install the native crash handler.
///
/// Re-execs the current binary as a dedicated crash reporter child process,
/// then attaches a `crash_handler::CrashHandler` in the main process that
/// will notify the reporter on SIGSEGV, SIGABRT, SIGBUS, SIGFPE.
pub fn install(_client: Arc<Bugsnag>) {
    // Ensure the crash dump directory exists
    let dump_dir = crash_dump_dir();
    let _ = fs::create_dir_all(&dump_dir);

    let socket_name = format!("pcloud-crash-{}", std::process::id());

    // Spawn the crash reporter as a child process
    let child = match spawn_monitor_process(&socket_name, &dump_dir) {
        Some(child) => child,
        None => {
            eprintln!("Warning: failed to spawn crash reporter process");
            return;
        }
    };

    let child_pid = child.id();

    // Give the server a moment to start listening
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Connect the crash handler client and attach signal handlers
    if let Ok(ipc_client) =
        minidumper::Client::with_name(minidumper::SocketName::path(&socket_name))
    {
        let crash_event = unsafe {
            crash_handler::make_crash_event(move |context: &crash_handler::CrashContext| {
                let _ = ipc_client.request_dump(context);
                crash_handler::CrashEventResult::Handled(true)
            })
        };

        match crash_handler::CrashHandler::attach(crash_event) {
            Ok(handler) => {
                // Allow the crash reporter child process to ptrace us.
                // Required when /proc/sys/kernel/yama/ptrace_scope >= 1
                // (the default on most Linux distros).
                #[cfg(target_os = "linux")]
                handler.set_ptracer(Some(child_pid));

                // Leak the handler so it stays active for the process lifetime.
                // The child process will exit when we disconnect (on_client_disconnected).
                std::mem::forget(handler);
            }
            Err(e) => {
                eprintln!("Warning: failed to attach crash handler: {}", e);
            }
        }
    } else {
        eprintln!("Warning: failed to connect to crash reporter process");
    }

    // Leak the child handle — we don't want to wait on it and the child
    // will exit on its own when the IPC client disconnects.
    std::mem::forget(child);
}

/// Spawn a child process running in crash monitor mode.
fn spawn_monitor_process(socket_name: &str, dump_dir: &Path) -> Option<Child> {
    let exe = std::env::current_exe().ok()?;

    Command::new(exe)
        .arg(CRASH_MONITOR_ARG)
        .arg(socket_name)
        .arg(dump_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit()) // Keep stderr for warnings
        .spawn()
        .ok()
}

/// Entry point for the crash monitor child process.
///
/// Called from `main()` when `--crash-monitor` is detected. This function
/// sets the process name for visibility in `ps`/`top`, then runs the
/// minidumper server loop until the main process disconnects or crashes.
pub fn run_monitor(socket_name: &str, dump_dir: &str) {
    // Set a descriptive process name visible in ps/top/htop
    set_process_name("pcloud [crash-reporter]");

    let dump_dir = PathBuf::from(dump_dir);
    let _ = fs::create_dir_all(&dump_dir);

    let shutdown = AtomicBool::new(false);
    run_monitor_server(socket_name, &dump_dir, &shutdown);
}

/// Set the process name shown by `ps`, `top`, `htop`, etc.
#[cfg(target_os = "linux")]
fn set_process_name(name: &str) {
    use std::ffi::CString;
    if let Ok(c_name) = CString::new(name) {
        // PR_SET_NAME has a 16-byte (including NUL) limit, but prctl will
        // silently truncate. For /proc/self/comm this is fine.
        unsafe {
            libc::prctl(libc::PR_SET_NAME, c_name.as_ptr());
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn set_process_name(_name: &str) {
    // No-op on other platforms; the process is still identifiable by its
    // command-line arguments in ps.
}

/// Run the minidumper server loop on the monitor process.
fn run_monitor_server(socket_name: &str, dump_dir: &Path, shutdown: &AtomicBool) {
    let dump_dir = dump_dir.to_path_buf();

    struct Handler {
        dump_dir: PathBuf,
    }

    impl minidumper::ServerHandler for Handler {
        fn create_minidump_file(&self) -> Result<(fs::File, PathBuf), std::io::Error> {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let path = self.dump_dir.join(format!("crash-{}.dmp", timestamp));
            let file = fs::File::create(&path)?;
            Ok((file, path))
        }

        fn on_minidump_created(
            &self,
            result: Result<minidumper::MinidumpBinary, minidumper::Error>,
        ) -> minidumper::LoopAction {
            match result {
                Ok(minidump) => {
                    // Try to upload; if upload succeeds, remove the file
                    let contents = minidump.contents.or_else(|| fs::read(&minidump.path).ok());

                    if let Some(data) = contents {
                        if upload_minidump(&data).is_ok() {
                            let _ = fs::remove_file(&minidump.path);
                        }
                        // If upload fails, the file stays for retry on next startup
                    }
                }
                Err(e) => {
                    eprintln!("Warning: minidump creation failed: {}", e);
                }
            }

            // Exit after handling the crash — the main process is about to terminate
            minidumper::LoopAction::Exit
        }

        fn on_message(&self, _kind: u32, _buffer: Vec<u8>) {
            // No custom messages expected
        }

        fn on_client_disconnected(&self, num_clients: usize) -> minidumper::LoopAction {
            if num_clients == 0 {
                minidumper::LoopAction::Exit
            } else {
                minidumper::LoopAction::Continue
            }
        }
    }

    let handler = Handler { dump_dir };

    let server = minidumper::Server::with_name(minidumper::SocketName::path(socket_name));
    match server {
        Ok(mut server) => {
            let _ = server.run(Box::new(handler), shutdown, None);
        }
        Err(e) => {
            eprintln!("Warning: failed to start crash monitor server: {}", e);
        }
    }
}

/// Upload a minidump file to Bugsnag's minidump endpoint.
fn upload_minidump(data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let api_key = super::config::api_key();
    let url = format!("{}?apiKey={}", BUGSNAG_MINIDUMP_URL, api_key);

    // Build a simple multipart body manually
    let boundary = "----pcloud-crash-boundary";
    let mut body = Vec::new();
    write!(body, "--{}\r\n", boundary)?;
    write!(
        body,
        "Content-Disposition: form-data; name=\"upload_file_minidump\"; filename=\"crash.dmp\"\r\n"
    )?;
    write!(body, "Content-Type: application/octet-stream\r\n\r\n")?;
    body.extend_from_slice(data);
    write!(body, "\r\n--{}--\r\n", boundary)?;

    let content_type = format!("multipart/form-data; boundary={}", boundary);

    ureq::post(&url).content_type(&content_type).send(&body)?;

    Ok(())
}
