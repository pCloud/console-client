//! runit backend (system service): `/etc/sv/pcloud-cli/run` symlinked into
//! `/var/service`.

use std::path::PathBuf;

use crate::error::{PCloudError, Result};

use super::{
    current_username, foreground_args, run, run_capture, write_executable, ServiceBackend,
    ServiceConfig, SERVICE_NAME,
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

impl RunitBackend {
    /// Render the `run` script. Pure — used by install and by tests.
    pub fn render(&self, cfg: &ServiceConfig) -> String {
        format!(
            "#!/bin/sh\n\
             exec 2>&1\n\
             exec chpst -u {user} {exe} {args}\n",
            user = current_username(),
            exe = cfg.exe.display(),
            args = foreground_args(cfg).join(" "),
        )
    }
}

impl ServiceBackend for RunitBackend {
    fn install(&self, cfg: &ServiceConfig) -> Result<()> {
        write_executable(&run_script(), &self.render(cfg))?;
        // Symlink into the active dir; runsvdir starts it automatically.
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
}
