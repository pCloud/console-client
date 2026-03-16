Verify that the installation instructions for a given GitHub release result in a working `pcloud` binary.

## Inputs

- `$ARGUMENTS` — a release tag (e.g. `v3.0.0-preview-04`). If empty, use the latest tag from `git describe --tags --abbrev=0`.

## Procedure

### 1. Detect container runtime and QEMU support

Check for `docker` or `podman` on the host (in that order). Prefer `docker` — even when it is a Podman alias (`Emulate Docker CLI using podman`), it handles `--platform` and QEMU binfmt dispatch more reliably than bare `podman` in rootless mode. If neither is found, report the error and stop.

**Podman rootless gotchas** (apply when using bare `podman`, not the `docker` alias):

- Rootless Podman needs a writable `XDG_RUNTIME_DIR`. In sandboxed environments (Claude Code, CI containers) `/run/user/<uid>` is often read-only. Work around by setting both `TMPDIR` and `XDG_RUNTIME_DIR` to a writable temp directory (e.g. `/tmp/podman-verify`).
- After the temp directory is cleaned up between sessions, Podman state becomes stale. Run `podman system migrate` to recover before running containers.
- Cross-arch `podman run --platform linux/arm64` often fails with "Exec format error" even when binfmt is registered, because rootless Podman doesn't always propagate binfmt into the container's mount namespace. The `docker` alias does not have this problem.

Verify the runtime works by running a trivial container:

```bash
docker run --rm ubuntu:22.04 echo OK
```

Detect the host CPU architecture via `uname -m`. Determine which architectures need QEMU emulation (i.e. any arch that differs from the host). For cross-arch containers, verify QEMU user-static binfmt support is registered:

```bash
ls /proc/sys/fs/binfmt_misc/qemu-aarch64 /proc/sys/fs/binfmt_misc/qemu-arm 2>/dev/null
```

If QEMU binfmt entries are missing, attempt to register them. **Important:** use the fully qualified image name — short names fail with Podman's default registries config:

```bash
docker run --rm --privileged docker.io/multiarch/qemu-user-static --reset -p yes
```

If registration fails, skip cross-arch tests and note it in the report.

### 2. Fetch release assets list

Use the GitHub API to list all assets for the tag:

```bash
curl -sL "https://api.github.com/repos/pCloud/console-client/releases/tags/<tag>"
```

Parse out the actual asset filenames from `browser_download_url` fields. These are the ground truth — the URLs that actually work for downloads.

### 3. Fetch release body and compare URLs

From the same API response, extract the release body. Parse all `curl` download URLs from the body text (look for `https://github.com/.../releases/download/...` patterns). Compare each URL's filename against the actual asset list from step 2. Report any mismatches — these are broken download links (404s).

**Known issue:** `cargo-deb` generates `.deb` filenames with `~` (Debian pre-release convention), but GitHub replaces `~` with `.` in asset names. If you see tilde-vs-dot mismatches, that's the root cause.

### 4. Architecture mapping

Map release assets to container platforms:

| Asset suffix | `--platform` | Debian arch | RPM arch |
|---|---|---|---|
| `linux-amd64` / `amd64.deb` / `x86_64.rpm` | `linux/amd64` | `amd64` | `x86_64` |
| `linux-arm64` / `arm64.deb` / `aarch64.rpm` | `linux/arm64` | `arm64` | `aarch64` |
| `linux-armhf` / `armhf.deb` / `armv7.rpm` | `linux/arm/v7` | `armhf` | `armv7` |

### 5. Test matrix

For each distro/version and architecture below, run the installation tests in containers. Use the appropriate `--platform` flag for each architecture.

**x86_64 (linux/amd64):**

| Test | Image | Install method |
|------|-------|---------------|
| Ubuntu 22.04 binary | `ubuntu:22.04` | Binary download + manual deps |
| Ubuntu 22.04 .deb | `ubuntu:22.04` | .deb package via `dpkg -i` + `apt-get -f install` |
| Ubuntu 24.04 binary | `ubuntu:24.04` | Binary download + manual deps |
| Ubuntu 24.04 .deb | `ubuntu:24.04` | .deb package via `dpkg -i` + `apt-get -f install` |
| Debian 12 binary | `debian:12` | Binary download + manual deps |
| Debian 12 .deb | `debian:12` | .deb package via `dpkg -i` + `apt-get -f install` |
| Fedora 39 binary | `fedora:39` | Binary download + manual deps |
| Fedora 39 .rpm | `fedora:39` | .rpm package via `rpm -i` |

**ARM 64-bit (linux/arm64):**

| Test | Image | Install method |
|------|-------|---------------|
| Ubuntu 22.04 binary | `ubuntu:22.04` | Binary download + manual deps |
| Ubuntu 22.04 .deb | `ubuntu:22.04` | .deb package via `dpkg -i` + `apt-get -f install` |
| Debian 12 binary | `debian:12` | Binary download + manual deps |
| Debian 12 .deb | `debian:12` | .deb package via `dpkg -i` + `apt-get -f install` |
| Fedora 39 binary | `fedora:39` | Binary download + manual deps |
| Fedora 39 .rpm | `fedora:39` | .rpm package via `rpm -i` |

**ARM 32-bit (linux/arm/v7):**

| Test | Image | Install method |
|------|-------|---------------|
| Ubuntu 22.04 binary | `ubuntu:22.04` | Binary download + manual deps |
| Ubuntu 22.04 .deb | `ubuntu:22.04` | .deb package via `dpkg -i` + `apt-get -f install` |
| Debian 12 binary | `debian:12` | Binary download + manual deps |
| Debian 12 .deb | `debian:12` | .deb package via `dpkg -i` + `apt-get -f install` |

> **Note:** Fedora dropped 32-bit ARM (armhfp) entirely after Fedora 36. There are no Fedora 37+ armv7 container images, so Fedora ARM 32-bit tests are skipped. Mark them as SKIP in the report if an armv7 RPM asset exists.

Tests for the host's native architecture run first (fastest). Cross-arch tests run after, using QEMU emulation — expect these to be significantly slower. Run independent tests in parallel where possible, but limit parallelism for QEMU tests to avoid overwhelming the host (max 2 concurrent cross-arch containers).

If QEMU is not available, skip cross-arch tests and note which architectures were skipped.

### 6. Per-container test steps

Use a generous timeout for QEMU-emulated containers (5+ minutes per container) since emulated execution is much slower than native.

**Distro-specific notes:**
- **Debian/Ubuntu** minimal images have no `curl` or `ca-certificates` — install them first.
- **Fedora** images already include `curl` — no need to install it.
- **RPM installs:** `rpm -i` will fail if dependencies are missing. Install deps with `dnf install -y` **before** running `rpm -i`, so the install step tests the package itself, not dependency resolution.

For **binary installs**:
1. Install `curl` (and `ca-certificates` on Debian/Ubuntu) if not already present
2. Download the binary using the **actual asset URL** from step 2 (not the possibly-broken URL from the release body)
3. `chmod +x` and move to `/usr/local/bin/`
4. Install runtime dependencies using the exact commands from the release notes (adapted for root — drop `sudo`)
5. Run `pcloud --version` and `pcloud --help`
6. Check `ldd /usr/local/bin/pcloud` for any `not found` libraries

For **package installs** (`.deb` / `.rpm`):
1. Install `curl` (and `ca-certificates` on Debian/Ubuntu) if not already present
2. Download the package using the **actual asset URL** from step 2
3. For `.deb`: install with `dpkg -i` (then `apt-get install -f -y` to resolve deps)
4. For `.rpm`: install deps with `dnf install -y` first, then `rpm -i` the package
5. Verify `which pcloud` resolves
6. Run `pcloud --version` and `pcloud --help`
7. Check `ldd $(which pcloud)` for any `not found` libraries

**Filtering output:** Each container test produces a lot of noise from package managers. Filter output to show only these key lines to keep results readable:

```
DOWNLOAD: OK/FAIL
VERSION: pcloud X.Y.Z
HELP: OK/FAIL
LDD: OK / LDD_MISSING: <lib>
```

Use `grep -E "^(DOWNLOAD|VERSION|HELP|LDD)"` to extract these from the container output, then append `RESULT: PASS` or `RESULT: FAIL` based on the exit code.

### 7. Report

Produce a summary table grouped by architecture:

```
Host arch: x86_64 | QEMU: arm64 OK, arm/v7 OK

### Broken download URLs in release notes
| URL in body | Issue | Correct URL |
|---|---|---|
| ...~preview... | tilde mangled to dot | ...preview... |

### x86_64 (native)
| Test                  | Download | Install | Runs | Missing libs |
|-----------------------|----------|---------|------|--------------|
| Ubuntu 22.04 binary   | OK       | OK      | OK   | (none)       |
| Ubuntu 22.04 .deb     | OK       | OK      | OK   | (none)       |
| ...                   | ...      | ...     | ...  | ...          |

### ARM 64-bit (QEMU)
| Test                  | Download | Install | Runs | Missing libs |
|-----------------------|----------|---------|------|--------------|
| Ubuntu 22.04 binary   | OK       | OK      | OK   | (none)       |
| ...                   | ...      | ...     | ...  | ...          |

### ARM 32-bit (QEMU)
| Test                  | Download | Install | Runs | Missing libs |
|-----------------------|----------|---------|------|--------------|
| Ubuntu 22.04 binary   | OK       | OK      | OK   | (none)       |
| ...                   | ...      | ...     | ...  | ...          |
| Fedora 39 binary      | SKIP     | SKIP    | SKIP | Fedora dropped armhfp |
| Fedora 39 .rpm        | SKIP     | SKIP    | SKIP | Fedora dropped armhfp |
```

Flag any failures with details. If download URLs were broken (step 3), include the correct URLs from the asset list. For QEMU tests, note if a failure might be QEMU-specific (e.g. segfault under emulation) vs a genuine packaging issue.