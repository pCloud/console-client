//! Bugsnag client configuration and singleton management.

use std::sync::{Arc, OnceLock};

use bugsnag::Bugsnag;

static CLIENT: OnceLock<Arc<Bugsnag>> = OnceLock::new();

/// Return the Bugsnag API key.
///
/// The key is embedded at compile time from `BUGSNAG_API_KEY` (resolved by
/// `build.rs`, which supplies a built-in default when the variable is unset).
/// It is stored obfuscated in the binary via `obfstr` and deobfuscated into a
/// fresh `String` on each call, so the plaintext key never appears as a
/// readable literal in the compiled output (frustrating a casual `strings`).
///
/// Under `#[cfg(test)]` this returns a mock value so that tests compile
/// without requiring the `BUGSNAG_API_KEY` environment variable.
pub(crate) fn api_key() -> String {
    #[cfg(test)]
    {
        "test-api-key".to_string()
    }
    #[cfg(not(test))]
    {
        obfstr::obfstring!(env!("BUGSNAG_API_KEY"))
    }
}

/// Derive the Bugsnag release stage from the version string.
fn release_stage() -> &'static str {
    let version = env!("PCLOUD_VERSION");
    if version.ends_with("-dev") {
        "development"
    } else if version.ends_with("-qa") {
        "qa"
    } else {
        "production"
    }
}

/// Create and store the Bugsnag client singleton. Returns a clone of the Arc.
pub fn create_client() -> Arc<Bugsnag> {
    CLIENT
        .get_or_init(|| {
            let mut client = Bugsnag::new(&api_key(), env!("CARGO_MANIFEST_DIR"));
            client.set_app_info(
                Some(env!("PCLOUD_VERSION")),
                Some(release_stage()),
                Some("rust"),
            );
            Arc::new(client)
        })
        .clone()
}

/// Access the client singleton if it has been initialized.
pub fn with_client(f: impl FnOnce(&Bugsnag)) {
    if let Some(client) = CLIENT.get() {
        f(client);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_release_stage_from_version() {
        // The actual version is set at compile time, so we just verify the function runs
        let stage = release_stage();
        assert!(
            stage == "development" || stage == "qa" || stage == "production",
            "unexpected release stage: {}",
            stage
        );
    }
}
