//! Build script for compiling the pclsync C library and linking dependencies.
//!
//! This script:
//! - Detects the target platform (Linux/macOS)
//! - Compiles the vendored SQLite amalgamation (`vendor/sqlite/sqlite3.c`)
//!   into its own static archive (`libsqlite3.a`)
//! - Compiles all pclsync .c source files into `libpclsync.a`
//! - Links required system libraries (fuse, pthread, openssl, zlib, udev)
//! - Sets up include paths for C headers
//! - Generates Rust bindings for C structs using bindgen
//! - Emits linker flags so the final binary can dead-strip unused C code
//!   (`-Wl,--gc-sections` on Linux, `-Wl,-dead_strip` on macOS)
//!
//! SQLite is vendored — there is no runtime or build-time dependency on the
//! host's libsqlite3. To bump the SQLite version, use `tools/update-sqlite.sh`.
//!
//! # Required System Dependencies
//!
//! ## Linux (Debian/Ubuntu)
//! ```bash
//! sudo apt-get install libfuse-dev libssl-dev zlib1g-dev libclang-dev
//! ```
//!
//! ## Linux (Fedora/RHEL)
//! ```bash
//! sudo dnf install fuse-devel openssl-devel zlib-devel clang-devel
//! ```
//!
//! ## macOS
//! ```bash
//! brew install macfuse openssl llvm
//! ```

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let pclsync_dir = manifest_dir.join("pclsync");
    let sqlite_dir = manifest_dir.join("vendor").join("sqlite");

    // Verify pclsync directory exists
    if !pclsync_dir.exists() {
        panic!(
            "pclsync directory not found at {:?}. \
             Please initialize git submodules: git submodule update --init",
            pclsync_dir
        );
    }

    // Verify vendored SQLite is present and matches its VERSION file.
    let sqlite_version = check_vendored_sqlite_version(&sqlite_dir);
    println!("cargo:rustc-env=SQLITE_VENDORED_VERSION={}", sqlite_version);

    // Configure the C compiler for pclsync
    let mut build = cc::Build::new();

    // Common compiler flags
    build
        .warnings(false)
        .std("gnu99")
        .flag_if_supported("-Wpointer-arith")
        .opt_level(2)
        .flag_if_supported("-fno-stack-protector")
        .flag_if_supported("-fomit-frame-pointer")
        // Per-function/data sections so the final binary link can
        // `--gc-sections` away pclsync code that pclsync doesn't actually use.
        .flag_if_supported("-ffunction-sections")
        .flag_if_supported("-fdata-sections")
        // GCC 14+ compiler demotion of certain warnings.
        .flag_if_supported("-Wno-error=int-conversion")
        .flag_if_supported("-Wno-error=incompatible-pointer-types")
        .define("PSYNC_DEFAULT_POSIX_DIR", "\".pcloud-cli\"");

    // Make pclsync `#include <sqlite3.h>` resolve to the vendored copy first.
    build.include(&sqlite_dir);

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
        // Added with pclsync 2.26.05.2: split-out utility modules and the
        // document-editing subsystem (now referenced unconditionally from
        // psynclib.c).
        "pstrings.c",
        "pencoding.c",
        "pdevicemap.c",
        "pdocument_editing.c",
        "pqsort.c",
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

    // Compile the vendored SQLite amalgamation into its own static archive.
    //
    // This MUST come *after* `build.compile("pclsync")`: with a single-pass
    // static linker, an archive only resolves undefined references that appear
    // before it on the command line. `cc::Build::compile` emits its
    // `cargo:rustc-link-lib=static=...` directive in call order, so compiling
    // SQLite second yields `-lpclsync ... -lsqlite3`. pclsync references
    // `sqlite3_*` symbols (e.g. `sqlite3_db_release_memory` in plibs.c) and
    // SQLite references nothing in pclsync, so this is the correct order.
    // Building SQLite first produced `-lsqlite3 ... -lpclsync`, which only
    // happened to link on x86_64 because `--gc-sections` dropped the
    // referencing pclsync section; the aarch64 test binary kept it and failed
    // with an undefined reference. The archive only needs to exist for the
    // final rustc link (after build.rs finishes), so compiling it here is fine.
    compile_sqlite(&sqlite_dir);

    // Link system libraries
    link_system_libraries(&target_os);

    // Ask the linker to drop unreferenced sections from the final binary. This
    // pairs with `-ffunction-sections -fdata-sections` on both C builds and is
    // what gives us "tree-shaking" of unused SQLite (and pclsync) code.
    match target_os.as_str() {
        "linux" => println!("cargo:rustc-link-arg=-Wl,--gc-sections"),
        "macos" => println!("cargo:rustc-link-arg=-Wl,-dead_strip"),
        _ => {}
    }

    // Generate bindings using bindgen
    generate_bindings(&pclsync_dir, &out_dir, &target_os);

    // Tell Cargo to rerun this script if pclsync sources change
    println!("cargo:rerun-if-changed=pclsync/");
    println!("cargo:rerun-if-changed=vendor/sqlite/");
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

    // SQLite is vendored at vendor/sqlite/; its include path is wired up in
    // main() and the static archive is built by compile_sqlite().

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

    // SQLite is vendored at vendor/sqlite/; its include path is wired up in
    // main() and the static archive is built by compile_sqlite().

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
            // Use pkg-config to find libraries when available.
            // (SQLite is vendored — compile_sqlite() already emits its
            // own static-link directive.)
            link_with_pkgconfig_or_fallback("fuse", "fuse");
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

            // SQLite is vendored — compile_sqlite() already emits its own
            // static-link directive.

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
            // (SQLite is vendored; not listed here.)
            println!("cargo:rustc-link-lib=fuse");
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

/// Read `vendor/sqlite/VERSION` and cross-check against the `SQLITE_VERSION`
/// macro in `vendor/sqlite/sqlite3.h`. Panics with a clear message if the
/// vendored sources are missing, or if the two strings disagree.
///
/// Returns the version string from `VERSION` for use as a build-env value.
fn check_vendored_sqlite_version(sqlite_dir: &Path) -> String {
    let version_path = sqlite_dir.join("VERSION");
    let header_path = sqlite_dir.join("sqlite3.h");
    let source_path = sqlite_dir.join("sqlite3.c");

    for required in [&version_path, &header_path, &source_path] {
        if !required.exists() {
            panic!(
                "Vendored SQLite source missing at {:?}. \
                 Run `tools/update-sqlite.sh <version>` to install it.",
                required
            );
        }
    }

    let pinned = fs::read_to_string(&version_path)
        .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", version_path, e))
        .trim()
        .to_string();

    let header = fs::read_to_string(&header_path)
        .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", header_path, e));

    // Look for `#define SQLITE_VERSION  "X.Y.Z"`.
    let header_version = header.lines().find_map(|line| {
        let line = line.trim_start();
        let rest = line.strip_prefix("#define")?.trim_start();
        let rest = rest.strip_prefix("SQLITE_VERSION")?;
        // Must be followed by whitespace, not `_NUMBER`, `_SOURCE_ID`, etc.
        let rest = rest.strip_prefix(|c: char| c.is_whitespace())?;
        let rest = rest.trim_start();
        let rest = rest.strip_prefix('"')?;
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    });

    let header_version = header_version.unwrap_or_else(|| {
        panic!(
            "Could not find `#define SQLITE_VERSION \"...\"` in {:?}",
            header_path
        )
    });

    if pinned != header_version {
        panic!(
            "Vendored SQLite version mismatch: VERSION says {:?} but \
             sqlite3.h says {:?}. Either fix vendor/sqlite/VERSION or \
             re-run tools/update-sqlite.sh.",
            pinned, header_version
        );
    }

    println!("cargo:rerun-if-changed={}", version_path.display());
    println!("cargo:rerun-if-changed={}", header_path.display());

    pinned
}

/// Compile the vendored SQLite amalgamation into a standalone static archive.
///
/// `cc::Build::compile("sqlite3")` writes `libsqlite3.a` into `$OUT_DIR` and
/// emits `cargo:rustc-link-lib=static=sqlite3` so the Rust binary links it.
///
/// Flag rationale:
/// - `SQLITE_THREADSAFE=1` — required; pclsync checks `sqlite3_threadsafe()`
///   at runtime in `pclsync/plibs.c` and opens the DB with `FULLMUTEX`.
/// - `ENABLE_COLUMN_METADATA`, `SECURE_DELETE` — small wins; the latter is
///   worthwhile because pclsync stores encrypted folder/file keys.
/// - The `OMIT_*` family removes APIs pclsync never calls (audited:
///   `pclsync/pdatabase.h`, `pclsync/plibs.c`).
/// - `-ffunction-sections -fdata-sections` puts each function/data symbol in
///   its own ELF section, so the final binary link can `--gc-sections` away
///   anything unreferenced.
/// - `opt_level(2)` + `NDEBUG` are unconditional: SQLite is ~9 MB of C, and
///   we don't want a 30 s debug build cost every time `cargo build` runs.
fn compile_sqlite(sqlite_dir: &Path) {
    let mut build = cc::Build::new();
    build
        .file(sqlite_dir.join("sqlite3.c"))
        .warnings(false)
        .opt_level(2)
        .define("NDEBUG", None)
        // Required
        .define("SQLITE_THREADSAFE", "1")
        .define("HAVE_USLEEP", "1")
        // Modest enables
        .define("SQLITE_ENABLE_COLUMN_METADATA", None)
        .define("SQLITE_SECURE_DELETE", None)
        // Size/security trims — pclsync uses none of these
        .define("SQLITE_OMIT_LOAD_EXTENSION", None)
        .define("SQLITE_OMIT_DEPRECATED", None)
        .define("SQLITE_OMIT_SHARED_CACHE", None)
        .define("SQLITE_OMIT_AUTHORIZATION", None)
        .define("SQLITE_OMIT_PROGRESS_CALLBACK", None)
        .define("SQLITE_OMIT_TRACE", None)
        .define("SQLITE_OMIT_UTF16", None)
        // Default tweaks
        .define("SQLITE_DEFAULT_MEMSTATUS", "0")
        .define("SQLITE_DEFAULT_WAL_SYNCHRONOUS", "1")
        .define("SQLITE_MAX_EXPR_DEPTH", "0")
        // Tree-shaking enablers (paired with --gc-sections / -dead_strip)
        .flag_if_supported("-ffunction-sections")
        .flag_if_supported("-fdata-sections");

    build.compile("sqlite3");
}
