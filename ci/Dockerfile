# AegisOS Local CI — matches GitHub Actions ubuntu-latest environment
#
# Usage:
#   docker build -t aegis-ci ci/
#   docker run --rm -v ${PWD}:/workspace aegis-ci
#
# This runs both host unit tests and QEMU boot integration tests
# in the same environment as GitHub Actions CI.

FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
ENV CARGO_TERM_COLOR=always
ENV PATH="/root/.cargo/bin:${PATH}"

# ─── System dependencies (matches CI: Install QEMU step) ───────────
RUN apt-get update -qq && \
    apt-get install -y -qq --no-install-recommends \
        curl \
        ca-certificates \
        build-essential \
        qemu-system-arm \
    && rm -rf /var/lib/apt/lists/*

# ─── Rust nightly + rust-src (matches CI: Install Rust nightly) ─────
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain nightly --component rust-src && \
    rustup target add aarch64-unknown-none

# ─── Verify tools ──────────────────────────────────────────────────
RUN rustc --version && \
    cargo --version && \
    qemu-system-aarch64 --version | head -1

WORKDIR /workspace

# ─── Entrypoint: run full CI suite ─────────────────────────────────
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
