# Project Context for Claude

## Overview

This project is a Rust rewrite of the pCloud console-client wrapper, which provides a CLI interface to the pclsync library for mounting pCloud storage as a FUSE filesystem.

## Project Scope

**What we're rewriting**: The console-client wrapper (originally C++)
**What we're keeping**: The pclsync library (C-based, built and linked via FFI)

## Original Projects

- **Console-Client (being rewritten)**: https://github.com/pcloudcom/console-client
- **pclsync Library (synclib)**: https://github.com/pCloud/pclsync
- **Original Language**: C++

## Architecture

```
+-----------------------------+
|   console-client (Rust)     |  <-- This is the Rust rewrite
|   - CLI interface           |
|   - FFI bindings to pclsync |
+-----------------------------+
              |
              v (FFI/C bindings)
+-----------------------------+
|   pclsync (C library)       |  <-- Unchanged, compiled from source
|   - Core filesystem library |
|   - libfuse integration     |
+-----------------------------+
```

## Module Structure

```
src/
|-- main.rs              # Application entry point, mode dispatch
|-- lib.rs               # Library exports, public API
|-- error.rs             # Error types hierarchy (PCloudError, etc.)
|
|-- cli/                 # Command-line interface
|   |-- mod.rs           # Module exports
|   |-- args.rs          # Clap-based argument parsing
|   +-- commands.rs      # Interactive command parsing
|
|-- ffi/                 # Foreign Function Interface layer
|   |-- mod.rs           # Re-exports, module organization
|   |-- raw.rs           # Unsafe extern "C" function declarations
|   |-- types.rs         # C struct definitions, constants
|   +-- callbacks.rs     # Callback trampolines with panic guards
|
|-- wrapper/             # Safe Rust wrappers over FFI
|   |-- mod.rs           # Re-exports PCloudClient, states
|   |-- client.rs        # Main PCloudClient struct
|   |-- auth.rs          # Authentication operations
|   |-- crypto.rs        # Crypto folder operations
|   +-- filesystem.rs    # Mount/unmount, sync folders
|
|-- daemon/              # Background daemon functionality
|   |-- mod.rs           # Re-exports, init function
|   |-- process.rs       # Daemonization, PID file management
|   |-- signals.rs       # Signal handlers (SIGTERM, SIGHUP)
|   +-- ipc.rs           # Unix socket IPC protocol
|
|-- security/            # Security utilities
|   |-- mod.rs           # Re-exports
|   +-- password.rs      # SecurePassword with zeroization
|
+-- utils/               # Common utilities
    |-- mod.rs           # Re-exports
    +-- cstring.rs       # C string conversion helpers
```

## Build Instructions

### Prerequisites

```bash
# Debian/Ubuntu
sudo apt-get install build-essential libfuse-dev libsqlite3-dev \
  libssl-dev libudev-dev libclang-dev

# Fedora/RHEL
sudo dnf install gcc fuse-devel sqlite-devel openssl-devel \
  systemd-devel clang-devel

# Arch Linux
sudo pacman -S base-devel fuse2 sqlite openssl systemd-libs clang

# macOS
brew install macfuse sqlite openssl llvm
```

### Build Commands

```bash
# Initialize submodules (pclsync source)
git submodule update --init

# Debug build
cargo build

# Release build
cargo build --release

# Run directly
cargo run -- -u user@email.com -p -m /mnt/pcloud

# Install
cargo install --path .
```

### Build System (build.rs)

The build script:
1. Compiles all pclsync C source files using the `cc` crate
2. Links system libraries (fuse, sqlite3, openssl, udev, pthread)
3. Generates Rust bindings for C structs using `bindgen`
4. Detects platform (Linux/macOS) for conditional compilation

Notes:
- pclsync uses OpenSSL3.x for TLS
- Key dependencies are found via pkg-config when available

## Testing Instructions

### Running Tests

```bash
# Run all tests
cargo test

# Run unit tests only (tests inside src/)
cargo test --lib

# Run integration tests only
cargo test --test integration

# Run specific test by name
cargo test test_help_flag

# Run tests matching a pattern
cargo test daemon

# Run tests with output visible
cargo test -- --nocapture

# Run tests showing all output including passing tests
cargo test -- --nocapture --show-output

# Run tests in release mode (faster but less debug info)
cargo test --release
```

### Test Organization

```
tests/
  integration.rs           # Entry point for integration tests
  integration/
    mod.rs                 # Module declaration
    cli_tests.rs           # CLI argument parsing tests
    daemon_tests.rs        # Daemon process management tests
    ipc_tests.rs           # IPC protocol tests

src/
  lib.rs                   # Unit tests for library module
  cli/
    args.rs                # Unit tests for argument parsing
    commands.rs            # Unit tests for interactive commands
  daemon/
    mod.rs                 # Unit tests for daemon module
    process.rs             # Unit tests for process management
    ipc.rs                 # Unit tests for IPC
  security/
    password.rs            # Unit tests for secure password handling
```

### Test Categories

1. **Unit Tests** (`cargo test --lib`)
   - Fast, isolated tests for individual functions
   - Located in `#[cfg(test)]` modules within source files
   - Test Rust wrapper logic without FFI

2. **Integration Tests** (`cargo test --test integration`)
   - Test the compiled binary behavior
   - Verify CLI argument parsing and validation
   - Test daemon/IPC without actual pclsync library

3. **CLI Tests**
   - Test `--help` and `--version` output
   - Verify required argument handling
   - Test conflicting argument detection

4. **Daemon Tests**
   - Test PID file management
   - Test daemon detection logic
   - Use tempfile for isolation

5. **IPC Tests**
   - Test command/response serialization
   - Verify bincode round-trip
   - Test length-prefix protocol

### Debugging Test Failures

```bash
# Show backtrace on failure
RUST_BACKTRACE=1 cargo test

# Run single-threaded for easier debugging
cargo test -- --test-threads=1

# Run with debug logging (if supported)
RUST_LOG=debug cargo test
```

## Security Considerations

### Password Handling

All passwords in this application are handled securely:

1. **SecurePassword Type**: Passwords are wrapped in `SecurePassword` from `src/security/password.rs`
   - Uses `secrecy` crate's `SecretString` internally
   - Automatic memory zeroization on drop via `zeroize` crate
   - Custom `Debug`/`Display` implementations that show `[REDACTED]`
   - `to_cstring()` method for safe FFI conversion with intermediate buffer zeroization

2. **IPC Password Handling**:
   - Unix socket has 0600 permissions (owner-only access)
   - Passwords in `DaemonCommand::StartCrypto` are immediately converted to `SecurePassword`
   - Original string is zeroized after conversion
   - Custom `Debug` implementation for `DaemonCommand` redacts passwords

3. **FFI Password Handling**:
   - Passwords are passed to C functions via `CString`
   - Intermediate buffers are zeroized after conversion
   - C library makes copies of password strings

### What is Protected

- Passwords in memory are zeroized when dropped
- No passwords appear in debug output, logs, or error messages
- IPC restricted to current user via Unix socket permissions

### What is NOT Protected

- Passwords in transit over IPC are not encrypted (Unix socket is local-only)
- Core dumps may contain password memory if not disabled
- Compiler optimizations might defeat zeroization in edge cases
- pclsync C library has its own memory management

### Best Practices for Development

1. Use `SecurePassword::new_zeroizing()` when converting from plain strings
2. Keep password exposure scopes as small as possible
3. Use `to_cstring()` for FFI instead of manual conversion
4. Never log or print password values
5. Never derive `Debug` on structs containing passwords - use custom impl

### FFI Safety

All FFI code follows these guidelines:

1. **Null pointer checks**: All FFI boundaries check for null pointers before dereference
2. **Panic safety**: Callback trampolines use `catch_unwind` to prevent panic unwinding across FFI
3. **Memory ownership**: Clear documentation of who allocates and who frees memory
4. **String encoding**: Proper handling of UTF-8 vs arbitrary bytes at FFI boundaries
5. **Thread safety**: Document thread safety guarantees for each FFI function

## Contributing Guidelines

### Code Style

- Follow Rust idioms and best practices
- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings
- Keep unsafe blocks as small as possible
- Document all safety invariants in unsafe blocks

### Pull Request Checklist

- [ ] Code compiles without warnings (`cargo build --release`)
- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code is formatted (`cargo fmt --check`)
- [ ] Documentation is updated for any API changes
- [ ] New functionality has tests
- [ ] Unsafe code is minimized and well-documented

### Commit Messages

- Follow the commit message guide in `COMMIT.md`
- **Do NOT** append a "Co-Authored-By:" line or any similar suffix to generated commit messages

### Adding New FFI Functions

When wrapping a new pclsync function:

1. Declare the C function in `src/ffi/raw.rs`
2. Add any new types to `src/ffi/types.rs`
3. Create a safe wrapper in `src/wrapper/`
4. Document safety requirements
5. Add tests

Example FFI wrapper pattern:

```rust
// In wrapper module
pub fn safe_function(&self, arg: &str) -> Result<()> {
    let c_arg = CString::new(arg).map_err(|_| PCloudError::InvalidArgument(...))?;

    // Safety: c_arg is valid and null-terminated
    let result = unsafe { raw::psync_function(c_arg.as_ptr()) };

    if result != 0 {
        return Err(SomeError::from_code(result).into());
    }

    Ok(())
}
```

## Known Issues and Limitations

### Platform-Specific

1. **Linux**: Requires FUSE 2.x; user may need to be in `fuse` group
2. **macOS**: Requires macFUSE; may need to allow system extension
3. **Windows**: Not supported (FUSE dependency)

### Technical Limitations

1. **Memory**: pclsync controls memory for sync operations
2. **Threading**: Callbacks may fire from pclsync's internal threads
3. **Singleton**: Only one PCloudClient instance per process
4. **Blocking**: Some pclsync operations block the calling thread

### Future Improvements

- [ ] Add configuration file support
- [ ] Implement logging to file in daemon mode
- [ ] Add selective sync support
- [ ] Improve error recovery
- [ ] Add progress reporting during sync

## Error Handling

### Error Hierarchy

```
PCloudError (top-level)
|-- FfiError           # FFI boundary errors
|-- AuthError          # Authentication errors
|-- CryptoError        # Crypto operation errors
|-- FilesystemError    # Mount/sync errors
|-- DaemonError        # Daemon/IPC errors
|-- Config             # Configuration errors
|-- Io                 # I/O errors
+-- InvalidArgument    # Invalid argument errors
```

### Converting C Error Codes

Each error type has `from_code()` or `from_*_code()` methods:

```rust
// Crypto errors have different codes for different operations
let err = CryptoError::from_setup_code(code);    // Setup operation
let err = CryptoError::from_start_code(code);    // Start operation
let err = CryptoError::from_stop_code(code);     // Stop operation
let err = CryptoError::from_generic_code(code);  // Generic crypto error

// Filesystem errors
let err = FilesystemError::from_code(code);
```

## IPC Protocol

### Socket Location

- Path: `/tmp/pcloud-<uid>.sock`
- Permissions: 0600 (owner only)

### Message Format

```
[4 bytes: length (u32 little-endian)][length bytes: bincode-encoded message]
```

### Commands (DaemonCommand)

- `Ping` - Check daemon is alive
- `Status` - Get current status
- `StartCrypto { password }` - Start crypto with password
- `StopCrypto` - Stop crypto
- `Finalize` - Sync and shutdown
- `Quit` - Immediate shutdown

### Responses (DaemonResponse)

- `Ok` - Command succeeded
- `OkWithMessage(String)` - Success with message
- `Error(String)` - Command failed
- `Status { authenticated, crypto_started, mounted, mountpoint }` - Status info
- `Pong` - Response to Ping

## Debugging Tips

### Build Issues

```bash
# Verbose build output
cargo build -vv

# Check what build.rs is doing
CARGO_LOG=cargo::core::compiler::build_context=trace cargo build

# Regenerate bindings
cargo clean && cargo build
```

### Runtime Issues

```bash
# Run with backtrace
RUST_BACKTRACE=1 ./target/debug/pcloud -u user@email.com -p

# Debug daemon
# 1. Don't daemonize, run in foreground with debug output
cargo run -- -u user@email.com -p -o -m /mnt/pcloud

# Check daemon status
ls -la /tmp/pcloud-$(id -u).*
cat /tmp/pcloud-$(id -u).pid
```

### FUSE Debugging

```bash
# Mount with debug output (add to pclsync if supported)
# Check /var/log/syslog for FUSE errors

# Verify FUSE is working
fusermount -V

# Check mount
mount | grep pcloud
```

## References

- pclsync header: `pclsync/psynclib.h` (main API)
- pclsync library (synclib): https://github.com/pCloud/pclsync
- Original console-client: https://github.com/pcloudcom/console-client
- Rust FFI book: https://doc.rust-lang.org/nomicon/ffi.html
- secrecy crate: https://docs.rs/secrecy/
- daemonize crate: https://docs.rs/daemonize/
