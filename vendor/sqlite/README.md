# Vendored SQLite amalgamation

This directory contains the SQLite amalgamation source, vendored from
[sqlite.org](https://sqlite.org/download.html). The pclsync C library and the
Rust binary are linked against this copy as a single static archive
(`libsqlite3.a`), so there is no runtime dependency on the host's libsqlite3.

Pinned version is recorded in `VERSION` and cross-checked against
`SQLITE_VERSION` in `sqlite3.h` by `build.rs` at compile time.

## Files

| File           | Source                                                     |
|----------------|------------------------------------------------------------|
| `sqlite3.c`    | `sqlite-amalgamation-XXXXXXX.zip` from sqlite.org          |
| `sqlite3.h`    | same                                                       |
| `sqlite3ext.h` | same                                                       |
| `VERSION`      | one-line text file with the SQLite version (e.g. `3.53.1`) |

`shell.c` is **not** vendored (we never embed the SQLite CLI).

## Updating

Run `tools/update-sqlite.sh <version>` from the repository root, e.g.:

```bash
./tools/update-sqlite.sh 3.53.1
```

The script downloads the matching amalgamation zip from sqlite.org, verifies
its SHA3-256 if `--sha3 <hex>` is passed, and replaces the four files above.
See the script header for details.

After updating, run `cargo build && cargo test` and commit `vendor/sqlite/` in
a single commit.

## Do not edit

These files are upstream and should not be patched in place. If you need to
change SQLite's compile-time behavior, change the `cc::Build::define(...)`
calls in `build.rs::compile_sqlite()` instead.

## Compile-time flags

The active flag set is defined in `build.rs::compile_sqlite()`. Highlights:

- `SQLITE_THREADSAFE=1` (required by pclsync's runtime check in `plibs.c`)
- `SQLITE_ENABLE_COLUMN_METADATA`, `SQLITE_SECURE_DELETE`
- `SQLITE_OMIT_LOAD_EXTENSION`, `SQLITE_OMIT_DEPRECATED`,
  `SQLITE_OMIT_SHARED_CACHE`, `SQLITE_OMIT_AUTHORIZATION`,
  `SQLITE_OMIT_PROGRESS_CALLBACK`, `SQLITE_OMIT_TRACE`, `SQLITE_OMIT_UTF16`
- `SQLITE_DEFAULT_MEMSTATUS=0`, `SQLITE_DEFAULT_WAL_SYNCHRONOUS=1`,
  `SQLITE_MAX_EXPR_DEPTH=0`
- `-ffunction-sections -fdata-sections` (so the final binary link can
  `--gc-sections` away unused SQLite functions)

No `SQLITE_ENABLE_FTS*`, `RTREE`, `JSON1`, `GEOPOLY`, or `DBSTAT_VTAB`:
pclsync uses none of these (audit: `pclsync/pdatabase.h`, `pclsync/plibs.c`).
