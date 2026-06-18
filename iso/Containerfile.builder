# syntax=docker/dockerfile:1.6
#
# Jarvis OS BUILDER base image (ADR 0021 P2).
#
# This is the heavy, slow-changing half of the ISO build: the Fedora
# toolchain + Qt6 devel headers + a from-source whisper-cli + the
# piper TTS tarball + numbat-cli. None of it depends on OUR source —
# it only changes when we bump a toolchain or a pinned tool version.
#
# Build + push it ONCE (CI: .github/workflows/build-builder.yml, or
# locally) to ghcr.io/<org>/jarvis-builder. The main iso/Containerfile
# then does `FROM ghcr.io/<org>/jarvis-builder` and compiles only our
# crates — dropping ~8-12 min (dnf + whisper + piper + numbat) off
# every ISO build.
#
# Rebuild this image when:
#   - bumping WHISPER_VERSION / PIPER_VERSION / NUMBAT_VERSION
#   - adding a system devel dependency
#   - moving to a new Fedora / Qt base

FROM registry.fedoraproject.org/fedora:44

# Devel deps for every Jarvis crate + the Qt shell. We ALWAYS install
# the compositor's Smithay deps here too (libseat / libinput / mesa /
# libdrm): the builder image is built rarely, so paying for them once
# removes the BUILD_COMPOSITOR branch from the per-ISO build and lets
# either mode use the same base.
RUN dnf -y install \
        rust cargo \
        cmake ninja-build clang gcc-c++ make \
        qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtwayland-devel \
        qt6-qtbase-private-devel qt6-qtdeclarative-private-devel \
        layer-shell-qt-devel \
        sqlite-devel \
        dbus-devel pkgconfig \
        wayland-devel libxkbcommon-devel \
        alsa-lib-devel \
        libseat-devel libinput-devel \
        mesa-libgbm-devel mesa-libEGL-devel \
        libdrm-devel systemd-devel \
        git curl \
    && dnf clean all

# whisper.cpp — static whisper-cli + the multilingual base model.
# (Same build as the old inline step; see iso/Containerfile history.)
ARG WHISPER_VERSION=v1.7.4
RUN git clone --depth 1 --branch ${WHISPER_VERSION} \
        https://github.com/ggml-org/whisper.cpp.git /src/whisper.cpp \
    && cmake -S /src/whisper.cpp -B /build/whisper -G Ninja \
        -DCMAKE_BUILD_TYPE=Release \
        -DBUILD_SHARED_LIBS=OFF \
        -DWHISPER_BUILD_EXAMPLES=ON \
    && cmake --build /build/whisper -j --target whisper-cli \
    && mkdir -p /out/whisper /out/whisper-models \
    && cp /build/whisper/bin/whisper-cli /out/whisper/whisper-cli \
    && bash /src/whisper.cpp/models/download-ggml-model.sh base \
    && cp /src/whisper.cpp/models/ggml-base.bin /out/whisper-models/ggml-base.bin \
    && rm -rf /src/whisper.cpp /build/whisper

# piper (TTS) — precompiled tarball + the voice model.
#
# Voice: en_US-amy-medium (female). The pt_BR piper catalog (faber/
# edresson/jeff/cadu) is all male, and the user wants a female voice
# for Lilith. Trade-off: Amy reads the pt-BR replies with English
# phonemes, so PT words get an anglicised pronunciation — accepted
# in exchange for a female voice. PIPER_VOICE_PATH is the HF dir;
# PIPER_VOICE is the file stem. Both must match the daemon's
# DEFAULT_MODEL in system/voice/src/tts.rs.
ARG PIPER_VERSION=2023.11.14-2
ARG PIPER_VOICE=en_US-amy-medium
ARG PIPER_VOICE_PATH=en/en_US/amy/medium
RUN mkdir -p /out/piper /out/piper-voices \
    && curl -fsSL -o /tmp/piper.tar.gz \
        https://github.com/rhasspy/piper/releases/download/${PIPER_VERSION}/piper_linux_x86_64.tar.gz \
    && tar -xzf /tmp/piper.tar.gz -C /out/piper --strip-components=1 \
    && rm /tmp/piper.tar.gz \
    && curl -fsSL -o /out/piper-voices/${PIPER_VOICE}.onnx \
        https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/${PIPER_VOICE_PATH}/${PIPER_VOICE}.onnx \
    && curl -fsSL -o /out/piper-voices/${PIPER_VOICE}.onnx.json \
        https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/${PIPER_VOICE_PATH}/${PIPER_VOICE}.onnx.json

# numbat-cli — Lilith's calculator backend.
ARG NUMBAT_VERSION=1.16.0
RUN CARGO_TARGET_DIR=/build/numbat cargo install \
        --root /out/numbat \
        --locked \
        --version ${NUMBAT_VERSION} \
        numbat-cli \
    && rm -rf /build/numbat

# ── Warm the Rust dependency cache (ADR 0021 P2, extended) ──────────────
# This is the single biggest cost of every per-merge main image build:
# recompiling OUR crates' third-party dependency tree (tokio, zbus,
# reqwest, rusqlite, rustfft, …) from scratch. The main build's cargo
# cache mounts DON'T persist on the ephemeral CI runner, so each run
# started cold and burned ~10 min before touching our code.
#
# Compile that tree ONCE here, into the exact target dir + cargo home the
# main build uses (/build/rust, /root/.cargo). This base then ships warm:
# the per-merge build reuses the compiled deps and only rebuilds our own
# (changed) crates — minutes instead of ten. The source is dropped right
# after; only the warm target + registry are kept.
#
# Keyed on the copied source, so it re-warms when Cargo.lock changes
# (build-builder.yml triggers on it). The -p list MUST mirror the main
# iso/Containerfile build list so exactly those deps get warmed.
COPY . /warm
RUN cd /warm \
    && CARGO_TARGET_DIR=/build/rust cargo build --release \
        -p jarvis-action-bus \
        -p jarvis-permission \
        -p jarvis-settings \
        -p jarvis-notifications \
        -p jarvis-lilith \
        -p jarvis-updater \
        -p jarvis-voice \
        -p jarvis-voice-ctl \
        -p jarvis-app \
        -p sdk-hello \
        -p jarvis-compat \
        -p jarvis-lock \
        -p jarvis-lock-ctl \
        -p jarvis-voiceprint-ctl \
        -p pam-jarvis \
    && rm -rf /warm

LABEL org.opencontainers.image.title="Jarvis OS builder base"
LABEL org.opencontainers.image.description="Fedora 42 + Rust/Qt6 toolchain + whisper-cli + piper + numbat, prebuilt for fast ISO builds"
