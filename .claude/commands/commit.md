Create a git commit for the currently staged changes following the project's commit message conventions from COMMIT.md.

Steps:
1. Run `git diff --cached` to review staged changes. If nothing is staged, run `git status` and tell the user there are no staged changes, then stop.
2. Read COMMIT.md for the full commit style reference.
3. Analyze the staged changes to determine the appropriate category and description.
4. Draft a commit message following the format: `{Category} | {Brief imperative description}`
   - Choose the category from COMMIT.md's category table (CLI, Auth, Crypto, Mount, Sync, Daemon, FFI, Build, Deps, CI, Docs, Tests).
   - Use imperative mood verbs (Add, Fix, Refactor, Drop, Bump, etc.) as defined in COMMIT.md.
   - Keep the subject line under 80 characters, capitalized, no trailing period.
5. Add an optional body (separated by a blank line) with bullet points if the change is non-trivial. The body explains what and why, never how, wrapped at 90 columns.
6. Present the draft commit message to the user for approval before committing.
7. Once approved, create the commit.

Rules:
- Follow the commit style defined in COMMIT.md exactly.
- Do NOT append a "Co-Authored-By:" line or any similar footer to the commit message.
- Do NOT amend previous commits unless explicitly asked.
- Do NOT push to remote unless explicitly asked.
