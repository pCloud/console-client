FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH="/usr/local/cargo/bin:${PATH}"

# System dependencies
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

# Rust stable
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain stable --profile minimal

# Packaging tools
RUN cargo install cargo-deb cargo-generate-rpm
