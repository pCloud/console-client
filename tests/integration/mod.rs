//! Integration tests for pCloud console-client.
//!
//! These tests verify the behavior of the CLI, daemon, and IPC functionality
//! without requiring the actual pclsync library to be running.
//!
//! # Test Modules
//!
//! - `cli_tests`: Tests for CLI argument parsing and validation
//! - `daemon_tests`: Tests for daemon process management
//! - `ipc_tests`: Tests for IPC protocol and serialization

mod cli_tests;
mod daemon_tests;
mod ipc_tests;
