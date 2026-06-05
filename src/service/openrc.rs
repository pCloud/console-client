//! OpenRC backend (system service): `/etc/init.d/pcloud-cli`.

use std::path::PathBuf;

use crate::error::Result;

use super::{
    current_username, foreground_args, run, run_capture, write_executable, ServiceBackend,
    ServiceConfig, SERVICE_NAME,
};

pub struct OpenRcBackend;

fn script_path() -> PathBuf {
    PathBuf::from("/etc/init.d").join(SERVICE_NAME)
}

impl OpenRcBackend {
    /// Render the OpenRC init script. Pure — used by install and by tests.
    pub fn render(&self, cfg: &ServiceConfig) -> String {
        format!(
            "#!/sbin/openrc-run\n\
             \n\
             name=\"pCloud\"\n\
             description=\"pCloud sync (FUSE mount + IPC daemon)\"\n\
             command=\"{exe}\"\n\
             command_args=\"{args}\"\n\
             command_user=\"{user}\"\n\
             supervisor=\"supervise-daemon\"\n\
             pidfile=\"/run/{name}.pid\"\n\
             respawn_delay=5\n\
             \n\
             depend() {{\n\
             \x20 need net\n\
             }}\n",
            exe = cfg.exe.display(),
            args = foreground_args(cfg).join(" "),
            user = current_username(),
            name = SERVICE_NAME,
        )
    }
}

impl ServiceBackend for OpenRcBackend {
    fn install(&self, cfg: &ServiceConfig) -> Result<()> {
        write_executable(&script_path(), &self.render(cfg))?;
        run("rc-update", &["add", SERVICE_NAME, "default"])?;
        if cfg.start_now {
            run("rc-service", &[SERVICE_NAME, "start"])?;
        }
        Ok(())
    }

    fn uninstall(&self, _cfg: &ServiceConfig) -> Result<()> {
        let _ = run("rc-service", &[SERVICE_NAME, "stop"]);
        let _ = run("rc-update", &["del", SERVICE_NAME, "default"]);
        let _ = std::fs::remove_file(script_path());
        Ok(())
    }

    fn restart(&self, _cfg: &ServiceConfig) -> Result<()> {
        run("rc-service", &[SERVICE_NAME, "restart"])
    }

    fn status(&self, _cfg: &ServiceConfig) -> Result<String> {
        Ok(run_capture("rc-service", &[SERVICE_NAME, "status"]))
    }

    fn describe(&self) -> &'static str {
        "OpenRC"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::{Scope, Trigger};

    #[test]
    fn script_has_command_and_user() {
        let cfg = ServiceConfig {
            scope: Scope::System,
            trigger: Trigger::Boot,
            mountpoint: PathBuf::from("/home/u/pCloud"),
            exe: PathBuf::from("/usr/bin/pcloud-cli"),
            start_now: true,
        };
        let s = OpenRcBackend.render(&cfg);
        assert!(s.starts_with("#!/sbin/openrc-run"));
        assert!(s.contains("command=\"/usr/bin/pcloud-cli\""));
        assert!(s.contains("command_args=\"start --foreground /home/u/pCloud\""));
        assert!(s.contains("command_user="));
        assert!(s.contains("supervise-daemon"));
    }
}
