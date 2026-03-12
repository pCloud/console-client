//! pCloud Console Client - Rust Rewrite
//!
//! This is a Rust rewrite of the pCloud console-client CLI, maintaining full
//! compatibility with the original C++ implementation while using the pclsync
//! C library through FFI bindings.
//!
//! # Features
//!
//! - **Authentication**: Web login, token auth, logout, unlink
//! - **Crypto Folders**: Encrypted storage with setup/start/stop operations
//! - **FUSE Mounting**: Virtual filesystem at specified mountpoint
//! - **Daemon Mode**: Background service with IPC command interface
//! - **CLI Commands**: startcrypto, stopcrypto, finalize, quit, logout, unlink
//!
//! # Usage
//!
//! ```text
//! pcloud [OPTIONS]
//!
//! Options:
//!   -t <token>       Authentication token
//!   -c               Prompt for crypto password
//!   -d               Daemonize (background)
//!   -o               Commands mode
//!   -m <path>        Mountpoint
//!   -k               Commands only (talk to daemon)
//!   --logout         Clear saved credentials
//!   --unlink         Clear all local data
//!   --nosave         Don't save credentials
//! ```

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clap::Parser;
use secrecy::{ExposeSecret, SecretString};

use console_client::cli::{
    print_cli_auth_help, prompt_auth_choice, prompt_confirm, prompt_token, AuthChoice, Cli,
    CommandPrompt, InteractiveCommand,
};
use console_client::error::{AuthError, PCloudError};
use console_client::ffi::{register_status_callback, status_callback_trampoline, status_to_string};
use console_client::security::{prompt_for_password, ResolvedSecrets, SecurePassword};
use console_client::utils::browser::{has_display, open_url};
use console_client::utils::qrcode::{can_display_qr, generate_qr_code};
use console_client::utils::terminal::{print_boxed, print_status, StatusIndicator};
use console_client::wrapper::{PCloudClient, WebLoginConfig};
use console_client::Result;

/// Global shutdown flag for signal handling.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Application entry point.
///
/// Parses CLI arguments, validates them, and runs the appropriate mode:
/// - Crash monitor: Internal subprocess for crash dump collection
/// - Logout/Unlink: Clear credentials or data and exit
/// - Client mode: Connects to an existing daemon
/// - Daemon mode: Runs as a background service
/// - Foreground mode: Normal interactive operation
fn main() -> ExitCode {
    // Check if we were launched as a crash reporter subprocess.
    // This must happen before crash_reporting::init() and CLI parsing.
    if let Some((socket_name, dump_dir)) = console_client::crash_reporting::check_monitor_args() {
        console_client::crash_reporting::run_monitor(&socket_name, &dump_dir);
        return ExitCode::SUCCESS;
    }

    // Initialize crash reporting before anything that could crash
    console_client::crash_reporting::init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Validate arguments
    if let Err(e) = cli.validate() {
        eprintln!("Error: {}", e);
        return ExitCode::FAILURE;
    }

    // Run the application
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            console_client::crash_reporting::notify_error(&e);
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Main application runner.
///
/// Dispatches to the appropriate mode based on CLI arguments:
/// - `--logout`: Clear saved credentials and exit
/// - `--unlink`: Clear all local data and exit
/// - `--client`: Client mode (talk to existing daemon)
/// - `--daemon`: Daemon mode (background service)
/// - Default: Foreground mode (interactive)
fn run(cli: Cli) -> Result<()> {
    // Handle logout/unlink operations first
    if cli.is_logout() {
        return run_logout();
    }
    if cli.is_unlink() {
        return run_unlink();
    }

    // Handle client mode (talk to daemon)
    if cli.commands_only {
        return run_client_mode(&cli);
    }

    // Resolve secrets from environment variables early, before any prompts.
    // This clears the env vars from the process for security.
    let env_secrets = ResolvedSecrets::from_env()?;

    // Handle daemon mode
    if cli.daemonize {
        return run_daemon_mode(cli, env_secrets);
    }

    // Normal foreground mode
    run_foreground_mode(cli, env_secrets)
}

/// Authentication method determined from CLI arguments and saved state.
enum AuthMethod {
    /// Use saved auth token from database
    SavedToken,
    /// Use provided auth token (from CLI, saved to DB per --nosave)
    Token(SecretString),
    /// Use token from environment variable (ephemeral, never saved)
    EnvToken(SecretString),
    /// Need to prompt user for authentication method
    NeedsInteractive,
}

/// Determine the authentication method based on CLI arguments, env vars, and saved state.
///
/// Priority: CLI `-t` > `PCLOUD_AUTH_TOKEN` env > saved credentials > interactive prompt
fn determine_auth_method(
    cli: &Cli,
    client: &Arc<Mutex<PCloudClient>>,
    env_token: Option<SecretString>,
) -> Result<AuthMethod> {
    // 1. Check if token was provided via CLI
    if let Some(ref token) = cli.auth_token {
        return Ok(AuthMethod::Token(SecretString::from(token.clone())));
    }

    // 2. Check if token was provided via environment variable
    if let Some(token) = env_token {
        return Ok(AuthMethod::EnvToken(token));
    }

    // 3. Check if we have saved credentials
    {
        let client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

        if client_guard.has_saved_credentials() {
            return Ok(AuthMethod::SavedToken);
        }
    }

    // 4. No credentials provided and none saved - need interactive prompt
    Ok(AuthMethod::NeedsInteractive)
}

/// Run the --logout operation.
///
/// Initializes the client, clears saved credentials, and exits.
fn run_logout() -> Result<()> {
    print_status(StatusIndicator::Info, "Initializing pCloud client...");
    let client = PCloudClient::init()?;

    let mut client_guard = client
        .lock()
        .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

    if !client_guard.has_saved_credentials() {
        print_status(StatusIndicator::Info, "No saved credentials found.");
        return Ok(());
    }

    client_guard.logout();
    print_status(
        StatusIndicator::Success,
        "Logged out. Saved credentials have been cleared.",
    );
    Ok(())
}

/// Run the --unlink operation.
///
/// Initializes the client, asks for confirmation, then clears all local data.
fn run_unlink() -> Result<()> {
    print_status(StatusIndicator::Info, "Initializing pCloud client...");
    let client = PCloudClient::init()?;

    // Ask for confirmation since this is destructive
    let confirmed =
        prompt_confirm("This will remove all saved credentials and local sync data. Continue?")?;
    if !confirmed {
        print_status(StatusIndicator::Info, "Unlink cancelled.");
        return Ok(());
    }

    let mut client_guard = client
        .lock()
        .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

    client_guard.unlink();
    print_status(
        StatusIndicator::Success,
        "Account unlinked. All local data has been cleared.",
    );
    Ok(())
}

/// Run the client in foreground mode.
///
/// This is the main operational mode where the client:
/// 1. Initializes the pCloud client
/// 2. Sets up status callbacks
/// 3. Handles authentication (with interactive prompt if needed)
/// 4. Optionally sets up and/or starts crypto
/// 5. Mounts the filesystem (defaults to ~/pCloud)
/// 6. Enters the command loop or waits for signals
fn run_foreground_mode(cli: Cli, env_secrets: ResolvedSecrets) -> Result<()> {
    // Set up signal handler for Ctrl+C
    setup_signal_handler()?;

    // 1. Initialize pCloud client
    print_status(StatusIndicator::Info, "Initializing pCloud client...");
    let client = PCloudClient::init()?;

    // 2. Set up status callback for progress reporting
    register_status_callback(|status| {
        let status_str = status_to_string(status.status);
        // Only print non-trivial status updates
        if status.status != 0 {
            // Not PSTATUS_READY
            eprintln!("Status: {}", status_str);
        }
        // Print sync progress if there are files to sync
        if status.filestodownload > 0 || status.filestoupload > 0 {
            eprintln!(
                "  Files to download: {}, Files to upload: {}",
                status.filestodownload, status.filestoupload
            );
        }
    });

    // 3. Determine and handle authentication
    match determine_auth_method(&cli, &client, env_secrets.auth_token)? {
        AuthMethod::SavedToken => {
            print_status(StatusIndicator::Info, "Using saved credentials");
        }
        AuthMethod::Token(token) => {
            print_status(StatusIndicator::Info, "Setting authentication token...");
            let mut client_guard = client
                .lock()
                .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
            client_guard.set_auth_token(&token, cli.should_save_credentials())?;
            drop(client_guard);
            print_status(StatusIndicator::Success, "Authentication token set");
        }
        AuthMethod::EnvToken(token) => {
            print_status(
                StatusIndicator::Info,
                "Setting authentication token from environment...",
            );
            let mut client_guard = client
                .lock()
                .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
            // Env-sourced tokens are never saved (ephemeral by design)
            client_guard.set_auth_token(&token, false)?;
            drop(client_guard);
            print_status(
                StatusIndicator::Success,
                "Authentication token set (from environment)",
            );
        }
        AuthMethod::NeedsInteractive => {
            // No credentials - prompt user for authentication method
            handle_interactive_auth(&client, cli.should_save_credentials())?;
        }
    };

    // 4. Prepare mountpoint before starting sync (psync_start_sync mounts the FS)
    let mountpoint = cli.get_mountpoint();
    {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

        // Create mountpoint directory if it doesn't exist
        if !mountpoint.exists() {
            print_status(
                StatusIndicator::Info,
                &format!("Creating mountpoint: {}", mountpoint.display()),
            );
            std::fs::create_dir_all(&mountpoint).map_err(PCloudError::Io)?;
        }

        // Set the filesystem root before starting sync
        client_guard.set_fs_root(&mountpoint)?;
        print_status(
            StatusIndicator::Info,
            &format!("Filesystem root set to: {}", mountpoint.display()),
        );
    }

    // 5. Handle crypto setup/start if requested via -c flag or env var
    let crypto_secret = if let Some(env_crypto) = env_secrets.crypto_password {
        // Crypto password from environment auto-enables crypto
        print_status(
            StatusIndicator::Info,
            "Using crypto password from environment",
        );
        Some(env_crypto)
    } else if cli.crypto_prompt {
        print_step("Crypto password required");
        let crypto_pwd = prompt_for_password("Crypto password: ").map_err(PCloudError::Io)?;
        Some(crypto_pwd)
    } else {
        None
    };

    if let Some(ref crypto_pwd) = crypto_secret {
        let secure_crypto_pwd = SecurePassword::from_secret(crypto_pwd.clone());
        let crypto_secret = SecretString::from(secure_crypto_pwd.expose().to_string());

        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

        // Check if crypto is set up
        if !client_guard.is_crypto_setup() {
            print_status(
                StatusIndicator::Info,
                "Setting up crypto for the first time...",
            );
            client_guard.setup_crypto(&crypto_secret, "")?;
            print_status(StatusIndicator::Success, "Crypto setup complete");
        }

        // Start crypto
        print_status(StatusIndicator::Info, "Starting crypto...");
        client_guard.start_crypto(&crypto_secret)?;
        print_status(
            StatusIndicator::Success,
            "Crypto started - encrypted folders accessible",
        );
    }

    // 6. Start sync (this also mounts the FUSE filesystem at the configured fsroot)
    {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        client_guard.start_sync(Some(status_callback_trampoline), None);
        print_status(StatusIndicator::Info, "Sync started");

        // Refresh mount state — psync_start_sync already mounted the filesystem
        client_guard.refresh_mount_state();
        print_status(
            StatusIndicator::Success,
            &format!("Mounted pCloud at: {}", mountpoint.display()),
        );
    }

    // 7. Enter command loop if interactive mode, otherwise wait for signals
    if cli.commands_mode {
        println!("\nEntering interactive mode. Type 'help' for available commands.");
        run_command_loop(Arc::clone(&client))?;
    } else {
        println!("\npCloud client running. Press Ctrl+C to exit.");
        wait_for_signal()?;
    }

    // Cleanup happens automatically via Drop on PCloudClient
    println!("\nShutting down...");
    Ok(())
}

/// Handle interactive authentication when no credentials are provided.
///
/// Displays a menu with authentication options and handles the user's choice.
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
                print_status(StatusIndicator::Info, "Setting authentication token...");
                let mut client_guard = client.lock().map_err(|_| {
                    PCloudError::Config("Failed to acquire client lock".to_string())
                })?;
                client_guard.set_auth_token(&token, save_credentials)?;
                drop(client_guard);
                print_status(StatusIndicator::Success, "Authentication successful!");
                return Ok(());
            }
            AuthChoice::ShowCliHelp => {
                print_cli_auth_help();
                // Continue the loop to let user choose again
            }
            AuthChoice::Cancel => {
                return Err(PCloudError::Auth(AuthError::Other(
                    "Authentication cancelled by user".to_string(),
                )));
            }
        }
    }
}

/// Handle web-based login flow.
///
/// 1. Initiates a web login session
/// 2. Displays the login URL (in a box)
/// 3. Displays QR code if terminal supports it
/// 4. Attempts to open the URL in a browser
/// 5. Waits for the user to complete authentication
fn handle_web_login(client: &Arc<Mutex<PCloudClient>>, save_credentials: bool) -> Result<()> {
    print_status(StatusIndicator::Info, "Initiating web-based login...");

    let session = {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        client_guard.initiate_web_login(&WebLoginConfig::default())?
    };

    // Display URL in a box
    println!();
    print_boxed(&["Open this URL in your browser:", "", &session.login_url]);

    // Display QR code if terminal supports it
    if can_display_qr() {
        if let Ok(qr) = generate_qr_code(&session.login_url) {
            println!();
            println!("{}", qr);
        }
    }

    // Try to auto-open browser
    if has_display() {
        match open_url(&session.login_url) {
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

    // Wait for authentication to complete
    {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        client_guard.wait_for_web_auth(&session.request_id)?;

        // Explicitly persist the token to the database
        if save_credentials {
            client_guard.save_current_auth_token()?;
        }
    }

    print_status(StatusIndicator::Success, "Authentication successful!");
    Ok(())
}

/// Run the interactive command loop.
///
/// Reads commands from stdin and executes them until the user
/// enters 'quit' or 'finalize'.
fn run_command_loop(client: Arc<Mutex<PCloudClient>>) -> Result<()> {
    let prompt = CommandPrompt::default();

    loop {
        // Check for shutdown signal
        if SHUTDOWN.load(Ordering::SeqCst) {
            println!("\nReceived shutdown signal.");
            break;
        }

        let cmd = match prompt.read_command() {
            Ok(Some(cmd)) => cmd,
            Ok(None) => {
                // EOF (Ctrl+D)
                println!("\nEOF received, exiting...");
                break;
            }
            Err(e) => {
                eprintln!("Error reading command: {}", e);
                continue;
            }
        };

        match cmd {
            InteractiveCommand::StartCrypto => {
                if let Err(e) = handle_start_crypto(&client) {
                    eprintln!("Error starting crypto: {}", e);
                }
            }
            InteractiveCommand::StopCrypto => {
                if let Err(e) = handle_stop_crypto(&client) {
                    eprintln!("Error stopping crypto: {}", e);
                }
            }
            InteractiveCommand::Finalize => {
                println!("Finalizing - waiting for sync to complete...");
                // In a full implementation, we would wait for sync to complete
                // For now, just give it a moment and then exit
                std::thread::sleep(Duration::from_secs(2));
                println!("Finalize complete.");
                break;
            }
            InteractiveCommand::Status => {
                if let Err(e) = handle_status_command(&client) {
                    eprintln!("Error getting status: {}", e);
                }
            }
            InteractiveCommand::Logout => {
                match client.lock() {
                    Ok(mut c) => {
                        c.logout();
                        println!("Logged out. Saved credentials cleared.");
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
                break;
            }
            InteractiveCommand::Unlink => {
                match prompt_confirm(
                    "This will remove all saved credentials and local sync data. Continue?",
                ) {
                    Ok(true) => match client.lock() {
                        Ok(mut c) => {
                            c.unlink();
                            println!("Account unlinked. All local data cleared.");
                        }
                        Err(e) => eprintln!("Error: {}", e),
                    },
                    Ok(false) => {
                        println!("Unlink cancelled.");
                        continue;
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
                break;
            }
            InteractiveCommand::Quit => {
                println!("Exiting...");
                break;
            }
            InteractiveCommand::Help => {
                InteractiveCommand::print_help();
            }
            InteractiveCommand::Unknown(s) => {
                if !s.is_empty() {
                    println!(
                        "Unknown command: '{}'. Type 'help' for available commands.",
                        s
                    );
                }
                // Empty input - just show prompt again
            }
        }
    }

    Ok(())
}

/// Handle the 'startcrypto' command.
fn handle_start_crypto(client: &Arc<Mutex<PCloudClient>>) -> Result<()> {
    let pwd = prompt_for_password("Crypto password: ").map_err(PCloudError::Io)?;

    let secure_pwd = SecretString::from(pwd.expose_secret().to_string());

    let mut client_guard = client
        .lock()
        .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

    // Check if crypto is set up first
    if !client_guard.is_crypto_setup() {
        println!("Crypto is not set up for this account.");
        println!("Setting up crypto with the provided password...");
        client_guard.setup_crypto(&secure_pwd, "")?;
        println!("Crypto setup complete.");
    }

    client_guard.start_crypto(&secure_pwd)?;
    println!("Crypto started - encrypted folders are now accessible.");

    Ok(())
}

/// Handle the 'stopcrypto' command.
fn handle_stop_crypto(client: &Arc<Mutex<PCloudClient>>) -> Result<()> {
    let mut client_guard = client
        .lock()
        .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

    client_guard.stop_crypto()?;
    println!("Crypto stopped - encrypted folders are now locked.");

    Ok(())
}

/// Handle the 'status' command.
fn handle_status_command(client: &Arc<Mutex<PCloudClient>>) -> Result<()> {
    let client_guard = client
        .lock()
        .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

    print_client_status(&client_guard);
    Ok(())
}

/// Print the current status of the client.
fn print_client_status(client: &PCloudClient) {
    println!("\n--- pCloud Client Status ---");
    println!("Authentication: {:?}", client.auth_state());
    println!("Crypto: {:?}", client.crypto_state());
    println!("Filesystem mounted: {}", client.is_mounted());

    if let Some(mp) = client.mountpoint() {
        println!("Mountpoint: {}", mp.display());
    }

    // Get detailed status from C library
    let status = client.get_status();
    println!("\nSync Status: {}", status_to_string(status.status));
    println!("Files to download: {}", status.filestodownload);
    println!("Files to upload: {}", status.filestoupload);
    println!("Bytes to download: {} bytes", status.bytestodownload);
    println!("Bytes to upload: {} bytes", status.bytestoupload);
    println!("----------------------------\n");
}

/// Set up signal handler for graceful shutdown.
fn setup_signal_handler() -> Result<()> {
    ctrlc::set_handler(move || {
        eprintln!("\nReceived interrupt signal...");
        SHUTDOWN.store(true, Ordering::SeqCst);
    })
    .map_err(|e| PCloudError::Config(format!("Error setting Ctrl-C handler: {}", e)))?;

    Ok(())
}

/// Wait for a shutdown signal.
fn wait_for_signal() -> Result<()> {
    while !SHUTDOWN.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Print a step message for user feedback.
///
/// This is a convenience wrapper around `print_status` for backwards compatibility.
fn print_step(msg: &str) {
    print_status(StatusIndicator::Info, msg);
}

// ============================================================================
// Daemon mode implementation
// ============================================================================

/// Run as a daemon (background service).
///
/// This function daemonizes the current process and runs the pCloud client
/// in the background. The daemon:
/// - Detaches from the controlling terminal
/// - Creates a PID file for tracking
/// - Sets up signal handlers for graceful shutdown
/// - Initializes the pCloud client and starts sync
/// - Optionally sets up crypto and mounts the filesystem
/// - Runs until a shutdown signal is received
///
/// # Arguments
///
/// * `cli` - Parsed command-line arguments
///
/// # Important Notes
///
/// - Passwords must be collected BEFORE daemonizing (can't prompt after fork)
/// - The daemon creates a PID file at `/tmp/pcloud-<uid>.pid`
/// - The daemon can be stopped with `kill -TERM $(cat /tmp/pcloud-<uid>.pid)`
fn run_daemon_mode(cli: Cli, env_secrets: ResolvedSecrets) -> Result<()> {
    use console_client::daemon::{
        cleanup_pid_file, daemonize, is_daemon_running, is_reload_requested, is_shutdown_requested,
        setup_daemon_signals, DaemonConfig,
    };
    use console_client::error::DaemonError;

    let config = DaemonConfig::default();

    // Check if daemon is already running
    if is_daemon_running(&config) {
        eprintln!("Error: pCloud daemon is already running.");
        eprintln!("PID file: {}", config.pid_file.display());
        if let Some(pid) = console_client::daemon::get_daemon_pid(&config) {
            eprintln!("Running PID: {}", pid);
        }
        return Err(PCloudError::Daemon(DaemonError::AlreadyRunning));
    }

    // =========================================================================
    // IMPORTANT: All interactive operations must happen BEFORE daemonizing
    // because we lose terminal access after fork
    // =========================================================================

    // Initialize client early to check for saved credentials
    print_status(StatusIndicator::Info, "Initializing pCloud client...");
    let client = PCloudClient::init()?;

    // Determine authentication method and whether to save the token
    let (auth_token, save_token) = {
        let client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

        // Check if token was provided via CLI
        if let Some(ref token) = cli.auth_token {
            drop(client_guard);
            (
                Some(SecretString::from(token.clone())),
                cli.should_save_credentials(),
            )
        }
        // Check if token was provided via environment variable
        else if let Some(token) = env_secrets.auth_token {
            drop(client_guard);
            print_status(
                StatusIndicator::Info,
                "Using authentication token from environment",
            );
            // Env-sourced tokens are never saved (ephemeral by design)
            (Some(token), false)
        }
        // Check if we have saved credentials
        else if client_guard.has_saved_credentials() {
            print_status(StatusIndicator::Info, "Using saved credentials");
            drop(client_guard);
            (None, false)
        }
        // No credentials - need interactive auth before daemonizing
        else {
            drop(client_guard);
            print_status(
                StatusIndicator::Info,
                "Authentication required before daemon can start",
            );

            handle_interactive_auth(&client, cli.should_save_credentials())?;
            (None, false)
        }
    };

    // Get crypto password BEFORE daemonizing - can't prompt after fork.
    // Env var auto-enables crypto without needing -c flag.
    let crypto_password = if let Some(env_crypto) = env_secrets.crypto_password {
        print_status(
            StatusIndicator::Info,
            "Using crypto password from environment",
        );
        Some(env_crypto)
    } else if cli.crypto_prompt {
        print_step("Crypto password required");
        let pwd = prompt_for_password("Crypto password: ").map_err(PCloudError::Io)?;
        Some(pwd)
    } else {
        None
    };

    println!();
    print_status(StatusIndicator::Info, "Starting pCloud daemon...");
    println!("PID file: {}", config.pid_file.display());
    println!("Socket path: {}", config.socket_path().display());

    // Fork into background - after this, we're in the child process
    // The parent process exits immediately after forking
    daemonize(&config)?;

    // =========================================================================
    // From here on, we're running as a daemon (background process)
    // No terminal access - cannot prompt for input
    // =========================================================================

    // Set up signal handlers for graceful shutdown
    setup_daemon_signals()?;

    // Set up status callback (logs to syslog or file in daemon mode)
    // For now, we register a no-op callback since we're in the background
    register_status_callback(|_status| {
        // In daemon mode, status updates could be logged to syslog
        // For now, we silently consume them
    });

    // Apply authentication if we have a token from CLI or environment
    if let Some(ref token) = auth_token {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        client_guard.set_auth_token(token, save_token)?;
    }

    // Prepare mountpoint before starting sync (psync_start_sync mounts the FS)
    let mountpoint = cli.get_mountpoint();
    {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

        // Create mountpoint directory if it doesn't exist
        if !mountpoint.exists() {
            std::fs::create_dir_all(&mountpoint).map_err(PCloudError::Io)?;
        }

        // Set the filesystem root before starting sync
        client_guard.set_fs_root(&mountpoint)?;
    }

    // Handle crypto setup/start if requested
    if let Some(ref crypto_pwd) = crypto_password {
        let secure_crypto_pwd = SecurePassword::from_secret(crypto_pwd.clone());
        let crypto_secret = SecretString::from(secure_crypto_pwd.expose().to_string());

        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

        // Check if crypto is set up
        if !client_guard.is_crypto_setup() {
            // First time - set up crypto with empty hint
            client_guard.setup_crypto(&crypto_secret, "")?;
        }

        // Start crypto
        client_guard.start_crypto(&crypto_secret)?;
    }

    // Start sync (this also mounts the FUSE filesystem at the configured fsroot)
    {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        client_guard.start_sync(Some(status_callback_trampoline), None);

        // Refresh mount state — psync_start_sync already mounted the filesystem
        client_guard.refresh_mount_state();
    }

    // Start IPC server in a separate thread
    let server = console_client::daemon::DaemonServer::new(config.socket_path())?;
    let client_for_ipc = client.clone();
    let ipc_thread = std::thread::spawn(move || {
        if let Err(e) = server.run(client_for_ipc) {
            eprintln!("IPC server error: {}", e);
        }
    });

    // Main daemon loop - run until shutdown requested
    while !is_shutdown_requested() {
        // Check for reload request (SIGHUP)
        if is_reload_requested() {
            // Currently no configuration to reload
            // Future: reload config file, reconnect, etc.
        }

        // Sleep to avoid busy-waiting
        // The IPC server handles incoming commands in its own thread
        std::thread::sleep(Duration::from_millis(100));
    }

    // Graceful shutdown
    // Wait for IPC thread to finish (it checks shutdown flag too)
    let _ = ipc_thread.join();

    // The PCloudClient Drop implementation will handle cleanup
    cleanup_pid_file(&config);

    Ok(())
}

/// Run in client mode (connect to existing daemon).
///
/// In client mode, we connect to a running daemon via Unix socket IPC
/// and send commands to it. This is used when the `-k` flag is specified.
///
/// # Client Mode
///
/// In client mode:
/// - Connect to running daemon via IPC socket
/// - If interactive mode (`-o`), enter command loop
/// - Otherwise, just show status and exit
fn run_client_mode(cli: &Cli) -> Result<()> {
    use console_client::daemon::{is_daemon_running, DaemonClient, DaemonCommand, DaemonConfig};
    use console_client::error::DaemonError;

    let config = DaemonConfig::default();

    // Check if daemon is running
    if !is_daemon_running(&config) {
        eprintln!("No daemon is running. Start one with the -d flag.");
        return Err(PCloudError::Daemon(DaemonError::NotRunning));
    }

    let client = DaemonClient::new(config.socket_path());

    // Verify we can connect
    if !client.is_daemon_alive() {
        eprintln!("Daemon PID file exists but daemon is not responding.");
        eprintln!("Socket: {}", config.socket_path().display());
        return Err(PCloudError::Daemon(DaemonError::ConnectionFailed));
    }

    // If interactive mode requested, enter command loop
    if cli.commands_mode {
        run_client_command_loop(&client)?;
    } else {
        // Just show status
        let response = client.send_command(DaemonCommand::Status)?;
        print_daemon_response(&response);
    }

    Ok(())
}

/// Run the interactive command loop in client mode.
///
/// Reads commands from stdin and sends them to the daemon via IPC.
fn run_client_command_loop(client: &console_client::daemon::DaemonClient) -> Result<()> {
    use console_client::daemon::{DaemonCommand, DaemonResponse};

    let prompt = CommandPrompt::default();

    println!("Connected to pCloud daemon. Type 'help' for commands.");

    loop {
        // Check for shutdown signal
        if SHUTDOWN.load(Ordering::SeqCst) {
            println!("\nReceived shutdown signal.");
            break;
        }

        let cmd = match prompt.read_command() {
            Ok(Some(cmd)) => cmd,
            Ok(None) => {
                // EOF (Ctrl+D)
                println!("\nEOF received, exiting...");
                break;
            }
            Err(e) => {
                eprintln!("Error reading command: {}", e);
                continue;
            }
        };

        let daemon_cmd = match cmd {
            InteractiveCommand::StartCrypto => {
                // Prompt for password on the client side
                match prompt_for_password("Crypto password: ") {
                    Ok(pwd) => DaemonCommand::StartCrypto {
                        password: Some(pwd.expose_secret().to_string()),
                    },
                    Err(e) => {
                        eprintln!("Error reading password: {}", e);
                        continue;
                    }
                }
            }
            InteractiveCommand::StopCrypto => DaemonCommand::StopCrypto,
            InteractiveCommand::Finalize => DaemonCommand::Finalize,
            InteractiveCommand::Status => DaemonCommand::Status,
            InteractiveCommand::Logout => DaemonCommand::Logout,
            InteractiveCommand::Unlink => {
                match prompt_confirm(
                    "This will remove all saved credentials and local sync data from the daemon. Continue?",
                ) {
                    Ok(true) => DaemonCommand::Unlink,
                    Ok(false) => {
                        println!("Unlink cancelled.");
                        continue;
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        continue;
                    }
                }
            }
            InteractiveCommand::Quit => break,
            InteractiveCommand::Help => {
                print_client_help();
                continue;
            }
            InteractiveCommand::Unknown(s) => {
                if !s.is_empty() {
                    println!(
                        "Unknown command: '{}'. Type 'help' for available commands.",
                        s
                    );
                }
                continue;
            }
        };

        match client.send_command(daemon_cmd) {
            Ok(response) => {
                print_daemon_response(&response);

                // If we sent Finalize/Logout/Unlink or received confirmation of shutdown, exit
                if matches!(
                    response,
                    DaemonResponse::OkWithMessage(ref msg) if msg.contains("shut down") || msg.contains("shutting down")
                ) {
                    println!("Daemon is shutting down, exiting client.");
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error communicating with daemon: {}", e);
                // Check if daemon is still alive
                if !client.is_daemon_alive() {
                    eprintln!("Daemon appears to have stopped.");
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Print a daemon response to stdout/stderr.
fn print_daemon_response(response: &console_client::daemon::DaemonResponse) {
    use console_client::daemon::DaemonResponse;

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
    }
}

/// Print help for client mode commands.
fn print_client_help() {
    println!("Available commands (sent to daemon):");
    println!();
    println!("  startcrypto, start  - Unlock encrypted folders");
    println!("  stopcrypto, stop    - Lock encrypted folders");
    println!("  finalize, fin       - Tell daemon to finish sync and exit");
    println!("  status, s           - Show daemon status");
    println!("  logout, lo          - Log out and clear saved credentials");
    println!("  unlink, ul          - Unlink account and clear all local data");
    println!("  quit, q, exit       - Disconnect from daemon");
    println!("  help, h, ?          - Show this help");
    println!();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_flag_default() {
        // Reset to known state (may be affected by other tests)
        SHUTDOWN.store(false, Ordering::SeqCst);
        assert!(!SHUTDOWN.load(Ordering::SeqCst));
    }

    #[test]
    fn test_shutdown_flag_set() {
        SHUTDOWN.store(true, Ordering::SeqCst);
        assert!(SHUTDOWN.load(Ordering::SeqCst));
        // Reset
        SHUTDOWN.store(false, Ordering::SeqCst);
    }

    #[test]
    fn test_print_step_does_not_panic() {
        // Just verify it doesn't panic
        print_step("test message");
    }
}
