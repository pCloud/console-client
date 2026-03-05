//! Web-based login functionality.
//!
//! This module provides safe Rust wrappers for the web-based login flow,
//! which allows users to authenticate via a browser instead of entering
//! credentials directly in the CLI.
//!
//! # Web Login Flow
//!
//! 1. Client requests a login session ID from the server
//! 2. Client constructs a URL with the session ID and displays it to the user
//! 3. User opens the URL in a browser and logs in
//! 4. Client polls the server waiting for authentication completion
//! 5. On success, the auth token is automatically set in the library
//!
//! # Example
//!
//! ```ignore
//! use console_client::wrapper::weblogin::{WebLoginConfig, WebLoginSession};
//! use console_client::wrapper::PCloudClient;
//!
//! let client = PCloudClient::init()?;
//! let mut guard = client.lock().unwrap();
//!
//! // Initiate web login
//! let session = guard.initiate_web_login(&WebLoginConfig::default())?;
//!
//! // Display the login URL to the user
//! println!("Open this URL: {}", session.login_url);
//!
//! // Wait for user to complete authentication
//! guard.wait_for_web_auth(&session.request_id)?;
//!
//! println!("Authentication successful!");
//! ```

use std::env;
use std::ffi::{c_void, CStr, CString};

use crate::error::{PCloudError, WebLoginError};
use crate::ffi::raw;
use crate::Result;

/// Web login base URL.
const WEB_LOGIN_BASE_URL: &str = "https://my.pcloud.com/webview/authentication";

/// Configuration for web login session.
///
/// Contains parameters that will be encoded in the login URL.
#[derive(Debug, Clone)]
pub struct WebLoginConfig {
    /// View type (always "login")
    pub view: String,
    /// Device name (from get_machine_name() FFI call)
    pub device: String,
    /// Unique device identifier
    pub device_id: String,
    /// Operating system version
    pub os_version: String,
    /// Application version (from Cargo.toml)
    pub app_version: String,
    /// Client identifier
    pub client_id: String,
    /// System language
    pub lang: String,
    /// Theme (dark/light)
    pub theme: String,
    /// Operating system type (3 = Desktop generic)
    pub os: u32,
}

impl Default for WebLoginConfig {
    fn default() -> Self {
        Self {
            view: "login".to_string(),
            device: get_machine_name_safe(),
            device_id: generate_device_id(),
            os_version: get_os_version(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            client_id: "pcloud-console".to_string(),
            lang: get_system_language(),
            theme: "dark".to_string(),
            os: 3, // Desktop generic
        }
    }
}

/// Web login session information.
///
/// Contains the request ID and the constructed login URL.
#[derive(Debug, Clone)]
pub struct WebLoginSession {
    /// The request ID from the server
    pub request_id: String,
    /// The complete login URL to display to the user
    pub login_url: String,
}

/// Safe wrapper around get_machine_name() FFI call.
///
/// Returns the machine hostname or a default value if the call fails.
pub fn get_machine_name_safe() -> String {
    unsafe {
        let ptr = raw::get_pc_name();
        if ptr.is_null() {
            return "pcloud-cli".to_string();
        }
        let name = CStr::from_ptr(ptr).to_string_lossy().to_string();
        raw::psync_free(ptr as *mut c_void);
        name
    }
}

/// Generate a unique device ID.
///
/// Uses a combination of machine name and a timestamp-based hash
/// to create a somewhat unique identifier.
fn generate_device_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut hasher = DefaultHasher::new();

    // Hash machine name
    get_machine_name_safe().hash(&mut hasher);

    // Hash current user
    if let Ok(user) = env::var("USER") {
        user.hash(&mut hasher);
    }

    // Hash home directory for additional uniqueness
    if let Ok(home) = env::var("HOME") {
        home.hash(&mut hasher);
    }

    // Include a stable timestamp component (installation time would be better)
    // For now, we just use the hash without time to keep it stable
    let hash = hasher.finish();

    format!("pcloud-cli-{:016x}", hash)
}

/// Get the operating system version.
///
/// On Linux, reads from /etc/os-release.
fn get_os_version() -> String {
    // Try to read from /etc/os-release
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                let value = line.trim_start_matches("PRETTY_NAME=");
                let value = value.trim_matches('"');
                return value.to_string();
            }
        }
    }

    // Fallback to uname
    #[cfg(unix)]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("uname").arg("-sr").output() {
            if let Ok(s) = String::from_utf8(output.stdout) {
                return s.trim().to_string();
            }
        }
    }

    "Linux".to_string()
}

/// Get the system language from environment.
fn get_system_language() -> String {
    // Try LANG, then LC_ALL, then LANGUAGE
    for var in &["LANG", "LC_ALL", "LANGUAGE"] {
        if let Ok(lang) = env::var(var) {
            // Extract just the language code (e.g., "en" from "en_US.UTF-8")
            let lang = lang.split('_').next().unwrap_or("en");
            let lang = lang.split('.').next().unwrap_or("en");
            if !lang.is_empty() && lang != "C" && lang != "POSIX" {
                return lang.to_string();
            }
        }
    }
    "en".to_string()
}

/// URL-encode a string.
fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                result.push(c);
            }
            _ => {
                for byte in c.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    result
}

/// Build the complete login URL with all parameters.
fn build_login_url(request_id: &str, config: &WebLoginConfig) -> Result<String> {
    let params = [
        ("view", config.view.as_str()),
        ("request_id", request_id),
        ("device", config.device.as_str()),
        ("deviceid", config.device_id.as_str()),
        ("osversion", config.os_version.as_str()),
        ("appversion", config.app_version.as_str()),
        ("clientid", config.client_id.as_str()),
        ("lang", config.lang.as_str()),
        ("theme", config.theme.as_str()),
    ];

    let query_string: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, url_encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    let url = format!(
        "{}?{}&os={}",
        WEB_LOGIN_BASE_URL, query_string, config.os
    );

    Ok(url)
}

/// Initiate a web login session.
///
/// Requests a login session ID from the server and constructs the login URL.
///
/// # Returns
///
/// A `WebLoginSession` containing the request ID and login URL on success.
pub fn initiate_web_login(config: &WebLoginConfig) -> Result<WebLoginSession> {
    let mut req_id_ptr: *mut libc::c_char = std::ptr::null_mut();

    let result = unsafe { raw::psync_get_login_req_id(&mut req_id_ptr) };

    if result != 0 {
        return Err(PCloudError::WebLogin(if result == -1 {
            WebLoginError::NetworkError
        } else {
            WebLoginError::RequestIdFailed(format!("API error code: {}", result))
        }));
    }

    if req_id_ptr.is_null() {
        return Err(PCloudError::WebLogin(WebLoginError::RequestIdFailed(
            "Null request ID returned".to_string(),
        )));
    }

    let request_id = unsafe {
        let s = CStr::from_ptr(req_id_ptr).to_string_lossy().to_string();
        raw::psync_free(req_id_ptr as *mut c_void);
        s
    };

    let login_url = build_login_url(&request_id, config)?;

    Ok(WebLoginSession {
        request_id,
        login_url,
    })
}

/// Wait for web authentication to complete.
///
/// Blocks until the user completes authentication in the browser or timeout occurs.
/// On success, the auth token is automatically set in the pclsync library.
///
/// # Arguments
///
/// * `request_id` - The request ID from `initiate_web_login()`
pub fn wait_for_web_auth(request_id: &str) -> Result<()> {
    let c_request_id = CString::new(request_id).map_err(|e| {
        PCloudError::WebLogin(WebLoginError::RequestIdFailed(format!(
            "Invalid request ID: {}",
            e
        )))
    })?;

    let result = unsafe { raw::psync_wait_auth_token(c_request_id.as_ptr()) };

    match result {
        0 => Ok(()),
        -1 => Err(PCloudError::WebLogin(WebLoginError::NetworkError)),
        code if code > 0 => {
            // Typically timeout or user didn't complete auth
            Err(PCloudError::WebLogin(WebLoginError::Timeout))
        }
        code => Err(PCloudError::WebLogin(WebLoginError::Unknown(code))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WebLoginConfig::default();
        assert_eq!(config.view, "login");
        assert_eq!(config.client_id, "pcloud-console");
        assert_eq!(config.theme, "dark");
        assert_eq!(config.os, 3);
        assert!(!config.app_version.is_empty());
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello"), "hello");
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a=b&c=d"), "a%3Db%26c%3Dd");
        assert_eq!(url_encode("test@example.com"), "test%40example.com");
    }

    #[test]
    fn test_build_login_url() {
        let config = WebLoginConfig {
            view: "login".to_string(),
            device: "test-device".to_string(),
            device_id: "test-id".to_string(),
            os_version: "Linux 5.0".to_string(),
            app_version: "1.0.0".to_string(),
            client_id: "pcloud-console".to_string(),
            lang: "en".to_string(),
            theme: "dark".to_string(),
            os: 3,
        };

        let url = build_login_url("test-request-id", &config).unwrap();

        assert!(url.starts_with(WEB_LOGIN_BASE_URL));
        assert!(url.contains("view=login"));
        assert!(url.contains("request_id=test-request-id"));
        assert!(url.contains("device=test-device"));
        assert!(url.contains("os=3"));
    }

    #[test]
    fn test_get_system_language() {
        let lang = get_system_language();
        // Should return some non-empty language code
        assert!(!lang.is_empty());
    }

    #[test]
    fn test_generate_device_id() {
        let id = generate_device_id();
        assert!(id.starts_with("pcloud-cli-"));
        assert!(id.len() > 20); // Should have a reasonable length

        // Should be deterministic (same input = same output)
        let id2 = generate_device_id();
        assert_eq!(id, id2);
    }

    #[test]
    fn test_get_os_version() {
        let version = get_os_version();
        // Should return some non-empty string
        assert!(!version.is_empty());
    }

    #[test]
    fn test_weblogin_session_debug() {
        let session = WebLoginSession {
            request_id: "test-id".to_string(),
            login_url: "https://example.com".to_string(),
        };
        let debug = format!("{:?}", session);
        assert!(debug.contains("test-id"));
        assert!(debug.contains("example.com"));
    }

    #[test]
    fn test_weblogin_config_clone() {
        let config = WebLoginConfig::default();
        let config2 = config.clone();
        assert_eq!(config.view, config2.view);
        assert_eq!(config.device_id, config2.device_id);
    }
}
