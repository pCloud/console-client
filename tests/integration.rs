//! Integration test entry point for pCloud console-client.
//!
//! This file serves as the entry point for integration tests. The actual
//! tests are organized in the `integration/` directory:
//!
//! - `cli_tests.rs`: CLI argument parsing and validation tests
//! - `daemon_tests.rs`: Daemon process management tests
//! - `ipc_tests.rs`: IPC protocol and serialization tests
//!
//! # Running Tests
//!
//! ```bash
//! # Run all tests
//! cargo test
//!
//! # Run integration tests only
//! cargo test --test integration
//!
//! # Run specific test
//! cargo test --test integration test_help_flag
//!
//! # Run with output
//! cargo test --test integration -- --nocapture
//! ```

mod integration;
