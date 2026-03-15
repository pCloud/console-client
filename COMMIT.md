# Git Commit Message Pattern

This document describes the commit message pattern used in this project.

## Format

```
{Category} | {Brief description of the change in imperative tone}
```

**Optional body with bullet points:**
```
- {Detail 1}
- {Detail 2}
- {Detail 3}
```

## Pattern Rules

### Subject Line

The subject line follows this structure:
- `{Category} | {Description}`
- **Category**: A scope label indicating the area of the project affected (see [Categories](#categories) below)
- **Pipe separator**: ` | ` (space-pipe-space)
- **Description**: Brief, imperative mood description of what the commit does (Fix, not Fixed / Fixes etc.).
- The subject line should be capitalized and must not end in a period
- The subject line must not exceed 80 characters

### Categories

**Feature areas (the CLI application):**

| Category | When to use                                                          |
|----------|----------------------------------------------------------------------|
| `CLI`    | General CLI code: argument parsing, commands, output, error handling |
| `Auth`   | Login flows, web login, token management, password input             |
| `Crypto` | Encrypted folder operations, crypto setup/teardown                   |
| `Mount`  | FUSE mount/unmount operations                                        |
| `Sync`   | Sync-related CLI controls and configuration                          |
| `Daemon` | Background process management, IPC, signal handling                  |

**Infrastructure:**

| Category | When to use                                               |
|----------|-----------------------------------------------------------|
| `FFI`    | C bindings layer: `src/ffi/`, `src/wrapper/`, unsafe code |
| `Build`  | `build.rs`, compilation flags, linking, platform logic    |
| `Deps`   | `Cargo.toml`/`Cargo.lock`, pclsync submodule bumps        |
| `CI`     | GitHub Actions workflows, release automation              |
| `Tests`  | Unit tests, integration tests, test infrastructure        |
| `Docs`   | README, COMMIT.md, CLAUDE.md, doc comments                |

### Use the Imperative

In keeping with the standard output of git itself, all commit subject lines must be written using the imperative:

**Good**

- Refactor subsystem X for readability
- Update getting started documentation
- Remove deprecated methods
- Release version 1.0.0

**Bad**

- Fixed bug with Y
- Changing behavior of X

***Very* Bad**

- More fixes for broken stuff
- Sweet new API methods
- 42

Your commit subject line must be able to complete the sentence

> If applied, this commit will ...

## Subject Line Standard Terminology

| First Word | Meaning                                              |
|------------|------------------------------------------------------|
| Add        | Create a capability e.g. feature, test, dependency.  |
| Cut/Drop   | Remove a capability e.g. feature, test, dependency.  |
| Fix        | Fix an issue e.g. bug, typo, accident, misstatement. |
| Bump       | Increase the version of something e.g. dependency.   |
| Make       | Change the build process, or tooling, or infra.      |
| Start      | Begin doing something; e.g. create a feature flag.   |
| Stop       | End doing something; e.g. remove a feature flag.     |
| Refactor   | A code change that MUST be just a refactoring.       |
| Reformat   | Refactor of formatting, e.g. omit whitespace.        |
| Optimize   | Refactor of performance, e.g. speed up code.         |
| Document   | Refactor of documentation, e.g. help files.          |

### Examples

- `Auth | Add web-based login with QR code authentication`
- `Mount | Fix crash when mounting without credentials`
- `Crypto | Add --password-file option to crypto setup`
- `Sync | Add --exclude flag for selective sync`
- `Daemon | Fix IPC socket cleanup on abnormal shutdown`
- `CLI | Improve error output formatting`
- `FFI | Add bindings for psync_crypto_mkdir`
- `Build | Link against system OpenSSL on Linux`
- `Deps | Bump pclsync submodule to v3.2.1`
- `CI | Add release build workflow for .deb packaging`
- `Docs | Update README with macOS FUSE setup instructions`

### Body Format (Optional)

When additional details are needed:
- **Leave a blank line after the subject line**
- **The body must only contain explanations as to what and why, never how.**
- **Use the Body to Explain the Background and Reasoning, not the Implementation.** Especially if the diff is rather large or extremely clustered, you can save all fellow developers some time by explaining why you did what.
- Use bulleted lists with `-` for multiple changes.
- Use backticks for code elements (`` `ClassName` ``, `` `methodName()` ``)
- The body copy must be wrapped at 90 columns

Example:
```
Crypto | Add `--password-file` option to `crypto setup`

- Allow reading the crypto password from a file descriptor
- Useful for scripted/automated setups where interactive input is not possible

Closes #42
```

## Closing Issues

Use GitHub keywords in the commit body to automatically close issues when merged:
- `Fixes #N` — closes the issue (implies a bug fix)
- `Closes #N` — closes the issue (general purpose)
- `Resolves #N` — closes the issue (general purpose)

Place these on their own line at the end of the body, after a blank line.

## GitHub Markdown Support

The pattern uses these GitHub-supported markdown elements:
- **Inline code**: `` `ClassName` `` for referencing code elements
- **Bullet lists**: `-` for listing multiple changes
- **Issue references**: `#N` automatically links to the corresponding GitHub issue

## Template

```
{Category} | {Imperative verb} {what changed}

- {Additional detail if needed}
- {Additional detail if needed}

Closes #N
```
