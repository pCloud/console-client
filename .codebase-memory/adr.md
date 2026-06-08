# pCloud Console Client — Architecture

Status: Active
Last reviewed: 2026-06-08
Scope: `/home/georgi-neykov/Projects/pcloud-console-client`
Branch at review: `backups_support-on-3x`
Index snapshot: 6299 graph nodes / 14928 edges (Rust + vendored C).

## 1. Mission and Scope

This repository is the **Rust rewrite of the pCloud console-client wrapper**. The original wrapper was a C++ CLI that linked against the C library `pclsync`. We are rewriting only the wrapper; `pclsync` is vendored as a git submodule (`pclsync/`) and consumed via FFI.

Out of scope: changing `pclsync` itself, supporting Windows (FUSE dependency), or implementing additional pCloud REST endpoints in Rust.

The binary is named `pcloud-cli` (`Cargo.toml`); the library crate is `console_client` (`src/lib.rs`). Packaging installs the binary as `/usr/bin/pcloud`.

## 2. Layered Architecture

```
+--------------------------------------------------------------+
| Entry point: src/main.rs                                     |
|   ExitCode dispatch -> clap-parsed Cli::command              |
+--------------------------------------------------------------+
| Surface modules                                              |
|   src/cli/        Clap tree, interactive prompts             |
|   src/tui/        Ratatui dashboard — PURE IPC CLIENT        |
|   src/service/    Boot/login autostart service install      |
+--------------------------------------------------------------+
| Domain modules                                               |
|   src/wrapper/    PCloudClient singleton (Arc<Mutex<...>>)   |
|     auth crypto filesystem backup weblogin                  |
|     status.rs  (serializable snapshots shared over IPC)     |
|   src/daemon/     Background process + Unix-socket IPC       |
|     process.rs (double-fork) signals.rs ipc.rs (bincode)    |
|     activity.rs (bounded activity ring buffer)              |
|   src/security/   SecurePassword, env-var secret intake     |
|   src/crash_reporting/  Optional (feature "crash-reporting") |
|   src/utils/      cstring, mount, browser, qrcode, deps, ... |
|   src/error.rs    PCloudError hierarchy (thiserror)         |
+--------------------------------------------------------------+
| Unsafe boundary                                              |
|   src/ffi/raw.rs       extern "C" declarations               |
|   src/ffi/types.rs     bindgen + manual C structs/constants  |
|   src/ffi/callbacks.rs Trampolines with panic catch_unwind   |
|   src/ffi/events.rs    describe_event / now_hms helpers       |
+--------------------------------------------------------------+
| pclsync (C, vendored submodule)                              |
|   Compiled by build.rs via the `cc` crate                    |
|   FUSE-enabled source set (Makefile OBJ + OBJFS)             |
|   Statically linked SQLite amalgamation (vendor/sqlite/)     |
|   System libs: fuse, openssl3, zlib, udev, pthread, m        |
+--------------------------------------------------------------+
```

Rule: **all unsafe lives in `src/ffi/`.** Every other module sees `pclsync` only through the `wrapper::` types. The wrapper layer owns the lock, the lifecycle, and the C↔Rust state mirroring. Notably the **TUI no longer touches the C library at all** — see §3 and §14.

## 3. Process Models

Three runtime shapes share the same `pcloud-cli` binary, but only the daemon hosts the pclsync engine:

1. **Daemon (the engine host)** — `pcloud-cli start [PATH]`.
   `run_start_subcommand()` calls `daemon::process::daemonize()` (double-fork via the `daemonize` crate), writes `/tmp/pcloud-cli-<uid>.pid`, opens a Unix-domain socket at `/tmp/pcloud-cli-<uid>.sock` (0600), and runs an IPC server thread alongside the sync loop. The daemon registers the pclsync event/status callbacks; the event callback feeds the **activity ring buffer** (`src/daemon/activity.rs`) so clients can poll history. This is the *only* process that initializes pclsync in normal operation.

2. **Foreground mount** — `pcloud-cli mount [PATH]`.
   `run_mount_subcommand()` installs a `ctrlc` handler, starts sync in the calling process, and blocks on a shutdown flag. Used for debugging and headless containers.

3. **Thin IPC clients** — the TUI plus `stop / status / crypto / backup`.
   These construct a `DaemonClient`, send a `DaemonCommand` over bincode, and render the `DaemonResponse`. None of them initialize pclsync. The **TUI dashboard** (bare invocation or `pcloud-cli tui`) is one of these clients: `run_tui_mode()` calls `ensure_daemon_running(...)` then `tui::run(daemon_client, &cli)`.

### Daemon auto-start
`main::ensure_daemon_running` is called from `status`, `crypto *`, `backup *`, **and the TUI launcher**. If no daemon is alive on the per-UID socket it re-spawns `pcloud-cli start` headlessly **only if** saved credentials exist; otherwise it errors with a hint to run `auth login` or pass `--token`. After spawning it polls the socket with a 5-second deadline (50 ms ticks).

### Fork safety constraint (critical)
`psync_set_auth` writes to SQLite, and SQLite handles are not fork-safe. `run_start_subcommand` therefore captures the CLI/env token into a local `Option<SecretString>` *before* `daemonize(&config)` and only invokes `set_auth_token` after the fork in the child process. Interactive login still has to happen pre-fork because the controlling terminal is gone afterwards — that path is taken only when no credentials are saved and no token was supplied. Any future change to the daemon startup ordering must preserve this rule.

## 4. The Singleton `PCloudClient`

`src/wrapper/client.rs` enforces a one-per-process invariant via a `OnceCell<Arc<Mutex<PCloudClient>>>`. This mirrors `pclsync`'s expectation of single initialization. Because the TUI is now a pure IPC client, the live client effectively exists only inside the daemon (and inside the foreground-mount process).

State carried in Rust (not the C library):
- `AuthState` (`NotAuthenticated | Authenticating | Authenticated | Failed(String)`)
- `CryptoState` (`NotSetup | SetupComplete | Started | Stopped | Failed(String)`)
- `fs_mounted: bool` and the latest mountpoint path

Because the C library can change real state from internal threads, `refresh_*` methods re-poll the C side, and the IPC handler calls them before answering `Status`/`StatusFull`. Treat the Rust-side fields as a cache, not the source of truth.

Lock discipline: keep `client.lock()` scopes minimal so the C callback thread is never starved.

## 5. FFI Layer

Sub-modules:
- `ffi/types.rs` — bindgen-generated structs + manual constants (PSTATUS_*, PSYNC_*, PEVENT_*, `pstatus_t`). Generated at build time by `build.rs` calling `bindgen` against `pclsync/psynclib.h`.
- `ffi/raw.rs` — `extern "C"` declarations for the pclsync entry points actually wrapped (auth, sync, crypto, filesystem, backup, settings).
- `ffi/callbacks.rs` — Trampolines for the C callback families (`status`, `event`, `notification`, `fs_start`) plus overlay callbacks (`crypto_start` / `crypto_stop`).
- `ffi/events.rs` — pure helpers: `describe_event(event_type, event_data)` turns a pclsync event into a `(human-readable description, is_error)` tuple; `now_hms()` formats a wall-clock `HH:MM:SS` timestamp. Used by the daemon's event callback to populate the activity log.

Callback protocol: a registered Rust closure is stored in a `Mutex`-protected global; the `extern "C"` trampoline retrieves it, wraps invocation in `catch_unwind`, and drops panics rather than unwinding across the FFI boundary. Status/event/notification callbacks do not overlap, but run on a dedicated pclsync thread, so the closure must be `Send + Sync` and short.

Memory ownership: anything pclsync returns via `psync_free`-style allocation is freed by us; each `raw::` declaration documents which side owns the buffer.

## 6. IPC Protocol (`src/daemon/ipc.rs`)

Wire format: `[u32 LE length][bincode-encoded message]`. Socket is user-private (0600). The IPC server holds a `DaemonContext` that bundles the `Arc<Mutex<PCloudClient>>` with the `Arc<ActivityLog>` so it can answer both control commands and dashboard/activity queries.

`DaemonCommand` variants:
- Lifecycle/auth: `Ping`, `Status`, `Finalize`, `Quit`, `Logout`, `Unlink` (destructive)
- Crypto: `StartCrypto { password: Option<String> }`, `StopCrypto`
- Backups: `BackupCreate { path }`, `BackupRemove { sync_id }`, `BackupList`, `BackupStatus { sync_id }`, `BackupStopDevice`, `BackupRootName`
- Dashboard (TUI): `StatusFull` and `ActivitySince { cursor }` for differential activity polling

`DaemonResponse` variants: `Ok | OkWithMessage | Error | Pong | Status {...} | BackupCreated {...} | BackupList(...) | BackupStatus(...) | BackupRootName(...) | StatusFull(Box<DashboardSnapshot>) | Activity { entries, cursor } | AuthWeb {...}`.

Security boundary: passwords cross the socket as `String` (local-and-owner-only), then are *immediately* converted to `SecurePassword` and the original `String` is `zeroize`d. `Debug` for `DaemonCommand` redacts password fields.

## 7. Serializable Snapshots (`src/wrapper/status.rs`)

The wrapper exposes plain, `Serialize`/`Deserialize` snapshot types so the daemon can hand a consistent view to IPC clients without exposing the live client:
- `StatusSnapshot` — a copy of the relevant `pstatus_t` fields (counts, bytes, speeds, storage flags) plus a derived `status_str` and `sync_engine_state()`.
- `SyncEngineState` — coarse enum (`Running | Paused | Stopped | Inactive`) derived from the status code.
- `ActivityEntry` — `seq` (monotonic u64), `timestamp`, `description`, `is_error`; the `seq` enables cursor-based incremental polling.
- `DashboardSnapshot` — the full TUI payload: `StatusSnapshot` + auth/crypto state + mount info + account/quota/crypto-folder fields. Returned (boxed) by `DaemonResponse::StatusFull`.

## 8. Activity Ring Buffer (`src/daemon/activity.rs`)

`ActivityLog` is a thread-safe bounded FIFO (`Mutex<VecDeque<ActivityEntry>>`, cap ~200) living in the daemon. The pclsync event callback calls `describe_event` (§5) and `push`es an entry with an auto-assigned monotonic `seq`. Clients call `since(cursor) -> (entries, new_cursor)` (via `ActivitySince`) to pull only entries newer than what they have. This is what lets the TUI render a live activity feed without registering any C callbacks of its own.

## 9. Service Integration (`src/service/`)

Installs/removes an OS service that runs the pCloud daemon on boot or login.
- `ServiceBackend` trait (`install` / `uninstall` / status / describe) with backends: `systemd.rs` (user + system units, linger for boot), `launchd.rs` (macOS LaunchAgent/LaunchDaemon), `openrc.rs`, `runit.rs`, and `fallback.rs` (XDG autostart for login + cron `@reboot` for boot).
- `detect_init()` probes `/run/systemd`, `/run/openrc`, `/run/runit` (and macOS) to choose an init system.
- `select_backend(&ServiceConfig)` resolves the `(Scope::{User,System}, Trigger::{Login,Boot})` combination to a concrete backend; top-level `install` / `uninstall` are the entry points used by the `service` CLI command.

## 10. Security Surfaces

- **`security::SecurePassword`** wraps `secrecy::SecretString`. Custom `Debug`/`Display` print `[REDACTED]`. `to_cstring()` zeroizes the intermediate buffer after handoff to C.
- **`security::env`** — `resolve_auth_token()`, `resolve_crypto_password()`, `ResolvedSecrets::from_env()`. Env knobs: `PCLOUD_AUTH_TOKEN[_FILE]`, `PCLOUD_CRYPTO_PASS[_FILE]`. Priority is direct var > `_FILE` var > `None` (`resolve_secret`); after a value is read both the direct and `_FILE` vars are `remove_var`'d from the process environment. Env-sourced tokens are **never persisted** — `apply_auth` applies them via `set_auth_token(&token, false)`, whereas a CLI `--token` uses the caller's `save` flag.
- **CLI token persistence** — only `auth login --token …` persists (`set_auth_token(.., true)`). The deferred-token apply in `run_start_subcommand` respects this via `save_deferred_token`.
- **Threat-model gaps** (intentionally accepted): plaintext passwords in transit on the Unix socket; potential core-dump exposure; compiler may optimize away zeroization. Documented in `CLAUDE.md`.

## 11. Crash Reporting (optional, off by default)

`src/crash_reporting/` — gated behind the **non-default** Cargo feature `crash-reporting` (`default = []`; the feature pulls in `bugsnag`, `crash-handler`, `minidumper`, `obfstr`, `ureq`). With the feature **off**, `crash_reporting::{check_monitor_args, init, notify_error}` compile to no-op stubs in `mod.rs`, so a stock build has no crash reporting.

When the feature is **on** (`native.rs`, `panic_hook.rs`, `config.rs`): the binary re-execs **itself** as a crash-monitor subprocess — `spawn_monitor_process` runs `std::env::current_exe()` with the `--crash-monitor <socket> <dump_dir>` args. At startup `main()` calls `check_monitor_args()` first (matching `args[1] == "--crash-monitor"` with ≥4 args); if matched it runs `run_monitor(socket_name, dump_dir)` and exits before any normal work. Otherwise `init()` installs the panic hook, and errors out of `run(cli)` pass through `notify_error(&e)` before being printed.

## 12. Error Hierarchy

Single top-level enum `PCloudError` in `src/error.rs`, with `thiserror`. Sub-errors: `FfiError`, `AuthError`, `WebLoginError`, `CryptoError`, `FilesystemError`, `DaemonError`, `BackupError`, plus `Config(String)`, `Io(io::Error)`, `InvalidArgument(String)`, `NotSupported(String)`, `CString(NulError)`. Sub-errors carry C-code constructors (`CryptoError::from_setup_code`/`from_start_code`/`from_stop_code`/`from_generic_code`, `FilesystemError::from_code`, …). `main` prints `Error: {e}` and returns `ExitCode::FAILURE`.

## 13. CLI Surface

Hierarchical clap tree (`src/cli/args.rs`); every node supports `--help`. Top-level `Command`:

```
auth        login [--token T] | logout | status | unlink [--yes]
mount       [PATH] [--token T]                  (foreground engine)
start       [PATH] [--token T]                  (daemonize: the engine host)
stop                                            (Finalize via IPC)
status                                          (IPC client; auto-starts daemon)
crypto      start [--password-file FILE] | stop | status
backup      add <PATH> | list | remove <ID> | stop-device | status [<ID>] | root-name
service     install / uninstall (boot or login autostart; --scope/--trigger)
tui                                             (explicit TUI launcher; IPC client)
doctor                                          (utils::deps::run_doctor)
completions <SHELL>                             (bash/zsh/fish script to stdout)
__complete  (hidden)                            (dynamic completion callback for bash)
```

Bare `pcloud-cli` (no subcommand) launches the TUI. Bare parent invocations like `pcloud-cli auth` reach `print_subcommand_help(...)` because each parent `*Args` declares its `op` as `Option<...>`.

### Shell completions
`completions <SHELL>` supports **bash, zsh, fish only** — a project-local `CompletionShell` enum; PowerShell and Elvish are intentionally excluded since the app targets Linux/macOS. zsh and fish use `clap_complete::generate` (both render per-command descriptions natively). bash is special-cased because `clap_complete` emits bare names for bash in every mode:
- `run_completions` emits a hand-written, description-aware bash script (`write_bash_completion`) that registers a function and calls back into the hidden **`__complete`** subcommand at completion time.
- `run_complete` resolves candidates with `clap_complete::engine::complete` (the **`unstable-dynamic`** feature, enabled in `Cargo.toml`) and prints `value\t<description>` lines; the bash function renders them cobra-style as padded `name  (description)` entries, so descriptions show while only the common prefix is inserted.
- Arguments carry `value_hint`s so completion offers the right value type instead of dumping the cwd: `DirPath` for `mount`/`start`/`service install`/`backup add`, `FilePath` for `crypto start --password-file`, `Other` (no file listing) for the numeric `backup remove`/`backup status` ids.

Depending on `clap_complete`'s `unstable-dynamic` engine is a deliberate trade (its API can shift across minor versions); the version is pinned to the `4.6` line and `tests/integration/cli_tests.rs` guards the `__complete` description output so a breaking bump fails CI. The command name baked into every script is `pcloud-cli`.

Version string / `after_long_help` are built from `PCLOUD_VERSION`, `PCLOUD_GIT_COMMIT_SHORT`, `PSYNC_LIB_VERSION`, `PCLSYNC_GIT_COMMIT_SHORT` env vars exported by `build.rs`.

## 14. TUI Architecture (pure IPC client)

Built with Ratatui 0.29 (re-exports crossterm as `ratatui::crossterm`). See `src/tui/CLAUDE.md` for the widget contract. **The TUI owns no pclsync engine and registers no C callbacks** — it is a thin client of the daemon:
- **mod.rs** — `run(daemon: DaemonClient, cli)` drives the Ratatui event loop; the caller (`run_tui_mode`) has already ensured a daemon is reachable.
- **app.rs** — `App` owns the `DaemonClient`. `tick()` (1 s) calls `poll_status()` → `DaemonCommand::StatusFull` and `poll_activity()` → `DaemonCommand::ActivitySince { cursor }`, tracking an `activity_cursor` so only new entries are pulled. Key-triggered actions (crypto unlock, backup add/remove, unlink, auth) send their command and immediately re-poll.
- **state.rs** — `InputMode` state machine (Normal / auth / password / backup / unlink-confirm screens).
- **ui.rs** dispatches per-screen rendering; **widgets/** is one file per visual component (tab bar, header, transfer, activity log, auth/password/unlink modals, backups screens, help/about). **theme.rs** is the only file allowed to name colors.

## 15. Build System (`build.rs`)

In order:
1. Compile vendored SQLite amalgamation `vendor/sqlite/sqlite3.c` into a static `libsqlite3.a` with a fixed feature set (`compile_sqlite()`). SQLite is **not** a system dependency.
2. Compile `pclsync/` C sources matching the Makefile `OBJ` + `OBJFS` sets via `cc`. Optional features (document editing, Linux overlay-icons client) are *not* wired in.
3. Link system libs: `fuse`, `openssl3`, `zlib`, `udev`, `pthread`, `m`. SSL backend fixed to `P_SSL_OPENSSL3` on Linux and macOS (`configure_linux`/`configure_macos`); switching requires a define change plus a `link_system_libraries` update.
4. Generate Rust FFI types from `pclsync/psynclib.h` via `bindgen`.
5. Detect platform; emit `-Wl,--gc-sections` (Linux) / `-Wl,-dead_strip` (macOS).
6. Export version/commit env vars consumed by `cli/args.rs`.

Dependencies discovered via `pkg-config` with hard-coded Homebrew fallbacks on macOS.

## 16. Packaging

- `.deb` via `cargo-deb` (`[package.metadata.deb]`), `.rpm` via `cargo-generate-rpm` (`[package.metadata.generate-rpm]`), Arch via `makepkg` (`pkg/arch/PKGBUILD`).
- All install the binary as `/usr/bin/pcloud` and ship `LICENSE`. Runtime deps always cover FUSE, OpenSSL/TLS, zlib, udev/systemd-libs. SQLite is **not** a runtime dependency.
- Supported floor: Ubuntu 22.04 / Debian 12 / Fedora 36 / RHEL 9 / Arch rolling. FUSE 2.x required (Debian 13+/Ubuntu 24.04+ need `libfuse2t64`; Fedora 40+ removed `fuse-libs`).

## 17. Tests

- Unit tests colocated as `#[cfg(test)]` modules (heaviest in `src/lib.rs`, `src/main.rs`, `src/cli/args.rs`, `src/daemon/{mod,process,ipc}.rs`, `src/security/{password,env}.rs`, and the new `src/service/mod.rs`).
- Integration tests in `tests/integration/` (`cli_tests.rs`, `daemon_tests.rs`, `ipc_tests.rs`, `signal_tests.rs`) driven from `tests/integration.rs`. `cli_tests.rs` also guards the shell-completion surface (supported shells, rejection of PowerShell/Elvish, the bash callback script, and the `__complete` description/value-hint output).
- Pre-commit hook `.githooks/pre-commit` runs `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo check`, `cargo test`. Enable per checkout: `git config core.hooksPath .githooks`.
- In the Claude Code sandbox, run with `TMPDIR=/tmp/claude-1000/rust-tmp cargo test`.

## 18. Cross-cutting Conventions

- **No `Co-Authored-By` footer** on generated commits (see `COMMIT.md`).
- Keep unsafe blocks minimal and document their invariant; passwords always go through `SecurePassword`; never derive `Debug` on password-bearing structs.
- New FFI functions follow the four-step recipe in `CLAUDE.md`: declare in `ffi/raw.rs`, add types to `ffi/types.rs`, wrap in `src/wrapper/`, add tests.
- Configuration is environment-driven; no config-file format yet (future work).

## 19. Known Constraints & Future Work

Constraints (accepted): singleton client (no multi-account multiplexing in one process); pclsync owns its own thread pool; no Windows support; plaintext-on-Unix-socket IPC mitigated by 0600 socket + same-user threat model; glibc 2.34+ floor from the Ubuntu 22.04 build host. Shell completions for bash depend on `clap_complete`'s `unstable-dynamic` engine (pinned, CI-guarded).

Backlog (from `CLAUDE.md`): config-file support, daemon log-to-file, selective sync, better error recovery, progress reporting during sync.

## 20. Map of Key Files

| Concern | File |
|---|---|
| Entry point + dispatch | `src/main.rs` |
| Crate root + module exports | `src/lib.rs` |
| Error hierarchy | `src/error.rs` |
| Clap CLI tree | `src/cli/args.rs`, `src/cli/mod.rs` |
| Shell completions (bash generator + `__complete`) | `src/main.rs` (`run_completions`, `run_complete`, `write_bash_completion`) |
| TUI dashboard (IPC client) | `src/tui/{mod,app,state,ui,theme}.rs`, `src/tui/widgets/*.rs` |
| Service install/uninstall | `src/service/{mod,systemd,launchd,openrc,runit,fallback}.rs` |
| Client singleton + state | `src/wrapper/client.rs` |
| Authentication flows | `src/wrapper/auth.rs`, `src/wrapper/weblogin.rs` |
| Crypto operations | `src/wrapper/crypto.rs` |
| Filesystem & sync folders | `src/wrapper/filesystem.rs` |
| Backups | `src/wrapper/backup.rs` |
| Serializable IPC snapshots | `src/wrapper/status.rs` |
| Daemonization & PID file | `src/daemon/process.rs` |
| Signal handling | `src/daemon/signals.rs` |
| IPC server + client + wire protocol | `src/daemon/ipc.rs` |
| Activity ring buffer | `src/daemon/activity.rs` |
| Secret intake | `src/security/env.rs` |
| Password container | `src/security/password.rs` |
| FFI declarations | `src/ffi/raw.rs` |
| FFI types | `src/ffi/types.rs` |
| Callback trampolines | `src/ffi/callbacks.rs` |
| Event description helpers | `src/ffi/events.rs` |
| Crash monitor subprocess (optional) | `src/crash_reporting/{mod,native,panic_hook,config}.rs` |
| Build orchestration | `build.rs` |
| Packaging metadata | `Cargo.toml`, `pkg/arch/PKGBUILD` |
| Vendored C library | `pclsync/` (submodule) |
| Vendored SQLite | `vendor/sqlite/` |
| Integration tests | `tests/integration.rs`, `tests/integration/*.rs` |
| Pre-commit checks | `.githooks/pre-commit` |
| TUI-specific guide | `src/tui/CLAUDE.md` |
| pclsync subsystem guide | `pclsync/CLAUDE.md` |
