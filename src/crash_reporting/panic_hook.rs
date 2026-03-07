//! Rust panic hook that reports panics to Bugsnag.

use std::panic;
use std::sync::Arc;

use bugsnag::Bugsnag;

/// Install a panic hook that reports to Bugsnag before delegating to the default hook.
pub fn install(client: Arc<Bugsnag>) {
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |info| {
        // Report to Bugsnag (best-effort, ignore errors)
        let _ = bugsnag::panic::handle(&client, info, None);

        // Still run the default hook so panics print to stderr as usual
        default_hook(info);
    }));
}
