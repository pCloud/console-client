Run the pre-commit quality checks for the project.

Execute the following checks in order, stopping on the first failure:

1. `cargo fmt --check` — verify code formatting
2. `cargo clippy -- -D warnings` — lint with all warnings treated as errors
3. `cargo check` — verify compilation
4. `cargo test` — run the full test suite

Report a summary of results when done. If any step fails, show the relevant errors and suggest fixes.
