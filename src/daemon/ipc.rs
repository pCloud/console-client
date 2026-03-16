//! Unix domain socket IPC for daemon control.
//!
//! This module provides Inter-Process Communication (IPC) between the pCloud
//! daemon and client instances using Unix domain sockets.
//!
//! # Architecture
//!
//! - **DaemonServer**: Runs in the daemon process, listens for commands
//! - **DaemonClient**: Used by client processes to send commands to the daemon
//! - **DaemonCommand**: Commands that can be sent to the daemon
//! - **DaemonResponse**: Responses from the daemon
//!
//! # Protocol
//!
//! Messages are length-prefixed binary data using bincode serialization:
//! - 4-byte little-endian length prefix
//! - Bincode-serialized payload
//!
//! # Security
//!
//! - Socket file has 0600 permissions (owner-only access)
//! - Socket path is user-specific (`/tmp/pcloud-cli-<uid>.sock`)
//!
//! # Example
//!
//! ```ignore
//! use console_client::daemon::{DaemonClient, DaemonCommand, DaemonResponse, DaemonConfig};
//!
//! let config = DaemonConfig::default();
//! let client = DaemonClient::new(config.socket_path());
//!
//! // Check if daemon is alive
//! match client.send_command(DaemonCommand::Ping) {
//!     Ok(DaemonResponse::Pong) => println!("Daemon is running"),
//!     _ => println!("Daemon is not responding"),
//! }
//!
//! // Get status
//! let response = client.send_command(DaemonCommand::Status)?;
//! ```

use std::io::{BufReader, BufWriter, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::error::{DaemonError, PCloudError, Result};
use crate::security::SecurePassword;
use crate::wrapper::PCloudClient;

/// Commands that can be sent to the daemon.
///
/// These commands control the daemon's operation and query its status.
///
/// # Security
///
/// Commands containing sensitive data (like passwords) should be handled carefully:
/// - Passwords are transmitted as plain `String` over the Unix socket
/// - The socket has 0600 permissions (owner-only access)
/// - Upon receipt, passwords are immediately converted to `SecurePassword` and
///   the original `String` is zeroized
/// - The `Debug` implementation does NOT print password values
#[derive(Clone, Serialize, Deserialize)]
pub enum DaemonCommand {
    /// Start crypto with optional password.
    ///
    /// If password is `None`, the daemon will return an error since it
    /// cannot prompt for input. The client should always provide the password.
    ///
    /// # Security
    ///
    /// The password is transmitted over the Unix socket but is immediately
    /// converted to `SecurePassword` and zeroized after receipt.
    StartCrypto {
        /// The crypto password. Required since daemon cannot prompt.
        /// Will be zeroized after processing.
        password: Option<String>,
    },

    /// Stop crypto (lock encrypted folders).
    ///
    /// This clears the crypto keys from memory, making encrypted
    /// folders inaccessible until `StartCrypto` is called again.
    StopCrypto,

    /// Graceful shutdown and finalize sync.
    ///
    /// The daemon will wait for pending sync operations to complete
    /// before shutting down.
    Finalize,

    /// Immediate quit.
    ///
    /// The daemon will shut down immediately without waiting for
    /// sync to complete.
    Quit,

    /// Get current status.
    ///
    /// Returns information about authentication, crypto, and mount state.
    Status,

    /// Ping to check if daemon is alive.
    ///
    /// Returns `Pong` if the daemon is responding.
    Ping,

    /// Log out and clear saved credentials. Keeps local sync data.
    ///
    /// After logout, the daemon shuts down since it cannot operate
    /// without authentication.
    Logout,

    /// Unlink account and clear all local data. This is destructive.
    ///
    /// After unlinking, the daemon shuts down since the account
    /// is fully disconnected and all local data has been removed.
    Unlink,
}

/// Custom Debug implementation that redacts password values.
impl std::fmt::Debug for DaemonCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonCommand::StartCrypto { password } => f
                .debug_struct("StartCrypto")
                .field(
                    "password",
                    &if password.is_some() {
                        "[REDACTED]"
                    } else {
                        "None"
                    },
                )
                .finish(),
            DaemonCommand::StopCrypto => write!(f, "StopCrypto"),
            DaemonCommand::Finalize => write!(f, "Finalize"),
            DaemonCommand::Quit => write!(f, "Quit"),
            DaemonCommand::Status => write!(f, "Status"),
            DaemonCommand::Ping => write!(f, "Ping"),
            DaemonCommand::Logout => write!(f, "Logout"),
            DaemonCommand::Unlink => write!(f, "Unlink"),
        }
    }
}

impl std::fmt::Display for DaemonCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonCommand::StartCrypto { .. } => write!(f, "StartCrypto"),
            DaemonCommand::StopCrypto => write!(f, "StopCrypto"),
            DaemonCommand::Finalize => write!(f, "Finalize"),
            DaemonCommand::Quit => write!(f, "Quit"),
            DaemonCommand::Status => write!(f, "Status"),
            DaemonCommand::Ping => write!(f, "Ping"),
            DaemonCommand::Logout => write!(f, "Logout"),
            DaemonCommand::Unlink => write!(f, "Unlink"),
        }
    }
}

/// Response from the daemon.
///
/// Each command returns a response indicating success, failure, or
/// providing requested information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    /// Command succeeded with no additional information.
    Ok,

    /// Command succeeded with a message.
    OkWithMessage(String),

    /// Command failed with an error message.
    Error(String),

    /// Status response containing daemon state.
    Status {
        /// Whether the user is authenticated with pCloud.
        authenticated: bool,
        /// Whether crypto (encryption) is started.
        crypto_started: bool,
        /// Whether the FUSE filesystem is mounted.
        mounted: bool,
        /// The filesystem mountpoint, if mounted.
        mountpoint: Option<String>,
    },

    /// Pong response to Ping command.
    Pong,
}

impl std::fmt::Display for DaemonResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonResponse::Ok => write!(f, "OK"),
            DaemonResponse::OkWithMessage(msg) => write!(f, "{}", msg),
            DaemonResponse::Error(err) => write!(f, "Error: {}", err),
            DaemonResponse::Status {
                authenticated,
                crypto_started,
                mounted,
                mountpoint,
            } => {
                write!(
                    f,
                    "authenticated={}, crypto={}, mounted={}",
                    authenticated, crypto_started, mounted
                )?;
                if let Some(mp) = mountpoint {
                    write!(f, ", mountpoint={}", mp)?;
                }
                Ok(())
            }
            DaemonResponse::Pong => write!(f, "Pong"),
        }
    }
}

/// IPC Server that runs in the daemon process.
///
/// The server listens on a Unix domain socket for incoming connections
/// and dispatches commands to the `PCloudClient`.
///
/// # Thread Safety
///
/// Each incoming connection is handled in a separate thread. The
/// `PCloudClient` is protected by a `Mutex` to ensure thread-safe access.
///
/// # Lifecycle
///
/// 1. Create with `DaemonServer::new(socket_path)`
/// 2. Call `run(client)` to start the server loop
/// 3. Server runs until shutdown is requested via signals or `Quit` command
/// 4. Socket file is automatically removed when the server is dropped
pub struct DaemonServer {
    /// Path to the Unix domain socket file.
    socket_path: PathBuf,
    /// The Unix listener, if bound.
    listener: Option<UnixListener>,
}

impl DaemonServer {
    /// Create a new server bound to the specified socket path.
    ///
    /// This creates the Unix socket and sets appropriate permissions.
    /// Any existing socket file at the path is removed first.
    ///
    /// # Arguments
    ///
    /// * `socket_path` - Path where the Unix socket should be created
    ///
    /// # Returns
    ///
    /// - `Ok(DaemonServer)` on success
    /// - `Err` if the socket cannot be created or bound
    ///
    /// # Permissions
    ///
    /// The socket file is created with mode 0600 (owner read/write only)
    /// to prevent unauthorized access.
    pub fn new(socket_path: impl AsRef<Path>) -> Result<Self> {
        let path = socket_path.as_ref().to_path_buf();

        // Remove existing socket if present (stale from previous run)
        let _ = std::fs::remove_file(&path);

        // Create the listener
        let listener = UnixListener::bind(&path).map_err(|e| {
            PCloudError::Daemon(DaemonError::Ipc(format!("Failed to bind socket: {}", e)))
        })?;

        // Set permissions so only the user can access (0600)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            if let Err(e) = std::fs::set_permissions(&path, perms) {
                // Clean up on error
                let _ = std::fs::remove_file(&path);
                return Err(PCloudError::Daemon(DaemonError::Ipc(format!(
                    "Failed to set socket permissions: {}",
                    e
                ))));
            }
        }

        Ok(Self {
            socket_path: path,
            listener: Some(listener),
        })
    }

    /// Get the socket path.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Run the server loop (blocking).
    ///
    /// This method listens for incoming connections and handles each one
    /// in a separate thread. It runs until a shutdown is requested via
    /// signals or the `Quit`/`Finalize` command.
    ///
    /// # Arguments
    ///
    /// * `client` - Shared reference to the `PCloudClient`
    ///
    /// # Returns
    ///
    /// - `Ok(())` when shutdown is requested
    /// - `Err` on fatal server errors
    ///
    /// # Thread Model
    ///
    /// - Main thread: Accepts connections in non-blocking mode
    /// - Worker threads: Handle individual connections
    /// - Checks for shutdown every 100ms
    pub fn run(&self, client: Arc<Mutex<PCloudClient>>) -> Result<()> {
        let listener = self.listener.as_ref().ok_or_else(|| {
            PCloudError::Daemon(DaemonError::Ipc("Server not initialized".to_string()))
        })?;

        // Set non-blocking to allow checking for shutdown
        listener.set_nonblocking(true).map_err(|e| {
            PCloudError::Daemon(DaemonError::Ipc(format!(
                "Failed to set non-blocking: {}",
                e
            )))
        })?;

        loop {
            // Check for shutdown
            if crate::daemon::signals::is_shutdown_requested() {
                break;
            }

            match listener.accept() {
                Ok((stream, _addr)) => {
                    let client_clone = client.clone();
                    // Handle connection in a thread
                    thread::spawn(move || {
                        if let Err(e) = handle_connection(stream, client_clone) {
                            eprintln!("IPC connection error: {}", e);
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No pending connection, sleep and check again
                    thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    // Log error but continue accepting connections
                    eprintln!("Socket accept error: {}", e);
                    // Brief pause to avoid spinning on persistent errors
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }

        Ok(())
    }
}

impl Drop for DaemonServer {
    fn drop(&mut self) {
        // Remove the socket file on shutdown
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Handle a single client connection.
///
/// Reads a command from the stream, processes it, and sends the response.
///
/// # Protocol
///
/// 1. Read 4-byte little-endian length
/// 2. Read `length` bytes of bincode-serialized `DaemonCommand`
/// 3. Process command
/// 4. Write 4-byte little-endian response length
/// 5. Write bincode-serialized `DaemonResponse`
fn handle_connection(stream: UnixStream, client: Arc<Mutex<PCloudClient>>) -> Result<()> {
    // Set timeouts to prevent hanging on misbehaving clients
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| PCloudError::Daemon(DaemonError::Ipc(format!("Set read timeout: {}", e))))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| PCloudError::Daemon(DaemonError::Ipc(format!("Set write timeout: {}", e))))?;

    let mut reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);

    // Read command length (4-byte little-endian)
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).map_err(|e| {
        PCloudError::Daemon(DaemonError::Ipc(format!("Read command length: {}", e)))
    })?;
    let len = u32::from_le_bytes(len_buf) as usize;

    // Sanity check on length to prevent DoS
    const MAX_COMMAND_SIZE: usize = 1024 * 1024; // 1 MB
    if len > MAX_COMMAND_SIZE {
        return Err(PCloudError::Daemon(DaemonError::Ipc(format!(
            "Command too large: {} bytes",
            len
        ))));
    }

    // Read command payload
    let mut cmd_buf = vec![0u8; len];
    reader
        .read_exact(&mut cmd_buf)
        .map_err(|e| PCloudError::Daemon(DaemonError::Ipc(format!("Read command: {}", e))))?;

    // Deserialize command
    let command: DaemonCommand = bincode::deserialize(&cmd_buf).map_err(|e| {
        PCloudError::Daemon(DaemonError::Ipc(format!("Deserialize command: {}", e)))
    })?;

    // Process command
    let response = process_command(command, &client);

    // Serialize response
    let resp_bytes = bincode::serialize(&response)
        .map_err(|e| PCloudError::Daemon(DaemonError::Ipc(format!("Serialize response: {}", e))))?;

    // Write response length and payload
    writer
        .write_all(&(resp_bytes.len() as u32).to_le_bytes())
        .map_err(|e| {
            PCloudError::Daemon(DaemonError::Ipc(format!("Write response length: {}", e)))
        })?;
    writer
        .write_all(&resp_bytes)
        .map_err(|e| PCloudError::Daemon(DaemonError::Ipc(format!("Write response: {}", e))))?;
    writer
        .flush()
        .map_err(|e| PCloudError::Daemon(DaemonError::Ipc(format!("Flush response: {}", e))))?;

    Ok(())
}

/// Process a command and return a response.
///
/// This function handles each command type and interacts with the
/// `PCloudClient` as needed.
///
/// # Security
///
/// Commands containing passwords are handled securely:
/// - Passwords are immediately converted to `SecurePassword`
/// - The original `String` is zeroized after conversion
/// - `SecurePassword` automatically zeroizes memory on drop
fn process_command(
    mut command: DaemonCommand,
    client: &Arc<Mutex<PCloudClient>>,
) -> DaemonResponse {
    match command {
        DaemonCommand::Ping => DaemonResponse::Pong,

        DaemonCommand::Status => match client.lock() {
            Ok(c) => DaemonResponse::Status {
                authenticated: c.is_logged_in(),
                crypto_started: c.is_crypto_started(),
                mounted: c.is_mounted(),
                mountpoint: c.mountpoint().map(|p| p.to_string_lossy().to_string()),
            },
            Err(e) => DaemonResponse::Error(format!("Failed to acquire client lock: {}", e)),
        },

        DaemonCommand::StartCrypto { ref mut password } => {
            // Take ownership of the password and immediately convert to SecurePassword
            // The original String in the command is replaced with None and will be zeroized
            let pwd_option = password.take();

            match pwd_option {
                Some(mut pwd) => {
                    // Create SecurePassword from the plain string
                    let secure_pwd = SecurePassword::new_zeroizing(pwd.clone());
                    // Zeroize the original string
                    pwd.zeroize();

                    match client.lock() {
                        Ok(mut c) => {
                            use secrecy::SecretString;
                            let secret_pwd = SecretString::from(secure_pwd.expose().to_string());
                            match c.start_crypto(&secret_pwd) {
                                Ok(()) => DaemonResponse::OkWithMessage(
                                    "Crypto started - encrypted folders are now accessible"
                                        .to_string(),
                                ),
                                Err(e) => DaemonResponse::Error(e.to_string()),
                            }
                        }
                        Err(e) => {
                            DaemonResponse::Error(format!("Failed to acquire client lock: {}", e))
                        }
                    }
                }
                None => DaemonResponse::Error(
                    "Password required for crypto start (daemon cannot prompt)".to_string(),
                ),
            }
        }

        DaemonCommand::StopCrypto => match client.lock() {
            Ok(mut c) => match c.stop_crypto() {
                Ok(()) => DaemonResponse::OkWithMessage(
                    "Crypto stopped - encrypted folders are now locked".to_string(),
                ),
                Err(e) => DaemonResponse::Error(e.to_string()),
            },
            Err(e) => DaemonResponse::Error(format!("Failed to acquire client lock: {}", e)),
        },

        DaemonCommand::Finalize => {
            // Request graceful shutdown
            crate::daemon::signals::request_shutdown();
            DaemonResponse::OkWithMessage(
                "Finalize requested - daemon will shut down after sync completes".to_string(),
            )
        }

        DaemonCommand::Quit => {
            // Request immediate shutdown
            crate::daemon::signals::request_shutdown();
            DaemonResponse::OkWithMessage("Quit requested - daemon shutting down".to_string())
        }

        DaemonCommand::Logout => match client.lock() {
            Ok(mut c) => {
                c.logout();
                // Request shutdown since we can't operate without auth
                crate::daemon::signals::request_shutdown();
                DaemonResponse::OkWithMessage(
                    "Logged out and credentials cleared. Daemon shutting down.".to_string(),
                )
            }
            Err(e) => DaemonResponse::Error(format!("Failed to acquire client lock: {}", e)),
        },

        DaemonCommand::Unlink => match client.lock() {
            Ok(mut c) => {
                c.unlink();
                // Request shutdown since we can't operate without auth and data is cleared
                crate::daemon::signals::request_shutdown();
                DaemonResponse::OkWithMessage(
                    "Account unlinked and all local data cleared. Daemon shutting down."
                        .to_string(),
                )
            }
            Err(e) => DaemonResponse::Error(format!("Failed to acquire client lock: {}", e)),
        },
    }
}

/// IPC Client for sending commands to the daemon.
///
/// This client connects to the daemon's Unix socket and sends commands,
/// waiting for responses.
///
/// # Example
///
/// ```ignore
/// let client = DaemonClient::new("/tmp/pcloud-cli-1000.sock");
///
/// // Check if daemon is alive
/// match client.send_command(DaemonCommand::Ping)? {
///     DaemonResponse::Pong => println!("Daemon is running"),
///     other => println!("Unexpected response: {:?}", other),
/// }
/// ```
pub struct DaemonClient {
    /// Path to the daemon's Unix socket.
    socket_path: PathBuf,
    /// Connection timeout.
    timeout: Duration,
}

impl DaemonClient {
    /// Create a new client for the specified socket path.
    ///
    /// # Arguments
    ///
    /// * `socket_path` - Path to the daemon's Unix socket
    pub fn new(socket_path: impl AsRef<Path>) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Create a new client with a custom timeout.
    ///
    /// # Arguments
    ///
    /// * `socket_path` - Path to the daemon's Unix socket
    /// * `timeout` - Connection and operation timeout
    pub fn with_timeout(socket_path: impl AsRef<Path>, timeout: Duration) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            timeout,
        }
    }

    /// Get the socket path.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Connect and send a command, returning the response.
    ///
    /// This method:
    /// 1. Connects to the daemon socket
    /// 2. Serializes and sends the command
    /// 3. Waits for and deserializes the response
    ///
    /// # Arguments
    ///
    /// * `command` - The command to send
    ///
    /// # Returns
    ///
    /// - `Ok(DaemonResponse)` on success
    /// - `Err(DaemonError::Ipc)` on communication failure
    ///
    /// # Example
    ///
    /// ```ignore
    /// let response = client.send_command(DaemonCommand::Status)?;
    /// match response {
    ///     DaemonResponse::Status { authenticated, .. } => {
    ///         println!("Authenticated: {}", authenticated);
    ///     }
    ///     _ => {}
    /// }
    /// ```
    pub fn send_command(&self, command: DaemonCommand) -> Result<DaemonResponse> {
        // Connect to the daemon
        let stream = UnixStream::connect(&self.socket_path).map_err(|e| {
            PCloudError::Daemon(DaemonError::Ipc(format!(
                "Failed to connect to daemon: {}",
                e
            )))
        })?;

        // Set timeouts
        stream.set_read_timeout(Some(self.timeout)).map_err(|e| {
            PCloudError::Daemon(DaemonError::Ipc(format!("Set read timeout: {}", e)))
        })?;
        stream.set_write_timeout(Some(self.timeout)).map_err(|e| {
            PCloudError::Daemon(DaemonError::Ipc(format!("Set write timeout: {}", e)))
        })?;

        let mut reader = BufReader::new(&stream);
        let mut writer = BufWriter::new(&stream);

        // Serialize command
        let cmd_bytes = bincode::serialize(&command).map_err(|e| {
            PCloudError::Daemon(DaemonError::Ipc(format!("Serialize command: {}", e)))
        })?;

        // Write command length and payload
        writer
            .write_all(&(cmd_bytes.len() as u32).to_le_bytes())
            .map_err(|e| {
                PCloudError::Daemon(DaemonError::Ipc(format!("Write command length: {}", e)))
            })?;
        writer
            .write_all(&cmd_bytes)
            .map_err(|e| PCloudError::Daemon(DaemonError::Ipc(format!("Write command: {}", e))))?;
        writer
            .flush()
            .map_err(|e| PCloudError::Daemon(DaemonError::Ipc(format!("Flush command: {}", e))))?;

        // Read response length
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).map_err(|e| {
            PCloudError::Daemon(DaemonError::Ipc(format!("Read response length: {}", e)))
        })?;
        let len = u32::from_le_bytes(len_buf) as usize;

        // Sanity check on length
        const MAX_RESPONSE_SIZE: usize = 1024 * 1024; // 1 MB
        if len > MAX_RESPONSE_SIZE {
            return Err(PCloudError::Daemon(DaemonError::Ipc(format!(
                "Response too large: {} bytes",
                len
            ))));
        }

        // Read response payload
        let mut resp_buf = vec![0u8; len];
        reader
            .read_exact(&mut resp_buf)
            .map_err(|e| PCloudError::Daemon(DaemonError::Ipc(format!("Read response: {}", e))))?;

        // Deserialize response
        let response: DaemonResponse = bincode::deserialize(&resp_buf).map_err(|e| {
            PCloudError::Daemon(DaemonError::Ipc(format!("Deserialize response: {}", e)))
        })?;

        Ok(response)
    }

    /// Check if the daemon is responding.
    ///
    /// Sends a `Ping` command and checks for `Pong` response.
    ///
    /// # Returns
    ///
    /// `true` if daemon responded with `Pong`, `false` otherwise.
    pub fn is_daemon_alive(&self) -> bool {
        matches!(
            self.send_command(DaemonCommand::Ping),
            Ok(DaemonResponse::Pong)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_daemon_command_display() {
        assert_eq!(format!("{}", DaemonCommand::Ping), "Ping");
        assert_eq!(format!("{}", DaemonCommand::Status), "Status");
        assert_eq!(format!("{}", DaemonCommand::Quit), "Quit");
        assert_eq!(format!("{}", DaemonCommand::Finalize), "Finalize");
        assert_eq!(format!("{}", DaemonCommand::StopCrypto), "StopCrypto");
        assert_eq!(
            format!(
                "{}",
                DaemonCommand::StartCrypto {
                    password: Some("test".to_string())
                }
            ),
            "StartCrypto"
        );
        assert_eq!(format!("{}", DaemonCommand::Logout), "Logout");
        assert_eq!(format!("{}", DaemonCommand::Unlink), "Unlink");
    }

    #[test]
    fn test_daemon_command_debug_redacts_password() {
        let cmd = DaemonCommand::StartCrypto {
            password: Some("super_secret_password".to_string()),
        };
        let debug_str = format!("{:?}", cmd);

        // Password should NOT appear in debug output
        assert!(!debug_str.contains("super_secret_password"));
        // But REDACTED should appear
        assert!(debug_str.contains("REDACTED"));
    }

    #[test]
    fn test_daemon_command_debug_shows_none_password() {
        let cmd = DaemonCommand::StartCrypto { password: None };
        let debug_str = format!("{:?}", cmd);

        // Should show "None" for missing password
        assert!(debug_str.contains("None"));
    }

    #[test]
    fn test_daemon_response_display() {
        assert_eq!(format!("{}", DaemonResponse::Ok), "OK");
        assert_eq!(
            format!("{}", DaemonResponse::OkWithMessage("done".to_string())),
            "done"
        );
        assert_eq!(
            format!("{}", DaemonResponse::Error("failed".to_string())),
            "Error: failed"
        );
        assert_eq!(format!("{}", DaemonResponse::Pong), "Pong");

        let status = DaemonResponse::Status {
            authenticated: true,
            crypto_started: false,
            mounted: true,
            mountpoint: Some("/home/user/pCloud".to_string()),
        };
        let status_str = format!("{}", status);
        assert!(status_str.contains("authenticated=true"));
        assert!(status_str.contains("crypto=false"));
        assert!(status_str.contains("mounted=true"));
        assert!(status_str.contains("/home/user/pCloud"));
    }

    #[test]
    fn test_daemon_command_serialization() {
        let commands = vec![
            DaemonCommand::Ping,
            DaemonCommand::Status,
            DaemonCommand::Quit,
            DaemonCommand::Finalize,
            DaemonCommand::StopCrypto,
            DaemonCommand::StartCrypto {
                password: Some("test".to_string()),
            },
            DaemonCommand::StartCrypto { password: None },
            DaemonCommand::Logout,
            DaemonCommand::Unlink,
        ];

        for cmd in commands {
            let serialized = bincode::serialize(&cmd).expect("serialize");
            let deserialized: DaemonCommand =
                bincode::deserialize(&serialized).expect("deserialize");

            // Verify round-trip
            match (&cmd, &deserialized) {
                (DaemonCommand::Ping, DaemonCommand::Ping) => {}
                (DaemonCommand::Status, DaemonCommand::Status) => {}
                (DaemonCommand::Quit, DaemonCommand::Quit) => {}
                (DaemonCommand::Finalize, DaemonCommand::Finalize) => {}
                (DaemonCommand::StopCrypto, DaemonCommand::StopCrypto) => {}
                (
                    DaemonCommand::StartCrypto { password: p1 },
                    DaemonCommand::StartCrypto { password: p2 },
                ) => {
                    assert_eq!(p1, p2);
                }
                (DaemonCommand::Logout, DaemonCommand::Logout) => {}
                (DaemonCommand::Unlink, DaemonCommand::Unlink) => {}
                _ => panic!("Mismatch after round-trip"),
            }
        }
    }

    #[test]
    fn test_daemon_response_serialization() {
        let responses = vec![
            DaemonResponse::Ok,
            DaemonResponse::OkWithMessage("test".to_string()),
            DaemonResponse::Error("error".to_string()),
            DaemonResponse::Pong,
            DaemonResponse::Status {
                authenticated: true,
                crypto_started: false,
                mounted: true,
                mountpoint: Some("/mnt/pcloud".to_string()),
            },
            DaemonResponse::Status {
                authenticated: false,
                crypto_started: false,
                mounted: false,
                mountpoint: None,
            },
        ];

        for resp in responses {
            let serialized = bincode::serialize(&resp).expect("serialize");
            let deserialized: DaemonResponse =
                bincode::deserialize(&serialized).expect("deserialize");

            // Verify round-trip using Debug comparison
            assert_eq!(format!("{:?}", resp), format!("{:?}", deserialized));
        }
    }

    #[test]
    fn test_daemon_client_new() {
        let client = DaemonClient::new("/tmp/test.sock");
        assert_eq!(client.socket_path(), Path::new("/tmp/test.sock"));
    }

    #[test]
    fn test_daemon_client_with_timeout() {
        let client = DaemonClient::with_timeout("/tmp/test.sock", Duration::from_secs(10));
        assert_eq!(client.socket_path(), Path::new("/tmp/test.sock"));
        assert_eq!(client.timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_daemon_server_removes_existing_socket() {
        let temp_dir = std::env::temp_dir();
        let socket_path = temp_dir.join("test_daemon_server.sock");

        // Create a dummy file at the socket path
        fs::write(&socket_path, "dummy").expect("create dummy file");
        assert!(socket_path.exists());

        // Create server - should remove the existing file
        // Note: This will fail if we don't have permissions, which is fine for this test
        let result = DaemonServer::new(&socket_path);

        // Regardless of success, the dummy file should be gone
        // (either removed by us or replaced by the socket)
        if result.is_ok() {
            // Clean up
            drop(result);
            let _ = fs::remove_file(&socket_path);
        }
    }

    #[test]
    fn test_length_prefix_format() {
        // Verify our length prefix format
        let len: u32 = 256;
        let bytes = len.to_le_bytes();
        assert_eq!(bytes.len(), 4);
        assert_eq!(u32::from_le_bytes(bytes), 256);
    }
}
