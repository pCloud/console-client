//! pCloud Console Client - Rust Rewrite
//!
//! Hierarchical CLI on top of the pclsync C library.
//!
//! # Usage
//!
//! ```text
//! pcloud-cli                          # launch interactive TUI dashboard
//! pcloud-cli auth   login|logout|status|unlink
//! pcloud-cli mount  [PATH] [--token TOKEN]
//! pcloud-cli start  [PATH] [--token TOKEN]
//! pcloud-cli stop
//! pcloud-cli status
//! pcloud-cli crypto start|stop|status
//! pcloud-cli backup add|list|remove|status|stop-device|root-name
//! pcloud-cli tui
//! pcloud-cli doctor
//! ```

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::{CommandFactory, Parser};
use secrecy::{ExposeSecret, SecretString};

use console_client::cli::{
    print_cli_auth_help, prompt_auth_choice, prompt_confirm, prompt_token, resolve_mountpoint,
    AuthArgs, AuthChoice, AuthOp, BackupArgs, BackupOp, Cli, Command, CryptoArgs, CryptoOp,
    MountArgs, StartArgs,
};
use console_client::daemon::{
    is_daemon_running, DaemonClient, DaemonCommand, DaemonConfig, DaemonResponse,
};
use console_client::error::{AuthError, DaemonError, PCloudError};
use console_client::ffi::events::{describe_event, now_hms};
use console_client::ffi::{
    event_callback_trampoline, register_event_callback, register_status_callback,
    status_callback_trampoline, status_to_string,
};
use console_client::security::{prompt_for_password, resolve_auth_token, resolve_crypto_password};
use console_client::utils::browser::{has_display, open_url};
use console_client::utils::qrcode::{can_display_qr, generate_qr_code};
use console_client::utils::terminal::{print_boxed, print_status, StatusIndicator};
use console_client::wrapper::{PCloudClient, WebLoginConfig};
use console_client::Result;

/// Global shutdown flag for signal handling.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Application entry point.
fn main() -> ExitCode {
    // Check if we were launched as a crash reporter subprocess.
    if let Some((socket_name, dump_dir)) = console_client::crash_reporting::check_monitor_args() {
        console_client::crash_reporting::run_monitor(&socket_name, &dump_dir);
        return ExitCode::SUCCESS;
    }

    console_client::crash_reporting::init();

    let cli = Cli::parse();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            console_client::crash_reporting::notify_error(&e);
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Dispatch based on the parsed subcommand.
fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None | Some(Command::Tui) => run_tui_mode(),
        Some(Command::Doctor) => console_client::utils::deps::run_doctor(),
        Some(Command::Auth(AuthArgs { op: Some(op) })) => run_auth_subcommand(op),
        Some(Command::Auth(AuthArgs { op: None })) => print_subcommand_help("auth"),
        Some(Command::Mount(args)) => run_mount_subcommand(args),
        Some(Command::Start(args)) => run_start_subcommand(args),
        Some(Command::Stop) => run_stop_subcommand(),
        Some(Command::Status) => run_status_subcommand(),
        Some(Command::Crypto(CryptoArgs { op: Some(op) })) => run_crypto_subcommand(op),
        Some(Command::Crypto(CryptoArgs { op: None })) => print_subcommand_help("crypto"),
        Some(Command::Backup(BackupArgs { op: Some(op) })) => run_backup_subcommand(op),
        Some(Command::Backup(BackupArgs { op: None })) => print_subcommand_help("backup"),
    }
}

/// Render the help text for a top-level subcommand group on stdout and exit 0.
///
/// Bare invocations like `pcloud-cli auth` reach this path because each parent
/// `*Args` struct declares its `op` as `Option<...>` (a missing subcommand is a
/// valid parse). We render via clap's own help formatter so the output matches
/// `--help` exactly, including the `pcloud-cli <group>` prefix in the usage
/// line (which is why we `build()` the top-level command first).
fn print_subcommand_help(name: &str) -> Result<()> {
    let mut cmd = Cli::command();
    cmd.build();
    if let Some(sub) = cmd.find_subcommand_mut(name) {
        let _ = sub.print_help();
        let _ = std::io::stdout().flush();
        println!();
    }
    Ok(())
}

// ============================================================================
// auth subcommands
// ============================================================================

fn run_auth_subcommand(op: AuthOp) -> Result<()> {
    match op {
        AuthOp::Login { token } => run_auth_login(token),
        AuthOp::Logout => run_auth_logout(),
        AuthOp::Status => run_auth_status(),
        AuthOp::Unlink { yes } => run_auth_unlink(yes),
    }
}

fn run_auth_login(token: Option<String>) -> Result<()> {
    print_status(StatusIndicator::Info, "Initializing pCloud client...");
    let client = PCloudClient::init()?;

    match token {
        Some(t) => {
            let secret = SecretString::from(t);
            let mut guard = client
                .lock()
                .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
            guard.set_auth_token(&secret, true)?;
            drop(guard);
            print_status(StatusIndicator::Success, "Auth token saved.");
        }
        None => handle_interactive_auth(&client, true)?,
    }
    Ok(())
}

fn run_auth_logout() -> Result<()> {
    print_status(StatusIndicator::Info, "Initializing pCloud client...");
    let client = PCloudClient::init()?;

    let mut guard = client
        .lock()
        .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

    if !guard.has_saved_credentials() {
        print_status(StatusIndicator::Info, "No saved credentials found.");
        return Ok(());
    }

    guard.logout();
    print_status(
        StatusIndicator::Success,
        "Logged out. Saved credentials cleared.",
    );
    Ok(())
}

fn run_auth_status() -> Result<()> {
    let client = PCloudClient::init()?;
    let guard = client
        .lock()
        .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
    if guard.has_saved_credentials() {
        println!("Logged in (saved credentials present).");
    } else {
        println!("Not logged in.");
    }
    Ok(())
}

fn run_auth_unlink(yes: bool) -> Result<()> {
    print_status(StatusIndicator::Info, "Initializing pCloud client...");
    let client = PCloudClient::init()?;

    if !yes {
        let confirmed = prompt_confirm(
            "This will remove all saved credentials and local sync data. Continue?",
        )?;
        if !confirmed {
            print_status(StatusIndicator::Info, "Unlink cancelled.");
            return Ok(());
        }
    }

    let mut guard = client
        .lock()
        .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
    guard.unlink();
    print_status(
        StatusIndicator::Success,
        "Account unlinked. All local data cleared.",
    );
    Ok(())
}

// ============================================================================
// mount (foreground)
// ============================================================================

fn run_mount_subcommand(args: MountArgs) -> Result<()> {
    setup_signal_handler()?;

    print_status(StatusIndicator::Info, "Initializing pCloud client...");
    let client = PCloudClient::init()?;

    register_status_callback(|status| {
        if status.status != 0 {
            eprintln!("Status: {}", status_to_string(status.status));
        }
        if status.filestodownload > 0 || status.filestoupload > 0 {
            eprintln!(
                "  Files to download: {}, Files to upload: {}",
                status.filestodownload, status.filestoupload
            );
        }
    });

    let env_token = resolve_auth_token()?;
    apply_auth(
        &client,
        args.token.as_deref(),
        env_token,
        /* save = */ true,
    )?;

    let mountpoint = resolve_mountpoint(args.path.as_deref());
    {
        let mut guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        console_client::utils::ensure_mountpoint(&mountpoint)?;
        guard.set_fs_root(&mountpoint)?;
        print_status(
            StatusIndicator::Info,
            &format!("Filesystem root set to: {}", mountpoint.display()),
        );
    }

    {
        let mut guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        guard.start_sync(Some(status_callback_trampoline), None);
        print_status(StatusIndicator::Info, "Sync started");
        guard.refresh_mount_state();
        print_status(
            StatusIndicator::Success,
            &format!("Mounted pCloud at: {}", mountpoint.display()),
        );
    }

    println!("\npCloud client running. Press Ctrl+C to exit.");
    wait_for_signal()?;
    println!("\nShutting down...");
    Ok(())
}

// ============================================================================
// start (daemon, background)
// ============================================================================

fn run_start_subcommand(args: StartArgs) -> Result<()> {
    use console_client::daemon::{
        cleanup_pid_file, daemonize, is_reload_requested, is_shutdown_requested,
        setup_daemon_signals,
    };

    let config = DaemonConfig::default();

    if is_daemon_running(&config) {
        eprintln!("Error: pCloud daemon is already running.");
        eprintln!("PID file: {}", config.pid_file.display());
        if let Some(pid) = console_client::daemon::get_daemon_pid(&config) {
            eprintln!("Running PID: {}", pid);
        }
        return Err(PCloudError::Daemon(DaemonError::AlreadyRunning));
    }

    print_status(StatusIndicator::Info, "Initializing pCloud client...");
    let client = PCloudClient::init()?;

    let env_token = resolve_auth_token()?;

    // Pre-fork: only run the interactive auth flow if absolutely necessary
    // (the controlling terminal is gone after fork). For CLI / env-supplied
    // tokens we DEFER `set_auth_token` until after the fork — `psync_set_auth`
    // touches the SQLite settings table, and SQLite handles are not fork-safe.
    let deferred_token: Option<SecretString> = if let Some(t) = args.token.as_deref() {
        Some(SecretString::from(t.to_string()))
    } else if let Some(t) = env_token {
        Some(t)
    } else {
        let guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        let has_creds = guard.has_saved_credentials();
        drop(guard);
        if !has_creds && !args.allow_unauthenticated {
            print_status(
                StatusIndicator::Info,
                "Authentication required before daemon can start",
            );
            handle_interactive_auth(&client, /* save = */ true)?;
        }
        // When `allow_unauthenticated` is set we start the engine with no
        // credentials; it idles in LOGIN_REQUIRED until a client drives login
        // over IPC (AuthBeginWeb / SetAuthToken).
        None
    };
    // Whether the deferred token came from a CLI flag (persist) or from the env
    // (ephemeral, never persisted). `args.token` having a value wins.
    let save_deferred_token = args.token.is_some();

    let mountpoint = resolve_mountpoint(args.path.as_deref());
    println!();
    print_status(StatusIndicator::Info, "Starting pCloud daemon...");
    println!("Mountpoint:   {}", mountpoint.display());
    println!("PID file:     {}", config.pid_file.display());
    println!("Socket path:  {}", config.socket_path().display());

    daemonize(&config)?;

    // -- We are now running in the daemon child process. --

    setup_daemon_signals()?;

    // Capture the live file-event stream into a ring buffer so IPC clients (the
    // TUI) can poll recent activity. Status itself is polled on demand via
    // `psync_get_status`, so the status callback stays a no-op.
    let activity = std::sync::Arc::new(console_client::daemon::activity::ActivityLog::new());
    register_status_callback(|_status| {});
    {
        let activity = activity.clone();
        register_event_callback(move |event_type, event_data| {
            if let Some((description, is_error)) = describe_event(event_type, event_data) {
                activity.push(now_hms(), description, is_error);
            }
        });
    }

    // Apply the deferred CLI/env token post-fork so the SQLite write happens
    // in the child's address space.
    if let Some(ref token) = deferred_token {
        let mut guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        guard.set_auth_token(token, save_deferred_token)?;
    }

    {
        let mut guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        console_client::utils::ensure_mountpoint(&mountpoint)?;
        guard.set_fs_root(&mountpoint)?;
    }

    {
        let mut guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        guard.start_sync(
            Some(status_callback_trampoline),
            Some(event_callback_trampoline),
        );
        guard.refresh_mount_state();
    }

    let server = console_client::daemon::DaemonServer::new(config.socket_path())?;
    let ctx = console_client::daemon::ipc::DaemonContext::new(client.clone(), activity);
    let ipc_thread = std::thread::spawn(move || {
        if let Err(e) = server.run(ctx) {
            eprintln!("IPC server error: {}", e);
        }
    });

    while !is_shutdown_requested() {
        if is_reload_requested() {
            // No configuration to reload yet.
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let _ = ipc_thread.join();
    cleanup_pid_file(&config);
    Ok(())
}

// ============================================================================
// stop
// ============================================================================

fn run_stop_subcommand() -> Result<()> {
    let config = DaemonConfig::default();
    let daemon_client = DaemonClient::new(config.socket_path());

    if !daemon_client.is_daemon_alive() {
        println!("No pCloud daemon is running.");
        return Ok(());
    }

    let response = daemon_client.send_command(DaemonCommand::Finalize)?;
    print_daemon_response(&response);
    if matches!(response, DaemonResponse::Error(_)) {
        return Err(PCloudError::Daemon(DaemonError::CommandFailed(
            response.to_string(),
        )));
    }
    Ok(())
}

// ============================================================================
// status
// ============================================================================

fn run_status_subcommand() -> Result<()> {
    let config = DaemonConfig::default();
    let daemon_client = DaemonClient::new(config.socket_path());

    if !daemon_client.is_daemon_alive() {
        ensure_daemon_running(&config, &daemon_client, false)?;
    }

    let response = daemon_client.send_command(DaemonCommand::Status)?;
    print_daemon_response(&response);
    Ok(())
}

// ============================================================================
// crypto subcommands
// ============================================================================

fn run_crypto_subcommand(op: CryptoOp) -> Result<()> {
    let config = DaemonConfig::default();
    let daemon_client = DaemonClient::new(config.socket_path());

    match op {
        CryptoOp::Start { password_file } => {
            let password = resolve_crypto_password_for_start(password_file.as_deref())?;
            ensure_daemon_running(&config, &daemon_client, false)?;
            let response = daemon_client.send_command(DaemonCommand::StartCrypto {
                password: Some(password.expose_secret().to_string()),
            })?;
            drop(password);
            print_daemon_response(&response);
            if matches!(response, DaemonResponse::Error(_)) {
                return Err(PCloudError::Daemon(DaemonError::CommandFailed(
                    response.to_string(),
                )));
            }
            Ok(())
        }
        CryptoOp::Stop => {
            ensure_daemon_running(&config, &daemon_client, false)?;
            let response = daemon_client.send_command(DaemonCommand::StopCrypto)?;
            print_daemon_response(&response);
            if matches!(response, DaemonResponse::Error(_)) {
                return Err(PCloudError::Daemon(DaemonError::CommandFailed(
                    response.to_string(),
                )));
            }
            Ok(())
        }
        CryptoOp::Status => {
            ensure_daemon_running(&config, &daemon_client, false)?;
            let response = daemon_client.send_command(DaemonCommand::Status)?;
            if let DaemonResponse::Status { crypto_started, .. } = response {
                if crypto_started {
                    println!("Crypto: started (encrypted folders unlocked)");
                } else {
                    println!("Crypto: stopped (encrypted folders locked)");
                }
                Ok(())
            } else {
                print_daemon_response(&response);
                Ok(())
            }
        }
    }
}

fn resolve_crypto_password_for_start(password_file: Option<&Path>) -> Result<SecretString> {
    if let Some(env_pwd) = resolve_crypto_password()? {
        return Ok(env_pwd);
    }
    if let Some(file) = password_file {
        let raw = std::fs::read_to_string(file).map_err(PCloudError::Io)?;
        let trimmed = raw.trim_end_matches(['\n', '\r']).to_string();
        if trimmed.is_empty() {
            return Err(PCloudError::InvalidArgument(format!(
                "Crypto password file {} is empty",
                file.display()
            )));
        }
        return Ok(SecretString::from(trimmed));
    }
    prompt_for_password("Crypto password: ").map_err(PCloudError::Io)
}

// ============================================================================
// backup subcommands (auto-start daemon, send IPC)
// ============================================================================

fn run_backup_subcommand(op: BackupOp) -> Result<()> {
    let config = DaemonConfig::default();
    let daemon_client = DaemonClient::new(config.socket_path());

    let cmd = backup_op_to_daemon_command(&op);

    if !daemon_client.is_daemon_alive() {
        ensure_daemon_running(&config, &daemon_client, false)?;
    }

    let response = daemon_client.send_command(cmd)?;
    print_daemon_response(&response);
    if matches!(response, DaemonResponse::Error(_)) {
        return Err(PCloudError::Daemon(DaemonError::CommandFailed(
            response.to_string(),
        )));
    }
    Ok(())
}

fn backup_op_to_daemon_command(op: &BackupOp) -> DaemonCommand {
    match op {
        BackupOp::Add { path } => DaemonCommand::BackupCreate {
            path: path.to_string_lossy().into_owned(),
        },
        BackupOp::List => DaemonCommand::BackupList,
        BackupOp::Remove { id } => DaemonCommand::BackupRemove { sync_id: *id },
        BackupOp::StopDevice => DaemonCommand::BackupStopDevice,
        BackupOp::Status { id } => DaemonCommand::BackupStatus { sync_id: *id },
        BackupOp::RootName => DaemonCommand::BackupRootName,
    }
}

// ============================================================================
// TUI mode (default / explicit)
// ============================================================================

fn run_tui_mode() -> Result<()> {
    // The TUI is a pure IPC client: it never initializes a pclsync engine of
    // its own. Ensure a daemon is running (auto-starting an unauthenticated one
    // on first run), then drive everything — including login — over IPC.
    let config = DaemonConfig::default();
    let daemon_client = DaemonClient::new(config.socket_path());
    ensure_daemon_running(
        &config,
        &daemon_client,
        /* allow_unauthenticated = */ true,
    )?;

    let cli_for_tui = Cli::default();
    console_client::tui::run(daemon_client, &cli_for_tui)
}

// ============================================================================
// Shared auth helpers
// ============================================================================

/// Apply auth via CLI token, env token, or saved credentials.
///
/// Token resolution order: `cli_token` > `env_token` > saved credentials >
/// interactive prompt. `save` controls whether a CLI-supplied token is
/// persisted; env-sourced tokens are never persisted regardless.
fn apply_auth(
    client: &Arc<Mutex<PCloudClient>>,
    cli_token: Option<&str>,
    env_token: Option<SecretString>,
    save: bool,
) -> Result<()> {
    if let Some(token) = cli_token {
        let mut guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        guard.set_auth_token(&SecretString::from(token.to_string()), save)?;
        return Ok(());
    }
    if let Some(token) = env_token {
        let mut guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        // Env-sourced tokens are never persisted.
        guard.set_auth_token(&token, false)?;
        return Ok(());
    }
    {
        let guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        if guard.has_saved_credentials() {
            return Ok(());
        }
    }
    handle_interactive_auth(client, save)
}

fn handle_interactive_auth(
    client: &Arc<Mutex<PCloudClient>>,
    save_credentials: bool,
) -> Result<()> {
    loop {
        match prompt_auth_choice()? {
            AuthChoice::WebLogin => {
                handle_web_login(client, save_credentials)?;
                return Ok(());
            }
            AuthChoice::EnterToken => {
                let token = prompt_token()?;
                let mut guard = client.lock().map_err(|_| {
                    PCloudError::Config("Failed to acquire client lock".to_string())
                })?;
                guard.set_auth_token(&token, save_credentials)?;
                drop(guard);
                print_status(StatusIndicator::Success, "Authentication successful!");
                return Ok(());
            }
            AuthChoice::ShowCliHelp => {
                print_cli_auth_help();
            }
            AuthChoice::Cancel => {
                return Err(PCloudError::Auth(AuthError::Other(
                    "Authentication cancelled by user".to_string(),
                )));
            }
        }
    }
}

fn handle_web_login(client: &Arc<Mutex<PCloudClient>>, save_credentials: bool) -> Result<()> {
    print_status(StatusIndicator::Info, "Initiating web-based login...");
    let session = {
        let mut guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        guard.initiate_web_login(&WebLoginConfig::default())?
    };

    println!();
    print_boxed(&["Open this URL in your browser:", "", &session.login_url]);

    if can_display_qr() {
        if let Ok(qr) = generate_qr_code(&session.login_url) {
            println!();
            println!("{}", qr);
        }
    }

    if has_display() {
        match open_url(&session.login_url, false) {
            Ok(true) => print_status(StatusIndicator::Success, "Browser opened automatically"),
            Ok(false) => print_status(
                StatusIndicator::Warning,
                "Could not find browser - please copy the URL",
            ),
            Err(_) => print_status(
                StatusIndicator::Warning,
                "Could not open browser - please copy the URL",
            ),
        }
    } else {
        print_status(
            StatusIndicator::Info,
            "No display detected - please copy the URL above",
        );
    }

    println!();
    print_status(
        StatusIndicator::Progress,
        "Waiting for authentication (timeout: 5 min)...",
    );

    {
        let mut guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        guard.wait_for_web_auth(&session.request_id)?;
        if save_credentials {
            guard.save_current_auth_token()?;
        }
    }

    print_status(StatusIndicator::Success, "Authentication successful!");
    Ok(())
}

// ============================================================================
// Daemon auto-start + IPC helpers
// ============================================================================

/// Ensure a daemon is alive on the per-UID socket.
///
/// If one is already running, returns immediately. Otherwise checks for saved
/// credentials and, if found, spawns `pcloud-cli start` (no mount path, so it
/// uses the default mountpoint) and polls the IPC socket until it responds or
/// the 5-second deadline elapses. If no credentials are saved, errors out
/// with a hint.
fn ensure_daemon_running(
    config: &DaemonConfig,
    daemon_client: &DaemonClient,
    allow_unauthenticated: bool,
) -> Result<()> {
    if daemon_client.is_daemon_alive() {
        return Ok(());
    }

    print_status(
        StatusIndicator::Info,
        "No daemon detected; attempting auto-start...",
    );

    // Authenticated callers (status/crypto/backup) require saved credentials
    // before auto-starting. The TUI passes `allow_unauthenticated` so it can
    // bring up a daemon that idles in LOGIN_REQUIRED and drive login over IPC.
    // We deliberately do NOT `psync_init()` here in that case — keeping the
    // caller a pure IPC client with no engine of its own.
    if !allow_unauthenticated {
        let client = PCloudClient::init()?;
        let guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        if !guard.has_saved_credentials() {
            drop(guard);
            drop(client);
            eprintln!(
                "Error: no pCloud daemon is running and no saved auth token was found.\n\
                 Hint: run `pcloud-cli auth login` once (interactive web login) or supply a token\n\
                 non-interactively via `pcloud-cli auth login --token <token>`. You can also\n\
                 set PCLOUD_AUTH_TOKEN (ephemeral) before invoking this command."
            );
            return Err(PCloudError::Daemon(DaemonError::NotRunning));
        }
    }

    console_client::daemon::process::spawn_background_daemon(allow_unauthenticated)?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if daemon_client.is_daemon_alive() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    eprintln!(
        "Error: spawned a daemon but it did not respond on {} within 5s.",
        config.socket_path().display()
    );
    Err(PCloudError::Daemon(DaemonError::ConnectionFailed))
}

/// Print a daemon response to stdout/stderr.
fn print_daemon_response(response: &DaemonResponse) {
    match response {
        DaemonResponse::Ok => println!("OK"),
        DaemonResponse::OkWithMessage(msg) => println!("{}", msg),
        DaemonResponse::Error(err) => eprintln!("Error: {}", err),
        DaemonResponse::Status {
            authenticated,
            crypto_started,
            mounted,
            mountpoint,
        } => {
            println!("\n--- Daemon Status ---");
            println!("Authenticated: {}", authenticated);
            println!("Crypto started: {}", crypto_started);
            println!("Mounted: {}", mounted);
            if let Some(mp) = mountpoint {
                println!("Mountpoint: {}", mp);
            }
            println!("---------------------\n");
        }
        DaemonResponse::Pong => println!("Pong"),
        DaemonResponse::BackupCreated { sync_id } => {
            println!("Backup created (sync id: {})", sync_id);
        }
        DaemonResponse::BackupList(list) => {
            if list.is_empty() {
                println!("No backups configured.");
            } else {
                println!(
                    "{:<6}  {:<40}  {:<32}  {:>12}",
                    "ID", "Local Path", "Remote Path", "Folder ID"
                );
                for b in list {
                    println!(
                        "{:<6}  {:<40}  {:<32}  {:>12}",
                        b.sync_id,
                        b.local_path.display(),
                        b.remote_path,
                        b.folder_id
                    );
                }
            }
        }
        DaemonResponse::BackupStatus(s) => {
            println!("Device: {}", s.device_name);
            println!("Backups ({}):", s.backups.len());
            for b in &s.backups {
                println!(
                    "  [{}] {} -> {}",
                    b.sync_id,
                    b.local_path.display(),
                    b.remote_path
                );
            }
        }
        DaemonResponse::BackupRootName(name) => println!("{}", name),
        // Dashboard/auth responses are consumed by the TUI over IPC, not the
        // CLI's response printer; fall back to their Display form if seen.
        other @ (DaemonResponse::StatusFull(_)
        | DaemonResponse::Activity { .. }
        | DaemonResponse::AuthWeb { .. }) => println!("{}", other),
    }
}

// ============================================================================
// Signal helpers (foreground mount only)
// ============================================================================

fn setup_signal_handler() -> Result<()> {
    ctrlc::set_handler(move || {
        eprintln!("\nReceived interrupt signal...");
        SHUTDOWN.store(true, Ordering::SeqCst);
    })
    .map_err(|e| PCloudError::Config(format!("Error setting Ctrl-C handler: {}", e)))?;
    Ok(())
}

fn wait_for_signal() -> Result<()> {
    while !SHUTDOWN.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_flag_default() {
        SHUTDOWN.store(false, Ordering::SeqCst);
        assert!(!SHUTDOWN.load(Ordering::SeqCst));
    }

    #[test]
    fn test_shutdown_flag_set() {
        SHUTDOWN.store(true, Ordering::SeqCst);
        assert!(SHUTDOWN.load(Ordering::SeqCst));
        SHUTDOWN.store(false, Ordering::SeqCst);
    }

    #[test]
    fn test_backup_op_to_daemon_command_maps_correctly() {
        let cmd = backup_op_to_daemon_command(&BackupOp::List);
        assert!(matches!(cmd, DaemonCommand::BackupList));

        let cmd = backup_op_to_daemon_command(&BackupOp::Remove { id: 42 });
        assert!(matches!(cmd, DaemonCommand::BackupRemove { sync_id: 42 }));

        let cmd = backup_op_to_daemon_command(&BackupOp::Status { id: None });
        assert!(matches!(cmd, DaemonCommand::BackupStatus { sync_id: None }));
    }
}
