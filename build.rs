//! Build script for compiling the pclsync C library and linking dependencies.
//!
//! This script:
//! - Detects the target platform (Linux/macOS)
//! - Compiles all pclsync .c source files
//! - Links required system libraries (fuse, sqlite3, pthread, openssl, zlib)
//! - Sets up include paths for C headers
//! - Generates Rust bindings for C structs using bindgen
//!
//! # Required System Dependencies
//!
//! ## Linux (Debian/Ubuntu)
//! ```bash
//! sudo apt-get install libfuse-dev libsqlite3-dev libssl-dev zlib1g-dev libclang-dev
//! ```
//!
//! ## Linux (Fedora/RHEL)
//! ```bash
//! sudo dnf install fuse-devel sqlite-devel openssl-devel zlib-devel clang-devel
//! ```
//!
//! ## macOS
//! ```bash
//! brew install macfuse openssl llvm
//! ```

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let pclsync_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("pclsync");

    // Verify pclsync directory exists
    if !pclsync_dir.exists() {
        panic!(
            "pclsync directory not found at {:?}. \
             Please initialize git submodules: git submodule update --init",
            pclsync_dir
        );
    }

    // Configure the C compiler
    let mut build = cc::Build::new();

    // Common compiler flags
    build
        .warnings(false)
        .std("gnu99")
        .flag_if_supported("-Wpointer-arith")
        .opt_level(2)
        .flag_if_supported("-fno-stack-protector")
        .flag_if_supported("-fomit-frame-pointer")
        // GCC 14+ compiler demotion of certain warnings.
        .flag_if_supported("-Wno-error=int-conversion")
        .flag_if_supported("-Wno-error=incompatible-pointer-types")
        .define("PSYNC_DEFAULT_POSIX_DIR", "\".pcloud-cli\"");

    // Set DEBUG_LEVEL for debug builds (D_NOTICE = 50)
    let profile = env::var("PROFILE").unwrap_or_default();
    if profile == "debug" {
        build.define("DEBUG_LEVEL", "50"); // D_NOTICE
    } else if profile == "release" {
        build.define("DEBUG_LEVEL", "30"); // D_ERROR
    }

    // Include path for pclsync headers
    build.include(&pclsync_dir);

    // Platform-specific configuration
    match target_os.as_str() {
        "linux" => configure_linux(&mut build, &pclsync_dir),
        "macos" => configure_macos(&mut build, &pclsync_dir),
        _ => {
            eprintln!(
                "Warning: Unsupported target OS '{}', attempting Linux-like build",
                target_os
            );
            configure_linux(&mut build, &pclsync_dir);
        }
    }

    // Add common source files (from Makefile OBJ)
    let common_sources = [
        "pcompat.c",
        "psynclib.c",
        "plocks.c",
        "plibs.c",
        "pcallbacks.c",
        "pdiff.c",
        "pstatus.c",
        "papi.c",
        "ptimer.c",
        "pupload.c",
        "pdownload.c",
        "pfolder.c",
        "psyncer.c",
        "ptasks.c",
        "psettings.c",
        "pnetlibs.c",
        "pcache.c",
        "pscanner.c",
        "plist.c",
        "plocalscan.c",
        "plocalnotify.c",
        "pp2p.c",
        "pcrypto.c",
        "pssl.c",
        "pssl-openssl3.c",
        "pfileops.c",
        "ptree.c",
        "ppassword.c",
        "prunratelimit.c",
        "pmemlock.c",
        "pnotifications.c",
        "pexternalstatus.c",
        "publiclinks.c",
        "pbusinessaccount.c",
        "pcontacts.c",
        "poverlay.c",
        "pcompression.c",
        "pasyncnet.c",
        "ppathstatus.c",
        "pdevice_monitor.c",
        "ptools.c",
        "miniz.c",
    ];

    // Add filesystem source files (from Makefile OBJFS)
    let fs_sources = [
        "pfs.c",
        "ppagecache.c",
        "pfsfolder.c",
        "pfstasks.c",
        "pfsupload.c",
        "pintervaltree.c",
        "pfsxattr.c",
        "pcloudcrypto.c",
        "pfscrypto.c",
        "pcrc32c.c",
        "pfsstatic.c",
    ];

    // Add all source files
    for source in common_sources.iter().chain(fs_sources.iter()) {
        let source_path = pclsync_dir.join(source);
        if source_path.exists() {
            build.file(&source_path);
        } else {
            eprintln!("Warning: Source file not found: {:?}", source_path);
        }
    }

    // Compile the library
    build.compile("pclsync");

    // Link system libraries
    link_system_libraries(&target_os);

    // Generate bindings using bindgen
    generate_bindings(&pclsync_dir, &out_dir, &target_os);

    // Tell Cargo to rerun this script if pclsync sources change
    println!("cargo:rerun-if-changed=pclsync/");
    println!("cargo:rerun-if-changed=build.rs");

    // Emit PCLOUD_VERSION with profile suffix
    let base_version = env::var("CARGO_PKG_VERSION").unwrap();
    let build_profile = env::var("PCLOUD_BUILD_PROFILE").unwrap_or_default();
    let profile = env::var("PROFILE").unwrap_or_default();

    let version = match build_profile.as_str() {
        "qa" => format!("{}-qa", base_version),
        _ if profile == "debug" => format!("{}-dev", base_version),
        _ => base_version,
    };

    println!("cargo:rustc-env=PCLOUD_VERSION={}", version);
    println!("cargo:rerun-if-env-changed=PCLOUD_BUILD_PROFILE");
    println!("cargo:rerun-if-env-changed=BUGSNAG_API_KEY");

    // Emit console-client git commit hash
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
    {
        if output.status.success() {
            let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let short = &hash[..7.min(hash.len())];
            println!("cargo:rustc-env=PCLOUD_GIT_COMMIT={}", hash);
            println!("cargo:rustc-env=PCLOUD_GIT_COMMIT_SHORT={}", short);
        }
    }

    // Emit pclsync submodule git commit hash
    if let Ok(output) = std::process::Command::new("git")
        .args(["-C", "pclsync", "rev-parse", "HEAD"])
        .output()
    {
        if output.status.success() {
            let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let short = &hash[..7.min(hash.len())];
            println!("cargo:rustc-env=PCLSYNC_GIT_COMMIT={}", hash);
            println!("cargo:rustc-env=PCLSYNC_GIT_COMMIT_SHORT={}", short);
        }
    }

    // Parse PSYNC_LIB_VERSION from pclsync/psettings.h
    let psettings_path = pclsync_dir.join("psettings.h");
    if let Ok(contents) = std::fs::read_to_string(&psettings_path) {
        for line in contents.lines() {
            if line.contains("PSYNC_LIB_VERSION") {
                if let Some(start) = line.find('"') {
                    if let Some(end) = line[start + 1..].find('"') {
                        let ver = &line[start + 1..start + 1 + end];
                        println!("cargo:rustc-env=PSYNC_LIB_VERSION={}", ver);
                    }
                }
            }
        }
    }

    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");
    println!("cargo:rerun-if-changed=pclsync/psettings.h");
}

/// Generate Rust bindings for pclsync C structs using bindgen.
///
/// This generates bindings for:
/// - pstatus_t: Sync status struct
/// - Callback function pointer types
/// - Event types and data structures
/// - Folder/file list types
fn generate_bindings(pclsync_dir: &Path, out_dir: &Path, target_os: &str) {
    let header_path = pclsync_dir.join("psynclib.h");

    if !header_path.exists() {
        eprintln!(
            "Warning: psynclib.h not found at {:?}, skipping bindgen",
            header_path
        );
        return;
    }

    let mut builder = bindgen::Builder::default()
        .header(header_path.to_string_lossy())
        // Include the pclsync directory for headers
        .clang_arg(format!("-I{}", pclsync_dir.display()))
        // Generate bindings for the status struct
        .allowlist_type("pstatus_t")
        // Generate bindings for folder/file types
        .allowlist_type("pfolder_t")
        .allowlist_type("pfile_t")
        .allowlist_type("pentry_t")
        .allowlist_type("pfolder_list_t")
        .allowlist_type("psync_folder_t")
        .allowlist_type("psync_folder_list_t")
        // Generate bindings for event types
        .allowlist_type("psync_file_event_t")
        .allowlist_type("psync_folder_event_t")
        .allowlist_type("psync_share_event_t")
        .allowlist_type("psync_eventdata_t")
        // Generate bindings for notification types
        .allowlist_type("psync_notification_t")
        .allowlist_type("psync_notification_list_t")
        .allowlist_type("psync_notification_action_t")
        // Generate bindings for share types
        .allowlist_type("psync_sharerequest_t")
        .allowlist_type("psync_sharerequest_list_t")
        .allowlist_type("psync_share_t")
        .allowlist_type("psync_share_list_t")
        // Generate bindings for new version type
        .allowlist_type("psync_new_version_t")
        // Generate bindings for suggested folders
        .allowlist_type("psuggested_folder_t")
        .allowlist_type("psuggested_folders_t")
        // Generate typedef aliases for common types
        .allowlist_type("psync_folderid_t")
        .allowlist_type("psync_fileid_t")
        .allowlist_type("psync_fileorfolderid_t")
        .allowlist_type("psync_userid_t")
        .allowlist_type("psync_shareid_t")
        .allowlist_type("psync_sharerequestid_t")
        .allowlist_type("psync_syncid_t")
        .allowlist_type("psync_eventtype_t")
        .allowlist_type("psync_synctype_t")
        .allowlist_type("psync_listtype_t")
        // Generate callback type definitions
        .allowlist_type("pstatus_change_callback_t")
        .allowlist_type("pevent_callback_t")
        .allowlist_type("pnotification_callback_t")
        .allowlist_type("psync_generic_callback_t")
        .allowlist_type("psync_malloc_t")
        .allowlist_type("psync_realloc_t")
        .allowlist_type("psync_free_t")
        // Use core types
        .use_core()
        // Generate Debug trait implementations
        .derive_debug(true)
        // Generate Default trait where possible
        .derive_default(true)
        // Generate Copy/Clone for simple types
        .derive_copy(true)
        // Layout tests help verify struct layout matches C
        .layout_tests(true)
        // Use explicit padding
        .explicit_padding(true)
        // Don't generate bindings for functions (we declare them manually)
        .ignore_functions()
        // Blocklist time_t to avoid conflicts
        .blocklist_type("time_t")
        // Map time_t to libc::time_t
        .raw_line("pub type time_t = libc::time_t;");

    // Add platform-specific defines
    match target_os {
        "linux" => {
            builder = builder
                .clang_arg("-DP_OS_LINUX")
                .clang_arg("-DP_OS_POSIX")
                .clang_arg("-D_FILE_OFFSET_BITS=64");
        }
        "macos" => {
            builder = builder
                .clang_arg("-DP_OS_MACOSX")
                .clang_arg("-DP_OS_BSD")
                .clang_arg("-DP_OS_POSIX")
                .clang_arg("-D_DARWIN_USE_64_BIT_INODE")
                .clang_arg("-D_FILE_OFFSET_BITS=64");
        }
        _ => {
            builder = builder.clang_arg("-DP_OS_POSIX");
        }
    }

    // Generate the bindings
    let bindings = builder.generate().expect("Failed to generate bindings");

    // Write bindings to $OUT_DIR/bindings.rs
    let bindings_path = out_dir.join("bindings.rs");
    bindings
        .write_to_file(&bindings_path)
        .expect("Failed to write bindings");

    println!("cargo:rerun-if-changed={}", header_path.display());
    eprintln!("Generated bindings at {:?}", bindings_path);
}

fn configure_linux(build: &mut cc::Build, _pclsync_dir: &PathBuf) {
    // Define Linux platform
    build.define("P_OS_LINUX", None);
    // Note: _GNU_SOURCE is defined in pcompat.h, so we don't need to define it again
    build.define("_FILE_OFFSET_BITS", "64");
    build.define("_GNU_SOURCE", None);

    // Use OpenSSL 3.x on Linux
    build.define("P_SSL_OPENSSL3", None);

    // Note: poverlay_lin.c is included via #include in poverlay.c,
    // so we don't compile it separately

    // Try to find SQLite3 include path using pkg-config
    match pkg_config::Config::new().probe("sqlite3") {
        Ok(sqlite) => {
            for include in &sqlite.include_paths {
                build.include(include);
            }
        }
        Err(e) => {
            eprintln!("Warning: pkg-config failed to find sqlite3: {}", e);
            eprintln!("Hint: Install libsqlite3-dev (Debian/Ubuntu) or sqlite-devel (Fedora/RHEL)");
        }
    }

    // Try to find FUSE include path using pkg-config
    if let Ok(fuse) = pkg_config::Config::new().probe("fuse") {
        for include in &fuse.include_paths {
            build.include(include);
        }
    } else {
        eprintln!("Warning: pkg-config failed to find fuse");
        eprintln!("Hint: Install libfuse-dev (Debian/Ubuntu) or fuse-devel (Fedora/RHEL)");
    }

    // Try to find OpenSSL include path using pkg-config
    match pkg_config::Config::new().probe("openssl") {
        Ok(openssl) => {
            for include in &openssl.include_paths {
                build.include(include);
            }
        }
        Err(e) => {
            eprintln!("Warning: pkg-config failed to find openssl: {}", e);
            eprintln!("Hint: Install libssl-dev (Debian/Ubuntu) or openssl-devel (Fedora/RHEL)");
        }
    }
}

fn configure_macos(build: &mut cc::Build, _pclsync_dir: &PathBuf) {
    // Define macOS platform
    build.define("P_OS_MACOSX", None);
    build.define("P_OS_BSD", None);
    build.define("P_OS_POSIX", None);
    build.define("_DARWIN_USE_64_BIT_INODE", None);
    build.define("_FILE_OFFSET_BITS", "64");

    // Use OpenSSL 3.x on macOS
    build.define("P_SSL_OPENSSL3", None);

    // Note: poverlay_mac.c is included via #include in poverlay.c,
    // so we don't compile it separately

    // Try to find OpenSSL include path using pkg-config
    if let Ok(openssl) = pkg_config::Config::new().probe("openssl") {
        for include in &openssl.include_paths {
            build.include(include);
        }
    } else {
        // Fall back to common Homebrew paths for OpenSSL
        let openssl_include_paths = [
            "/usr/local/opt/openssl/include",
            "/opt/homebrew/opt/openssl/include",
            "/usr/local/opt/openssl@3/include",
            "/opt/homebrew/opt/openssl@3/include",
        ];

        for path in &openssl_include_paths {
            let p = PathBuf::from(path);
            if p.exists() {
                build.include(path);
                break;
            }
        }
    }

    // Try pkg-config for FUSE, fall back to common paths
    if let Ok(fuse) = pkg_config::Config::new().probe("fuse") {
        for include in &fuse.include_paths {
            build.include(include);
        }
    } else {
        // Fall back to common macOS FUSE paths
        let fuse_include_paths = [
            "/usr/local/include/fuse",
            "/opt/homebrew/include/fuse",
            "/Library/Frameworks/macFUSE.framework/Headers",
        ];

        for path in &fuse_include_paths {
            let p = PathBuf::from(path);
            if p.exists() {
                build.include(path);
                break;
            }
        }
    }
}

fn link_system_libraries(target_os: &str) {
    match target_os {
        "linux" => {
            // Use pkg-config to find libraries when available
            link_with_pkgconfig_or_fallback("fuse", "fuse");
            link_with_pkgconfig_or_fallback("sqlite3", "sqlite3");
            link_with_pkgconfig_or_fallback("openssl", "ssl");
            // OpenSSL needs both libssl and libcrypto
            if pkg_config::Config::new().probe("openssl").is_err() {
                println!("cargo:rustc-link-lib=crypto");
            }

            // zlib for pcompression.c (deflate/inflate)
            link_with_pkgconfig_or_fallback("zlib", "z");

            // libudev for device monitoring
            link_with_pkgconfig_or_fallback("libudev", "udev");

            // pthread is always available on Linux
            println!("cargo:rustc-link-lib=pthread");
            // math library needed for some crypto operations
            println!("cargo:rustc-link-lib=m");
        }
        "macos" => {
            // Link libraries on macOS
            // Try pkg-config first, fall back to direct linking

            // FUSE (macFUSE) — link directly; pkg-config is unreliable
            // when cross-compiling (e.g. arm64 pkg-config on x86_64 target).
            // macFUSE installs a universal dylib at /usr/local/lib/libfuse.dylib.
            println!("cargo:rustc-link-lib=fuse");
            println!("cargo:rustc-link-search=/usr/local/lib");
            println!("cargo:rustc-link-search=/opt/homebrew/lib");

            // SQLite3 — use system SQLite (always available on macOS)
            println!("cargo:rustc-link-lib=sqlite3");

            // OpenSSL 3.x
            if pkg_config::Config::new().probe("openssl").is_err() {
                println!("cargo:rustc-link-lib=ssl");
                println!("cargo:rustc-link-lib=crypto");
                // Homebrew OpenSSL paths
                println!("cargo:rustc-link-search=/usr/local/opt/openssl/lib");
                println!("cargo:rustc-link-search=/opt/homebrew/opt/openssl/lib");
                println!("cargo:rustc-link-search=/usr/local/opt/openssl@3/lib");
                println!("cargo:rustc-link-search=/opt/homebrew/opt/openssl@3/lib");
            }

            // zlib for pcompression.c (deflate/inflate)
            println!("cargo:rustc-link-lib=z");

            // Cocoa framework for macOS
            println!("cargo:rustc-link-lib=framework=Cocoa");

            // IOKit framework for pdevice_monitor.c (USB device monitoring)
            println!("cargo:rustc-link-lib=framework=IOKit");
        }
        _ => {
            // Fallback: try to link common libraries
            println!("cargo:rustc-link-lib=fuse");
            println!("cargo:rustc-link-lib=sqlite3");
            println!("cargo:rustc-link-lib=ssl");
            println!("cargo:rustc-link-lib=crypto");
            println!("cargo:rustc-link-lib=udev");
            println!("cargo:rustc-link-lib=pthread");
            println!("cargo:rustc-link-lib=m");
        }
    }
}

/// Try to find a library using pkg-config, fall back to direct linking
fn link_with_pkgconfig_or_fallback(pkg_name: &str, lib_name: &str) {
    if pkg_config::Config::new()
        .cargo_metadata(true)
        .probe(pkg_name)
        .is_err()
    {
        println!("cargo:rustc-link-lib={}", lib_name);
    }
}
