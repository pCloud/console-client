# pCloud Console Client

A command-line client for pCloud in Rust with FFI bindings to the [pclsync](https://github.com/pCloud/pclsync) C filesystem library.

>**NOTE:** For the legacy C++/cmake console client's go [here](https://github.com/pCloud/console-client/tree/2.x).


## Features

- Mount pCloud storage as a FUSE filesystem
- Encrypted folder support (Crypto)
- Background daemon mode with IPC control
- Secure password handling with automatic zeroization
- Cross-platform support (Linux, macOS)
- Full CLI compatibility with the original C++ client

## Prerequisites

### Linux (Debian/Ubuntu)

```bash
sudo apt-get install \
  build-essential \
  libfuse-dev \
  libsqlite3-dev \
  libssl-dev \
  zlib1g-dev \
  libclang-dev
```

### Linux (Fedora/RHEL)

```bash
sudo dnf install \
  gcc \
  fuse-devel \
  sqlite-devel \
  openssl-devel \
  systemd-devel \
  zlib-devel \
  clang-devel
```

### Linux (Arch Linux)

```bash
sudo pacman -S \
  base-devel \
  fuse2 \
  sqlite \
  openssl \
  systemd-libs \
  clang
```

### macOS

```bash
brew install macfuse sqlite openssl llvm
```

**Note**: macFUSE requires a system extension. After installation, you may need to:
1. Open System Preferences > Security & Privacy
2. Allow the macFUSE system extension
3. Restart your Mac

## Building

### Clone with Submodules

```bash
git clone --recursive https://github.com/pCloud/console-client.git
cd console-client
```

If you already cloned without `--recursive`:

```bash
git submodule update --init
```

### Build

```bash
# Debug build (faster compilation)
cargo build

# Release build (optimized binary)
cargo build --release

# The binary will be at:
# - Debug: target/debug/pcloud
# - Release: target/release/pcloud
```

### Build Profiles

The build system supports three profiles that control the version suffix and
whether crash reporting is included:

| Profile     | Command                                                                                      | Version output  | Crash reporting |
|-------------|----------------------------------------------------------------------------------------------|-----------------|-----------------|
| Development | `cargo build`                                                                                | `0.1.0-dev`     | No              |
| QA          | `BUGSNAG_API_KEY=<key> PCLOUD_BUILD_PROFILE=qa cargo build --release --features crash-reporting` | `0.1.0-qa`  | Yes             |
| Release     | `BUGSNAG_API_KEY=<key> cargo build --release --features crash-reporting`                      | `0.1.0`         | Yes             |

- **Development** — Default `cargo build` with no extra flags. Fast iteration,
  debug symbols, no crash reporting, no API key needed.
- **QA** — Release-optimized binary with crash reporting enabled. Reports are
  tagged with release stage `qa` in Bugsnag so they can be filtered from
  production crashes.
- **Release** — Production binary. Reports are tagged with release stage
  `production` in Bugsnag.

The version string is available at runtime via `pcloud --version`.

### Install

```bash
# Install to ~/.cargo/bin (make sure it's in your PATH)
cargo install --path .

# Or copy manually
sudo cp target/release/pcloud /usr/local/bin/
```

### Linux Packages

Pre-built packages can be produced for Debian/Ubuntu, Fedora/RHEL, and Arch Linux.
All packages install the binary to `/usr/bin/pcloud`.

#### .deb (Debian/Ubuntu)

```bash
cargo install cargo-deb
cargo deb
# Output: target/debian/pcloud_<version>_<arch>.deb

sudo dpkg -i target/debian/pcloud_*.deb
```

Runtime dependencies: `libfuse2`, `libsqlite3-0`, `libssl3`, `zlib1g`, `libudev1`.

#### .rpm (Fedora/RHEL)

```bash
cargo install cargo-generate-rpm
cargo build --release
cargo generate-rpm
# Output: target/generate-rpm/pcloud-<version>-<release>.<arch>.rpm

sudo rpm -i target/generate-rpm/pcloud-*.rpm
```

Runtime dependencies: `fuse-libs`, `sqlite-libs`, `openssl-libs >= 3.0`, `zlib`, `systemd-libs`.

#### Arch Linux

```bash
cd pkg/arch
makepkg -si
```

Runtime dependencies: `fuse2`, `sqlite`, `openssl`, `zlib`, `systemd-libs`.

## Usage

### Basic Usage

```bash
# Mount pCloud with password prompt
pcloud -u user@email.com -p -m /mnt/pcloud

# With crypto support (prompts for separate crypto password)
pcloud -u user@email.com -p -c -m /mnt/pcloud

# Use login password as crypto password
pcloud -u user@email.com -p -y -m /mnt/pcloud

# Interactive mode (allows runtime commands)
pcloud -u user@email.com -p -o -m /mnt/pcloud
```

### Daemon Mode

Run pCloud client as a background service:

```bash
# Start as daemon
pcloud -u user@email.com -p -d -m /mnt/pcloud

# Send commands to running daemon
pcloud -u user@email.com -k -o
> startcrypto
> stopcrypto
> status
> quit
```

The daemon creates:
- PID file at `/tmp/pcloud-<uid>.pid`
- Unix socket at `/tmp/pcloud-<uid>.sock`

To stop the daemon:

```bash
# Graceful shutdown
pcloud -u user@email.com -k -o
> finalize

# Or using the PID file
kill $(cat /tmp/pcloud-$(id -u).pid)
```

### New User Registration

```bash
pcloud -u newuser@email.com -p -n
```

After registration, verify your email before logging in.

### Command Reference

| Flag | Long            | Description                               |
|------|-----------------|-------------------------------------------|
| -u   | --username      | pCloud account email (required)           |
| -p   | --password      | Prompt for password                       |
| -c   | --crypto        | Prompt for crypto password                |
| -y   | --passascrypto  | Use login password as crypto password     |
| -d   | --daemon        | Run as background daemon                  |
| -o   | --commands      | Enable interactive command mode           |
| -m   | --mountpoint    | Directory to mount pCloud                 |
| -k   | --client        | Send commands to running daemon           |
| -n   | --newuser       | Register new account                      |
| -s   | --savepassword  | Save password for auto-login              |

### Interactive Commands

When running with `-o` (commands mode) or `-k -o` (client mode):

| Command           | Aliases       | Description                      |
|-------------------|---------------|----------------------------------|
| startcrypto       | start         | Unlock encrypted folders         |
| stopcrypto        | stop          | Lock encrypted folders           |
| status            | s             | Show current status              |
| finalize          | fin           | Sync and exit gracefully         |
| quit              | q, exit       | Exit immediately                 |
| help              | h, ?          | Show help                        |

## Architecture

```
src/
|-- main.rs              # Entry point and application flow
|-- lib.rs               # Library exports
|-- error.rs             # Error types (PCloudError, AuthError, etc.)
|-- cli/                 # CLI argument parsing
|   |-- mod.rs           # Module exports
|   |-- args.rs          # Clap argument definitions (Cli struct)
|   +-- commands.rs      # Interactive command parsing
|-- crash_reporting/     # Bugsnag crash reporting (feature-gated)
|   |-- mod.rs           # Public API: init(), notify_error(), app_version()
|   |-- config.rs        # Bugsnag client singleton and release stage
|   |-- panic_hook.rs    # Rust panic hook for Bugsnag reporting
|   +-- native.rs        # Minidump crash handling and upload
|-- ffi/                 # FFI bindings to pclsync C library
|   |-- mod.rs           # Module exports and re-exports
|   |-- raw.rs           # C function declarations (extern "C")
|   |-- types.rs         # C type definitions (bindgen + manual)
|   +-- callbacks.rs     # Callback trampolines (status, event, etc.)
|-- wrapper/             # Safe Rust wrappers over FFI
|   |-- mod.rs           # Module exports
|   |-- client.rs        # PCloudClient (main API)
|   |-- auth.rs          # Authentication operations
|   |-- crypto.rs        # Crypto (encryption) operations
|   +-- filesystem.rs    # Mount/unmount, sync folders
|-- daemon/              # Background daemon functionality
|   |-- mod.rs           # Module exports
|   |-- process.rs       # Daemonization, PID file management
|   |-- signals.rs       # Signal handling (SIGTERM, SIGHUP)
|   +-- ipc.rs           # Unix socket IPC (client/server)
|-- security/            # Security utilities
|   |-- mod.rs           # Module exports
|   +-- password.rs      # SecurePassword with zeroization
+-- utils/               # Common utilities
    |-- mod.rs           # Module exports
    +-- cstring.rs       # C string conversion helpers
```

## Security

This client implements several security measures:

### Password Protection

- Passwords are wrapped in `SecurePassword` type using the `secrecy` crate's `SecretString`
- Memory is automatically zeroized when passwords go out of scope
- No passwords appear in debug output, logs, or error messages
- Terminal password input does not echo characters

### IPC Security

- Unix domain socket has 0600 permissions (owner-only)
- Socket path includes user ID to prevent conflicts
- Passwords sent via IPC are immediately zeroized after receipt

### FFI Safety

- All unsafe FFI calls are wrapped in safe Rust functions
- Null pointers are checked before dereferencing
- Panic guards prevent unwinding across FFI boundaries
- C error codes are converted to Rust Result types

### What is NOT Protected

- Passwords in transit over IPC are not encrypted (Unix socket is local-only)
- Core dumps may contain password memory if not disabled
- pclsync C library has its own memory management

## Crash Reporting

When built with the `crash-reporting` Cargo feature, the client reports crashes
to [Bugsnag](https://www.bugsnag.com) for both Rust and C code.

### What is reported

| Crash type          | Mechanism                                      |
|---------------------|------------------------------------------------|
| Rust panics         | Custom `panic::set_hook` sends a Bugsnag error |
| Native signals      | `crash-handler` catches SIGSEGV, SIGABRT, SIGBUS, SIGFPE; a monitor thread writes a minidump and uploads it to Bugsnag |
| Non-fatal errors    | Top-level application errors are sent via `notify_error()` |

Native crash handling uses an out-of-process model: a dedicated monitor thread
runs a `minidumper::Server` over IPC. When a signal fires, the handler requests
the monitor to write a minidump from the crashed process via `ptrace`, then
uploads it. If the upload fails at crash time the dump is queued to
`$XDG_DATA_HOME/pcloud/crashes/` and retried on the next startup.

### Enabling crash reporting

Crash reporting is gated behind the `crash-reporting` Cargo feature and is
**off by default** — plain `cargo build` produces a binary with no Bugsnag
dependency and no API key requirement.

To enable it, pass the feature flag and provide a Bugsnag API key at build time:

```bash
BUGSNAG_API_KEY=<your-key> cargo build --release --features crash-reporting
```

### How the API key is provided

The Bugsnag API key is injected at **compile time** through the `BUGSNAG_API_KEY`
environment variable. The build script (`build.rs`) declares
`cargo:rerun-if-env-changed=BUGSNAG_API_KEY` so Cargo will rebuild when the
value changes.

Inside the source the key is read with `env!("BUGSNAG_API_KEY")`, which means:

- The key must be set in the environment **when `cargo build` runs**. If the
  `crash-reporting` feature is enabled and the variable is missing, compilation
  fails with a clear error.
- The key is embedded in the binary as a string literal. For distribution
  builds, run `strip` on the binary and upload Breakpad symbols separately (see
  below) to avoid shipping debug info alongside the key.
- Development builds (`cargo build` without `--features crash-reporting`) never
  reference the variable, so no key is needed for day-to-day work.

### Symbol upload

For symbolicated stack traces in the Bugsnag dashboard, debug symbols must be
uploaded separately. A helper script is provided:

```bash
BUGSNAG_API_KEY=<your-key> ./scripts/upload-symbols.sh
```

The script:
1. Builds a release binary with full debug info (`-C debuginfo=2`)
2. Generates a Breakpad `.sym` file with `dump_syms`
3. Uploads the symbols to Bugsnag via `bugsnag-cli`
4. Strips the binary for distribution

Prerequisites: `dump_syms` (`cargo install dump_syms`) and `bugsnag-cli`
(`npm install -g @bugsnag/cli`).

### Signal handler compatibility

The crash handler only intercepts **crash signals** (SIGSEGV, SIGBUS, SIGABRT,
SIGFPE). It does not conflict with the existing signal handlers:

- `ctrlc` crate handles SIGINT in foreground mode
- `nix` handles SIGTERM/SIGHUP/SIGINT in daemon mode
- SIGPIPE is ignored by the pclsync C library

## Migrating from C++ Version

The Rust version maintains full CLI compatibility with the original C++ client. Your existing scripts should work unchanged.

### Differences

1. **Binary name**: The Rust version is named `pcloud` (configurable in Cargo.toml)
2. **Error messages**: More descriptive and structured
3. **Exit codes**: Standardized (0 for success, non-zero for errors)
4. **Improved signal handling**: Graceful shutdown on SIGTERM/SIGINT

### Unchanged

- All CLI flags work identically (`-u`, `-p`, `-c`, `-y`, `-d`, `-o`, `-m`, `-k`, `-n`, `-s`)
- IPC protocol is compatible (can control Rust daemon from C++ client and vice versa)
- Mountpoint and sync behavior identical (uses same pclsync library)
- Interactive commands are the same (startcrypto, stopcrypto, finalize, quit)

### Migration Steps

1. Build the Rust version
2. Stop any running C++ daemon (`pcloud -k finalize` or kill the process)
3. Replace the binary in your PATH
4. Start with the same arguments you used before

## Known Limitations

1. **Platform support**: Primarily tested on Linux; macOS support is available but less tested
2. **Windows**: Not supported (pclsync FUSE dependency requires Unix-like OS)
3. **Memory management**: The pclsync C library controls memory allocation for sync operations
4. **Threading**: pclsync uses internal threading; callbacks may fire from any thread
5. **FUSE version**: Requires FUSE 2.x; FUSE 3.x may require additional configuration
6. **Saved passwords**: Password storage location is determined by pclsync library

## Troubleshooting

### Build Errors

**"pclsync directory not found"**
```bash
git submodule update --init
```

**"libfuse not found" / "sqlite3 not found"**
Install the development packages for your distribution (see Prerequisites).

**"bindgen failed"**
Ensure `libclang-dev` (Linux) or `llvm` (macOS) is installed.

### Runtime Errors

**"Failed to mount filesystem"**
- Ensure the mountpoint directory exists
- Check that you have permissions to mount FUSE filesystems
- On Linux, you may need to be in the `fuse` group: `sudo usermod -aG fuse $USER`

**"Daemon is already running"**
```bash
# Check for existing process
cat /tmp/pcloud-$(id -u).pid
# Kill if necessary
kill $(cat /tmp/pcloud-$(id -u).pid)
```

**"Connection failed" in client mode**
- Ensure a daemon is running with `-d` flag
- Check socket file exists: `ls -la /tmp/pcloud-$(id -u).sock`

### FUSE Issues on Linux

If you get permission errors when mounting:

```bash
# Add yourself to the fuse group
sudo usermod -aG fuse $USER
# Log out and back in for group changes to take effect

# Or allow non-root users (system-wide)
echo 'user_allow_other' | sudo tee -a /etc/fuse.conf
```

### macOS Code Signing

When running on macOS, you may see security prompts. Allow the pcloud binary in:
System Preferences > Security & Privacy > General

## License

BSD-3-Clause (follows original pCloud project licensing)

## Credits

- Original pCloud console-client: https://github.com/pCloud/console-client
- pclsync library (synclib): https://github.com/pCloud/pclsync

## Contributing

Contributions are welcome! Please ensure:

1. Code follows Rust idioms and best practices
2. All unsafe code is well-documented and minimized
3. Tests are added for new functionality
4. Documentation is updated as needed

Run tests before submitting:

```bash
cargo test
cargo clippy
cargo fmt --check
```
