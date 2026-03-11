FROM ubuntu:22.04

ARG RUST_VERSION=1.94.0

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH="/usr/local/cargo/bin:${PATH}"

# Configure multi-arch package sources based on host architecture.
# On amd64: add arm64 + armhf foreign arches via ports.ubuntu.com
# On arm64: add armhf foreign arch only (arm64 packages are native)
RUN set -eux; \
    NATIVE_ARCH=$(dpkg --print-architecture); \
    if [ "$NATIVE_ARCH" = "amd64" ]; then \
      dpkg --add-architecture arm64; \
      dpkg --add-architecture armhf; \
      . /etc/os-release; \
      echo "deb [arch=arm64,armhf] http://ports.ubuntu.com/ubuntu-ports ${VERSION_CODENAME} main restricted universe multiverse" > /etc/apt/sources.list.d/arm-ports.list; \
      echo "deb [arch=arm64,armhf] http://ports.ubuntu.com/ubuntu-ports ${VERSION_CODENAME}-updates main restricted universe multiverse" >> /etc/apt/sources.list.d/arm-ports.list; \
      echo "deb [arch=arm64,armhf] http://ports.ubuntu.com/ubuntu-ports ${VERSION_CODENAME}-security main restricted universe multiverse" >> /etc/apt/sources.list.d/arm-ports.list; \
      if [ -f /etc/apt/sources.list.d/ubuntu.sources ]; then \
        sed -i '/^Architectures:/d; /^Types:/a Architectures: amd64' /etc/apt/sources.list.d/ubuntu.sources; \
      elif [ -f /etc/apt/sources.list ]; then \
        sed -i 's/^deb http/deb [arch=amd64] http/' /etc/apt/sources.list; \
      fi; \
    elif [ "$NATIVE_ARCH" = "arm64" ]; then \
      dpkg --add-architecture armhf; \
    fi

# System dependencies (native)
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    ca-certificates \
    libfuse-dev \
    libsqlite3-dev \
    libssl-dev \
    zlib1g-dev \
    libclang-dev \
    libudev-dev \
    pkg-config \
    git \
    && rm -rf /var/lib/apt/lists/*

# Cross-compilers (architecture-dependent)
# amd64: cross-compile to both arm64 and armhf
# arm64: cross-compile to armhf only (arm64 is native)
RUN set -eux; \
    NATIVE_ARCH=$(dpkg --print-architecture); \
    apt-get update; \
    if [ "$NATIVE_ARCH" = "amd64" ]; then \
      apt-get install -y --no-install-recommends \
        gcc-aarch64-linux-gnu g++-aarch64-linux-gnu \
        gcc-arm-linux-gnueabihf g++-arm-linux-gnueabihf; \
    elif [ "$NATIVE_ARCH" = "arm64" ]; then \
      apt-get install -y --no-install-recommends \
        gcc-arm-linux-gnueabihf g++-arm-linux-gnueabihf; \
    fi; \
    rm -rf /var/lib/apt/lists/*

# Multi-arch dev libraries for cross-compilation targets
RUN set -eux; \
    NATIVE_ARCH=$(dpkg --print-architecture); \
    apt-get update; \
    if [ "$NATIVE_ARCH" = "amd64" ]; then \
      apt-get install -y --no-install-recommends \
        libfuse-dev:arm64 libsqlite3-dev:arm64 libssl-dev:arm64 zlib1g-dev:arm64 libudev-dev:arm64 \
        libfuse-dev:armhf libsqlite3-dev:armhf libssl-dev:armhf zlib1g-dev:armhf libudev-dev:armhf; \
    elif [ "$NATIVE_ARCH" = "arm64" ]; then \
      apt-get install -y --no-install-recommends \
        libfuse-dev:armhf libsqlite3-dev:armhf libssl-dev:armhf zlib1g-dev:armhf libudev-dev:armhf; \
    fi; \
    rm -rf /var/lib/apt/lists/*

# Rust stable with cross-compilation targets
RUN set -eux; \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain "${RUST_VERSION}" --profile minimal; \
    rustup component add clippy rustfmt; \
    NATIVE_ARCH=$(dpkg --print-architecture); \
    if [ "$NATIVE_ARCH" = "amd64" ]; then \
      rustup target add aarch64-unknown-linux-gnu armv7-unknown-linux-gnueabihf; \
    elif [ "$NATIVE_ARCH" = "arm64" ]; then \
      rustup target add armv7-unknown-linux-gnueabihf; \
    fi

# Packaging tools
RUN cargo install cargo-deb cargo-generate-rpm