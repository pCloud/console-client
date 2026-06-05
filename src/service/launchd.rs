//! macOS launchd backend: a per-user LaunchAgent (recommended) or a system
//! LaunchDaemon.
//!
//! macFUSE mounts require a logged-in user session, so the LaunchAgent
//! (`--user`, the default) is the reliable choice; `--system` LaunchDaemons are
//! supported but warned about.

use std::path::PathBuf;

use crate::error::{PCloudError, Result};

use super::{
    current_username, run, run_capture, write_file, Scope, ServiceBackend, ServiceConfig,
    LAUNCHD_LABEL,
};

pub struct LaunchdBackend;

fn plist_path(cfg: &ServiceConfig) -> Result<PathBuf> {
    let file = format!("{}.plist", LAUNCHD_LABEL);
    match cfg.scope {
        Scope::System => Ok(PathBuf::from("/Library/LaunchDaemons").join(file)),
        Scope::User => {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| PCloudError::Service("HOME is not set".to_string()))?;
            Ok(PathBuf::from(home).join("Library/LaunchAgents").join(file))
        }
    }
}

fn uid() -> u32 {
    unsafe { libc::getuid() }
}

/// launchd domain target: `gui/<uid>` for agents, `system` for daemons.
fn domain(cfg: &ServiceConfig) -> String {
    match cfg.scope {
        Scope::System => "system".to_string(),
        Scope::User => format!("gui/{}", uid()),
    }
}

/// launchd service target: `<domain>/<label>`.
fn service_target(cfg: &ServiceConfig) -> String {
    format!("{}/{}", domain(cfg), LAUNCHD_LABEL)
}

impl LaunchdBackend {
    /// Render the plist. Pure — used by install and by tests.
    pub fn render(&self, cfg: &ServiceConfig) -> String {
        let mut args = vec![cfg.exe.display().to_string()];
        args.extend(super::foreground_args(cfg));
        let program_args: String = args
            .iter()
            .map(|a| format!("    <string>{}</string>\n", a))
            .collect();
        // System LaunchDaemons run as root unless told otherwise; pin to the
        // installing user so the per-user DB/mount are correct.
        let username_key = match cfg.scope {
            Scope::System => format!(
                "  <key>UserName</key>\n  <string>{}</string>\n",
                current_username()
            ),
            Scope::User => String::new(),
        };
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
             \x20 <key>Label</key>\n\
             \x20 <string>{label}</string>\n\
             \x20 <key>ProgramArguments</key>\n\
             \x20 <array>\n\
             {program_args}\
             \x20 </array>\n\
             {username_key}\
             \x20 <key>RunAtLoad</key>\n\
             \x20 <true/>\n\
             \x20 <key>KeepAlive</key>\n\
             \x20 <dict>\n\
             \x20   <key>SuccessfulExit</key>\n\
             \x20   <false/>\n\
             \x20 </dict>\n\
             \x20 <key>StandardErrorPath</key>\n\
             \x20 <string>/tmp/{name}.err.log</string>\n\
             \x20 <key>StandardOutPath</key>\n\
             \x20 <string>/tmp/{name}.out.log</string>\n\
             </dict>\n\
             </plist>\n",
            label = LAUNCHD_LABEL,
            name = super::SERVICE_NAME,
        )
    }
}

impl ServiceBackend for LaunchdBackend {
    fn install(&self, cfg: &ServiceConfig) -> Result<()> {
        if cfg.scope == Scope::System {
            eprintln!(
                "Warning: macFUSE mounts need a logged-in user session; a system \
                 LaunchDaemon may fail to mount at boot. A user LaunchAgent (--user) \
                 is recommended."
            );
        }
        let path = plist_path(cfg)?;
        write_file(&path, &self.render(cfg))?;
        if cfg.start_now {
            // `bootstrap` loads the job; RunAtLoad starts it. Replace any prior
            // instance first.
            let _ = run("launchctl", &["bootout", &service_target(cfg)]);
            run(
                "launchctl",
                &["bootstrap", &domain(cfg), &path.display().to_string()],
            )?;
        }
        Ok(())
    }

    fn uninstall(&self, cfg: &ServiceConfig) -> Result<()> {
        let _ = run("launchctl", &["bootout", &service_target(cfg)]);
        let path = plist_path(cfg)?;
        let _ = std::fs::remove_file(&path);
        Ok(())
    }

    fn restart(&self, cfg: &ServiceConfig) -> Result<()> {
        run("launchctl", &["kickstart", "-k", &service_target(cfg)])
    }

    fn status(&self, cfg: &ServiceConfig) -> Result<String> {
        Ok(run_capture("launchctl", &["print", &service_target(cfg)]))
    }

    fn describe(&self) -> &'static str {
        "launchd"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::Trigger;

    fn cfg(scope: Scope) -> ServiceConfig {
        ServiceConfig {
            scope,
            trigger: Trigger::Login,
            mountpoint: PathBuf::from("/Users/u/pCloud"),
            exe: PathBuf::from("/usr/local/bin/pcloud-cli"),
            start_now: true,
        }
    }

    #[test]
    fn agent_plist_has_program_args_and_runatload() {
        let p = LaunchdBackend.render(&cfg(Scope::User));
        assert!(p.contains("<string>com.pcloud.cli</string>"));
        assert!(p.contains("<string>/usr/local/bin/pcloud-cli</string>"));
        assert!(p.contains("<string>--foreground</string>"));
        assert!(p.contains("<string>/Users/u/pCloud</string>"));
        assert!(p.contains("<key>RunAtLoad</key>"));
        assert!(p.contains("<key>KeepAlive</key>"));
        assert!(!p.contains("<key>UserName</key>"));
    }

    #[test]
    fn daemon_plist_pins_username() {
        let p = LaunchdBackend.render(&cfg(Scope::System));
        assert!(p.contains("<key>UserName</key>"));
    }
}
