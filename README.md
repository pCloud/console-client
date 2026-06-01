pCloud Console Client
---------------------
[![CI](https://github.com/pCloud/console-client/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/pCloud/console-client/actions/workflows/ci.yml)
[![Latest Release](https://img.shields.io/github/v/release/pCloud/console-client)](https://github.com/pCloud/console-client/releases/latest)

A command-line client for pCloud in Rust with FFI bindings to the [pclsync](https://github.com/pCloud/pclsync) C filesystem library.

>Version `2.x` of the console client is available [here](https://github.com/pCloud/console-client/tree/2.x).

## Installation

Pre-built binaries and packages along with installation instructions are available on the [Releases](https://github.com/pCloud/console-client/releases/latest) page for:

- **Linux** — x86_64, ARM 64-bit, ARM 32-bit (`.deb`, `.rpm`, and standalone binaries; ARM 32-bit `.rpm` not available as Fedora dropped armhfp support)
- **macOS** — Universal binary (Apple Silicon & Intel)
- **Arch Linux** — PKGBUILD included in the repository at [`pkg/arch/PKGBUILD`](pkg/arch/PKGBUILD)

To build from source instead, see [Building](#building) below.

## Features

- Mount pCloud storage as a FUSE filesystem
- Encrypted folder support (Crypto)
- Background daemon mode with IPC control
- Secure password handling with automatic zeroization
- Cross-platform support (Linux, macOS)
- CLI compatible with the original C++ client (credentials saved by default)

## Building

> **Most users don't need to build from source.** Pre-built binaries and packages are available on the [Releases](https://github.com/pCloud/console-client/releases/latest) page.

### Build Prerequisites

#### Linux (Debian/Ubuntu)

```bash
sudo apt-get install \
  build-essential \
  libfuse-dev \
  libsqlite3-dev \
  libssl-dev \
  zlib1g-dev \
  libudev-dev \
  libclang-dev
```

#### Linux (Fedora/RHEL)

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

#### Linux (Arch Linux)

```bash
sudo pacman -S \
  base-devel \
  fuse2 \
  sqlite \
  openssl \
  systemd-libs \
  clang
```

#### macOS

```bash
brew install macfuse sqlite openssl@3 llvm
```

**Note**: macFUSE requires a system extension. After installation, you may need to:
1. Open System Preferences > Security & Privacy
2. Allow the macFUSE system extension
3. Restart your Mac

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
# - Debug: target/debug/pcloud-cli
# - Release: target/release/pcloud-cli
```

### Build Profiles

The build system supports three profiles that control the version suffix and
whether crash reporting is included:

| Profile     | Command                                                                                      | Crash reporting |
|-------------|----------------------------------------------------------------------------------------------|-----------------|
| Development | `cargo build`                                                                                | No              |
| QA          | `BUGSNAG_API_KEY=<key> PCLOUD_BUILD_PROFILE=qa cargo build --release --features crash-reporting` | Yes             |
| Release     | `BUGSNAG_API_KEY=<key> cargo build --release --features crash-reporting`                      | Yes             |

- **Development** — Default `cargo build` with no extra flags. Fast iteration,
  debug symbols, no crash reporting, no API key needed.
- **QA** — Release-optimized binary with crash reporting enabled. Reports are
  tagged with release stage `qa` in Bugsnag so they can be filtered from
  production crashes.
- **Release** — Production binary. Reports are tagged with release stage
  `production` in Bugsnag.

The version string is available at runtime via `pcloud-cli --version`.

### Install

```bash
# Install to ~/.cargo/bin (make sure it's in your PATH)
cargo install --path .

# Or copy manually
sudo cp target/release/pcloud-cli /usr/local/bin/
```

### Linux Packages

Pre-built packages can be produced for Debian/Ubuntu, Fedora/RHEL, and Arch Linux.
All packages install the binary to `/usr/bin/pcloud-cli`.

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

`pcloud-cli` uses a hierarchical command tree. Every node supports `--help`,
e.g. `pcloud-cli auth --help` or `pcloud-cli backup add --help`.

### Quick start

```bash
# Launch the interactive TUI dashboard (also runs on bare `pcloud-cli`)
pcloud-cli

# Authenticate once — opens the browser-based login flow
pcloud-cli auth login

# ...or pass a token non-interactively (persisted)
pcloud-cli auth login --token <auth-token>

# Foreground mount (blocks until Ctrl+C)
pcloud-cli mount /mnt/pcloud

# Background mount (daemonized; auto-mounts and listens on the IPC socket)
pcloud-cli start /mnt/pcloud

# Unlock the Crypto folder (auto-starts a daemon if needed)
pcloud-cli crypto start

# Check combined daemon / mount / auth / crypto state
pcloud-cli status

# Gracefully stop the daemon
pcloud-cli stop
```

### Daemon lifecycle

- `pcloud-cli start [PATH]` — daemonizes, mounts the filesystem, listens for IPC.
- `pcloud-cli stop` — sends `Finalize`; daemon waits for sync to finish and exits.
- Commands that need a running daemon (`status`, `crypto *`, `backup *`) auto-spawn
  `pcloud-cli start` when no daemon is alive **and** saved credentials exist.

The daemon creates:
- PID file at `/tmp/pcloud-cli-<uid>.pid`
- Unix socket at `/tmp/pcloud-cli-<uid>.sock`

### Command reference

| Command                                | Description                                  |
|----------------------------------------|----------------------------------------------|
| `pcloud-cli`                           | Launch the interactive TUI dashboard         |
| `pcloud-cli tui`                       | Explicit TUI launcher                        |
| `pcloud-cli auth login [--token TOK]`  | Interactive web login, or persist a token    |
| `pcloud-cli auth logout`               | Clear saved token (keeps local sync data)    |
| `pcloud-cli auth status`               | Report whether saved credentials exist       |
| `pcloud-cli auth unlink [--yes]`       | Destructive: clear creds **and** local data  |
| `pcloud-cli mount [PATH] [--token T]`  | Foreground mount (blocks)                    |
| `pcloud-cli start [PATH] [--token T]`  | Background daemon mount                      |
| `pcloud-cli stop`                      | Graceful daemon shutdown                     |
| `pcloud-cli status`                    | Combined auth / mount / crypto / daemon state|
| `pcloud-cli crypto start [--password-file FILE]` | Unlock the Crypto folder          |
| `pcloud-cli crypto stop`               | Lock the Crypto folder                       |
| `pcloud-cli crypto status`             | Show Crypto folder state                     |
| `pcloud-cli backup add <PATH>`         | Register a folder as a backup                |
| `pcloud-cli backup list`               | List backups for this device                 |
| `pcloud-cli backup remove <ID>`        | Remove a backup by sync id                   |
| `pcloud-cli backup status [<ID>]`      | Backup status (optional sync id filter)      |
| `pcloud-cli backup stop-device`        | Stop all backups on the current device       |
| `pcloud-cli backup root-name`          | Print the backup root folder name            |
| `pcloud-cli doctor`                    | Dependency and environment diagnostics       |

### Environment variables

| Variable                  | Effect                                                         |
|---------------------------|----------------------------------------------------------------|
| `PCLOUD_AUTH_TOKEN`       | Ephemeral auth token (read once, cleared, never saved)         |
| `PCLOUD_AUTH_TOKEN_FILE`  | Path to a file containing an ephemeral auth token              |
| `PCLOUD_CRYPTO_PASS`      | Crypto password consumed by `pcloud-cli crypto start`          |
| `PCLOUD_CRYPTO_PASS_FILE` | Path to a file containing the crypto password                  |
| `PCLOUD_MOUNTPOINT`       | Default mountpoint for `mount` / `start` when `PATH` is omitted|

Direct env vars take priority over `_FILE` variants. Env-sourced tokens are
ephemeral and never persist to the local database.

## Architecture

```
src/
|-- main.rs              # Entry point and application flow
|-- lib.rs               # Library exports
|-- error.rs             # Error types (PCloudError, AuthError, etc.)
|-- cli/                 # CLI argument parsing
|   |-- mod.rs           # Module exports
|   |-- args.rs          # Clap subcommand tree (Cli, Command, *Args / *Op)
|   +-- auth_prompt.rs   # Interactive auth menu (web / token)
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

This section covers migration from both the original C++ `pcloud` client and from
earlier v3.x preview releases of the Rust rewrite.

### What Changed

| Area | C++ / earlier v3.x previews | Current |
|---|---|---|
| **Binary name** | `pcloud` | `pcloud-cli` |
| **Interface** | Flat flags (`-d`, `-k`, `-m`, `-t`, `-c`, `-o`, `--non-interactive`, `--logout`, `--unlink`, `--doctor`, `--nosave`) | Hierarchical subcommands (`auth`, `mount`, `start`, `stop`, `status`, `crypto`, `backup`, `tui`, `doctor`) |
| **Default mode** | Plain CLI | TUI dashboard (`pcloud-cli` with no args) |
| **Runtime paths** | `/tmp/pcloud-<uid>.pid`, `.sock` | `/tmp/pcloud-cli-<uid>.pid`, `.sock` |
| **Credentials** | Saved only with `-s` | Saved by default; use `PCLOUD_AUTH_TOKEN` env for ephemeral |
| **Removed flags** | `-u`, `-p`, `-s`, `-n`, `-y` | All flat-mode flags (above) — replaced by subcommands |
| **Removed REPL** | `-o` / `-k` interactive prompt | Dropped — every operation has a first-class subcommand |
| **Exit codes** | Varied | Standardized (0 = success, non-zero = error) |

### Unchanged

- IPC protocol is compatible (bincode over Unix socket, same command set)
- Mountpoint and sync behavior identical (same pclsync library)
- Crypto folder support is preserved, now via `pcloud-cli crypto start`

### Step-by-Step Migration

1. **Stop the old daemon**
   ```bash
   # If an older daemon is running:
   kill $(cat /tmp/pcloud-$(id -u).pid 2>/dev/null) 2>/dev/null
   kill $(cat /tmp/pcloud-cli-$(id -u).pid 2>/dev/null) 2>/dev/null
   rm -f /tmp/pcloud-$(id -u).{pid,sock} /tmp/pcloud-cli-$(id -u).{pid,sock}
   ```

2. **Install the new binary**
   Replace `pcloud` in your PATH with `pcloud-cli` (see [Installation](#installation)).

3. **Update scripts and aliases**
   ```bash
   # Before (legacy flat flags)
   pcloud -t <token> -d -m /mnt/pcloud
   pcloud -k -o                          # client mode REPL
   pcloud --logout                       # clear creds
   pcloud --unlink                       # clear creds + local data
   pcloud --doctor                       # diagnostics

   # After (hierarchical subcommands)
   pcloud-cli auth login --token <token> # persist token once
   pcloud-cli start /mnt/pcloud          # background daemon
   pcloud-cli status                     # was: -k -o then 'status'
   pcloud-cli stop                       # was: -k -o then 'finalize'
   pcloud-cli auth logout                # was: --logout
   pcloud-cli auth unlink                # was: --unlink (use --yes to skip prompt)
   pcloud-cli doctor                     # was: --doctor
   ```

4. **Update systemd units** (if applicable)
   ```ini
   ExecStart=/usr/bin/pcloud-cli start /mnt/pcloud
   ExecStop=/usr/bin/pcloud-cli stop
   PIDFile=/tmp/pcloud-cli-%U.pid
   ```

5. **Update monitoring or health checks**
   - PID file is now at `/tmp/pcloud-cli-<uid>.pid`
   - Socket is now at `/tmp/pcloud-cli-<uid>.sock`

### Compatibility Notes

- The flat-flag interface (`-d`, `-k`, `-m`, `-t`, `-c`, `-o`, `--non-interactive`,
  `--logout`, `--unlink`, `--doctor`, `--nosave`) is gone. All operations are
  expressed as subcommands. `pcloud-cli --help` and `pcloud-cli <CMD> --help`
  list everything available.
- The in-daemon interactive REPL (entered with `-o` or `-k`) has been removed.
  Use the proper subcommands (`pcloud-cli status`, `pcloud-cli crypto start`,
  `pcloud-cli stop`, `pcloud-cli backup …`) instead.
- Authentication is still token-based; obtain a token from pCloud account
  settings or use `pcloud-cli auth login` for the browser-based flow.

## Supported Platforms

Pre-built binaries are compiled on Ubuntu 22.04 and linked against **glibc 2.35**.
Any Linux distribution shipping glibc 2.35 or later can run them directly.

| Distribution | Minimum Version | Notes |
|---|---|---|
| Ubuntu | 22.04 LTS | |
| Debian | 12 (Bookworm) | |
| Fedora | 36 | Fedora 40+ removed `fuse-libs` from default repos; see [FUSE 2 note](#fuse-issues-on-linux) |
| RHEL / AlmaLinux | 9 | RHEL 8 ships glibc 2.28 — build from source if needed |
| Arch Linux | Rolling | |

### Additional requirements

- **FUSE 2.x** — FUSE 3.x is not supported. On Debian 13+ and Ubuntu 24.04+ install the `libfuse2t64` compatibility package.
- **OpenSSL 3.x** — distributions shipping only OpenSSL 1.1 (e.g. RHEL 8) must install `openssl3` or build it from source.

Older distributions can still be used by building from source on the target system (see [Building](#building)).

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
cat /tmp/pcloud-cli-$(id -u).pid
# Kill if necessary
kill $(cat /tmp/pcloud-cli-$(id -u).pid)
```

**"No daemon is running" / "Connection failed"**
- Start a daemon: `pcloud-cli start [PATH]` (commands like `status`, `crypto *`,
  `backup *` auto-start one if saved credentials exist)
- Check socket file exists: `ls -la /tmp/pcloud-cli-$(id -u).sock`

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

When running on macOS, you may see security prompts. Allow the `pcloud-cli` binary in:
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
