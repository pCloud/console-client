//! Dependency and environment diagnostics for the pCloud console client.
//!
//! This module provides runtime checks for required shared libraries,
//! FUSE availability, and system configuration. It powers the `--doctor`
//! flag and enriches init-failure error messages with distro-specific
//! install hints.

use std::path::Path;

use crate::utils::terminal::{eprint_status, StatusIndicator};

// ---------------------------------------------------------------------------
// Distro detection
// ---------------------------------------------------------------------------

/// Recognized Linux distributions (used for tailored install hints).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Distro {
    Ubuntu,
    Debian,
    Fedora,
    Rhel,
    Arch,
    MacOs,
    Unknown(String),
}

impl std::fmt::Display for Distro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Distro::Ubuntu => write!(f, "Ubuntu"),
            Distro::Debian => write!(f, "Debian"),
            Distro::Fedora => write!(f, "Fedora"),
            Distro::Rhel => write!(f, "RHEL/AlmaLinux"),
            Distro::Arch => write!(f, "Arch Linux"),
            Distro::MacOs => write!(f, "macOS"),
            Distro::Unknown(id) => write!(f, "{}", id),
        }
    }
}

/// Parse `/etc/os-release` content into `(ID, ID_LIKE, VERSION_ID)`.
fn parse_os_release(content: &str) -> (String, String, String) {
    let mut id = String::new();
    let mut id_like = String::new();
    let mut version_id = String::new();

    for line in content.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("ID=") {
            id = val.trim_matches('"').to_lowercase();
        } else if let Some(val) = line.strip_prefix("ID_LIKE=") {
            id_like = val.trim_matches('"').to_lowercase();
        } else if let Some(val) = line.strip_prefix("VERSION_ID=") {
            version_id = val.trim_matches('"').to_string();
        }
    }

    (id, id_like, version_id)
}

/// Detect the current Linux distribution by reading `/etc/os-release`.
pub fn detect_distro() -> Distro {
    detect_distro_from(Path::new("/etc/os-release"))
}

/// Testable helper: detect distro from an arbitrary path.
fn detect_distro_from(path: &Path) -> Distro {
    if cfg!(target_os = "macos") {
        return Distro::MacOs;
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Distro::Unknown("unknown".into()),
    };

    distro_from_os_release(&content)
}

/// Determine distro from the text content of an os-release file.
fn distro_from_os_release(content: &str) -> Distro {
    let (id, id_like, _version_id) = parse_os_release(content);

    match id.as_str() {
        "ubuntu" => Distro::Ubuntu,
        "debian" => Distro::Debian,
        "fedora" => Distro::Fedora,
        "rhel" | "almalinux" | "rocky" | "centos" => Distro::Rhel,
        "arch" | "manjaro" | "endeavouros" => Distro::Arch,
        _ => {
            // Fall back to ID_LIKE
            if id_like.contains("ubuntu") || id_like.contains("debian") {
                Distro::Debian
            } else if id_like.contains("fedora") || id_like.contains("rhel") {
                Distro::Rhel
            } else if id_like.contains("arch") {
                Distro::Arch
            } else if id.is_empty() {
                Distro::Unknown("unknown".into())
            } else {
                Distro::Unknown(id)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Library dependency table
// ---------------------------------------------------------------------------

/// A required shared library with per-distro package names.
struct LibDep {
    /// Human-readable name
    name: &'static str,
    /// Linux soname to probe via dlopen
    #[cfg(target_os = "linux")]
    soname: &'static str,
    /// Debian/Ubuntu package
    pkg_deb: &'static str,
    /// Fedora/RHEL package
    pkg_rpm: &'static str,
    /// Arch package
    pkg_arch: &'static str,
    /// macOS formula (empty = system-provided)
    #[cfg(target_os = "macos")]
    pkg_brew: &'static str,
}

#[cfg(target_os = "linux")]
static REQUIRED_LIBS: &[LibDep] = &[
    LibDep {
        name: "FUSE",
        soname: "libfuse.so.2",
        pkg_deb: "libfuse2t64",
        pkg_rpm: "fuse-libs",
        pkg_arch: "fuse2",
    },
    LibDep {
        name: "SQLite",
        soname: "libsqlite3.so.0",
        pkg_deb: "libsqlite3-0",
        pkg_rpm: "sqlite-libs",
        pkg_arch: "sqlite",
    },
    LibDep {
        name: "OpenSSL (ssl)",
        soname: "libssl.so.3",
        pkg_deb: "libssl3",
        pkg_rpm: "openssl-libs",
        pkg_arch: "openssl",
    },
    LibDep {
        name: "OpenSSL (crypto)",
        soname: "libcrypto.so.3",
        pkg_deb: "libssl3",
        pkg_rpm: "openssl-libs",
        pkg_arch: "openssl",
    },
    LibDep {
        name: "zlib",
        soname: "libz.so.1",
        pkg_deb: "zlib1g",
        pkg_rpm: "zlib",
        pkg_arch: "zlib",
    },
    LibDep {
        name: "libudev",
        soname: "libudev.so.1",
        pkg_deb: "libudev1",
        pkg_rpm: "systemd-libs",
        pkg_arch: "systemd-libs",
    },
];

#[cfg(target_os = "macos")]
static REQUIRED_LIBS: &[LibDep] = &[
    LibDep {
        name: "macFUSE",
        pkg_deb: "",
        pkg_rpm: "",
        pkg_arch: "",
        pkg_brew: "macfuse",
    },
    LibDep {
        name: "SQLite",
        pkg_deb: "",
        pkg_rpm: "",
        pkg_arch: "",
        pkg_brew: "",
    },
    LibDep {
        name: "OpenSSL",
        pkg_deb: "",
        pkg_rpm: "",
        pkg_arch: "",
        pkg_brew: "openssl",
    },
];

// ---------------------------------------------------------------------------
// Runtime checks
// ---------------------------------------------------------------------------

/// Result of a single dependency check.
struct CheckResult {
    name: String,
    detail: String,
    ok: bool,
}

/// Try to dlopen a library with `RTLD_LAZY` to see if it can be loaded.
///
/// Returns the resolved path on success (by reading `/proc/self/maps`),
/// or an error string on failure.
#[cfg(target_os = "linux")]
fn probe_library(soname: &str) -> Result<String, String> {
    use std::ffi::{CStr, CString};

    let c_soname = CString::new(soname).map_err(|_| "invalid soname".to_string())?;

    // Safety: dlopen with RTLD_LAZY is safe; we close the handle immediately.
    let handle = unsafe { libc::dlopen(c_soname.as_ptr(), libc::RTLD_LAZY) };

    if handle.is_null() {
        let err = unsafe { libc::dlerror() };
        let msg = if err.is_null() {
            "not found".to_string()
        } else {
            let c_str = unsafe { CStr::from_ptr(err) };
            c_str.to_string_lossy().into_owned()
        };
        return Err(msg);
    }

    // Resolve path from /proc/self/maps
    let path = resolve_library_path(soname).unwrap_or_else(|| "loaded".to_string());

    // Close the handle (decrement refcount)
    unsafe {
        libc::dlclose(handle);
    }

    Ok(path)
}

/// Search `/proc/self/maps` for the resolved path of a loaded library.
#[cfg(target_os = "linux")]
fn resolve_library_path(soname: &str) -> Option<String> {
    let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
    for line in maps.lines() {
        if line.contains(soname) {
            // Lines look like: addr-addr perms offset dev inode   /path/to/lib.so.X
            if let Some(path_start) = line.find('/') {
                return Some(line[path_start..].to_string());
            }
        }
    }
    None
}

/// Check that `/dev/fuse` exists and is accessible.
#[cfg(target_os = "linux")]
fn check_dev_fuse() -> CheckResult {
    let path = Path::new("/dev/fuse");
    if !path.exists() {
        return CheckResult {
            name: "/dev/fuse".into(),
            detail: "not found (is the fuse kernel module loaded?)".into(),
            ok: false,
        };
    }

    // Try opening for read to check permissions
    match std::fs::File::open(path) {
        Ok(_) => CheckResult {
            name: "/dev/fuse".into(),
            detail: "accessible".into(),
            ok: true,
        },
        Err(e) => CheckResult {
            name: "/dev/fuse".into(),
            detail: format!("permission denied ({})", e),
            ok: false,
        },
    }
}

/// Check that `fusermount` (or `fusermount3`) is in PATH.
#[cfg(target_os = "linux")]
fn check_fusermount() -> CheckResult {
    for cmd in &["fusermount3", "fusermount"] {
        if let Ok(output) = std::process::Command::new("which").arg(cmd).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return CheckResult {
                    name: "fusermount".into(),
                    detail: path,
                    ok: true,
                        };
            }
        }
    }

    CheckResult {
        name: "fusermount".into(),
        detail: "not found in PATH".into(),
        ok: false,
    }
}

/// Check if the current user is in the `fuse` group.
#[cfg(target_os = "linux")]
fn check_fuse_group() -> CheckResult {
    // Get current groups
    let ngroups = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
    if ngroups < 0 {
        return CheckResult {
            name: "fuse group".into(),
            detail: "could not query groups".into(),
            ok: false,
        };
    }

    let mut groups = vec![0u32; ngroups as usize];
    let result = unsafe { libc::getgroups(ngroups, groups.as_mut_ptr()) };
    if result < 0 {
        return CheckResult {
            name: "fuse group".into(),
            detail: "could not query groups".into(),
            ok: false,
        };
    }
    groups.truncate(result as usize);

    // Look up the "fuse" group GID
    let fuse_name = std::ffi::CString::new("fuse").unwrap();
    let grp = unsafe { libc::getgrnam(fuse_name.as_ptr()) };

    if grp.is_null() {
        // No "fuse" group on this system — that's fine on many distros
        return CheckResult {
            name: "fuse group".into(),
            detail: "no 'fuse' group on this system (OK on most distros)".into(),
            ok: true,
        };
    }

    let fuse_gid = unsafe { (*grp).gr_gid };

    // Also check if the user is root
    let uid = unsafe { libc::getuid() };
    if uid == 0 {
        return CheckResult {
            name: "fuse group".into(),
            detail: "running as root".into(),
            ok: true,
        };
    }

    if groups.contains(&fuse_gid) {
        CheckResult {
            name: "fuse group".into(),
            detail: "member".into(),
            ok: true,
        }
    } else {
        CheckResult {
            name: "fuse group".into(),
            detail: format!(
                "not a member (add with: sudo usermod -aG fuse {})",
                std::env::var("USER").unwrap_or_else(|_| "USER".into())
            ),
            ok: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Install hint generation
// ---------------------------------------------------------------------------

/// Get a package name for the given library on the given distro.
fn package_for(dep: &LibDep, distro: &Distro) -> &'static str {
    match distro {
        Distro::Ubuntu | Distro::Debian => dep.pkg_deb,
        Distro::Fedora | Distro::Rhel => dep.pkg_rpm,
        Distro::Arch => dep.pkg_arch,
        #[cfg(target_os = "macos")]
        Distro::MacOs => dep.pkg_brew,
        _ => dep.pkg_deb, // best guess fallback
    }
}

/// Build a one-line install command for the given missing library sonames.
pub fn get_install_hint(distro: &Distro, missing_sonames: &[&str]) -> String {
    if missing_sonames.is_empty() {
        return String::new();
    }

    // Collect unique package names
    let mut packages: Vec<&str> = Vec::new();
    for soname in missing_sonames {
        for dep in REQUIRED_LIBS {
            #[cfg(target_os = "linux")]
            let matches = dep.soname == *soname;
            #[cfg(target_os = "macos")]
            let matches = dep.name.to_lowercase().contains(&soname.to_lowercase());

            if matches {
                let pkg = package_for(dep, distro);
                if !pkg.is_empty() && !packages.contains(&pkg) {
                    packages.push(pkg);
                }
            }
        }
    }

    if packages.is_empty() {
        return String::new();
    }

    let pkg_list = packages.join(" ");

    match distro {
        Distro::Ubuntu | Distro::Debian => format!("  sudo apt install {}", pkg_list),
        Distro::Fedora => format!("  sudo dnf install {}", pkg_list),
        Distro::Rhel => format!("  sudo dnf install {}", pkg_list),
        Distro::Arch => format!("  sudo pacman -S {}", pkg_list),
        #[cfg(target_os = "macos")]
        Distro::MacOs => format!("  brew install {}", pkg_list),
        _ => format!("  Install packages: {}", pkg_list),
    }
}

/// Build a hint string for a specific init error code.
pub fn init_error_hint(error_code: u32) -> Option<String> {
    let distro = detect_distro();

    match error_code {
        5 => {
            // SSL initialization failed
            let hint = get_install_hint(&distro, &["libssl.so.3"]);
            let mut msg = String::from(
                "This usually means OpenSSL 3.x is not installed or cannot be loaded.",
            );
            if !hint.is_empty() {
                msg.push_str(&format!("\n{}", hint));
            }
            msg.push_str("\n\nTip: Run 'pcloud --doctor' for a full dependency check.");
            Some(msg)
        }
        3 => {
            // Database open failed
            Some(
                "The SQLite database could not be opened. Check disk space and permissions.\n\n\
                 Tip: Run 'pcloud --doctor' for a full dependency check."
                    .into(),
            )
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Doctor command
// ---------------------------------------------------------------------------

/// Run all dependency and environment checks, printing a report to stderr.
///
/// Returns `Ok(())` if all checks pass, or an error if any critical check fails.
pub fn run_doctor() -> crate::error::Result<()> {
    let distro = detect_distro();

    eprintln!();
    eprintln!("pCloud Dependency Check");
    eprintln!("=======================");
    eprintln!();

    // -- Shared library checks --
    eprintln!("Shared Libraries:");
    #[allow(unused_mut)]
    let mut missing_sonames: Vec<&str> = Vec::new();
    #[allow(unused_mut)]
    let mut any_lib_failed = false;

    #[cfg(target_os = "linux")]
    for dep in REQUIRED_LIBS {
        match probe_library(dep.soname) {
            Ok(path) => {
                let detail = format!(
                    "{:<28} {}",
                    format!("{} ({})", dep.name, dep.soname),
                    path
                );
                eprint_status(StatusIndicator::Success, &detail);
            }
            Err(msg) => {
                let detail = format!(
                    "{:<28} {}",
                    format!("{} ({})", dep.name, dep.soname),
                    msg
                );
                eprint_status(StatusIndicator::Error, &detail);
                missing_sonames.push(dep.soname);
                any_lib_failed = true;
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        eprint_status(
            StatusIndicator::Warning,
            "Library probing not implemented for this platform",
        );
    }

    eprintln!();

    // -- Environment checks (Linux only) --
    #[cfg(target_os = "linux")]
    {
        eprintln!("Environment:");
        let env_checks = vec![check_dev_fuse(), check_fusermount(), check_fuse_group()];

        for check in &env_checks {
            let detail = format!("{:<28} {}", check.name, check.detail);
            if check.ok {
                eprint_status(StatusIndicator::Success, &detail);
            } else {
                eprint_status(StatusIndicator::Error, &detail);
                any_lib_failed = true;
            }
        }
        eprintln!();
    }

    // -- Distro & hints --
    eprintln!("Detected: {}", distro);

    if !missing_sonames.is_empty() {
        eprintln!();
        eprintln!("Missing packages:");
        let hint = get_install_hint(&distro, &missing_sonames);
        if !hint.is_empty() {
            eprintln!("{}", hint);
        }
    }

    eprintln!();

    if any_lib_failed {
        Err(crate::error::PCloudError::Config(
            "Some dependency checks failed — see above for details.".into(),
        ))
    } else {
        eprint_status(StatusIndicator::Success, "All checks passed!");
        eprintln!();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Distro detection tests --

    #[test]
    fn test_parse_ubuntu() {
        let content = r#"
ID=ubuntu
ID_LIKE=debian
VERSION_ID="22.04"
"#;
        assert_eq!(distro_from_os_release(content), Distro::Ubuntu);
    }

    #[test]
    fn test_parse_debian() {
        let content = "ID=debian\nVERSION_ID=\"12\"\n";
        assert_eq!(distro_from_os_release(content), Distro::Debian);
    }

    #[test]
    fn test_parse_fedora() {
        let content = "ID=fedora\nVERSION_ID=\"39\"\n";
        assert_eq!(distro_from_os_release(content), Distro::Fedora);
    }

    #[test]
    fn test_parse_rhel() {
        let content = "ID=rhel\nVERSION_ID=\"9.3\"\n";
        assert_eq!(distro_from_os_release(content), Distro::Rhel);
    }

    #[test]
    fn test_parse_almalinux() {
        let content = "ID=almalinux\nID_LIKE=\"rhel centos fedora\"\nVERSION_ID=\"9.3\"\n";
        assert_eq!(distro_from_os_release(content), Distro::Rhel);
    }

    #[test]
    fn test_parse_rocky() {
        let content = "ID=rocky\nID_LIKE=\"rhel centos fedora\"\nVERSION_ID=\"9.3\"\n";
        assert_eq!(distro_from_os_release(content), Distro::Rhel);
    }

    #[test]
    fn test_parse_arch() {
        let content = "ID=arch\n";
        assert_eq!(distro_from_os_release(content), Distro::Arch);
    }

    #[test]
    fn test_parse_manjaro_falls_back_to_arch() {
        let content = "ID=manjaro\n";
        assert_eq!(distro_from_os_release(content), Distro::Arch);
    }

    #[test]
    fn test_parse_mint_falls_back_to_debian() {
        let content = "ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n";
        assert_eq!(distro_from_os_release(content), Distro::Debian);
    }

    #[test]
    fn test_parse_unknown() {
        let content = "ID=nixos\n";
        assert_eq!(distro_from_os_release(content), Distro::Unknown("nixos".into()));
    }

    #[test]
    fn test_parse_empty() {
        let content = "";
        assert_eq!(
            distro_from_os_release(content),
            Distro::Unknown("unknown".into())
        );
    }

    #[test]
    fn test_parse_quoted_values() {
        let content = "ID=\"ubuntu\"\nVERSION_ID=\"22.04\"\n";
        assert_eq!(distro_from_os_release(content), Distro::Ubuntu);
    }

    // -- Install hint tests --

    #[cfg(target_os = "linux")]
    #[test]
    fn test_install_hint_ubuntu() {
        let hint = get_install_hint(&Distro::Ubuntu, &["libssl.so.3"]);
        assert!(hint.contains("sudo apt install"));
        assert!(hint.contains("libssl3"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_install_hint_fedora() {
        let hint = get_install_hint(&Distro::Fedora, &["libfuse.so.2"]);
        assert!(hint.contains("sudo dnf install"));
        assert!(hint.contains("fuse-libs"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_install_hint_arch() {
        let hint = get_install_hint(&Distro::Arch, &["libfuse.so.2"]);
        assert!(hint.contains("sudo pacman -S"));
        assert!(hint.contains("fuse2"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_install_hint_multiple() {
        let hint = get_install_hint(&Distro::Ubuntu, &["libssl.so.3", "libfuse.so.2"]);
        assert!(hint.contains("libssl3"));
        assert!(hint.contains("libfuse2t64"));
    }

    #[test]
    fn test_install_hint_empty() {
        let hint = get_install_hint(&Distro::Ubuntu, &[]);
        assert!(hint.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_install_hint_deduplicates_packages() {
        // libssl.so.3 and libcrypto.so.3 both map to "libssl3" on Debian
        let hint = get_install_hint(&Distro::Ubuntu, &["libssl.so.3", "libcrypto.so.3"]);
        // Should contain libssl3 only once
        let count = hint.matches("libssl3").count();
        assert_eq!(count, 1, "libssl3 should appear exactly once, got: {}", hint);
    }

    // -- init_error_hint tests --

    #[test]
    fn test_init_error_hint_ssl() {
        let hint = init_error_hint(5);
        assert!(hint.is_some());
        let msg = hint.unwrap();
        assert!(msg.contains("OpenSSL"));
        assert!(msg.contains("--doctor"));
    }

    #[test]
    fn test_init_error_hint_database() {
        let hint = init_error_hint(3);
        assert!(hint.is_some());
        let msg = hint.unwrap();
        assert!(msg.contains("SQLite"));
        assert!(msg.contains("--doctor"));
    }

    #[test]
    fn test_init_error_hint_unknown_code() {
        assert!(init_error_hint(99).is_none());
    }

    // -- Distro display --

    #[test]
    fn test_distro_display() {
        assert_eq!(format!("{}", Distro::Ubuntu), "Ubuntu");
        assert_eq!(format!("{}", Distro::Fedora), "Fedora");
        assert_eq!(format!("{}", Distro::Unknown("nixos".into())), "nixos");
    }
}
