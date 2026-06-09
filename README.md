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

| Profile     | Command                                                                  | Crash reporting |
|-------------|--------------------------------------------------------------------------|-----------------|
| Development | `cargo build`                                                            | No              |
| QA          | `PCLOUD_BUILD_PROFILE=qa cargo build --release --features crash-reporting` | Yes             |
| Release     | `cargo build --release --features crash-reporting`                       | Yes             |

(The QA/Release builds use the built-in Bugsnag key; prefix either command with
`BUGSNAG_API_KEY=<key>` to report to a different project.)

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
| `pcloud-cli completions <SHELL>`       | Print a shell completion script to stdout    |

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

### Shell completions

`pcloud-cli completions <SHELL>` prints a completion script to **stdout** for the
given shell. Supported values are `bash`, `zsh`, and `fish` (the app targets
Linux and macOS, so PowerShell and Elvish are intentionally not offered).

```bash
# Preview the generated script
pcloud-cli completions bash
```

Completions include each subcommand's description (e.g. `backup  (Manage pCloud
backups for the current device)`) in all three shells, and complete arguments by
type — directory paths for `mount`/`start`/`backup add`, files for
`crypto start --password-file`, and nothing spurious for numeric ids.

The generated script completes the command **`pcloud-cli`**, so that binary must
be reachable by that name on your `PATH` (e.g. after `cargo install --path .`,
`sudo cp target/release/pcloud-cli /usr/local/bin/`, or installing a package).
This matters when testing a local build: invoking it by a relative path such as
`./target/release/pcloud-cli` will not trigger completion. The bash script also
calls back into the binary at completion time, so `pcloud-cli` must be runnable
from your `PATH`.

> **Packages already do this for you.** The `.deb`, `.rpm`, and Arch packages can
> ship completion files; the steps below are for source builds or manual setup.

#### Linux

**Bash** — load for the current shell, or install persistently:

```bash
# Current shell only (quick test)
source <(pcloud-cli completions bash)

# Per-user (loaded automatically by bash-completion)
mkdir -p ~/.local/share/bash-completion/completions
pcloud-cli completions bash > ~/.local/share/bash-completion/completions/pcloud-cli

# System-wide (all users)
pcloud-cli completions bash | sudo tee /etc/bash_completion.d/pcloud-cli > /dev/null
```

**Zsh** — write the script to a directory on your `fpath`, then ensure `compinit`
runs:

```bash
mkdir -p ~/.zfunc
pcloud-cli completions zsh > ~/.zfunc/_pcloud-cli

# Add to ~/.zshrc (before compinit) if not already present:
#   fpath=(~/.zfunc $fpath)
#   autoload -Uz compinit && compinit
exec zsh   # reload
```

**Fish** — fish autoloads from its completions directory:

```bash
mkdir -p ~/.config/fish/completions
pcloud-cli completions fish > ~/.config/fish/completions/pcloud-cli.fish
```

#### macOS

The default shell on macOS is **zsh**.

**Zsh** — install into the Homebrew `site-functions` directory (already on
`fpath` for Homebrew zsh setups):

```bash
pcloud-cli completions zsh > "$(brew --prefix)/share/zsh/site-functions/_pcloud-cli"
exec zsh
```

If you don't use Homebrew, write to a personal dir instead and add it to `fpath`
as shown in the Linux/Zsh steps above:

```bash
mkdir -p ~/.zfunc
pcloud-cli completions zsh > ~/.zfunc/_pcloud-cli
```

**Bash** — macOS ships an old Bash, so install Homebrew's `bash-completion@2`
first, then drop the script into its completions directory:

```bash
brew install bash-completion@2
pcloud-cli completions bash > "$(brew --prefix)/etc/bash_completion.d/pcloud-cli"
```

Make sure your `~/.bash_profile` sources bash-completion:

```bash
[[ -r "$(brew --prefix)/etc/profile.d/bash_completion.sh" ]] && \
  . "$(brew --prefix)/etc/profile.d/bash_completion.sh"
```

**Fish**:

```bash
mkdir -p ~/.config/fish/completions
pcloud-cli completions fish > ~/.config/fish/completions/pcloud-cli.fish
```

After installing, restart the shell (or `exec $SHELL`) and type `pcloud-cli <TAB>`
to verify subcommands and flags complete.

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
| Native signals      | `crash-handler` catches SIGSEGV, SIGABRT, SIGBUS, SIGFPE; an out-of-process reporter writes a minidump and uploads it to Bugsnag |
| Non-fatal errors    | Top-level application errors are sent via `notify_error()` |

Native crash handling uses an out-of-process model: the binary re-execs itself
with a hidden `--crash-monitor` flag to spawn a dedicated **reporter child
process** that runs a `minidumper::Server` over IPC. When a signal fires, the
in-process handler asks the reporter to write a minidump of the crashed process
via `ptrace`, then the reporter uploads it. (A separate process is required on
Linux because the kernel forbids `ptrace` between threads of the same process.)
If the upload fails at crash time the dump is queued to
`$XDG_DATA_HOME/pcloud/crashes/` and retried on the next startup.

**Where each kind is active.** The Rust panic hook and the deferred-dump upload
are installed once at startup and survive `fork`, so they cover *every* process,
including the background daemon. The native (signal) handler is fork-sensitive —
the reporter is spawned as a child of the monitored process and `PR_SET_PTRACER`
is declared on it, and both relationships are broken by the daemon's
double-fork. It is therefore installed **after** any daemonization, in the
long-running engine processes that can actually take a native crash: the
foreground `mount`, and the `start` daemon/foreground body once it is past
`daemonize()` (this covers the background daemon, where the pclsync C engine and
FUSE run). Short-lived, IPC-only commands (`status`, `stop`, the TUI, …) rely on
the panic hook only.

### Enabling crash reporting

Crash reporting is gated behind the `crash-reporting` Cargo feature and is
**off by default** — plain `cargo build` produces a binary with no Bugsnag
dependency and no API key requirement.

To enable it, just pass the feature flag — a default Bugsnag API key is baked in,
so it builds out of the box:

```bash
cargo build --release --features crash-reporting
```

To report to a different Bugsnag project, override the key at build time:

```bash
BUGSNAG_API_KEY=<your-key> cargo build --release --features crash-reporting
```

### How the API key is provided

The Bugsnag API key is resolved at **compile time** by the build script
(`build.rs`): it uses the `BUGSNAG_API_KEY` environment variable if set and
non-empty, and otherwise falls back to a built-in default. The resolved value is
forwarded into the crate (and read in the source via `env!("BUGSNAG_API_KEY")`)
only when the `crash-reporting` feature is enabled. `build.rs` declares
`cargo:rerun-if-env-changed=BUGSNAG_API_KEY` so Cargo rebuilds when the value
changes. This means:

- Building with `--features crash-reporting` never fails for a missing key — the
  default is used. Set `BUGSNAG_API_KEY` only to target a different project.
- The key is **obfuscated** in the binary with [`obfstr`](https://docs.rs/obfstr):
  it is stored xor-encoded and deobfuscated into a fresh `String` at each use, so
  the plaintext key does not appear as a readable literal (a plain `strings` on
  the binary will not surface it). A Bugsnag *notifier* key is a client-side
  ingestion key that ships in every client — it is not a secret — but obfuscation
  raises the bar against casual extraction. For distribution builds, still
  `strip` the binary and upload Breakpad symbols separately (see below).
- Development builds (`cargo build` without `--features crash-reporting`) never
  reference the variable and contain no key.

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
