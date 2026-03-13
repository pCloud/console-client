Generate a changelog, create an annotated git tag, and show QA focus areas.

Steps:

1. **Detect the range boundary:**
   - If `$ARGUMENTS` is provided (non-empty), use that as the base ref — skip to step 3.
   - Otherwise, auto-detect:
     - Find the most recent git tag on the current branch: `git describe --tags --abbrev=0 2>/dev/null`.
     - Find the most recent version bump commit: `git log --oneline --grep="Build | Bump the version to" -1`.
     - If both exist and the tag is on or after the bump commit (i.e., the tag's commit is an ancestor of or equal to the bump commit), use the tag.
     - If they disagree, use whichever is more recent (closer to HEAD).
     - If only one exists, use that one.
     - If neither exists, report an error and stop.

2. **Confirm with user:** Show the detected base ref, its commit message, and the list of commit subject lines from `git log --oneline --no-merges <base>..HEAD`. Ask the user to confirm or provide a different ref before proceeding. Do NOT continue until the user confirms.

3. **Gather changes:** Run `git log --oneline --no-merges <base>..HEAD` and `git diff --stat <base>..HEAD`.

4. **Read current version** from `Cargo.toml` — the `version` field under `[package]`.

5. **Compose the tag message** using this exact format (version as title, then grouped changelog):

```
v{version}
----------

## New Features & Enhancements
- **{Feature}**: {description}

## Bug Fixes
- **{Category} / {file(s)}**: {description}

## Build & Tooling
- **{Area}**: {description}

## Format / Cleanup
- {description}
```

   - Group each commit into the most appropriate section based on the commit message and the changes.
   - Omit any section that has no entries.
   - Write descriptions in past tense, concise, focusing on what changed and why it matters.
   - Collapse related commits into a single entry when appropriate.

1. **Show the draft tag message** to the user for approval. Do NOT create the tag until the user approves.

2. **Ask if the user wants to create the annotated git tag** on HEAD: `git tag -a v{version} -m "{message}"`.

3. **Show QA Focus Areas** in the conversation (NOT in the tag message). Present a concise Markdown table with columns `Area` and `What to test`, targeting a QA engineer. Focus on areas affected by the changes in this release.

Rules:
- Do NOT push tags to remote unless explicitly asked.
- Do NOT modify any files — this command only reads the repo and creates a tag.
- If the tag `v{version}` already exists, report this to the user and stop.