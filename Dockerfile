# syntax=docker/dockerfile:1.7
ARG ACCELERATOR=vulkan
ARG KASM_IMAGE=kasmweb/core-ubuntu-noble:1.17.0
ARG CUDA_VERSION=13.1.0
ARG RUST_VERSION=1.98.0
ARG BUN_VERSION=1.4.3
ARG NODE_VERSION=24

FROM oven/bun:${BUN_VERSION} AS bun
FROM node:${NODE_VERSION}-bookworm-slim AS node
FROM rust:${RUST_VERSION}-bookworm AS rust
FROM ubuntu:24.04 AS build-cpu
FROM build-cpu AS build-vulkan
FROM nvidia/cuda:${CUDA_VERSION}-devel-ubuntu24.04 AS build-cuda

FROM build-${ACCELERATOR} AS builder
ARG ACCELERATOR
ARG RUST_VERSION
ARG BUILD_JOBS=2
ARG CUDA_ARCHITECTURES=75-virtual;80-virtual;86-real;89-real;90-virtual;120a-real
ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    RUSTUP_TOOLCHAIN=${RUST_VERSION} \
    PATH=/usr/local/cargo/bin:${PATH} \
    CARGO_BUILD_JOBS=${BUILD_JOBS} \
    CMAKE_BUILD_PARALLEL_LEVEL=${BUILD_JOBS} \
    CARGO_INCREMENTAL=0 \
    CARGO_NET_GIT_FETCH_WITH_CLI=true \
    GGML_NATIVE=OFF \
    CMAKE_CUDA_ARCHITECTURES=${CUDA_ARCHITECTURES}
COPY --from=bun /usr/local/bin/bun /usr/local/bin/bun
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=rust /usr/local/cargo /usr/local/cargo
COPY --from=rust /usr/local/rustup /usr/local/rustup
COPY docker/install-build-deps.sh scripts/cuda-gcc-version /tmp/lettuceai-build/
RUN bash /tmp/lettuceai-build/install-build-deps.sh "$ACCELERATOR"
WORKDIR /build
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target,id=lettuceai-${ACCELERATOR} \
    bash docker/build-source.sh "$ACCELERATOR"

FROM nvidia/cuda:${CUDA_VERSION}-runtime-ubuntu24.04 AS cuda-runtime
FROM ${KASM_IMAGE} AS runtime-common
USER root
ENV HOME=/home/kasm-default-profile \
    LETTUCE_AUTOSTART=1 \
    LETTUCE_KEYRING_PROVIDER=auto \
    LETTUCE_KEYRING_TIMEOUT=120
WORKDIR /home/kasm-default-profile
RUN export DEBIAN_FRONTEND=noninteractive && apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates dbus dbus-x11 libsecret-tools zenity python3 \
      libwebkit2gtk-4.1-0 libgtk-3-0t64 libsoup-3.0-0 libasound2t64 \
      librsvg2-2 libvulkan1 mesa-vulkan-drivers vulkan-tools \
      xdg-utils util-linux \
    && rm -rf /var/lib/apt/lists/*
COPY --chmod=755 docker/install-keyring.sh /usr/local/lib/lettuceai/install-keyring
ARG KEYRING_PROVIDER=gnome
RUN /usr/local/lib/lettuceai/install-keyring "$KEYRING_PROVIDER"
COPY --chmod=755 docker/launch.sh /usr/local/bin/lettuceai
COPY --chmod=755 docker/keyring-session.sh /usr/local/lib/lettuceai/keyring-session
COPY --chmod=755 docker/autostart.sh /usr/local/lib/lettuceai/autostart
COPY --chmod=755 docker/container-state.py /usr/local/bin/lettuceai-container-status
COPY docker/write-build-info.py /usr/local/lib/lettuceai/write-build-info.py
COPY --chmod=755 docker/install-binary.sh /usr/local/lib/lettuceai/install-binary
COPY docker/lettuceai.desktop /tmp/lettuceai.desktop
COPY apps/tauri/icons/128x128.png /usr/share/icons/lettuceai.png
RUN install -Dm755 /tmp/lettuceai.desktop "$HOME/Desktop/lettuceai.desktop" \
    && install -d "$HOME/.config/autostart" /opt/lettuceai/bin \
    && printf '%s\n' '[Desktop Entry]' 'Type=Application' 'Name=LettuceAI' \
         'Exec=/usr/local/lib/lettuceai/autostart' \
         > "$HOME/.config/autostart/lettuceai.desktop" \
    && chown 1000:0 "$HOME" \
    && /dockerstartup/set_user_permission.sh "$HOME" \
    && install -d -o 1000 -g 0 /home/kasm-user \
    && rm /tmp/lettuceai.desktop

FROM runtime-common AS runtime-cpu
FROM runtime-common AS runtime-vulkan
FROM runtime-common AS runtime-cuda
COPY --from=cuda-runtime /usr/local/cuda/targets/x86_64-linux/lib/ /usr/local/lib/lettuceai-cuda/
RUN printf '%s\n' /usr/local/lib/lettuceai-cuda > /etc/ld.so.conf.d/lettuceai-cuda.conf && ldconfig

FROM runtime-${ACCELERATOR} AS runtime
ENV HOME=/home/kasm-user
WORKDIR /home/kasm-user

FROM runtime AS source
ARG APP_VERSION=auto
ARG SOURCE_REVISION=unknown
ARG ACCELERATOR
COPY --from=builder --chmod=755 /out/lettuceai /opt/lettuceai/bin/lettuceai
COPY --from=builder /out/version /opt/lettuceai/package-version
RUN python3 /usr/local/lib/lettuceai/write-build-info.py source "$APP_VERSION" "$SOURCE_REVISION" "$ACCELERATOR"
USER 1000

FROM runtime AS prebuilt
ARG BINARY_PATH=docker/artifacts/lettuceai.deb
ARG APP_VERSION=auto
ARG SOURCE_REVISION=unknown
ARG ACCELERATOR
COPY ${BINARY_PATH} /tmp/lettuceai-package
RUN /usr/local/lib/lettuceai/install-binary /tmp/lettuceai-package \
    && rm /tmp/lettuceai-package \
    && chown -R 1000:0 /home/kasm-user
RUN python3 /usr/local/lib/lettuceai/write-build-info.py prebuilt "$APP_VERSION" "$SOURCE_REVISION" "$ACCELERATOR"
USER 1000
