Bump the application's version across all files that track it.

Steps:
1. Read the current version from `Cargo.toml` (the `version` field under `[package]`).
2. Ask the user what the new version should be, showing the current version for reference.
3. Once the user provides a new version, update all three files:
   - `Cargo.toml` — the `version` field under `[package]`
   - `pkg/arch/PKGBUILD` — the `pkgver` field
   - `Cargo.lock` — run `cargo generate-lockfile` (or `cargo check`) to regenerate it with the new version
4. Verify all three files contain the new version string.
5. Stage the three changed files and create a commit with the message: `Build | Bump the version to <new version>`
6. After committing, ask the user if they want to create a git tag `v<new version>` for the commit. If yes, create an annotated tag with `git tag -a v<new version> -m "v<new version>"`.

Rules:
- Do NOT modify any fields other than the version.
- Do NOT append a "Co-Authored-By:" line or any similar footer to the commit message.
- Do NOT push to remote unless explicitly asked.