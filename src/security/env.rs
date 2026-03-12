//! Environment variable secret injection.
//!
//! Resolves secrets (auth token, crypto password) from environment variables,
//! supporting both direct values and file-based injection for container deployments.
//!
//! # Environment Variables
//!
//! | Secret | Direct env var | File env var |
//! |---|---|---|
//! | Auth token | `PCLOUD_AUTH_TOKEN` | `PCLOUD_AUTH_TOKEN_FILE` |
//! | Crypto password | `PCLOUD_CRYPTO_PASS` | `PCLOUD_CRYPTO_PASS_FILE` |
//!
//! # Security
//!
//! - Environment variables are cleared from the process after reading
//! - File contents are trimmed (trailing newlines removed)
//! - Secrets are returned as `SecretString` for zeroization on drop

use std::fs;
use std::path::Path;

use secrecy::SecretString;

use crate::error::PCloudError;
use crate::Result;

/// Environment variable names for auth token.
const ENV_AUTH_TOKEN: &str = "PCLOUD_AUTH_TOKEN";
const ENV_AUTH_TOKEN_FILE: &str = "PCLOUD_AUTH_TOKEN_FILE";

/// Environment variable names for crypto password.
const ENV_CRYPTO_PASS: &str = "PCLOUD_CRYPTO_PASS";
const ENV_CRYPTO_PASS_FILE: &str = "PCLOUD_CRYPTO_PASS_FILE";

/// Secrets resolved from environment variables.
#[derive(Debug)]
pub struct ResolvedSecrets {
    /// Auth token from `PCLOUD_AUTH_TOKEN` or `PCLOUD_AUTH_TOKEN_FILE`
    pub auth_token: Option<SecretString>,
    /// Crypto password from `PCLOUD_CRYPTO_PASS` or `PCLOUD_CRYPTO_PASS_FILE`
    pub crypto_password: Option<SecretString>,
}

impl ResolvedSecrets {
    /// Resolve all supported secrets from environment variables.
    ///
    /// Both env var pairs are checked and cleared after reading.
    pub fn from_env() -> Result<Self> {
        let auth_token = resolve_secret(ENV_AUTH_TOKEN, ENV_AUTH_TOKEN_FILE)?;
        let crypto_password = resolve_secret(ENV_CRYPTO_PASS, ENV_CRYPTO_PASS_FILE)?;
        Ok(Self {
            auth_token,
            crypto_password,
        })
    }
}

/// Resolve a secret from a direct env var or a file-path env var.
///
/// Priority: direct env var > `_FILE` env var > `None`.
///
/// Both env vars are always cleared from the process after reading,
/// regardless of which one (if any) provided the value.
///
/// # Errors
///
/// Returns an error if:
/// - The `_FILE` env var points to a non-existent file
/// - The file is empty after trimming
/// - The file cannot be read
fn resolve_secret(env_name: &str, env_file_name: &str) -> Result<Option<SecretString>> {
    // Read both, then clear both unconditionally
    let direct = std::env::var(env_name).ok();
    let file_path = std::env::var(env_file_name).ok();

    // Clear env vars from process memory
    // Safety: remove_var is safe in single-threaded context or before spawning threads.
    // We call this early in startup before any worker threads are created.
    #[allow(unused_unsafe)]
    unsafe {
        std::env::remove_var(env_name);
        std::env::remove_var(env_file_name);
    }

    // Direct value takes priority
    if let Some(value) = direct {
        if value.is_empty() {
            return Err(PCloudError::Config(format!(
                "Environment variable {} is set but empty",
                env_name
            )));
        }
        return Ok(Some(SecretString::from(value)));
    }

    // Fall back to file-based injection
    if let Some(path_str) = file_path {
        let path = Path::new(&path_str);

        if !path.exists() {
            return Err(PCloudError::Config(format!(
                "Secret file not found: {} (from {})",
                path_str, env_file_name
            )));
        }

        let contents = fs::read_to_string(path).map_err(|e| {
            PCloudError::Config(format!("Failed to read secret file {}: {}", path_str, e))
        })?;

        let trimmed = contents.trim().to_string();
        if trimmed.is_empty() {
            return Err(PCloudError::Config(format!(
                "Secret file is empty: {} (from {})",
                path_str, env_file_name
            )));
        }

        return Ok(Some(SecretString::from(trimmed)));
    }

    Ok(None)
}

/// Convenience: resolve auth token from environment.
pub fn resolve_auth_token() -> Result<Option<SecretString>> {
    resolve_secret(ENV_AUTH_TOKEN, ENV_AUTH_TOKEN_FILE)
}

/// Convenience: resolve crypto password from environment.
pub fn resolve_crypto_password() -> Result<Option<SecretString>> {
    resolve_secret(ENV_CRYPTO_PASS, ENV_CRYPTO_PASS_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    use std::io::Write;
    use std::sync::Mutex;

    // Serialize env-var tests since set_var/remove_var are process-global.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn clear_env() {
        #[allow(unused_unsafe)]
        unsafe {
            std::env::remove_var(ENV_AUTH_TOKEN);
            std::env::remove_var(ENV_AUTH_TOKEN_FILE);
            std::env::remove_var(ENV_CRYPTO_PASS);
            std::env::remove_var(ENV_CRYPTO_PASS_FILE);
        }
    }

    #[test]
    fn test_direct_env_var() {
        let _lock = ENV_MUTEX.lock().unwrap();
        clear_env();

        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var(ENV_AUTH_TOKEN, "my-token-123");
        }

        let result = resolve_secret(ENV_AUTH_TOKEN, ENV_AUTH_TOKEN_FILE).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().expose_secret(), "my-token-123");

        // Env var should be cleared
        assert!(std::env::var(ENV_AUTH_TOKEN).is_err());
    }

    #[test]
    fn test_file_env_var() {
        let _lock = ENV_MUTEX.lock().unwrap();
        clear_env();

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("token");
        {
            let mut f = fs::File::create(&file_path).unwrap();
            writeln!(f, "file-token-456").unwrap();
        }

        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var(ENV_AUTH_TOKEN_FILE, file_path.to_str().unwrap());
        }

        let result = resolve_secret(ENV_AUTH_TOKEN, ENV_AUTH_TOKEN_FILE).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().expose_secret(), "file-token-456");

        // Env var should be cleared
        assert!(std::env::var(ENV_AUTH_TOKEN_FILE).is_err());
    }

    #[test]
    fn test_direct_takes_priority_over_file() {
        let _lock = ENV_MUTEX.lock().unwrap();
        clear_env();

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("token");
        {
            let mut f = fs::File::create(&file_path).unwrap();
            writeln!(f, "file-token").unwrap();
        }

        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var(ENV_AUTH_TOKEN, "direct-token");
            std::env::set_var(ENV_AUTH_TOKEN_FILE, file_path.to_str().unwrap());
        }

        let result = resolve_secret(ENV_AUTH_TOKEN, ENV_AUTH_TOKEN_FILE).unwrap();
        assert_eq!(result.unwrap().expose_secret(), "direct-token");

        // Both env vars should be cleared
        assert!(std::env::var(ENV_AUTH_TOKEN).is_err());
        assert!(std::env::var(ENV_AUTH_TOKEN_FILE).is_err());
    }

    #[test]
    fn test_neither_set_returns_none() {
        let _lock = ENV_MUTEX.lock().unwrap();
        clear_env();

        let result = resolve_secret(ENV_AUTH_TOKEN, ENV_AUTH_TOKEN_FILE).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_missing_file_returns_error() {
        let _lock = ENV_MUTEX.lock().unwrap();
        clear_env();

        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var(ENV_AUTH_TOKEN_FILE, "/nonexistent/path/token");
        }

        let result = resolve_secret(ENV_AUTH_TOKEN, ENV_AUTH_TOKEN_FILE);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_empty_file_returns_error() {
        let _lock = ENV_MUTEX.lock().unwrap();
        clear_env();

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("empty");
        fs::File::create(&file_path).unwrap();

        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var(ENV_AUTH_TOKEN_FILE, file_path.to_str().unwrap());
        }

        let result = resolve_secret(ENV_AUTH_TOKEN, ENV_AUTH_TOKEN_FILE);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("empty"));
    }

    #[test]
    fn test_empty_direct_var_returns_error() {
        let _lock = ENV_MUTEX.lock().unwrap();
        clear_env();

        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var(ENV_AUTH_TOKEN, "");
        }

        let result = resolve_secret(ENV_AUTH_TOKEN, ENV_AUTH_TOKEN_FILE);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("empty"));
    }

    #[test]
    fn test_file_contents_trimmed() {
        let _lock = ENV_MUTEX.lock().unwrap();
        clear_env();

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("token");
        {
            let mut f = fs::File::create(&file_path).unwrap();
            write!(f, "  my-token  \n\n").unwrap();
        }

        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var(ENV_CRYPTO_PASS_FILE, file_path.to_str().unwrap());
        }

        let result = resolve_secret(ENV_CRYPTO_PASS, ENV_CRYPTO_PASS_FILE).unwrap();
        assert_eq!(result.unwrap().expose_secret(), "my-token");
    }

    #[test]
    fn test_resolved_secrets_from_env() {
        let _lock = ENV_MUTEX.lock().unwrap();
        clear_env();

        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var(ENV_AUTH_TOKEN, "token-abc");
            std::env::set_var(ENV_CRYPTO_PASS, "crypto-xyz");
        }

        let secrets = ResolvedSecrets::from_env().unwrap();
        assert_eq!(secrets.auth_token.unwrap().expose_secret(), "token-abc");
        assert_eq!(
            secrets.crypto_password.unwrap().expose_secret(),
            "crypto-xyz"
        );
    }

    #[test]
    fn test_resolved_secrets_none_when_unset() {
        let _lock = ENV_MUTEX.lock().unwrap();
        clear_env();

        let secrets = ResolvedSecrets::from_env().unwrap();
        assert!(secrets.auth_token.is_none());
        assert!(secrets.crypto_password.is_none());
    }
}
