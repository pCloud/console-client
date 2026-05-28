#!/usr/bin/env bash
# Update the vendored SQLite amalgamation in vendor/sqlite/.
#
# Usage:
#   tools/update-sqlite.sh <version> [--sha3 <hex>] [--year <YYYY>] [--url <url>]
#
# Examples:
#   tools/update-sqlite.sh 3.53.1
#   tools/update-sqlite.sh 3.53.1 --sha3 3c07136e4f6b5dd0c395be86455014039597bc65b6851f7111e88f71b6e06114
#   tools/update-sqlite.sh 3.53.1 --year 2026
#   tools/update-sqlite.sh 3.53.1 --url https://sqlite.org/2026/sqlite-amalgamation-3530100.zip
#
# Behavior:
#   - Encodes the version as sqlite.org's release number (3.53.1 -> 3530100)
#   - Downloads sqlite-amalgamation-<release>.zip from sqlite.org
#   - If --sha3 <hex> is given, verifies the download against it (SHA3-256)
#   - Extracts and replaces vendor/sqlite/{sqlite3.c,sqlite3.h,sqlite3ext.h}
#   - Writes <version> into vendor/sqlite/VERSION
#
# Year inference:
#   sqlite.org's URL layout is /<year>/<file>. The year is inferred by probing
#   the current year first, then walking backwards a few years. Override with
#   --year or pass the full --url to skip inference.
#
# After running:
#   cargo build && cargo test
#   git add vendor/sqlite/ && git commit -m "Deps | Bump SQLite to <version>"
#
# Dependencies: bash, curl, unzip, openssl (only if --sha3 is given).
set -euo pipefail

usage() {
    sed -n '2,/^set -euo/p' "$0" | sed -e 's/^# \{0,1\}//' -e '/^set -euo/d'
    exit "${1:-2}"
}

VERSION=""
EXPECTED_SHA3=""
OVERRIDE_YEAR=""
OVERRIDE_URL=""

while [ $# -gt 0 ]; do
    case "$1" in
        -h|--help)   usage 0 ;;
        --sha3)      EXPECTED_SHA3="${2:?--sha3 requires a hex argument}"; shift 2 ;;
        --year)      OVERRIDE_YEAR="${2:?--year requires YYYY}"; shift 2 ;;
        --url)       OVERRIDE_URL="${2:?--url requires a URL}"; shift 2 ;;
        --) shift; break ;;
        -*) echo "Unknown option: $1" >&2; usage 2 ;;
        *)
            if [ -z "$VERSION" ]; then
                VERSION="$1"
                shift
            else
                echo "Unexpected positional argument: $1" >&2
                usage 2
            fi
            ;;
    esac
done

if [ -z "$VERSION" ]; then
    echo "Error: <version> is required (e.g. 3.53.1)" >&2
    usage 2
fi

# Validate "MAJOR.MINOR.PATCH" (optionally with a 4th component, which we ignore
# beyond the standard encoding).
if ! [[ "$VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(\.([0-9]+))?$ ]]; then
    echo "Error: version must look like 3.53.1 (or 3.53.1.2), got: $VERSION" >&2
    exit 2
fi

MAJOR="${BASH_REMATCH[1]}"
MINOR="${BASH_REMATCH[2]}"
PATCH="${BASH_REMATCH[3]}"
BUILD="${BASH_REMATCH[5]:-0}"

# Release number: XYYZZAA where X=major, YY=minor, ZZ=patch, AA=build.
# Example: 3.53.1 -> 3530100, 3.46.0.1 -> 3460001.
RELEASE=$(printf '%d%02d%02d%02d' "$MAJOR" "$MINOR" "$PATCH" "$BUILD")
FILENAME="sqlite-amalgamation-${RELEASE}.zip"

# Resolve URL.
if [ -n "$OVERRIDE_URL" ]; then
    URL="$OVERRIDE_URL"
elif [ -n "$OVERRIDE_YEAR" ]; then
    URL="https://sqlite.org/${OVERRIDE_YEAR}/${FILENAME}"
else
    # Probe the current year, then the previous few. SQLite mirrors releases
    # under the year of release.
    CURRENT_YEAR=$(date +%Y)
    URL=""
    for offset in 0 -1 -2 -3 -4; do
        YEAR=$((CURRENT_YEAR + offset))
        CANDIDATE="https://sqlite.org/${YEAR}/${FILENAME}"
        STATUS=$(curl -sIL -o /dev/null -w "%{http_code}" "$CANDIDATE" || true)
        if [ "$STATUS" = "200" ]; then
            URL="$CANDIDATE"
            break
        fi
    done
    if [ -z "$URL" ]; then
        echo "Error: could not find ${FILENAME} on sqlite.org for any year ${CURRENT_YEAR}..(-4)." >&2
        echo "Pass --year YYYY or --url <url> explicitly." >&2
        exit 1
    fi
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR_DIR="${ROOT}/vendor/sqlite"

if [ ! -d "$VENDOR_DIR" ]; then
    echo "Error: $VENDOR_DIR does not exist" >&2
    exit 1
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "==> Downloading $URL"
curl -fsSL -o "${TMP}/sqlite.zip" "$URL"

if [ -n "$EXPECTED_SHA3" ]; then
    echo "==> Verifying SHA3-256"
    if ! command -v openssl >/dev/null 2>&1; then
        echo "Error: openssl is required for --sha3 verification" >&2
        exit 1
    fi
    ACTUAL_SHA3=$(openssl dgst -sha3-256 "${TMP}/sqlite.zip" | awk '{print $NF}')
    if [ "$ACTUAL_SHA3" != "$EXPECTED_SHA3" ]; then
        echo "Error: SHA3-256 mismatch" >&2
        echo "  expected: $EXPECTED_SHA3" >&2
        echo "  actual:   $ACTUAL_SHA3" >&2
        exit 1
    fi
    echo "    ok: $ACTUAL_SHA3"
else
    echo "==> Skipping SHA3 verification (no --sha3 given)"
fi

echo "==> Extracting"
unzip -q "${TMP}/sqlite.zip" -d "$TMP"
SRC_DIR="${TMP}/sqlite-amalgamation-${RELEASE}"
for f in sqlite3.c sqlite3.h sqlite3ext.h; do
    if [ ! -f "${SRC_DIR}/${f}" ]; then
        echo "Error: ${SRC_DIR}/${f} not found in archive" >&2
        exit 1
    fi
done

echo "==> Cross-checking SQLITE_VERSION in sqlite3.h"
HEADER_VERSION=$(grep -E '^#define SQLITE_VERSION[[:space:]]+' "${SRC_DIR}/sqlite3.h" \
                 | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
if [ "$HEADER_VERSION" != "$VERSION" ]; then
    echo "Warning: requested $VERSION but sqlite3.h reports $HEADER_VERSION" >&2
    echo "         continuing, but VERSION file will record the requested value." >&2
fi

echo "==> Installing into $VENDOR_DIR"
cp "${SRC_DIR}/sqlite3.c"    "${VENDOR_DIR}/sqlite3.c"
cp "${SRC_DIR}/sqlite3.h"    "${VENDOR_DIR}/sqlite3.h"
cp "${SRC_DIR}/sqlite3ext.h" "${VENDOR_DIR}/sqlite3ext.h"
printf '%s\n' "$VERSION" > "${VENDOR_DIR}/VERSION"

echo
echo "Done. Vendored SQLite ${VERSION} from ${URL}"
echo
echo "Next steps:"
echo "  cargo build && cargo test"
echo "  git add vendor/sqlite/ && git commit -m \"Deps | Bump SQLite to ${VERSION}\""
