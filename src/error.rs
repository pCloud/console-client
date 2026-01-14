//! Error types for the pCloud console client.
//!
//! This module defines the error hierarchy for the application:
//! - `PCloudError`: Top-level error enum for all pCloud operations
//! - Specialized error types for FFI, authentication, crypto, etc.
//!
//! All errors implement `std::error::Error` via the `thiserror` crate.

use thiserror::Error;

/// Main error type for pCloud operations.
///
/// This enum encompasses all possible errors that can occur during
/// pCloud client operations, from FFI boundary errors to high-level
/// application errors.
#[derive(Error, Debug)]
pub enum PCloudError {
    /// Error occurred at the FFI boundary with the pclsync C library
    #[error("FFI error: {0}")]
    Ffi(#[from] FfiError),

    /// Authentication-related error
    #[error("Authentication error: {0}")]
    Auth(#[from] AuthError),

    /// Crypto (encryption) operation error
    #[error("Crypto error: {0}")]
    Crypto(#[from] CryptoError),

    /// Filesystem operation error
    #[error("Filesystem error: {0}")]
    Filesystem(#[from] FilesystemError),

    /// Daemon/IPC operation error
    #[error("Daemon error: {0}")]
    Daemon(#[from] DaemonError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Generic I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid argument provided
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Operation not supported on this platform
    #[error("Not supported: {0}")]
    NotSupported(String),

    /// C string conversion error (null byte in string)
    #[error("C string error: {0}")]
    CString(#[from] std::ffi::NulError),
}

/// Errors that occur at the FFI boundary with the pclsync C library.
#[derive(Error, Debug)]
pub enum FfiError {
    /// Library initialization failed
    #[error("Failed to initialize pclsync library: {message} (error code: {code})")]
    InitFailed { code: u32, message: String },

    /// Null pointer was returned from C function
    #[error("Null pointer returned from C function: {context}")]
    NullPointer { context: &'static str },

    /// Invalid UTF-8 string from C
    #[error("Invalid UTF-8 in string from C library")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    /// C string conversion error
    #[error("C string conversion error: {0}")]
    CStringError(#[from] std::ffi::NulError),

    /// C function returned an error code
    #[error("C function returned error code {code}: {message}")]
    CError { code: i32, message: String },

    /// Callback registration failed
    #[error("Failed to register callback: {0}")]
    CallbackRegistration(String),

    /// Library not initialized
    #[error("pclsync library not initialized")]
    NotInitialized,

    /// Library already initialized
    #[error("pclsync library already initialized")]
    AlreadyInitialized,
}

/// Authentication-related errors.
#[derive(Error, Debug)]
pub enum AuthError {
    /// Login is required before this operation
    #[error("Login required")]
    LoginRequired,

    /// Bad username or password
    #[error("Invalid credentials")]
    BadCredentials,

    /// Invalid or expired authentication token
    #[error("Invalid or expired authentication token")]
    BadToken,

    /// User account is full
    #[error("Account storage is full")]
    AccountFull,

    /// User mismatch - trying to login as different user
    #[error("User mismatch: cannot login as different user without unlinking first")]
    UserMismatch,

    /// Registration failed
    #[error("Registration failed: {0}")]
    RegistrationFailed(String),

    /// Email verification required
    #[error("Email verification required")]
    EmailVerificationRequired,

    /// Network error during authentication
    #[error("Network error during authentication")]
    NetworkError,

    /// Generic authentication error with message
    #[error("Authentication error: {0}")]
    Other(String),
}

/// Crypto (encryption) operation errors.
///
/// These map to the PSYNC_CRYPTO_* error codes from pclsync.
#[derive(Error, Debug)]
pub enum CryptoError {
    /// Crypto is not supported
    #[error("Crypto not supported")]
    NotSupported,

    /// Crypto setup failed - key generation error
    #[error("Key generation failed during crypto setup")]
    KeyGenFailed,

    /// Cannot connect to server for crypto operation
    #[error("Cannot connect to server for crypto operation")]
    CantConnect,

    /// Must be logged in for crypto operations
    #[error("Not logged in - crypto operations require authentication")]
    NotLoggedIn,

    /// Crypto is already set up for this account
    #[error("Crypto already set up")]
    AlreadySetup,

    /// Crypto has not been set up yet
    #[error("Crypto not set up - run crypto setup first")]
    NotSetup,

    /// Crypto is already started
    #[error("Crypto already started")]
    AlreadyStarted,

    /// Crypto is not started
    #[error("Crypto not started")]
    NotStarted,

    /// Unknown key format encountered
    #[error("Unknown key format")]
    UnknownKeyFormat,

    /// Wrong crypto password provided
    #[error("Bad crypto password")]
    BadPassword,

    /// Crypto keys don't match
    #[error("Crypto keys don't match")]
    KeysDontMatch,

    /// RSA operation error
    #[error("RSA operation failed")]
    RsaError,

    /// Folder not found for crypto operation
    #[error("Folder not found")]
    FolderNotFound,

    /// File not found for crypto operation
    #[error("File not found")]
    FileNotFound,

    /// Invalid encryption key
    #[error("Invalid encryption key")]
    InvalidKey,

    /// Folder is not encrypted
    #[error("Folder is not encrypted")]
    FolderNotEncrypted,

    /// Internal crypto error
    #[error("Internal crypto error")]
    InternalError,

    /// Unknown crypto error
    #[error("Unknown crypto error (code: {0})")]
    Unknown(i32),
}

/// Filesystem operation errors.
///
/// These map to the PERROR_* constants from pclsync.
#[derive(Error, Debug)]
pub enum FilesystemError {
    /// Local folder not found
    #[error("Local folder not found: {0}")]
    LocalFolderNotFound(String),

    /// Remote folder not found
    #[error("Remote folder not found: {0}")]
    RemoteFolderNotFound(String),

    /// Database could not be opened
    #[error("Failed to open database")]
    DatabaseOpen,

    /// No home directory found
    #[error("No home directory found")]
    NoHomeDir,

    /// SSL initialization failed
    #[error("SSL initialization failed")]
    SslInitFailed,

    /// Database error
    #[error("Database error")]
    DatabaseError,

    /// Access denied to local folder
    #[error("Access denied to local folder: {0}")]
    LocalFolderAccessDenied(String),

    /// Access denied to remote folder
    #[error("Access denied to remote folder")]
    RemoteFolderAccessDenied,

    /// Folder is already being synced
    #[error("Folder already syncing")]
    FolderAlreadySyncing,

    /// Invalid sync type specified
    #[error("Invalid sync type")]
    InvalidSyncType,

    /// Device is offline
    #[error("Device is offline")]
    Offline,

    /// Invalid sync ID
    #[error("Invalid sync ID")]
    InvalidSyncId,

    /// Parent or subfolder is already syncing
    #[error("Parent or subfolder already syncing")]
    ParentOrSubfolderAlreadySyncing,

    /// Local path is on pCloud Drive (not allowed)
    #[error("Local path is on pCloud Drive - not allowed")]
    LocalIsOnPDrive,

    /// Mount point error
    #[error("Mount point error: {0}")]
    MountPoint(String),

    /// FUSE filesystem error
    #[error("FUSE error: {0}")]
    Fuse(String),

    /// Mountpoint does not exist
    #[error("Mountpoint not found: {0}")]
    MountpointNotFound(std::path::PathBuf),

    /// Path is not a directory
    #[error("Not a directory: {0}")]
    NotADirectory(std::path::PathBuf),

    /// Filesystem is already mounted
    #[error("Filesystem already mounted")]
    AlreadyMounted,

    /// Filesystem is not mounted
    #[error("Filesystem not mounted")]
    NotMounted,

    /// Mount operation failed
    #[error("Mount failed with error code: {0}")]
    MountFailed(i32),

    /// Unmount operation failed
    #[error("Unmount failed with error code: {0}")]
    UnmountFailed(i32),

    /// Invalid mountpoint path (e.g., non-UTF8)
    #[error("Invalid mountpoint path")]
    InvalidPath,

    /// Unknown filesystem error
    #[error("Unknown filesystem error (code: {0})")]
    Unknown(i32),
}

/// Daemon and IPC operation errors.
#[derive(Error, Debug)]
pub enum DaemonError {
    /// Failed to daemonize the process
    #[error("Failed to daemonize: {0}")]
    DaemonizeFailed(String),

    /// PID file error
    #[error("PID file error: {0}")]
    PidFile(String),

    /// IPC socket error
    #[error("IPC socket error: {0}")]
    Socket(String),

    /// IPC communication error
    #[error("IPC error: {0}")]
    Ipc(String),

    /// Failed to connect to daemon
    #[error("Failed to connect to daemon - is it running?")]
    ConnectionFailed,

    /// Daemon is not running
    #[error("Daemon is not running")]
    NotRunning,

    /// Daemon is already running
    #[error("Daemon is already running")]
    AlreadyRunning,

    /// Invalid command received
    #[error("Invalid daemon command: {0}")]
    InvalidCommand(String),

    /// Command execution failed
    #[error("Command failed: {0}")]
    CommandFailed(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Signal handling error
    #[error("Signal handling error: {0}")]
    Signal(String),
}

// Conversion implementations for pclsync error codes

impl FfiError {
    /// Create an InitFailed error from a pclsync error code.
    ///
    /// Maps the error code to a human-readable message based on known
    /// PERROR_* constants from pclsync.
    pub fn init_failed(code: u32) -> Self {
        let message = match code {
            0 => "Unknown error".to_string(),
            1 => "Local folder not found".to_string(),
            2 => "Remote folder not found".to_string(),
            3 => "Failed to open database".to_string(),
            4 => "No home directory found".to_string(),
            5 => "SSL initialization failed".to_string(),
            6 => "Database error".to_string(),
            7 => "Local folder access denied".to_string(),
            8 => "Remote folder access denied".to_string(),
            _ => format!("Unknown initialization error"),
        };
        FfiError::InitFailed { code, message }
    }
}

impl CryptoError {
    /// Convert a pclsync crypto setup error code to a CryptoError.
    pub fn from_setup_code(code: i32) -> Self {
        match code {
            0 => panic!("from_setup_code called with success code"),
            -1 => CryptoError::NotSupported,
            1 => CryptoError::KeyGenFailed,
            2 => CryptoError::CantConnect,
            3 => CryptoError::NotLoggedIn,
            4 => CryptoError::AlreadySetup,
            _ => CryptoError::Unknown(code),
        }
    }

    /// Convert a pclsync crypto start error code to a CryptoError.
    pub fn from_start_code(code: i32) -> Self {
        match code {
            0 => panic!("from_start_code called with success code"),
            -1 => CryptoError::NotSupported,
            1 => CryptoError::AlreadyStarted,
            2 => CryptoError::CantConnect,
            3 => CryptoError::NotLoggedIn,
            4 => CryptoError::NotSetup,
            5 => CryptoError::UnknownKeyFormat,
            6 => CryptoError::BadPassword,
            7 => CryptoError::KeysDontMatch,
            _ => CryptoError::Unknown(code),
        }
    }

    /// Convert a pclsync crypto stop error code to a CryptoError.
    pub fn from_stop_code(code: i32) -> Self {
        match code {
            0 => panic!("from_stop_code called with success code"),
            -1 => CryptoError::NotSupported,
            1 => CryptoError::NotStarted,
            _ => CryptoError::Unknown(code),
        }
    }

    /// Convert a pclsync generic crypto error code to a CryptoError.
    pub fn from_generic_code(code: i32) -> Self {
        match code {
            0 => panic!("from_generic_code called with success code"),
            -1 => CryptoError::NotStarted,
            -2 => CryptoError::RsaError,
            -3 => CryptoError::FolderNotFound,
            -4 => CryptoError::FileNotFound,
            -5 => CryptoError::InvalidKey,
            -6 => CryptoError::CantConnect,
            -7 => CryptoError::FolderNotEncrypted,
            -8 => CryptoError::InternalError,
            _ => CryptoError::Unknown(code),
        }
    }
}

impl FilesystemError {
    /// Convert a pclsync error code to a FilesystemError.
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => FilesystemError::LocalFolderNotFound(String::new()),
            2 => FilesystemError::RemoteFolderNotFound(String::new()),
            3 => FilesystemError::DatabaseOpen,
            4 => FilesystemError::NoHomeDir,
            5 => FilesystemError::SslInitFailed,
            6 => FilesystemError::DatabaseError,
            7 => FilesystemError::LocalFolderAccessDenied(String::new()),
            8 => FilesystemError::RemoteFolderAccessDenied,
            9 => FilesystemError::FolderAlreadySyncing,
            10 => FilesystemError::InvalidSyncType,
            11 => FilesystemError::Offline,
            12 => FilesystemError::InvalidSyncId,
            13 => FilesystemError::ParentOrSubfolderAlreadySyncing,
            14 => FilesystemError::LocalIsOnPDrive,
            _ => FilesystemError::Unknown(code as i32),
        }
    }
}

/// Result type alias for pCloud operations.
pub type Result<T> = std::result::Result<T, PCloudError>;

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // PCloudError Tests
    // =========================================================================

    #[test]
    fn test_pcloud_error_display() {
        let err = PCloudError::Auth(AuthError::LoginRequired);
        assert!(err.to_string().contains("Login required"));

        let err = PCloudError::Config("bad config".to_string());
        assert!(err.to_string().contains("bad config"));

        let err = PCloudError::InvalidArgument("invalid".to_string());
        assert!(err.to_string().contains("invalid"));
    }

    #[test]
    fn test_pcloud_error_from_ffi() {
        let ffi_err = FfiError::init_failed(5);
        let pcloud_err: PCloudError = ffi_err.into();
        assert!(matches!(pcloud_err, PCloudError::Ffi(_)));
    }

    #[test]
    fn test_pcloud_error_from_auth() {
        let auth_err = AuthError::BadCredentials;
        let pcloud_err: PCloudError = auth_err.into();
        assert!(matches!(pcloud_err, PCloudError::Auth(_)));
    }

    #[test]
    fn test_pcloud_error_from_crypto() {
        let crypto_err = CryptoError::NotStarted;
        let pcloud_err: PCloudError = crypto_err.into();
        assert!(matches!(pcloud_err, PCloudError::Crypto(_)));
    }

    #[test]
    fn test_pcloud_error_from_filesystem() {
        let fs_err = FilesystemError::NotMounted;
        let pcloud_err: PCloudError = fs_err.into();
        assert!(matches!(pcloud_err, PCloudError::Filesystem(_)));
    }

    #[test]
    fn test_pcloud_error_from_daemon() {
        let daemon_err = DaemonError::NotRunning;
        let pcloud_err: PCloudError = daemon_err.into();
        assert!(matches!(pcloud_err, PCloudError::Daemon(_)));
    }

    #[test]
    fn test_pcloud_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let pcloud_err: PCloudError = io_err.into();
        assert!(matches!(pcloud_err, PCloudError::Io(_)));
    }

    // =========================================================================
    // FFI Error Tests
    // =========================================================================

    #[test]
    fn test_ffi_error_display() {
        let err = FfiError::init_failed(5);
        assert!(err.to_string().contains("initialize"));
        assert!(err.to_string().contains("SSL"));
        assert!(err.to_string().contains("5"));

        let err = FfiError::NullPointer { context: "test" };
        assert!(err.to_string().contains("Null pointer"));
        assert!(err.to_string().contains("test"));

        let err = FfiError::CError {
            code: -1,
            message: "fail".to_string(),
        };
        assert!(err.to_string().contains("-1"));
        assert!(err.to_string().contains("fail"));
    }

    #[test]
    fn test_ffi_error_not_initialized() {
        let err = FfiError::NotInitialized;
        assert!(err.to_string().contains("not initialized"));
    }

    #[test]
    fn test_ffi_error_already_initialized() {
        let err = FfiError::AlreadyInitialized;
        assert!(err.to_string().contains("already initialized"));
    }

    #[test]
    fn test_ffi_error_init_failed_codes() {
        // Test known error codes
        let err = FfiError::init_failed(3);
        assert!(err.to_string().contains("database"));

        let err = FfiError::init_failed(4);
        assert!(err.to_string().contains("home directory"));

        let err = FfiError::init_failed(5);
        assert!(err.to_string().contains("SSL"));

        let err = FfiError::init_failed(6);
        assert!(err.to_string().contains("Database error"));

        // Test unknown error code
        let err = FfiError::init_failed(999);
        assert!(err.to_string().contains("999"));
        assert!(err.to_string().contains("Unknown"));
    }

    // =========================================================================
    // Auth Error Tests
    // =========================================================================

    #[test]
    fn test_auth_error_variants() {
        assert!(AuthError::LoginRequired.to_string().contains("Login"));
        assert!(AuthError::BadCredentials.to_string().contains("credential"));
        assert!(AuthError::BadToken.to_string().contains("token"));
        assert!(AuthError::AccountFull.to_string().contains("full"));
        assert!(AuthError::UserMismatch.to_string().contains("mismatch"));
        assert!(AuthError::NetworkError.to_string().contains("Network"));
        assert!(AuthError::EmailVerificationRequired
            .to_string()
            .contains("verification"));
    }

    #[test]
    fn test_auth_error_registration_failed() {
        let err = AuthError::RegistrationFailed("email exists".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Registration"));
        assert!(msg.contains("email exists"));
    }

    #[test]
    fn test_auth_error_other() {
        let err = AuthError::Other("custom error".to_string());
        assert!(err.to_string().contains("custom error"));
    }

    // =========================================================================
    // Crypto Error Tests
    // =========================================================================

    #[test]
    fn test_crypto_error_from_setup_code() {
        assert!(matches!(
            CryptoError::from_setup_code(-1),
            CryptoError::NotSupported
        ));
        assert!(matches!(
            CryptoError::from_setup_code(1),
            CryptoError::KeyGenFailed
        ));
        assert!(matches!(
            CryptoError::from_setup_code(2),
            CryptoError::CantConnect
        ));
        assert!(matches!(
            CryptoError::from_setup_code(3),
            CryptoError::NotLoggedIn
        ));
        assert!(matches!(
            CryptoError::from_setup_code(4),
            CryptoError::AlreadySetup
        ));
        assert!(matches!(
            CryptoError::from_setup_code(99),
            CryptoError::Unknown(99)
        ));
    }

    #[test]
    fn test_crypto_error_from_start_code() {
        assert!(matches!(
            CryptoError::from_start_code(-1),
            CryptoError::NotSupported
        ));
        assert!(matches!(
            CryptoError::from_start_code(1),
            CryptoError::AlreadyStarted
        ));
        assert!(matches!(
            CryptoError::from_start_code(2),
            CryptoError::CantConnect
        ));
        assert!(matches!(
            CryptoError::from_start_code(3),
            CryptoError::NotLoggedIn
        ));
        assert!(matches!(
            CryptoError::from_start_code(4),
            CryptoError::NotSetup
        ));
        assert!(matches!(
            CryptoError::from_start_code(5),
            CryptoError::UnknownKeyFormat
        ));
        assert!(matches!(
            CryptoError::from_start_code(6),
            CryptoError::BadPassword
        ));
        assert!(matches!(
            CryptoError::from_start_code(7),
            CryptoError::KeysDontMatch
        ));
    }

    #[test]
    fn test_crypto_error_from_stop_code() {
        assert!(matches!(
            CryptoError::from_stop_code(-1),
            CryptoError::NotSupported
        ));
        assert!(matches!(
            CryptoError::from_stop_code(1),
            CryptoError::NotStarted
        ));
        assert!(matches!(
            CryptoError::from_stop_code(99),
            CryptoError::Unknown(99)
        ));
    }

    #[test]
    fn test_crypto_error_from_generic_code() {
        assert!(matches!(
            CryptoError::from_generic_code(-1),
            CryptoError::NotStarted
        ));
        assert!(matches!(
            CryptoError::from_generic_code(-2),
            CryptoError::RsaError
        ));
        assert!(matches!(
            CryptoError::from_generic_code(-3),
            CryptoError::FolderNotFound
        ));
        assert!(matches!(
            CryptoError::from_generic_code(-4),
            CryptoError::FileNotFound
        ));
        assert!(matches!(
            CryptoError::from_generic_code(-5),
            CryptoError::InvalidKey
        ));
        assert!(matches!(
            CryptoError::from_generic_code(-6),
            CryptoError::CantConnect
        ));
        assert!(matches!(
            CryptoError::from_generic_code(-7),
            CryptoError::FolderNotEncrypted
        ));
        assert!(matches!(
            CryptoError::from_generic_code(-8),
            CryptoError::InternalError
        ));
    }

    #[test]
    fn test_crypto_error_display() {
        assert!(CryptoError::NotSupported.to_string().contains("not supported"));
        assert!(CryptoError::BadPassword.to_string().contains("password"));
        assert!(CryptoError::NotStarted.to_string().contains("not started"));
        assert!(CryptoError::AlreadyStarted
            .to_string()
            .contains("already started"));
    }

    // =========================================================================
    // Filesystem Error Tests
    // =========================================================================

    #[test]
    fn test_filesystem_error_from_code() {
        assert!(matches!(
            FilesystemError::from_code(1),
            FilesystemError::LocalFolderNotFound(_)
        ));
        assert!(matches!(
            FilesystemError::from_code(2),
            FilesystemError::RemoteFolderNotFound(_)
        ));
        assert!(matches!(
            FilesystemError::from_code(3),
            FilesystemError::DatabaseOpen
        ));
        assert!(matches!(
            FilesystemError::from_code(4),
            FilesystemError::NoHomeDir
        ));
        assert!(matches!(
            FilesystemError::from_code(5),
            FilesystemError::SslInitFailed
        ));
        assert!(matches!(
            FilesystemError::from_code(6),
            FilesystemError::DatabaseError
        ));
        assert!(matches!(
            FilesystemError::from_code(7),
            FilesystemError::LocalFolderAccessDenied(_)
        ));
        assert!(matches!(
            FilesystemError::from_code(8),
            FilesystemError::RemoteFolderAccessDenied
        ));
        assert!(matches!(
            FilesystemError::from_code(9),
            FilesystemError::FolderAlreadySyncing
        ));
        assert!(matches!(
            FilesystemError::from_code(10),
            FilesystemError::InvalidSyncType
        ));
        assert!(matches!(
            FilesystemError::from_code(11),
            FilesystemError::Offline
        ));
        assert!(matches!(
            FilesystemError::from_code(12),
            FilesystemError::InvalidSyncId
        ));
        assert!(matches!(
            FilesystemError::from_code(13),
            FilesystemError::ParentOrSubfolderAlreadySyncing
        ));
        assert!(matches!(
            FilesystemError::from_code(14),
            FilesystemError::LocalIsOnPDrive
        ));
        assert!(matches!(
            FilesystemError::from_code(999),
            FilesystemError::Unknown(999)
        ));
    }

    #[test]
    fn test_filesystem_error_display() {
        let err = FilesystemError::LocalFolderNotFound("/tmp/test".to_string());
        assert!(err.to_string().contains("/tmp/test"));

        let err = FilesystemError::MountPoint("error".to_string());
        assert!(err.to_string().contains("error"));

        let err = FilesystemError::MountpointNotFound(std::path::PathBuf::from("/mnt/test"));
        assert!(err.to_string().contains("/mnt/test"));

        let err = FilesystemError::NotADirectory(std::path::PathBuf::from("/tmp/file.txt"));
        assert!(err.to_string().contains("/tmp/file.txt"));
    }

    // =========================================================================
    // Daemon Error Tests
    // =========================================================================

    #[test]
    fn test_daemon_error_variants() {
        assert!(DaemonError::NotRunning.to_string().contains("not running"));
        assert!(DaemonError::AlreadyRunning
            .to_string()
            .contains("already running"));
        assert!(DaemonError::ConnectionFailed
            .to_string()
            .contains("connect"));
    }

    #[test]
    fn test_daemon_error_with_message() {
        let err = DaemonError::DaemonizeFailed("fork failed".to_string());
        assert!(err.to_string().contains("fork failed"));

        let err = DaemonError::PidFile("permission denied".to_string());
        assert!(err.to_string().contains("permission denied"));

        let err = DaemonError::Socket("bind failed".to_string());
        assert!(err.to_string().contains("bind failed"));

        let err = DaemonError::Ipc("connection reset".to_string());
        assert!(err.to_string().contains("connection reset"));

        let err = DaemonError::InvalidCommand("unknown".to_string());
        assert!(err.to_string().contains("unknown"));

        let err = DaemonError::CommandFailed("timeout".to_string());
        assert!(err.to_string().contains("timeout"));

        let err = DaemonError::Serialization("bad format".to_string());
        assert!(err.to_string().contains("bad format"));

        let err = DaemonError::Signal("SIGKILL".to_string());
        assert!(err.to_string().contains("SIGKILL"));
    }

    // =========================================================================
    // Error Chain Tests
    // =========================================================================

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PCloudError>();
        assert_send_sync::<FfiError>();
        assert_send_sync::<AuthError>();
        assert_send_sync::<CryptoError>();
        assert_send_sync::<FilesystemError>();
        assert_send_sync::<DaemonError>();
    }

    #[test]
    fn test_error_debug_impl() {
        let err = PCloudError::Config("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Config"));
    }

    #[test]
    fn test_result_type_alias() {
        fn test_fn() -> Result<i32> {
            Ok(42)
        }
        assert_eq!(test_fn().unwrap(), 42);

        fn test_err() -> Result<i32> {
            Err(PCloudError::Config("test".to_string()))
        }
        assert!(test_err().is_err());
    }
}
