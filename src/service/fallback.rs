//! Per-user fallbacks for hosts without a recognized per-user init integration:
//! XDG autostart (`.desktop`, login) and a cron `@reboot` line (boot).
//!
//! These have no supervisor, so they launch the self-daemonizing `start` (not
//! `--foreground`).

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::daemon::{is_daemon_running, DaemonConfig};
use crate::error::{PCloudError, Result};

use super::{write_file, ServiceBackend, ServiceConfig, SERVICE_NAME};

/// Marker appended to our crontab line so we can find/remove it idempotently.
const CRON_TAG: &str = "# pcloud-cli service (managed)";

/// Spawn the self-daemonizing `start` detached from this process.
fn spawn_start(cfg: &ServiceConfig) -> Result<()> {
    Command::new(&cfg.exe)
        .arg("start")
        .arg(&cfg.mountpoint)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| PCloudError::Service(format!("failed to spawn `start`: {}", e)))
}

fn stop_daemon_best_effort(cfg: &ServiceConfig) {
    let _ = Command::new(&cfg.exe).arg("stop").status();
}

fn running_line() -> String {
    if is_daemon_running(&DaemonConfig::default()) {
        "daemon running".to_string()
    } else {
        "daemon not running".to_string()
    }
}

// ---------------------------------------------------------------------------
// XDG autostart (login)
// ---------------------------------------------------------------------------

pub struct XdgAutostartBackend;

fn desktop_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| PCloudError::Service("HOME is not set".to_string()))?;
    Ok(PathBuf::from(home)
        .join(".config/autostart")
        .join(format!("{}.desktop", SERVICE_NAME)))
}

impl XdgAutostartBackend {
    /// Render the `.desktop` autostart entry. Pure — used by install and tests.
    pub fn render(&self, cfg: &ServiceConfig) -> String {
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=pCloud\n\
             Comment=Start pCloud sync on login\n\
             Exec={exe} start {mount}\n\
             Terminal=false\n\
             X-GNOME-Autostart-enabled=true\n",
            exe = cfg.exe.display(),
            mount = cfg.mountpoint.display(),
        )
    }
}

impl ServiceBackend for XdgAutostartBackend {
    fn install(&self, cfg: &ServiceConfig) -> Result<()> {
        write_file(&desktop_path()?, &self.render(cfg))?;
        if cfg.start_now {
            spawn_start(cfg)?;
        }
        Ok(())
    }

    fn uninstall(&self, cfg: &ServiceConfig) -> Result<()> {
        let _ = std::fs::remove_file(desktop_path()?);
        stop_daemon_best_effort(cfg);
        Ok(())
    }

    fn restart(&self, cfg: &ServiceConfig) -> Result<()> {
        stop_daemon_best_effort(cfg);
        spawn_start(cfg)
    }

    fn status(&self, _cfg: &ServiceConfig) -> Result<String> {
        let installed = desktop_path().map(|p| p.exists()).unwrap_or(false);
        Ok(format!("autostart entry={}, {}", installed, running_line()))
    }

    fn describe(&self) -> &'static str {
        "XDG autostart (login)"
    }
}

// ---------------------------------------------------------------------------
// cron @reboot (boot)
// ---------------------------------------------------------------------------

pub struct CronBackend;

/// The managed crontab line for this config.
fn cron_line(cfg: &ServiceConfig) -> String {
    format!(
        "@reboot {} start {} {}",
        cfg.exe.display(),
        cfg.mountpoint.display(),
        CRON_TAG
    )
}

/// Current user crontab (empty string when there is none).
fn read_crontab() -> String {
    match Command::new("crontab").arg("-l").output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => String::new(),
    }
}

/// Install `content` as the user crontab.
fn write_crontab(content: &str) -> Result<()> {
    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| PCloudError::Service(format!("failed to run crontab: {}", e)))?;
    child
        .stdin
        .take()
        .ok_or_else(|| PCloudError::Service("crontab stdin unavailable".to_string()))?
        .write_all(content.as_bytes())
        .map_err(PCloudError::Io)?;
    let status = child
        .wait()
        .map_err(|e| PCloudError::Service(format!("crontab failed: {}", e)))?;
    if status.success() {
        Ok(())
    } else {
        Err(PCloudError::Service(
            "crontab - returned an error".to_string(),
        ))
    }
}

/// Remove any previously-managed lines from a crontab body.
fn strip_managed(crontab: &str) -> String {
    crontab
        .lines()
        .filter(|l| !l.contains(CRON_TAG))
        .collect::<Vec<_>>()
        .join("\n")
}

impl ServiceBackend for CronBackend {
    fn install(&self, cfg: &ServiceConfig) -> Result<()> {
        let mut body = strip_managed(&read_crontab());
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&cron_line(cfg));
        body.push('\n');
        write_crontab(&body)?;
        if cfg.start_now {
            spawn_start(cfg)?;
        }
        Ok(())
    }

    fn uninstall(&self, cfg: &ServiceConfig) -> Result<()> {
        let body = strip_managed(&read_crontab());
        let body = if body.trim().is_empty() {
            String::new()
        } else {
            format!("{}\n", body.trim_end())
        };
        write_crontab(&body)?;
        stop_daemon_best_effort(cfg);
        Ok(())
    }

    fn restart(&self, cfg: &ServiceConfig) -> Result<()> {
        stop_daemon_best_effort(cfg);
        spawn_start(cfg)
    }

    fn status(&self, _cfg: &ServiceConfig) -> Result<String> {
        let installed = read_crontab().contains(CRON_TAG);
        Ok(format!("@reboot entry={}, {}", installed, running_line()))
    }

    fn describe(&self) -> &'static str {
        "cron @reboot (boot)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::{Scope, Trigger};

    fn cfg(trigger: Trigger) -> ServiceConfig {
        ServiceConfig {
            scope: Scope::User,
            trigger,
            mountpoint: PathBuf::from("/home/u/pCloud"),
            exe: PathBuf::from("/usr/bin/pcloud-cli"),
            start_now: false,
        }
    }

    #[test]
    fn desktop_entry_uses_plain_start() {
        let d = XdgAutostartBackend.render(&cfg(Trigger::Login));
        assert!(d.contains("[Desktop Entry]"));
        assert!(d.contains("Exec=/usr/bin/pcloud-cli start /home/u/pCloud"));
        assert!(!d.contains("--foreground"));
    }

    #[test]
    fn cron_line_is_reboot_tagged() {
        let l = cron_line(&cfg(Trigger::Boot));
        assert!(l.starts_with("@reboot /usr/bin/pcloud-cli start /home/u/pCloud"));
        assert!(l.ends_with(CRON_TAG));
    }

    #[test]
    fn strip_managed_removes_only_tagged_lines() {
        let before = format!("0 5 * * * echo hi\n{}\n", cron_line(&cfg(Trigger::Boot)));
        let after = strip_managed(&before);
        assert!(after.contains("echo hi"));
        assert!(!after.contains(CRON_TAG));
    }
}
