//! runit backend (system service): `/etc/sv/pcloud-cli/run` symlinked into
//! `/var/service`.

use std::path::PathBuf;

use crate::error::{PCloudError, Result};

use super::{
    current_username, foreground_args, run, run_capture, sh_quote, write_executable, write_file,
    ServiceBackend, ServiceConfig, SERVICE_NAME,
};

pub struct RunitBackend;

fn service_dir() -> PathBuf {
    PathBuf::from("/etc/sv").join(SERVICE_NAME)
}

fn run_script() -> PathBuf {
    service_dir().join("run")
}

/// Active-services directory scanned by `runsvdir` (Void Linux default).
fn active_link() -> PathBuf {
    PathBuf::from("/var/service").join(SERVICE_NAME)
}

/// `down` file: when present in the service dir, `runsv` does not auto-start the
/// service (used to honor `--no-start`).
fn down_file() -> PathBuf {
    service_dir().join("down")
}

impl RunitBackend {
    /// Render the `run` script. Pure — used by install and by tests.
    pub fn render(&self, cfg: &ServiceConfig) -> String {
        format!(
            "#!/bin/sh\n\
             exec 2>&1\n\
             exec chpst -u {user} {exe} {args}\n",
            user = sh_quote(&current_username()),
            exe = sh_quote(&cfg.exe.display().to_string()),
            args = foreground_args(cfg)
                .iter()
                .map(|a| sh_quote(a))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

impl ServiceBackend for RunitBackend {
    fn install(&self, cfg: &ServiceConfig) -> Result<()> {
        write_executable(&run_script(), &self.render(cfg))?;
        // runit has no enable/disable: symlinking into the active dir makes
        // `runsvdir` pick the service up and start it. Honor `--no-start` via the
        // `down` file convention (start later with `sv up pcloud-cli`).
        if cfg.start_now {
            let _ = std::fs::remove_file(down_file());
        } else {
            write_file(&down_file(), "")?;
        }
        let link = active_link();
        if !link.exists() {
            std::os::unix::fs::symlink(service_dir(), &link).map_err(PCloudError::Io)?;
        }
        Ok(())
    }

    fn uninstall(&self, _cfg: &ServiceConfig) -> Result<()> {
        let _ = run("sv", &["stop", SERVICE_NAME]);
        let _ = std::fs::remove_file(active_link());
        let _ = std::fs::remove_dir_all(service_dir());
        Ok(())
    }

    fn restart(&self, _cfg: &ServiceConfig) -> Result<()> {
        run("sv", &["restart", SERVICE_NAME])
    }

    fn status(&self, _cfg: &ServiceConfig) -> Result<String> {
        Ok(run_capture("sv", &["status", SERVICE_NAME]))
    }

    fn describe(&self) -> &'static str {
        "runit"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::{Scope, Trigger};

    #[test]
    fn run_script_uses_chpst_and_foreground() {
        let cfg = ServiceConfig {
            scope: Scope::System,
            trigger: Trigger::Boot,
            mountpoint: PathBuf::from("/home/u/pCloud"),
            exe: PathBuf::from("/usr/bin/pcloud-cli"),
            start_now: true,
        };
        let s = RunitBackend.render(&cfg);
        assert!(s.starts_with("#!/bin/sh"));
        assert!(s.contains("exec chpst -u "));
        assert!(s.contains("/usr/bin/pcloud-cli start --foreground /home/u/pCloud"));
    }

    #[test]
    fn mountpoint_with_spaces_is_quoted_in_run_script() {
        let cfg = ServiceConfig {
            scope: Scope::System,
            trigger: Trigger::Boot,
            mountpoint: PathBuf::from("/home/u/My Cloud"),
            exe: PathBuf::from("/usr/bin/pcloud-cli"),
            start_now: true,
        };
        let s = RunitBackend.render(&cfg);
        assert!(s.contains("start --foreground '/home/u/My Cloud'"), "{s}");
    }
}
