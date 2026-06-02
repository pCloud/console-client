# pCloud Console Client — Architecture

Status: Active
Last reviewed: 2026-06-02
Scope: `/home/georgi-neykov/Projects/pcloud-console-client`
Index snapshot: 4869 graph nodes / 22087 edges, 179 source files (Rust + vendored C).

## 1. Mission and Scope

This repository is the **Rust rewrite of the pCloud console-client wrapper**. The original wrapper was a C++ CLI that linked against the C library `pclsync`. We are rewriting only the wrapper; `pclsync` is vendored as a git submodule (`pclsync/`) and consumed via FFI.

Out of scope: changing `pclsync` itself, supporting Windows (FUSE dependency), or implementing additional pCloud REST endpoints in Rust.

The binary is named `pcloud-cli` (`Cargo.toml`); the library crate is `console_client` (`src/lib.rs`).

## 2. Layered Architecture

```
+--------------------------------------------------------------+
| Entry point: src/main.rs                                     |
|   ExitCode dispatch -> clap-parsed Cli::command              |
+--------------------------------------------------------------+
| Surface modules                                              |
|   src/cli/        Clap tree, interactive prompts             |
|   src/tui/        Ratatui dashboard (default when no subcmd) |
+--------------------------------------------------------------+
| Domain modules                                               |
|   src/wrapper/    PCloudClient singleton (Arc<Mutex<...>>)   |
|     auth.rs, crypto.rs, filesystem.rs, backup.rs, weblogin   |
|   src/daemon/     Background process + Unix-socket IPC       |
|     process.rs (double-fork) signals.rs ipc.rs (bincode)     |
|   src/security/   SecurePassword, env-var secret intake      |
|   src/crash_reporting/  Native handler + monitor subprocess  |
|   src/utils/      cstring, mount, browser, qrcode, deps,     |
|                   terminal helpers                           |
|   src/error.rs    PCloudError hierarchy (thiserror)          |
+--------------------------------------------------------------+
| Unsafe boundary                                              |
|   src/ffi/raw.rs       extern "C" declarations               |
|   src/ffi/types.rs     bindgen + manual C structs/constants  |
|   src/ffi/callbacks.rs Trampolines with panic catch_unwind   |
+--------------------------------------------------------------+
| pclsync (C, vendored submodule)                              |
|   Compiled by build.rs via the `cc` crate                    |
|   FUSE-enabled source set (Makefile OBJ + OBJFS)             |
|   Statically linked SQLite amalgamation (vendor/sqlite/)     |
|   System libs: fuse, openssl3, zlib, udev, pthread, m        |
+--------------------------------------------------------------+
```

Rule: **all unsafe lives in `src/ffi/`.** Every other module sees `pclsync` only through the `wrapper::` types. The wrapper layer owns the lock, the lifecycle, and the C↔Rust state mirroring.

## 3. Process Models

Three runtime shapes share the same binary:

1. **TUI (default)** — `pcloud-cli` with no subcommand or `pcloud-cli tui`.
   `run_tui_mode()` in `src/main.rs` calls `tui::run()`, which registers C-library callbacks that forward into an `mpsc::channel`, starts sync inside the same process, and drives Ratatui's event loop until the user quits. The C library, the TUI render thread, and a web-auth helper thread all share one `Arc<Mutex<PCloudClient>>`.

2. **Foreground mount** — `pcloud-cli mount [PATH]`.
   `run_mount_subcommand()` installs a `ctrlc` handler, starts sync in the calling process, and blocks on a shutdown flag. Used for debugging and for headless containers.

3. **Daemon + thin CLI clients** — `pcloud-cli start [PATH]` plus any of `stop / status / crypto / backup`.
   `run_start_subcommand()` calls `daemon::process::daemonize()` (double-fork via the `daemonize` crate), writes `/tmp/pcloud-cli-<uid>.pid`, opens a Unix-domain socket at `/tmp/pcloud-cli-<uid>.sock` (0600), and runs an IPC server thread alongside the sync loop. The CLI clients are thin: they construct a `DaemonClient`, send a `DaemonCommand` over bincode, and print the `DaemonResponse`.

### Daemon auto-start
`main::ensure_daemon_running` is called from `status`, `crypto *`, and `backup *`. If no daemon is alive on the per-UID socket it re-spawns `pcloud-cli start` headlessly **only if** saved credentials exist; otherwise it errors with a hint to run `auth login` or pass `--token`. After spawning it polls the socket with a 5-second deadline (50 ms ticks).

### Fork safety constraint (critical)
`psync_set_auth` writes to SQLite, and SQLite handles are not fork-safe. `run_start_subcommand` therefore captures the CLI/env token into a local `Option<SecretString>` *before* `daemonize(&config)` and only invokes `set_auth_token` after the fork in the child process. Interactive login still has to happen pre-fork because the controlling terminal is gone afterwards — that path is taken only when no credentials are saved and no token was supplied. Any future change to the daemon startup ordering must preserve this rule.

## 4. The Singleton `PCloudClient`

`src/wrapper/client.rs` enforces a one-per-process invariant via a `OnceCell<Arc<Mutex<PCloudClient>>>`. This mirrors `pclsync`'s expectation of single initialization and lets every subsystem (TUI, daemon IPC handler, foreground mount) share one C-library handle.

State carried in Rust (not the C library):
- `AuthState` (`NotAuthenticated | Authenticating | Authenticated | Failed(String)`)
- `CryptoState` (`NotSetup | SetupComplete | Started | Stopped | Failed(String)`)
- `fs_mounted: bool` and the latest mountpoint path

Because the C library can change real state from internal threads, `refresh_*` methods on the client re-poll the C side. The daemon IPC handler explicitly calls `refresh_*` before answering `Status` (commit `7403064`). Treat the Rust-side fields as a cache, not the source of truth.

Lock discipline: keep `client.lock()` scopes minimal so the C callback thread is never starved. The TUI module's `CLAUDE.md` calls this out as well.

## 5. FFI Layer

Sub-modules:
- `ffi/types.rs` — bindgen-generated structs + manual constants (PSTATUS_*, PSYNC_*, PEVENT_*, plus the `pstatus_t` reexport). Generated at build time by `build.rs` calling `bindgen` against `pclsync/psynclib.h`.
- `ffi/raw.rs` — `extern "C"` declarations for the pclsync entry points actually wrapped (auth, sync, crypto, filesystem, backup, settings).
- `ffi/callbacks.rs` — Trampolines for the four C callback families (`status`, `event`, `notification`, `fs_start`) plus application-level overlay callbacks (`crypto_start` / `crypto_stop`).

Callback protocol: a registered Rust closure is stored in a `Mutex`-protected global; the `extern "C"` trampoline retrieves it, wraps invocation in `catch_unwind`, and silently drops panics rather than unwinding across the FFI boundary. Status/event/notification callbacks are guaranteed by `pclsync` not to overlap each other, but they do run on a dedicated pclsync thread, so the Rust closure must be `Send + Sync` and short.

Memory ownership: anything pclsync returns via `psync_free`-style allocation is freed by us; the docstrings on each `raw::` declaration state which side owns the buffer.

## 6. IPC Protocol (`src/daemon/ipc.rs`)

Wire format: `[u32 LE length][bincode-encoded message]`. Socket is user-private (0600).

`DaemonCommand` variants implemented today:
- `Ping`, `Status`, `Finalize`, `Quit`
- `StartCrypto { password: Option<String> }`, `StopCrypto`
- `Logout`, `Unlink` (destructive)
- `BackupCreate { path }`, `BackupRemove { sync_id }`, `BackupList`, `BackupStatus { sync_id }`, `BackupStopDevice`, `BackupRootName`

`DaemonResponse` variants: `Ok | OkWithMessage | Error | Pong | Status {...} | BackupCreated {...} | BackupList(...) | BackupStatus(...) | BackupRootName(...)`.

Security boundary: passwords cross the socket as `String` (the socket is local-and-owner-only), then are *immediately* converted to `SecurePassword` and the original `String` is `zeroize`d. `Debug` for `DaemonCommand` redacts password fields. CLI surface: `run_crypto_subcommand` calls `drop(password)` right after the response is received.

## 7. Security Surfaces

- **`security::SecurePassword`** wraps `secrecy::SecretString`. Custom `Debug`/`Display` print `[REDACTED]`. `to_cstring()` zeroizes the intermediate buffer after handoff to C.
- **`security::env`** — `resolve_auth_token()`, `resolve_crypto_password()`, `ResolvedSecrets::from_env()`. The four env knobs are `PCLOUD_AUTH_TOKEN[_FILE]` and `PCLOUD_CRYPTO_PASS[_FILE]`. Direct vars beat `_FILE` variants; env-sourced tokens are **never persisted** (the boolean to `set_auth_token` is `false`). The env vars are cleared from the process after reading.
- **CLI token persistence** — only `auth login --token …` persists (save flag `true`). The deferred-token apply in `run_start_subcommand` respects this distinction via `save_deferred_token`.
- **Threat model gaps** (intentionally accepted): plaintext passwords in transit on the Unix socket; potential core-dump exposure; compiler may optimize away zeroization. Documented in `CLAUDE.md`.

## 8. Crash Reporting

`src/crash_reporting/` (Module: `native.rs`, `panic_hook.rs`, `config.rs`).
The binary can re-exec itself as a crash-monitor subprocess — `main()` checks `check_monitor_args()` before doing anything else and, if matched, calls `run_monitor(socket_name, dump_dir)` and exits. Otherwise `init()` installs the panic hook for the normal run. Errors out of `run(cli)` go through `notify_error(&e)` before being printed.

## 9. Error Hierarchy

Single top-level enum `PCloudError` in `src/error.rs`, with `thiserror` for `Display`/`From`. Sub-errors:
`FfiError`, `AuthError`, `WebLoginError`, `CryptoError`, `FilesystemError`, `DaemonError`, `BackupError`, plus structural variants `Config(String)`, `Io(std::io::Error)`, `InvalidArgument(String)`, `NotSupported(String)`, `CString(NulError)`.

Sub-errors carry constructors keyed off C return codes (`CryptoError::from_setup_code`, `from_start_code`, `from_stop_code`, `from_generic_code`, `FilesystemError::from_code`, etc.). Wrapper functions translate C codes to the appropriate variant; the CLI surface just bubbles `Result<()>` up to `main`, which prints `Error: {e}` and returns `ExitCode::FAILURE`.

## 10. CLI Surface

Hierarchical clap tree (`src/cli/args.rs`), every node supports `--help`. The top-level enum is `Command`:

```
auth   login [--token T] | logout | status | unlink [--yes]
mount  [PATH] [--token T]                  (foreground)
start  [PATH] [--token T]                  (daemonize)
stop                                       (Finalize via IPC)
status                                     (auto-starts daemon if creds exist)
crypto start [--password-file FILE] | stop | status
backup add <PATH> | list | remove <ID> | stop-device | status [<ID>] | root-name
tui                                        (explicit TUI launcher)
doctor                                     (utils::deps::run_doctor)
```

Bare invocations like `pcloud-cli auth` reach `print_subcommand_help("auth")` — every parent `*Args` struct declares its `op` as `Option<...>`, so a missing subcommand is a valid parse rather than a clap error.

Version string and `after_long_help` are built from `PCLOUD_VERSION`, `PCLOUD_GIT_COMMIT_SHORT`, `PSYNC_LIB_VERSION`, and `PCLSYNC_GIT_COMMIT_SHORT` env vars exported by `build.rs`.

## 11. TUI Architecture

Built with Ratatui 0.29 (re-exports crossterm as `ratatui::crossterm`). See `src/tui/CLAUDE.md` for the widget contract and theme rules; the highlights:

- **mod.rs** wires three C callbacks (`register_status_callback`, `register_event_callback`, `register_fs_start_callback`) to send `TuiEvent` over an mpsc channel; the main render thread drains it via `try_recv` each frame.
- **app.rs** owns the input handler, crypto/auth operations, and the 1-second tick that polls `PCloudClient` state.
- **state.rs** drives the `InputMode` state machine (Normal / AuthMenu / AuthToken / AuthWebWaiting / PasswordPrompt / HintPrompt / UnlinkConfirm).
- **ui.rs** dispatches per-screen rendering; **widgets/** is one file per visual component.
- **theme.rs** is the only file allowed to name colors.
- `needs_clear` is set on layout-incompatible transitions (full-screen auth ↔ dashboard) so Ratatui's diff buffer doesn't leave stale cells.

Web-auth runs on a `std::thread::spawn`; the main loop notices completion via the tick poll, not via the channel.

## 12. Build System (`build.rs`, 25 KB)

Build script responsibilities, in order:
1. Compile vendored SQLite amalgamation `vendor/sqlite/sqlite3.c` into a standalone static `libsqlite3.a` with a fixed feature set (`compile_sqlite()`). SQLite is **not** a system dependency.
2. Compile all `pclsync/` C sources matching the Makefile `OBJ` + `OBJFS` sets via the `cc` crate. Optional features (document editing, Linux overlay-icons client) are *not* wired in.
3. Link system libraries: `fuse`, `openssl3`, `zlib`, `udev`, `pthread`, `m`. SSL backend is fixed to `P_SSL_OPENSSL3` on both Linux and macOS (`configure_linux` / `configure_macos`). Switching backends requires both a define change and an update to `link_system_libraries`.
4. Generate Rust FFI types from `pclsync/psynclib.h` via `bindgen` into the FFI types module.
5. Detect platform; emit `-Wl,--gc-sections` (Linux) or `-Wl,-dead_strip` (macOS) so unreferenced sections drop out.
6. Export version/commit env vars consumed by `cli/args.rs`.

Dependencies are discovered via `pkg-config` with hard-coded Homebrew fallbacks on macOS.

## 13. Packaging

Three formats; metadata lives in-repo:
- `.deb` via `cargo-deb` (`[package.metadata.deb]` in `Cargo.toml`)
- `.rpm` via `cargo-generate-rpm` (`[package.metadata.generate-rpm]` in `Cargo.toml`)
- Arch via `makepkg` (`pkg/arch/PKGBUILD`)

All install the binary as `/usr/bin/pcloud` and ship `LICENSE`. Runtime dependencies differ per format but always cover FUSE, OpenSSL/TLS, zlib, and udev/systemd-libs. SQLite is **not** a runtime dependency.

Supported floor: Ubuntu 22.04 / Debian 12 / Fedora 36 / RHEL 9 / Arch rolling. FUSE 2.x is required (Debian 13+ and Ubuntu 24.04+ need `libfuse2t64`; Fedora 40+ removed `fuse-libs`).

## 14. Tests

- **Unit tests** colocated as `#[cfg(test)]` modules — heaviest in `src/lib.rs`, `src/main.rs`, `src/cli/args.rs`, `src/daemon/{mod,process,ipc}.rs`, `src/security/password.rs`.
- **Integration tests** in `tests/integration/` (`cli_tests.rs`, `daemon_tests.rs`, `ipc_tests.rs`, `signal_tests.rs`), all driven from `tests/integration.rs`.
- The pre-commit hook at `.githooks/pre-commit` runs `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo check`, `cargo test`. Enable per checkout with `git config core.hooksPath .githooks`.
- In the Claude Code sandbox, run with `TMPDIR=/tmp/claude-1000/rust-tmp cargo test` so the test scratch dir is writable.

## 15. Cross-cutting Conventions

- **No `Co-Authored-By` footer** on generated commits (see `COMMIT.md`).
- Keep unsafe blocks minimal and document the invariant they rely on; passwords always go through `SecurePassword`.
- Don't derive `Debug` on structs that hold a password — implement it manually with `[REDACTED]`.
- New FFI functions follow the four-step recipe in `CLAUDE.md`: declare in `ffi/raw.rs`, add types to `ffi/types.rs`, wrap in `src/wrapper/`, add tests.
- Configuration is environment-driven; no config file format exists yet (listed as a future improvement in `CLAUDE.md`).

## 16. Known Constraints & Future Work

Constraints (accepted):
- Singleton client; cannot multiplex pCloud accounts in one process.
- pclsync owns its own thread pool — callbacks can fire at any moment.
- No Windows support (FUSE dependency).
- Plaintext-on-Unix-socket for IPC; mitigated by 0600 socket and same-user threat model.
- glibc 2.34+ floor from the Ubuntu 22.04 build host.

Backlog (from `CLAUDE.md`):
- Config-file support, daemon log-to-file, selective sync, better error recovery, progress reporting during sync.

## 17. Map of Key Files

| Concern | File |
|---|---|
| Entry point + dispatch | `src/main.rs` |
| Crate root + module exports | `src/lib.rs` |
| Error hierarchy | `src/error.rs` |
| Clap CLI tree | `src/cli/args.rs`, `src/cli/mod.rs`, `src/cli/auth_prompt.rs` |
| TUI dashboard | `src/tui/{mod,app,state,event_types,ui,theme}.rs`, `src/tui/widgets/*.rs` |
| Client singleton + state | `src/wrapper/client.rs` |
| Authentication flows | `src/wrapper/auth.rs`, `src/wrapper/weblogin.rs` |
| Crypto operations | `src/wrapper/crypto.rs` |
| Filesystem & sync folders | `src/wrapper/filesystem.rs` |
| Backups | `src/wrapper/backup.rs` |
| Daemonization & PID file | `src/daemon/process.rs` |
| Signal handling | `src/daemon/signals.rs` |
| IPC server + client + wire protocol | `src/daemon/ipc.rs` |
| Secret intake | `src/security/env.rs` |
| Password container | `src/security/password.rs` |
| FFI declarations | `src/ffi/raw.rs` |
| FFI types | `src/ffi/types.rs` |
| Callback trampolines | `src/ffi/callbacks.rs` |
| Crash monitor subprocess | `src/crash_reporting/{native,panic_hook,config}.rs` |
| Build orchestration | `build.rs` |
| Packaging metadata | `Cargo.toml`, `pkg/arch/PKGBUILD` |
| Vendored C library | `pclsync/` (submodule) |
| Vendored SQLite | `vendor/sqlite/` |
| Integration tests | `tests/integration.rs`, `tests/integration/*.rs` |
| Pre-commit checks | `.githooks/pre-commit` |
| TUI-specific guide | `src/tui/CLAUDE.md` |
| pclsync subsystem guide | `pclsync/CLAUDE.md` |

