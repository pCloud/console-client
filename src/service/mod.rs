//! OS service integration: install/uninstall/restart a service that starts the
//! pCloud daemon on boot or user login.
//!
//! The service runs `pcloud-cli start --foreground <mountpoint>`, i.e. the
//! engine **and** the IPC server, so the TUI and `pcloud-cli` subcommands attach
//! to it over the usual Unix socket. Authentication is via saved credentials
//! (`pcloud-cli auth login` once); the service itself needs no token.
//!
//! A [`ServiceBackend`] is chosen from `(platform, init system, scope, trigger)`
//! by [`select_backend`]:
//!
//! | scope \ platform | systemd | macOS | OpenRC/runit | other Linux |
//! |---|---|---|---|---|
//! | `--user` (login) | user unit | LaunchAgent | XDG autostart | XDG autostart |
//! | `--user --boot`  | user unit + linger | LaunchAgent | cron `@reboot` | cron `@reboot` |
//! | `--system`       | system unit | LaunchDaemon | OpenRC/runit | error |

mod fallback;
mod launchd;
mod openrc;
mod runit;
mod systemd;

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{PCloudError, Result};

/// Service artifact name (systemd unit, OpenRC/runit service, `.desktop`).
pub const SERVICE_NAME: &str = "pcloud-cli";
/// launchd label / plist basename on macOS.
pub const LAUNCHD_LABEL: &str = "com.pcloud.cli";

/// Whether the service is installed for the calling user or system-wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Per-user service for the calling user (default).
    User,
    /// System-wide service (usually needs root).
    System,
}

/// When the service should start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// Start when the user logs in (default).
    Login,
    /// Start at boot, before/without an interactive login.
    Boot,
}

/// Everything a backend needs to render and manage the service.
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub scope: Scope,
    pub trigger: Trigger,
    /// Mount path passed to `start --foreground`.
    pub mountpoint: PathBuf,
    /// Absolute path to this binary (resolve via `std::env::current_exe()`).
    pub exe: PathBuf,
    /// Whether to start the service immediately after installing.
    pub start_now: bool,
}

/// Operations a service backend implements.
pub trait ServiceBackend {
    /// Generate the artifact, register it with the supervisor, and (if
    /// `start_now`) start it.
    fn install(&self, cfg: &ServiceConfig) -> Result<()>;
    /// Stop, unregister, and remove the artifact.
    fn uninstall(&self, cfg: &ServiceConfig) -> Result<()>;
    /// Restart the service.
    fn restart(&self, cfg: &ServiceConfig) -> Result<()>;
    /// Human-readable status line.
    fn status(&self, cfg: &ServiceConfig) -> Result<String>;
    /// Short label naming this backend (for user-facing messages).
    fn describe(&self) -> &'static str;
}

/// Detected service supervisor for the current host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitSystem {
    Systemd,
    OpenRc,
    Runit,
    Launchd,
    /// No recognized supervisor; only the per-user fallback is available.
    Unknown,
}

/// Detect the active init/service system.
pub fn detect_init() -> InitSystem {
    if cfg!(target_os = "macos") {
        return InitSystem::Launchd;
    }
    if Path::new("/run/systemd/system").is_dir() {
        return InitSystem::Systemd;
    }
    if Path::new("/run/openrc").exists() || command_on_path("openrc-run") {
        return InitSystem::OpenRc;
    }
    if Path::new("/run/runit").exists() || Path::new("/etc/runit").exists() {
        return InitSystem::Runit;
    }
    InitSystem::Unknown
}

/// Choose the backend for `cfg` on this host.
///
/// `--user` always resolves to a working per-user mechanism (systemd user unit,
/// launchd LaunchAgent, or the XDG-autostart / cron fallback). `--system` on a
/// host with no recognized system supervisor is an error.
pub fn select_backend(cfg: &ServiceConfig) -> Result<Box<dyn ServiceBackend>> {
    match detect_init() {
        InitSystem::Launchd => Ok(Box::new(launchd::LaunchdBackend)),
        InitSystem::Systemd => Ok(Box::new(systemd::SystemdBackend)),
        InitSystem::OpenRc => match cfg.scope {
            Scope::System => Ok(Box::new(openrc::OpenRcBackend)),
            Scope::User => Ok(fallback_backend(cfg)),
        },
        InitSystem::Runit => match cfg.scope {
            Scope::System => Ok(Box::new(runit::RunitBackend)),
            Scope::User => Ok(fallback_backend(cfg)),
        },
        InitSystem::Unknown => match cfg.scope {
            Scope::User => Ok(fallback_backend(cfg)),
            Scope::System => Err(PCloudError::NotSupported(
                "no supported system service manager detected (systemd/OpenRC/runit); \
                 try installing as a user service with --user"
                    .to_string(),
            )),
        },
    }
}

/// Per-user fallback when there is no per-user init integration: XDG autostart
/// for login, a cron `@reboot` line for boot.
fn fallback_backend(cfg: &ServiceConfig) -> Box<dyn ServiceBackend> {
    match cfg.trigger {
        Trigger::Login => Box::new(fallback::XdgAutostartBackend),
        Trigger::Boot => Box::new(fallback::CronBackend),
    }
}

/// True when this host uses the per-user XDG/cron fallback for `cfg`.
fn is_user_fallback(cfg: &ServiceConfig) -> bool {
    cfg.scope == Scope::User
        && matches!(
            detect_init(),
            InitSystem::OpenRc | InitSystem::Runit | InitSystem::Unknown
        )
}

// ---------------------------------------------------------------------------
// Top-level operations (used by the CLI)
// ---------------------------------------------------------------------------

/// Install the service for `cfg`; returns the chosen backend's label.
pub fn install(cfg: &ServiceConfig) -> Result<&'static str> {
    let backend = select_backend(cfg)?;
    backend.install(cfg)?;
    Ok(backend.describe())
}

/// Restart the service; returns the chosen backend's label.
///
/// On the per-user fallback the install-time trigger is unknown here, but the
/// XDG and cron backends share the same stop+respawn behavior, so restart works
/// regardless of which one was installed.
pub fn restart(cfg: &ServiceConfig) -> Result<&'static str> {
    if is_user_fallback(cfg) {
        fallback::XdgAutostartBackend.restart(cfg)?;
        return Ok("XDG autostart / cron");
    }
    let backend = select_backend(cfg)?;
    backend.restart(cfg)?;
    Ok(backend.describe())
}

/// Query service status; returns `(backend label, status line)`.
///
/// On the per-user fallback the install-time trigger is unknown here, so report
/// **both** the XDG autostart entry and the cron `@reboot` line so the installed
/// one is always shown.
pub fn status(cfg: &ServiceConfig) -> Result<(&'static str, String)> {
    if is_user_fallback(cfg) {
        let xdg = fallback::XdgAutostartBackend.status(cfg)?;
        let cron = fallback::CronBackend.status(cfg)?;
        return Ok(("XDG autostart / cron", format!("{}; {}", xdg, cron)));
    }
    let backend = select_backend(cfg)?;
    let line = backend.status(cfg)?;
    Ok((backend.describe(), line))
}

/// Uninstall the service; returns the chosen backend's label.
///
/// On the per-user fallback the install-time trigger is unknown here, so remove
/// **both** the XDG autostart entry and the cron `@reboot` line.
pub fn uninstall(cfg: &ServiceConfig) -> Result<&'static str> {
    if is_user_fallback(cfg) {
        fallback::XdgAutostartBackend.uninstall(cfg)?;
        fallback::CronBackend.uninstall(cfg)?;
        return Ok("XDG autostart / cron");
    }
    let backend = select_backend(cfg)?;
    backend.uninstall(cfg)?;
    Ok(backend.describe())
}

/// Best-effort `loginctl disable-linger` for the calling user.
///
/// Used after a systemd `--user` uninstall to undo the lingering that a
/// `--user --boot` install enabled. Failures are ignored (linger may never have
/// been enabled). NOTE: this disables lingering for the user entirely, which may
/// affect other services the user relies on — callers should confirm first.
pub fn disable_user_linger() {
    let _ = run("loginctl", &["disable-linger", &current_username()]);
}

// ---------------------------------------------------------------------------
// Shared helpers for backends
// ---------------------------------------------------------------------------

/// Build the `start --foreground <mountpoint>` argument vector (supervised
/// services run the engine in the foreground so the supervisor owns the
/// process).
pub(crate) fn foreground_args(cfg: &ServiceConfig) -> Vec<String> {
    vec![
        "start".to_string(),
        "--foreground".to_string(),
        cfg.mountpoint.display().to_string(),
    ]
}

// ---------------------------------------------------------------------------
// Argument quoting
//
// Mountpoints (and the binary path) can contain spaces or shell/markup
// metacharacters. Each backend renders into a different syntax, so we quote
// per-dialect. All helpers are "quote only if needed": strings made of safe
// characters pass through unchanged, keeping the generated files readable and
// the common case identical to the unquoted output.
// ---------------------------------------------------------------------------

/// True if `s` can be passed unquoted in a shell/init command line.
fn is_cmdline_safe(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-:@+,=".contains(&b))
}

/// POSIX-shell single-quote `s` when needed (OpenRC `command_args`, the runit
/// `run` script, the cron `@reboot` line).
pub(crate) fn sh_quote(s: &str) -> String {
    if is_cmdline_safe(s) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Quote `s` for a systemd unit command line: single-quote grouping (which
/// systemd understands) plus `%`→`%%` so unit specifiers are not expanded.
pub(crate) fn systemd_quote(s: &str) -> String {
    sh_quote(&s.replace('%', "%%"))
}

/// Quote `s` for a `.desktop` `Exec=` value per the XDG desktop-entry spec:
/// reserved characters are double-quoted and backslash-escaped, and the field
/// code `%` is doubled.
pub(crate) fn desktop_quote(s: &str) -> String {
    if is_cmdline_safe(s) {
        s.to_string()
    } else {
        let escaped = s
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('%', "%%");
        format!("\"{}\"", escaped)
    }
}

/// XML-escape `s` for inclusion in a launchd plist `<string>` element.
pub(crate) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Is `name` an executable on `PATH`?
fn command_on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

/// Write `content` to `path`, creating parent directories.
pub(crate) fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(PCloudError::Io)?;
    }
    std::fs::write(path, content).map_err(PCloudError::Io)
}

/// Write `content` to `path` and mark it executable (0755) — for init scripts.
pub(crate) fn write_executable(path: &Path, content: &str) -> Result<()> {
    write_file(path, content)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(PCloudError::Io)
}

/// Run a supervisor command, returning an error (including stderr) on failure.
pub(crate) fn run(program: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| PCloudError::Service(format!("failed to run `{} ...`: {}", program, e)))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(PCloudError::Service(format!(
            "`{} {}` failed: {}",
            program,
            args.join(" "),
            stderr.trim()
        )))
    }
}

/// Run a command and capture trimmed stdout (best-effort; non-zero exit is not
/// treated as an error — used for status queries).
pub(crate) fn run_capture(program: &str, args: &[&str]) -> String {
    match Command::new(program).args(args).output() {
        Ok(out) => {
            let mut s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() {
                s = String::from_utf8_lossy(&out.stderr).trim().to_string();
            }
            s
        }
        Err(e) => format!("(could not query: {})", e),
    }
}

/// The calling user's login name (for `User=` directives and messages).
pub(crate) fn current_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "root".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(scope: Scope, trigger: Trigger) -> ServiceConfig {
        ServiceConfig {
            scope,
            trigger,
            mountpoint: PathBuf::from("/home/u/pCloud"),
            exe: PathBuf::from("/usr/bin/pcloud-cli"),
            start_now: true,
        }
    }

    #[test]
    fn foreground_args_shape() {
        let a = foreground_args(&cfg(Scope::User, Trigger::Login));
        assert_eq!(a, vec!["start", "--foreground", "/home/u/pCloud"]);
    }

    #[test]
    fn select_backend_user_always_resolves() {
        // Regardless of detected init, --user must yield some backend.
        for trig in [Trigger::Login, Trigger::Boot] {
            assert!(select_backend(&cfg(Scope::User, trig)).is_ok());
        }
    }

    #[test]
    fn sh_quote_leaves_safe_strings_bare() {
        assert_eq!(sh_quote("/home/u/pCloud"), "/home/u/pCloud");
        assert_eq!(sh_quote("--foreground"), "--foreground");
        assert_eq!(sh_quote(""), "''");
    }

    #[test]
    fn sh_quote_wraps_and_escapes() {
        assert_eq!(sh_quote("/home/u/My Cloud"), "'/home/u/My Cloud'");
        assert_eq!(sh_quote("a'b"), "'a'\\''b'");
        assert_eq!(sh_quote("$(rm -rf)"), "'$(rm -rf)'");
    }

    #[test]
    fn systemd_quote_doubles_percent_and_groups() {
        // Bare when safe.
        assert_eq!(systemd_quote("/home/u/pCloud"), "/home/u/pCloud");
        // % is a specifier and forces quoting + doubling.
        assert_eq!(systemd_quote("/a%b"), "'/a%%b'");
        assert_eq!(systemd_quote("/home/u/My Cloud"), "'/home/u/My Cloud'");
    }

    #[test]
    fn desktop_quote_escapes_reserved_chars() {
        assert_eq!(desktop_quote("/home/u/pCloud"), "/home/u/pCloud");
        assert_eq!(desktop_quote("/home/u/My Cloud"), "\"/home/u/My Cloud\"");
        assert_eq!(desktop_quote("a$b`c"), "\"a\\$b\\`c\"");
    }

    #[test]
    fn xml_escape_handles_markup() {
        assert_eq!(xml_escape("/Users/u/pCloud"), "/Users/u/pCloud");
        assert_eq!(xml_escape("a&b<c>d"), "a&amp;b&lt;c&gt;d");
    }
}
