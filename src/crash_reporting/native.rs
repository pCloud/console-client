//! Native crash handling via minidump generation and upload.
//!
//! Uses an out-of-process model: a monitor thread runs a `minidumper::Server`
//! that listens for crash notifications from the `crash_handler::CrashHandler`
//! attached to the main process. On crash, the monitor thread writes a minidump
//! file and uploads it to Bugsnag's minidump endpoint.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use bugsnag::Bugsnag;

/// Bugsnag minidump upload endpoint.
const BUGSNAG_MINIDUMP_URL: &str = "https://notify.bugsnag.com/minidump";

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
                Ok(contents) => {
                    if upload_minidump(&contents).is_ok() {
                        let _ = fs::remove_file(&path);
                    }
                }
                Err(_) => {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}

/// Install the native crash handler.
///
/// Spawns a monitor thread running a `minidumper::Server`, then attaches a
/// `crash_handler::CrashHandler` in the current process that will notify
/// the monitor on SIGSEGV, SIGABRT, SIGBUS, SIGFPE.
pub fn install(_client: Arc<Bugsnag>) {
    // Ensure the crash dump directory exists
    let dump_dir = crash_dump_dir();
    let _ = fs::create_dir_all(&dump_dir);

    let socket_name = format!("pcloud-crash-{}", std::process::id());

    // Spawn the monitor thread with the minidumper server
    let server_socket_name = socket_name.clone();
    let server_dump_dir = dump_dir.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = shutdown.clone();

    std::thread::Builder::new()
        .name("crash-monitor".into())
        .spawn(move || {
            run_monitor_server(&server_socket_name, &server_dump_dir, &server_shutdown);
        })
        .expect("failed to spawn crash monitor thread");

    // Give the server a moment to start listening
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Connect the crash handler client and attach signal handlers
    let client_socket_name = socket_name;
    if let Ok(ipc_client) =
        minidumper::Client::with_name(minidumper::SocketName::path(&client_socket_name))
    {
        // The crash event handler: on crash, request a minidump from the monitor
        let crash_event = unsafe {
            crash_handler::make_crash_event(move |context: &crash_handler::CrashContext| {
                let _ = ipc_client.request_dump(context);
                crash_handler::CrashEventResult::Handled(true)
            })
        };

        match crash_handler::CrashHandler::attach(crash_event) {
            Ok(handler) => {
                // Leak the handler so it stays active for the process lifetime
                std::mem::forget(handler);
            }
            Err(e) => {
                eprintln!("Warning: failed to attach crash handler: {}", e);
            }
        }
    } else {
        eprintln!("Warning: failed to connect crash handler to monitor");
    }
}

/// Run the minidumper server loop on the monitor thread.
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

            // Exit after handling the crash — the process is about to terminate
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
        "Content-Disposition: form-data; name=\"minidump\"; filename=\"crash.dmp\"\r\n"
    )?;
    write!(body, "Content-Type: application/octet-stream\r\n\r\n")?;
    body.extend_from_slice(data);
    write!(body, "\r\n--{}--\r\n", boundary)?;

    let content_type = format!("multipart/form-data; boundary={}", boundary);

    ureq::post(&url).content_type(&content_type).send(&body)?;

    Ok(())
}
