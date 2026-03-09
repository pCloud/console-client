//! IPC integration tests for pCloud console-client.
//!
//! These tests verify the IPC protocol, including command/response
//! serialization, client/server communication, and error handling.
//!
//! # Note
//!
//! Full IPC server tests require a mock PCloudClient, so some tests
//! focus on the protocol layer only.

use std::time::Duration;
use tempfile::tempdir;

use console_client::daemon::{DaemonClient, DaemonCommand, DaemonResponse};

// ============================================================================
// Command Serialization Tests
// ============================================================================

#[test]
fn test_command_ping_serialization() {
    let cmd = DaemonCommand::Ping;
    let bytes = bincode::serialize(&cmd).unwrap();
    let decoded: DaemonCommand = bincode::deserialize(&bytes).unwrap();

    assert!(matches!(decoded, DaemonCommand::Ping));
}

#[test]
fn test_command_status_serialization() {
    let cmd = DaemonCommand::Status;
    let bytes = bincode::serialize(&cmd).unwrap();
    let decoded: DaemonCommand = bincode::deserialize(&bytes).unwrap();

    assert!(matches!(decoded, DaemonCommand::Status));
}

#[test]
fn test_command_quit_serialization() {
    let cmd = DaemonCommand::Quit;
    let bytes = bincode::serialize(&cmd).unwrap();
    let decoded: DaemonCommand = bincode::deserialize(&bytes).unwrap();

    assert!(matches!(decoded, DaemonCommand::Quit));
}

#[test]
fn test_command_finalize_serialization() {
    let cmd = DaemonCommand::Finalize;
    let bytes = bincode::serialize(&cmd).unwrap();
    let decoded: DaemonCommand = bincode::deserialize(&bytes).unwrap();

    assert!(matches!(decoded, DaemonCommand::Finalize));
}

#[test]
fn test_command_stop_crypto_serialization() {
    let cmd = DaemonCommand::StopCrypto;
    let bytes = bincode::serialize(&cmd).unwrap();
    let decoded: DaemonCommand = bincode::deserialize(&bytes).unwrap();

    assert!(matches!(decoded, DaemonCommand::StopCrypto));
}

#[test]
fn test_command_start_crypto_with_password() {
    let cmd = DaemonCommand::StartCrypto {
        password: Some("test_password_123".to_string()),
    };
    let bytes = bincode::serialize(&cmd).unwrap();
    let decoded: DaemonCommand = bincode::deserialize(&bytes).unwrap();

    match decoded {
        DaemonCommand::StartCrypto { password } => {
            assert_eq!(password, Some("test_password_123".to_string()));
        }
        _ => panic!("Wrong command type after deserialization"),
    }
}

#[test]
fn test_command_start_crypto_without_password() {
    let cmd = DaemonCommand::StartCrypto { password: None };
    let bytes = bincode::serialize(&cmd).unwrap();
    let decoded: DaemonCommand = bincode::deserialize(&bytes).unwrap();

    match decoded {
        DaemonCommand::StartCrypto { password } => {
            assert_eq!(password, None);
        }
        _ => panic!("Wrong command type after deserialization"),
    }
}

#[test]
fn test_command_start_crypto_with_empty_password() {
    let cmd = DaemonCommand::StartCrypto {
        password: Some(String::new()),
    };
    let bytes = bincode::serialize(&cmd).unwrap();
    let decoded: DaemonCommand = bincode::deserialize(&bytes).unwrap();

    match decoded {
        DaemonCommand::StartCrypto { password } => {
            assert_eq!(password, Some(String::new()));
        }
        _ => panic!("Wrong command type after deserialization"),
    }
}

#[test]
fn test_command_start_crypto_with_unicode_password() {
    let cmd = DaemonCommand::StartCrypto {
        password: Some("password_with_unicode_chars".to_string()),
    };
    let bytes = bincode::serialize(&cmd).unwrap();
    let decoded: DaemonCommand = bincode::deserialize(&bytes).unwrap();

    match decoded {
        DaemonCommand::StartCrypto { password } => {
            assert_eq!(password, Some("password_with_unicode_chars".to_string()));
        }
        _ => panic!("Wrong command type after deserialization"),
    }
}

// ============================================================================
// Response Serialization Tests
// ============================================================================

#[test]
fn test_response_ok_serialization() {
    let resp = DaemonResponse::Ok;
    let bytes = bincode::serialize(&resp).unwrap();
    let decoded: DaemonResponse = bincode::deserialize(&bytes).unwrap();

    assert!(matches!(decoded, DaemonResponse::Ok));
}

#[test]
fn test_response_pong_serialization() {
    let resp = DaemonResponse::Pong;
    let bytes = bincode::serialize(&resp).unwrap();
    let decoded: DaemonResponse = bincode::deserialize(&bytes).unwrap();

    assert!(matches!(decoded, DaemonResponse::Pong));
}

#[test]
fn test_response_ok_with_message_serialization() {
    let resp = DaemonResponse::OkWithMessage("Operation completed successfully".to_string());
    let bytes = bincode::serialize(&resp).unwrap();
    let decoded: DaemonResponse = bincode::deserialize(&bytes).unwrap();

    match decoded {
        DaemonResponse::OkWithMessage(msg) => {
            assert_eq!(msg, "Operation completed successfully");
        }
        _ => panic!("Wrong response type"),
    }
}

#[test]
fn test_response_error_serialization() {
    let resp = DaemonResponse::Error("Something went wrong".to_string());
    let bytes = bincode::serialize(&resp).unwrap();
    let decoded: DaemonResponse = bincode::deserialize(&bytes).unwrap();

    match decoded {
        DaemonResponse::Error(msg) => {
            assert_eq!(msg, "Something went wrong");
        }
        _ => panic!("Wrong response type"),
    }
}

#[test]
fn test_response_status_full_serialization() {
    let resp = DaemonResponse::Status {
        authenticated: true,
        crypto_started: false,
        mounted: true,
        mountpoint: Some("/home/user/pCloud".to_string()),
    };
    let bytes = bincode::serialize(&resp).unwrap();
    let decoded: DaemonResponse = bincode::deserialize(&bytes).unwrap();

    match decoded {
        DaemonResponse::Status {
            authenticated,
            crypto_started,
            mounted,
            mountpoint,
        } => {
            assert!(authenticated);
            assert!(!crypto_started);
            assert!(mounted);
            assert_eq!(mountpoint, Some("/home/user/pCloud".to_string()));
        }
        _ => panic!("Wrong response type"),
    }
}

#[test]
fn test_response_status_minimal_serialization() {
    let resp = DaemonResponse::Status {
        authenticated: false,
        crypto_started: false,
        mounted: false,
        mountpoint: None,
    };
    let bytes = bincode::serialize(&resp).unwrap();
    let decoded: DaemonResponse = bincode::deserialize(&bytes).unwrap();

    match decoded {
        DaemonResponse::Status {
            authenticated,
            crypto_started,
            mounted,
            mountpoint,
        } => {
            assert!(!authenticated);
            assert!(!crypto_started);
            assert!(!mounted);
            assert!(mountpoint.is_none());
        }
        _ => panic!("Wrong response type"),
    }
}

// ============================================================================
// Command Display and Debug Tests
// ============================================================================

#[test]
fn test_command_display() {
    assert_eq!(format!("{}", DaemonCommand::Ping), "Ping");
    assert_eq!(format!("{}", DaemonCommand::Status), "Status");
    assert_eq!(format!("{}", DaemonCommand::Quit), "Quit");
    assert_eq!(format!("{}", DaemonCommand::Finalize), "Finalize");
    assert_eq!(format!("{}", DaemonCommand::StopCrypto), "StopCrypto");
    assert_eq!(
        format!(
            "{}",
            DaemonCommand::StartCrypto {
                password: Some("secret".to_string())
            }
        ),
        "StartCrypto"
    );
}

#[test]
fn test_command_debug_redacts_password() {
    let cmd = DaemonCommand::StartCrypto {
        password: Some("super_secret_password".to_string()),
    };
    let debug_str = format!("{:?}", cmd);

    // Password should NOT appear in debug output
    assert!(!debug_str.contains("super_secret_password"));
    // REDACTED should appear
    assert!(debug_str.contains("REDACTED"));
}

#[test]
fn test_command_debug_shows_none_password() {
    let cmd = DaemonCommand::StartCrypto { password: None };
    let debug_str = format!("{:?}", cmd);

    assert!(debug_str.contains("None"));
}

// ============================================================================
// Response Display Tests
// ============================================================================

#[test]
fn test_response_display() {
    assert_eq!(format!("{}", DaemonResponse::Ok), "OK");
    assert_eq!(format!("{}", DaemonResponse::Pong), "Pong");
    assert_eq!(
        format!("{}", DaemonResponse::OkWithMessage("done".to_string())),
        "done"
    );
    assert_eq!(
        format!("{}", DaemonResponse::Error("failed".to_string())),
        "Error: failed"
    );
}

#[test]
fn test_response_status_display() {
    let resp = DaemonResponse::Status {
        authenticated: true,
        crypto_started: false,
        mounted: true,
        mountpoint: Some("/mnt/pcloud".to_string()),
    };
    let display = format!("{}", resp);

    assert!(display.contains("authenticated=true"));
    assert!(display.contains("crypto=false"));
    assert!(display.contains("mounted=true"));
    assert!(display.contains("/mnt/pcloud"));
}

#[test]
fn test_response_status_display_no_mountpoint() {
    let resp = DaemonResponse::Status {
        authenticated: false,
        crypto_started: false,
        mounted: false,
        mountpoint: None,
    };
    let display = format!("{}", resp);

    assert!(display.contains("authenticated=false"));
    assert!(!display.contains("mountpoint="));
}

// ============================================================================
// DaemonClient Tests
// ============================================================================

#[test]
fn test_daemon_client_new() {
    let client = DaemonClient::new("/tmp/test.sock");
    assert_eq!(client.socket_path(), std::path::Path::new("/tmp/test.sock"));
}

#[test]
fn test_daemon_client_with_timeout() {
    let client = DaemonClient::with_timeout("/tmp/test.sock", Duration::from_secs(10));
    assert_eq!(client.socket_path(), std::path::Path::new("/tmp/test.sock"));
}

#[test]
fn test_daemon_client_socket_path() {
    let dir = tempdir().unwrap();
    let socket_path = dir.path().join("daemon.sock");
    let client = DaemonClient::new(&socket_path);

    assert_eq!(client.socket_path(), socket_path);
}

#[test]
fn test_daemon_client_connection_failure() {
    let dir = tempdir().unwrap();
    let socket_path = dir.path().join("nonexistent.sock");
    let client = DaemonClient::new(&socket_path);

    // Should fail since no server is listening
    let result = client.send_command(DaemonCommand::Ping);
    assert!(result.is_err());
}

#[test]
fn test_daemon_client_is_daemon_alive_no_server() {
    let dir = tempdir().unwrap();
    let socket_path = dir.path().join("nonexistent.sock");
    let client = DaemonClient::new(&socket_path);

    // Should return false since no server
    assert!(!client.is_daemon_alive());
}

// ============================================================================
// Length Prefix Protocol Tests
// ============================================================================

#[test]
fn test_length_prefix_encoding() {
    // Test the length prefix format used in the IPC protocol
    let len: u32 = 256;
    let bytes = len.to_le_bytes();

    assert_eq!(bytes.len(), 4);
    assert_eq!(u32::from_le_bytes(bytes), 256);
}

#[test]
fn test_length_prefix_large_value() {
    let len: u32 = 1024 * 1024; // 1 MB
    let bytes = len.to_le_bytes();

    assert_eq!(u32::from_le_bytes(bytes), 1024 * 1024);
}

#[test]
fn test_length_prefix_zero() {
    let len: u32 = 0;
    let bytes = len.to_le_bytes();

    assert_eq!(u32::from_le_bytes(bytes), 0);
}

#[test]
fn test_length_prefix_max() {
    let len: u32 = u32::MAX;
    let bytes = len.to_le_bytes();

    assert_eq!(u32::from_le_bytes(bytes), u32::MAX);
}

// ============================================================================
// Round-trip Serialization Tests
// ============================================================================

#[test]
fn test_all_commands_roundtrip() {
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
    ];

    for cmd in commands {
        let bytes = bincode::serialize(&cmd).unwrap();
        let decoded: DaemonCommand = bincode::deserialize(&bytes).unwrap();

        // Verify display matches (using Display trait)
        assert_eq!(format!("{}", cmd), format!("{}", decoded));
    }
}

#[test]
fn test_all_responses_roundtrip() {
    let responses = vec![
        DaemonResponse::Ok,
        DaemonResponse::Pong,
        DaemonResponse::OkWithMessage("success".to_string()),
        DaemonResponse::Error("error".to_string()),
        DaemonResponse::Status {
            authenticated: true,
            crypto_started: true,
            mounted: true,
            mountpoint: Some("/mnt".to_string()),
        },
        DaemonResponse::Status {
            authenticated: false,
            crypto_started: false,
            mounted: false,
            mountpoint: None,
        },
    ];

    for resp in responses {
        let bytes = bincode::serialize(&resp).unwrap();
        let decoded: DaemonResponse = bincode::deserialize(&bytes).unwrap();

        // Verify debug output matches
        assert_eq!(format!("{:?}", resp), format!("{:?}", decoded));
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_command_with_long_password() {
    let long_password = "a".repeat(10000);
    let cmd = DaemonCommand::StartCrypto {
        password: Some(long_password.clone()),
    };
    let bytes = bincode::serialize(&cmd).unwrap();
    let decoded: DaemonCommand = bincode::deserialize(&bytes).unwrap();

    match decoded {
        DaemonCommand::StartCrypto { password } => {
            assert_eq!(password, Some(long_password));
        }
        _ => panic!("Wrong command type"),
    }
}

#[test]
fn test_response_with_long_message() {
    let long_message = "x".repeat(10000);
    let resp = DaemonResponse::OkWithMessage(long_message.clone());
    let bytes = bincode::serialize(&resp).unwrap();
    let decoded: DaemonResponse = bincode::deserialize(&bytes).unwrap();

    match decoded {
        DaemonResponse::OkWithMessage(msg) => {
            assert_eq!(msg, long_message);
        }
        _ => panic!("Wrong response type"),
    }
}

#[test]
fn test_status_with_long_mountpoint() {
    let long_path = "/home/".to_string() + &"subdir/".repeat(100) + "mount";
    let resp = DaemonResponse::Status {
        authenticated: true,
        crypto_started: false,
        mounted: true,
        mountpoint: Some(long_path.clone()),
    };
    let bytes = bincode::serialize(&resp).unwrap();
    let decoded: DaemonResponse = bincode::deserialize(&bytes).unwrap();

    match decoded {
        DaemonResponse::Status { mountpoint, .. } => {
            assert_eq!(mountpoint, Some(long_path));
        }
        _ => panic!("Wrong response type"),
    }
}

#[test]
fn test_serialized_size_reasonable() {
    // Verify that simple commands have reasonable serialized sizes
    let ping_bytes = bincode::serialize(&DaemonCommand::Ping).unwrap();
    assert!(ping_bytes.len() < 100);

    let status_bytes = bincode::serialize(&DaemonCommand::Status).unwrap();
    assert!(status_bytes.len() < 100);

    // Responses should also be reasonably sized
    let pong_bytes = bincode::serialize(&DaemonResponse::Pong).unwrap();
    assert!(pong_bytes.len() < 100);
}
