//! systemd backend: user service (`~/.config/systemd/user`) or system service
//! (`/etc/systemd/system`, `User=`).

use std::path::PathBuf;

use crate::error::{PCloudError, Result};

use super::{
    current_username, foreground_args, run, run_capture, write_file, Scope, ServiceBackend,
    ServiceConfig, Trigger, SERVICE_NAME,
};

pub struct SystemdBackend;

fn unit_filename() -> String {
    format!("{}.service", SERVICE_NAME)
}

/// Path of the unit file for the given scope.
fn unit_path(cfg: &ServiceConfig) -> Result<PathBuf> {
    match cfg.scope {
        Scope::System => Ok(PathBuf::from("/etc/systemd/system").join(unit_filename())),
        Scope::User => {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| PCloudError::Service("HOME is not set".to_string()))?;
            Ok(PathBuf::from(home)
                .join(".config/systemd/user")
                .join(unit_filename()))
        }
    }
}

/// `systemctl` arguments prefixed with `--user` for user scope.
fn sctl<'a>(cfg: &ServiceConfig, rest: &[&'a str]) -> Vec<&'a str> {
    let mut v = Vec::with_capacity(rest.len() + 1);
    if cfg.scope == Scope::User {
        v.push("--user");
    }
    v.extend_from_slice(rest);
    v
}

impl SystemdBackend {
    /// Render the unit file contents. Pure — used by install and by tests.
    pub fn render(&self, cfg: &ServiceConfig) -> String {
        let exec_start = format!("{} {}", cfg.exe.display(), foreground_args(cfg).join(" "));
        let (user_line, wanted_by) = match cfg.scope {
            Scope::System => (
                format!("User={}\n", current_username()),
                "multi-user.target",
            ),
            Scope::User => (String::new(), "default.target"),
        };
        format!(
            "[Unit]\n\
             Description=pCloud sync (FUSE mount + IPC daemon)\n\
             After=network-online.target\n\
             Wants=network-online.target\n\
             \n\
             [Service]\n\
             Type=simple\n\
             {user_line}\
             ExecStart={exec_start}\n\
             ExecStop={exe} stop\n\
             Restart=on-failure\n\
             RestartSec=5\n\
             \n\
             [Install]\n\
             WantedBy={wanted_by}\n",
            exe = cfg.exe.display(),
        )
    }
}

impl ServiceBackend for SystemdBackend {
    fn install(&self, cfg: &ServiceConfig) -> Result<()> {
        let path = unit_path(cfg)?;
        write_file(&path, &self.render(cfg))?;

        run("systemctl", &sctl(cfg, &["daemon-reload"]))?;

        // `--user --boot` needs lingering so the user manager runs at boot.
        if cfg.scope == Scope::User && cfg.trigger == Trigger::Boot {
            run("loginctl", &["enable-linger", &current_username()])?;
        }

        let enable = if cfg.start_now {
            sctl(cfg, &["enable", "--now", SERVICE_NAME])
        } else {
            sctl(cfg, &["enable", SERVICE_NAME])
        };
        run("systemctl", &enable)?;
        Ok(())
    }

    fn uninstall(&self, cfg: &ServiceConfig) -> Result<()> {
        // Best-effort disable; ignore failure if it was never enabled.
        let _ = run("systemctl", &sctl(cfg, &["disable", "--now", SERVICE_NAME]));
        let path = unit_path(cfg)?;
        let _ = std::fs::remove_file(&path);
        run("systemctl", &sctl(cfg, &["daemon-reload"]))?;
        Ok(())
    }

    fn restart(&self, cfg: &ServiceConfig) -> Result<()> {
        run("systemctl", &sctl(cfg, &["restart", SERVICE_NAME]))
    }

    fn status(&self, cfg: &ServiceConfig) -> Result<String> {
        let active = run_capture("systemctl", &sctl(cfg, &["is-active", SERVICE_NAME]));
        let enabled = run_capture("systemctl", &sctl(cfg, &["is-enabled", SERVICE_NAME]));
        Ok(format!("active={}, enabled={}", active, enabled))
    }

    fn describe(&self) -> &'static str {
        "systemd"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(scope: Scope) -> ServiceConfig {
        ServiceConfig {
            scope,
            trigger: Trigger::Login,
            mountpoint: PathBuf::from("/home/u/pCloud"),
            exe: PathBuf::from("/usr/bin/pcloud-cli"),
            start_now: true,
        }
    }

    #[test]
    fn user_unit_has_no_user_directive_and_default_target() {
        let u = SystemdBackend.render(&cfg(Scope::User));
        assert!(u.contains("ExecStart=/usr/bin/pcloud-cli start --foreground /home/u/pCloud"));
        assert!(u.contains("ExecStop=/usr/bin/pcloud-cli stop"));
        assert!(u.contains("WantedBy=default.target"));
        assert!(!u.contains("User="));
        assert!(u.contains("Type=simple"));
        assert!(u.contains("Restart=on-failure"));
    }

    #[test]
    fn system_unit_has_user_directive_and_multiuser_target() {
        let u = SystemdBackend.render(&cfg(Scope::System));
        assert!(u.contains("User="));
        assert!(u.contains("WantedBy=multi-user.target"));
    }
}
