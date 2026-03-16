#!/bin/bash
# Generate and upload Breakpad symbol files to Bugsnag.
#
# Prerequisites:
#   - dump_syms:    cargo install dump_syms
#   - bugsnag-cli:  npm install -g @bugsnag/cli
#
# Usage:
#   BUGSNAG_API_KEY=<key> ./scripts/upload-symbols.sh

set -euo pipefail

if [ -z "${BUGSNAG_API_KEY:-}" ]; then
    echo "Error: BUGSNAG_API_KEY environment variable is required" >&2
    exit 1
fi

echo "Building release with debug info..."
RUSTFLAGS="-C debuginfo=2" cargo build --release --features crash-reporting

echo "Generating Breakpad symbol file..."
dump_syms ./target/release/pcloud-cli > pcloud-cli.sym

echo "Uploading symbols to Bugsnag..."
bugsnag-cli upload breakpad ./pcloud-cli.sym --api-key="$BUGSNAG_API_KEY"

echo "Stripping binary for distribution..."
strip ./target/release/pcloud-cli

echo "Done. Symbols uploaded and binary stripped."
rm -f pcloud-cli.sym
