# ─── AegisOS Dev Container ──────────────────────────────────────────
# Ubuntu 24.04 + Rust nightly + Kani verifier + QEMU aarch64
# For bare-metal AArch64 microkernel development & formal verification

FROM ubuntu:24.04

# Avoid interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# ─── System packages ────────────────────────────────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
    # Build essentials
    build-essential \
    curl \
    ca-certificates \
    git \
    pkg-config \
    # QEMU for AArch64 emulation
    qemu-system-arm \
    # Kani dependency: CBMC uses Java for some solvers (optional but recommended)
    # Debugging tools
    gdb-multiarch \
    # Misc
    sudo \
    && rm -rf /var/lib/apt/lists/*

# ─── Non-root user ──────────────────────────────────────────────────
ARG USERNAME=aegis
ARG USER_UID=1000
ARG USER_GID=1000

RUN if id -u ${USER_UID} >/dev/null 2>&1; then \
        OLD_USER=$(getent passwd ${USER_UID} | cut -d: -f1); \
        userdel -r "$OLD_USER" 2>/dev/null || true; \
    fi \
    && groupadd --force --gid ${USER_GID} ${USERNAME} \
    && useradd --uid ${USER_UID} --gid ${USER_GID} -m ${USERNAME} \
    && echo "${USERNAME} ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/${USERNAME} \
    && chmod 0440 /etc/sudoers.d/${USERNAME}

USER ${USERNAME}
WORKDIR /home/${USERNAME}

# ─── Rust toolchain (nightly + rust-src for -Zbuild-std) ────────────
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    --default-toolchain nightly \
    --component rust-src

ENV PATH="/home/${USERNAME}/.cargo/bin:${PATH}"

# ─── Kani verifier ─────────────────────────────────────────────────
# Install kani-verifier crate, then run setup to download CBMC + toolchain
RUN cargo install --locked kani-verifier \
    && cargo kani setup

# ─── Verify installation ───────────────────────────────────────────
RUN rustc --version \
    && cargo --version \
    && cargo kani --version \
    && qemu-system-aarch64 --version

# ─── Workspace ──────────────────────────────────────────────────────
WORKDIR /workspaces/aegis
