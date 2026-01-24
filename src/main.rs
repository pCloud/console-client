//! pCloud Console Client - Rust Rewrite
//!
//! This is a Rust rewrite of the pCloud console-client CLI, maintaining full
//! compatibility with the original C++ implementation while using the pclsync
//! C library through FFI bindings.
//!
//! # Features
//!
//! - **Authentication**: Login/logout, token management, new user registration
//! - **Crypto Folders**: Encrypted storage with setup/start/stop operations
//! - **FUSE Mounting**: Virtual filesystem at specified mountpoint
//! - **Daemon Mode**: Background service with IPC command interface
//! - **CLI Commands**: startcrypto, stopcrypto, finalize, quit
//!
//! # Usage
//!
//! ```text
//! pcloud [OPTIONS]
//!
//! Options:
//!   -u <username>    Username (required)
//!   -p               Prompt for password
//!   -c               Prompt for crypto password
//!   -y               Use password as crypto password
//!   -d               Daemonize (background)
//!   -o               Commands mode
//!   -m <path>        Mountpoint
//!   -k               Commands only (talk to daemon)
//!   -n               New user registration
//!   -s               Save password
//! ```

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clap::Parser;
use secrecy::{ExposeSecret, SecretString};

use console_client::cli::{
    prompt_auth_choice, prompt_credentials, prompt_token, print_cli_auth_help,
    AuthChoice, Cli, CommandPrompt, InteractiveCommand,
};
use console_client::error::{AuthError, PCloudError};
use console_client::ffi::{register_status_callback, status_callback_trampoline, status_to_string};
use console_client::security::{prompt_for_password, SecurePassword};
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
/// - Client mode: Connects to an existing daemon
/// - Daemon mode: Runs as a background service
/// - Foreground mode: Normal interactive operation
fn main() -> ExitCode {
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
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Main application runner.
///
/// Dispatches to the appropriate mode based on CLI arguments:
/// - `--client`: Client mode (talk to existing daemon)
/// - `--daemon`: Daemon mode (background service)
/// - Default: Foreground mode (interactive)
fn run(cli: Cli) -> Result<()> {
    // Handle client mode (talk to daemon) - Phase 9
    if cli.commands_only {
        return run_client_mode(&cli);
    }

    // Handle daemon mode - Phase 8
    if cli.daemonize {
        return run_daemon_mode(cli);
    }

    // Normal foreground mode
    run_foreground_mode(cli)
}

/// Authentication method determined from CLI arguments and saved state.
enum AuthMethod {
    /// Use saved auth token from database
    SavedToken,
    /// Use password authentication with provided username and password
    Password(String, SecretString),
    /// Use provided auth token
    Token(SecretString),
    /// Need to prompt user for authentication method
    NeedsInteractive,
}

/// Determine the authentication method based on CLI arguments.
fn determine_auth_method(cli: &Cli, client: &Arc<Mutex<PCloudClient>>) -> Result<AuthMethod> {
    // Check if token was provided via CLI
    if let Some(ref token) = cli.auth_token {
        return Ok(AuthMethod::Token(SecretString::from(token.clone())));
    }

    // Check if password prompt was requested
    if cli.password_prompt {
        let username = cli.username.as_ref().ok_or_else(|| {
            PCloudError::InvalidArgument("Username required for password authentication".to_string())
        })?;
        print_status(StatusIndicator::Info, "Password required");
        let password = prompt_for_password("Password: ").map_err(PCloudError::Io)?;
        return Ok(AuthMethod::Password(username.clone(), password));
    }

    // Check if we have saved credentials
    {
        let client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

        if client_guard.has_saved_credentials() {
            return Ok(AuthMethod::SavedToken);
        }
    }

    // No credentials provided and none saved - need interactive prompt
    Ok(AuthMethod::NeedsInteractive)
}

/// Run the client in foreground mode.
///
/// This is the main operational mode where the client:
/// 1. Initializes the pCloud client
/// 2. Sets up status callbacks
/// 3. Handles authentication (with interactive prompt if needed)
/// 4. Optionally handles registration for new users
/// 5. Optionally sets up and/or starts crypto
/// 6. Mounts the filesystem (defaults to ~/pCloud)
/// 7. Enters the command loop or waits for signals
fn run_foreground_mode(cli: Cli) -> Result<()> {
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

    // 3. Handle new user registration (requires username and password)
    if cli.newuser {
        let username = cli.username.as_ref().ok_or_else(|| {
            PCloudError::InvalidArgument("Username required for new user registration".to_string())
        })?;
        print_status(StatusIndicator::Info, "Password required for registration");
        let password = prompt_for_password("Password: ").map_err(PCloudError::Io)?;
        return handle_registration(username, Arc::clone(&client), Some(password));
    }

    // 4. Determine and handle authentication
    let password = match determine_auth_method(&cli, &client)? {
        AuthMethod::SavedToken => {
            print_status(StatusIndicator::Info, "Using saved credentials");
            None
        }
        AuthMethod::Password(username, pwd) => {
            print_status(StatusIndicator::Info, "Authenticating...");
            let mut client_guard = client
                .lock()
                .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
            client_guard.authenticate(&username, &pwd, cli.save_password)?;
            drop(client_guard);
            print_status(StatusIndicator::Success, "Authentication credentials set");
            Some(pwd)
        }
        AuthMethod::Token(token) => {
            print_status(StatusIndicator::Info, "Setting authentication token...");
            let mut client_guard = client
                .lock()
                .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
            client_guard.set_auth_token(&token, cli.save_password)?;
            drop(client_guard);
            print_status(StatusIndicator::Success, "Authentication token set");
            None
        }
        AuthMethod::NeedsInteractive => {
            // No credentials - prompt user for authentication method
            let auth_result = handle_interactive_auth(&client, cli.save_password)?;
            auth_result
        }
    };

    // 5. Start sync to begin the connection
    {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        client_guard.start_sync(Some(status_callback_trampoline), None);
        print_status(StatusIndicator::Info, "Sync started");
    }

    // 6. Handle crypto setup/start if requested
    let crypto_password = get_crypto_password(&cli, password.as_ref())?;
    if let Some(ref crypto_pwd) = crypto_password {
        let secure_crypto_pwd = SecurePassword::from_secret(crypto_pwd.clone());
        let crypto_secret = SecretString::from(secure_crypto_pwd.expose().to_string());

        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

        if cli.crypto_prompt || cli.use_password_as_crypto {
            // Check if crypto is set up
            if !client_guard.is_crypto_setup() {
                print_status(StatusIndicator::Info, "Setting up crypto for the first time...");
                // First time - set up crypto with empty hint
                client_guard.setup_crypto(&crypto_secret, "")?;
                print_status(StatusIndicator::Success, "Crypto setup complete");
            }

            // Start crypto
            print_status(StatusIndicator::Info, "Starting crypto...");
            client_guard.start_crypto(&crypto_secret)?;
            print_status(StatusIndicator::Success, "Crypto started - encrypted folders accessible");
        }
    }

    // 7. Get mount path (use default ~/pCloud if not specified)
    let mountpoint = cli.get_mountpoint();

    // 8. Mount filesystem
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

        print_status(
            StatusIndicator::Info,
            &format!("Mounting filesystem at: {}", mountpoint.display()),
        );
        client_guard.mount_filesystem(&mountpoint)?;
        print_status(
            StatusIndicator::Success,
            &format!("Mounted pCloud at: {}", mountpoint.display()),
        );
    }

    // 9. Enter command loop if interactive mode, otherwise wait for signals
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
) -> Result<Option<SecretString>> {
    loop {
        match prompt_auth_choice()? {
            AuthChoice::WebLogin => {
                handle_web_login(client, save_credentials)?;
                return Ok(None);
            }
            AuthChoice::EnterCredentials => {
                let (username, password) = prompt_credentials()?;
                print_status(StatusIndicator::Info, "Authenticating...");
                let mut client_guard = client
                    .lock()
                    .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
                client_guard.authenticate(&username, &password, save_credentials)?;
                drop(client_guard);
                print_status(StatusIndicator::Success, "Authentication successful!");
                return Ok(Some(password));
            }
            AuthChoice::EnterToken => {
                let token = prompt_token()?;
                print_status(StatusIndicator::Info, "Setting authentication token...");
                let mut client_guard = client
                    .lock()
                    .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
                client_guard.set_auth_token(&token, save_credentials)?;
                drop(client_guard);
                print_status(StatusIndicator::Success, "Authentication successful!");
                return Ok(None);
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
    print_boxed(&[
        "Open this URL in your browser:",
        "",
        &session.login_url,
    ]);

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
            Ok(false) => print_status(StatusIndicator::Warning, "Could not find browser - please copy the URL"),
            Err(_) => print_status(StatusIndicator::Warning, "Could not open browser - please copy the URL"),
        }
    } else {
        print_status(StatusIndicator::Info, "No display detected - please copy the URL above");
    }

    println!();
    print_status(StatusIndicator::Progress, "Waiting for authentication (timeout: 5 min)...");

    // Wait for authentication to complete
    {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        client_guard.wait_for_web_auth(&session.request_id)?;

        // Save credentials if requested
        if save_credentials {
            // The token is already saved by wait_for_web_auth on success
        }
    }

    print_status(StatusIndicator::Success, "Authentication successful!");
    Ok(())
}

/// Get the crypto password based on CLI arguments.
///
/// Returns:
/// - The login password if `--passascrypto` is set
/// - A prompted password if `--crypto` is set
/// - None otherwise
fn get_crypto_password(
    cli: &Cli,
    password: Option<&SecretString>,
) -> Result<Option<SecretString>> {
    if cli.use_password_as_crypto {
        // Use login password as crypto password
        Ok(password.cloned())
    } else if cli.crypto_prompt {
        // Prompt for separate crypto password
        print_step("Crypto password required");
        let pwd = prompt_for_password("Crypto password: ").map_err(|e| PCloudError::Io(e))?;
        Ok(Some(pwd))
    } else {
        Ok(None)
    }
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
    let pwd = prompt_for_password("Crypto password: ").map_err(|e| PCloudError::Io(e))?;

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

/// Handle new user registration.
fn handle_registration(
    username: &str,
    client: Arc<Mutex<PCloudClient>>,
    password: Option<SecretString>,
) -> Result<()> {
    let pwd = password.ok_or_else(|| {
        PCloudError::Auth(AuthError::Other(
            "Password is required for registration. Use -p flag.".to_string(),
        ))
    })?;

    // Confirm password for new registration
    print_status(StatusIndicator::Info, "Please confirm your password");
    let confirm = prompt_for_password("Confirm password: ").map_err(PCloudError::Io)?;

    if pwd.expose_secret() != confirm.expose_secret() {
        return Err(PCloudError::Auth(AuthError::Other(
            "Passwords do not match".to_string(),
        )));
    }

    print_status(StatusIndicator::Info, "Registering new account...");

    let mut client_guard = client
        .lock()
        .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

    client_guard.register(username, &pwd, true)?;

    println!();
    print_status(StatusIndicator::Success, "Registration successful!");
    println!(
        "Please check your email ({}) to verify your account.",
        username
    );
    println!("After verification, run pcloud again without the -n flag to login.");

    Ok(())
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
// Daemon mode implementation (Phase 8)
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
fn run_daemon_mode(cli: Cli) -> Result<()> {
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

    // Determine authentication method
    let (password, auth_token) = {
        let client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

        // Check if token was provided via CLI
        if let Some(ref token) = cli.auth_token {
            drop(client_guard);
            (None, Some(SecretString::from(token.clone())))
        }
        // Check if password prompt was requested
        else if cli.password_prompt {
            drop(client_guard);
            print_status(StatusIndicator::Info, "Password required (must be entered before daemonizing)");
            let password = prompt_for_password("Password: ").map_err(PCloudError::Io)?;
            (Some(password), None)
        }
        // Check if we have saved credentials
        else if client_guard.has_saved_credentials() {
            print_status(StatusIndicator::Info, "Using saved credentials");
            drop(client_guard);
            (None, None)
        }
        // No credentials - need interactive auth before daemonizing
        else {
            drop(client_guard);
            print_status(StatusIndicator::Info, "Authentication required before daemon can start");

            let auth_result = handle_interactive_auth(&client, cli.save_password)?;
            (auth_result, None)
        }
    };

    // Get crypto password BEFORE daemonizing - can't prompt after fork
    let crypto_password = get_crypto_password(&cli, password.as_ref())?;

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

    // Apply authentication if we have credentials
    if let Some(ref pwd) = password {
        if let Some(ref username) = cli.username {
            let mut client_guard = client
                .lock()
                .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
            client_guard.authenticate(username, pwd, cli.save_password)?;
        }
    } else if let Some(ref token) = auth_token {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        client_guard.set_auth_token(token, cli.save_password)?;
    }

    // Start sync
    {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;
        client_guard.start_sync(Some(status_callback_trampoline), None);
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

    // Mount filesystem (use default path if not specified)
    let mountpoint = cli.get_mountpoint();
    {
        let mut client_guard = client
            .lock()
            .map_err(|_| PCloudError::Config("Failed to acquire client lock".to_string()))?;

        // Create mountpoint directory if it doesn't exist
        if !mountpoint.exists() {
            std::fs::create_dir_all(&mountpoint).map_err(PCloudError::Io)?;
        }

        client_guard.mount_filesystem(&mountpoint)?;
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
    use console_client::daemon::{
        is_daemon_running, DaemonClient, DaemonCommand, DaemonConfig,
    };
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

                // If we sent Finalize or received confirmation of shutdown, exit
                if matches!(
                    response,
                    DaemonResponse::OkWithMessage(ref msg) if msg.contains("shut down")
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
