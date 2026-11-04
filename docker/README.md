# LettuceAI in a Kasm desktop

This image adds LettuceAI to a Kasm Ubuntu Noble desktop. It uses the base image's normal Kasm entrypoint, desktop and VNC server. There is no second desktop server or host display mount. The default base matches Kasm 1.17.0; change `KASM_IMAGE` to match your deployment or to extend your own Noble-based Kasm image. Other software in a custom base is retained.

The Dockerfile has two final targets:

| Target | Input | Build tools in the final image |
| --- | --- | --- |
| `prebuilt` (default) | A Linux amd64 `.deb` or ELF executable | None added |
| `source` | This checkout, including its locked dependencies | None added |

Both targets support `ACCELERATOR=cpu`, `vulkan` or `cuda`. The accelerator must match a prebuilt executable; choosing CUDA does not add CUDA support to a CPU binary. CUDA runtime libraries are included only in the CUDA image. Source builds select the corresponding Cargo features and use the repository's pinned CUDA host compiler. No GPU is needed while building the image.

## Run a ready-made package

From the repository root:

```bash
cp docker/env.example docker/.env
mkdir -p docker/artifacts
cp /path/to/LettuceAI_3.0.0_amd64.deb docker/artifacts/lettuceai.deb
```

Set `VNC_PW` in `docker/.env`, then start one of these services:

```bash
# NVIDIA CUDA
docker compose --env-file docker/.env -f docker/compose.yaml --profile cuda up --build cuda

# AMD/Intel Vulkan: use the host device groups
export RENDER_GID=$(stat -c '%g' /dev/dri/renderD128)
export VIDEO_GID=$(stat -c '%g' /dev/dri/card0)
docker compose --env-file docker/.env -f docker/compose.yaml --profile vulkan up --build vulkan

# NVIDIA Vulkan
docker compose --env-file docker/.env -f docker/compose.yaml --profile vulkan-nvidia up --build vulkan-nvidia

# CPU, without GPU device access
docker compose --env-file docker/.env -f docker/compose.yaml --profile cpu up --build cpu
```

Use the render/card paths belonging to your GPU if they differ. NVIDIA profiles require the host NVIDIA driver and NVIDIA Container Toolkit configured for Docker; `/dev/dri` profiles require the host's AMD/Intel driver. The image installs Mesa Vulkan drivers and the Vulkan loader, while NVIDIA's runtime supplies driver libraries matching the host. The profiles expose devices at container creation, because a Dockerfile cannot grant GPU access by itself.

Open `https://localhost:6901` with user `kasm_user` and your `VNC_PW`. For a remote Docker host, use an SSH tunnel or deliberately change `LETTUCE_BIND_ADDRESS` in `docker/.env`. A standalone Kasm desktop uses its usual self-signed certificate. Ports, persistent storage and GPU reservations in this Compose file are for standalone deployment; a Kasm Workspaces server manages them itself.

For a bare executable, set `BINARY_PATH` to its path relative to the repository root, or copy it to the default artifact path. The installer detects its contents. AppImages and archives must be extracted first; a raw binary must target Ubuntu 24.04-compatible Linux amd64 and use the selected accelerator's runtime libraries. `.deb` packages are preferable when the build also carries resources or additional libraries. Old packages whose executable is `lettuce-tauri` are supported.

## Build from source

```bash
BUILD_TARGET=source docker compose --env-file docker/.env -f docker/compose.yaml --profile cuda up --build cuda
```

The other profiles work the same way. To build an image for Kasm Workspaces without starting a standalone desktop:

```bash
docker build --target source \
  --build-arg ACCELERATOR=vulkan \
  --build-arg KASM_IMAGE=kasmweb/core-ubuntu-noble:1.17.0 \
  -t lettuceai:kasm-vulkan .

docker build --target prebuilt \
  --build-arg ACCELERATOR=cuda \
  --build-arg BINARY_PATH=docker/artifacts/lettuceai.deb \
  -t lettuceai:kasm-cuda .
```

BuildKit caches Cargo registry, Git sources and target output by accelerator. `BUILD_JOBS` defaults to 2. `CUDA_ARCHITECTURES` can narrow the CUDA build to your GPUs, for example `--build-arg CUDA_ARCHITECTURES=89-real`. `CUDA_VERSION`, `RUST_VERSION`, `BUN_VERSION` and `NODE_VERSION` are build arguments; their defaults follow the current release tooling. Bun, Node and Rust are copied from their official images into the builder only. Node 24 also handles frontend package executables whose shebang invokes `node`. Vulkan build dependencies include `glslc` and SPIRV-Headers. The source target runs Tauri directly rather than the local CUDA wrapper that requires `nvidia-smi` during compilation.

## Keyring and persistence

The launcher uses the standard `org.freedesktop.secrets` interface. By default it reuses Kasm's working desktop D-Bus session and its configured Secret Service, including KWallet or another compatible implementation. If no usable session bus exists, it creates one. D-Bus activates the configured provider and that provider displays its own creation/unlock prompt. Before the app starts, the launcher verifies that a temporary credential can be stored, read and removed. A locked or unwritable store stops app startup with an error.

Choose the provider installed in the image with the build argument `KEYRING_PROVIDER=gnome` (default), `kwallet` or `external`. The last option retains the Secret Service already configured in a custom base image. The KWallet image installs Noble's KWallet 5, which implements Secret Service. This does not change LettuceAI's backend or create a plaintext fallback. Changing providers does not transfer existing credentials between wallets.

`LETTUCE_KEYRING_PROVIDER=auto` is the default runtime policy. `external` requires the desktop's configured Secret Service, `kwallet` can explicitly start an installed KWallet service, and `gnome` starts an isolated GNOME session with an explicit password prompt or password file. Unattended password-file unlock is specific to this last mode; KWallet and other providers use their native unlock mechanisms. No provider is silently substituted for an existing desktop service in auto mode.

The `lettuceai-home` volume retains the whole Kasm user home, including application data, keyring/wallet files and settings. There is no application-data symlink. When migrating an existing desktop, retain both its application data and credential-store files (`~/.local/share/keyrings` for GNOME, or the KWallet data and configuration); copying only the database does not copy OS-stored API keys. Do not use `docker compose down -v` when keeping these credentials. Run one profile at a time against the shared home.

Set `LETTUCE_AUTOSTART=0` to start the app manually from its desktop icon or `lettuceai` in a terminal.

For unattended GNOME unlock, keep `KEYRING_PROVIDER=gnome` and mount an external password file readable by Kasm's UID 1000:

```bash
export LETTUCE_KEYRING_SECRET=/absolute/path/to/keyring-password
docker compose --env-file docker/.env \
  -f docker/compose.yaml -f docker/compose.keyring.yaml \
  --profile cuda up --build cuda
```

The password file contains one password and is mounted read-only. Keep it outside the image and repository; it must use the same password as the persistent keyring. The override explicitly selects the GNOME unlock mechanism. The main Compose file sets a memlock allowance and `IPC_LOCK` without using `privileged: true`.

## Multiple GPUs

CUDA and NVIDIA Vulkan expose all NVIDIA GPUs by default. AMD/Intel Vulkan passes the complete `/dev/dri` directory, including every render node. There is no fixed GPU-count limit in these profiles. The backend/model configuration still controls how work is distributed; exposing multiple devices does not force every operation to split across them.

To select a subset of NVIDIA GPUs, copy `docker/compose.gpus.example.yaml` to your own override and edit its `device_ids` list. IDs can be indices or the UUIDs printed by `nvidia-smi -L`. The override replaces the default `count: all` reservation, because Docker forbids combining `count` and `device_ids` in one request. It requires Docker Compose 2.24.4 or newer.

```bash
export LETTUCE_GPU_CONFIG="$PWD/docker/my-gpus.yaml"
cp docker/compose.gpus.example.yaml "$LETTUCE_GPU_CONFIG"
# Edit device_ids, then include the override on first startup:
docker compose --env-file docker/.env -f docker/compose.yaml \
  -f "$LETTUCE_GPU_CONFIG" --profile cuda up --build cuda
```

Keep `LETTUCE_GPU_CONFIG` exported when using the management helper so updates retain your selected devices.

## Updates, status and rollback

```bash
bash docker/manage.sh status cuda
```

Status shows the running image ID, application version, source revision when supplied, accelerator, binary SHA-256, application PID and lifecycle state. States distinguish `starting`, `waiting_for_keyring`, `verifying_keyring`, `running`, `stopped` and `failed`. A Kasm desktop can be healthy while the app is waiting for its wallet password. Status verifies the process identity, PID namespace and container init start time, so an old record in the persistent home is not presented as a running app in a new container. Read it inside Kasm with `lettuceai-container-status`.

To update a prebuilt app, replace the package at `BINARY_PATH` with the new release, then:

```bash
bash docker/manage.sh update cuda
```

For a source build, update your checkout first, then:

```bash
BUILD_TARGET=source SOURCE_REVISION=$(git rev-parse HEAD) bash docker/manage.sh update vulkan
```

The helper builds the new image before recreating the running container. A failed build leaves the existing container running. The previous image is tagged `lettuceai:rollback-<profile>`, then Compose recreates only that service and keeps the persistent home and password-file mount. Kasm's normal health check is awaited; the app's separate status is printed afterward. If using the password-file override, keep `LETTUCE_KEYRING_SECRET` exported for update/status commands too. The helper does not fetch a release, pull your Git branch, modify a running container or delete volumes.

```bash
bash docker/manage.sh rollback cuda
```

Rollback switches to the previous image and preserves the same home. It rolls back application binaries, not database schema migrations; retain an app backup before upgrading across incompatible database versions. `.deb` versions are read from the package; source versions come from `package.json`. For a raw ELF, supply `APP_VERSION` if you want a release version instead of `unknown`.

In an existing Kasm Workspaces deployment, publish the rebuilt image to your registry, update the Workspace image reference and start a new session with the same persistent profile. Kasm manages replacement of those sessions; the Compose helper is for standalone deployment. Prefer versioned image tags when distributing updates.

## Validation

The shell scripts can be syntax checked without compiling Rust:

```bash
for script in docker/*.sh docker/tests/*.sh; do bash -n "$script" || exit; done
docker build --check --target prebuilt --build-arg ACCELERATOR=vulkan .
python3 -m unittest discover -s docker/tests -p 'test_*.py' -v
```

A small integration image exercises a real Secret Service, encrypted persistence, wrong-password rejection, file arguments, lifecycle status and reuse of an already-running service. It does not build LettuceAI or run Cargo:

```bash
docker build -f docker/tests/Dockerfile -t lettuceai-keyring-test .
docker run --rm --cap-add IPC_LOCK --ulimit memlock=-1:-1 lettuceai-keyring-test
bash docker/tests/persistence.sh
```

## Add the image to existing Kasm Workspaces

Build or push the selected image, then use it as the Workspace's Docker image. Keep the normal Kasm entrypoint. Enable its persistent profile at `/home/kasm-user`.

For NVIDIA, allocate a GPU to the Workspace and set `NVIDIA_VISIBLE_DEVICES=all` and `NVIDIA_DRIVER_CAPABILITIES=compute,utility,graphics,display`. For AMD/Intel Vulkan, expose `/dev/dri` and add the host render/video group IDs through the Workspace's Docker run configuration. Apply the `IPC_LOCK` capability and unlimited memlock allowance there too. To unlock unattended, mount the password file and set `LETTUCE_KEYRING_PASSWORD_FILE` to its container path; otherwise the user gets the password prompt in their Kasm desktop.

The same launcher can be installed into a custom Kasm image. It does not replace Kasm startup scripts, configure passwordless sudo or install unrelated desktop applications.

References: [Kasm custom images](https://docs.kasm.com/docs/1.18.0/how-to/building_images), [Docker GPU access](https://docs.docker.com/compose/how-tos/gpu-support/), [NVIDIA runtime capabilities](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/docker-specialized.html), [GNOME Keyring options](https://manpages.debian.org/unstable/gnome-keyring/gnome-keyring-daemon.1.en.html).
