# Building and releasing LettuceAI

Run **Actions → Build and release → Run workflow** from the branch to build.
By default, Android and desktop installers remain in Actions artifacts for 30 days.
Enable `publish` to publish them together in one GitHub release.

| Input | Behavior |
| --- | --- |
| `channel: dev` | Separate dev application; publishing creates a GitHub prerelease and a unique `dev-…` tag. |
| `channel: stable` | Stable application identity; publishing uses `v<version>`. |
| `version` | Stable version such as `3.0.0`; blank uses the checked-in version. Dev versions are automatic. |
| `cuda` | Include the additional Windows and Linux CUDA packages. |
| `publish` | Off by default: retain Actions artifacts without creating a release or publishing AUR packages. Enable to publish a GitHub release. |
| `aur` | Checkbox to update AUR packages after publishing the GitHub release; on by default. |
| `previous_tag` | Optional ancestor tag to use as the start of the commit list. |

For example, select `channel: stable`, enter `version: 3.0.0`, and enable `publish` to build version
`3.0.0` and publish `v3.0.0`. Pushing a `v3.0.0` tag also builds that version, with
CUDA included. A named prerelease such as `3.0.0-beta.1` is marked as a GitHub
prerelease while retaining the stable application identity.

CI applies the selected version to the Rust workspace, frontend packages, and Tauri
bundle configuration. The workspace lockfile is updated offline without upgrading
dependencies. Version overrides affect only the CI checkout; source files are not
committed back. The checked-in version files must still agree with one another.
Published releases are never overwritten.

## Builds without a release

Leave `publish` unchecked when running the workflow. Download the installers from
the run's **Artifacts** section: `release-package-<desktop target>` and
`release-package-android`. Checksums are included in each package artifact;
`release-metadata` contains the version, source commit, and change list.
Artifacts are retained for 30 days. No GitHub release or tag is created, and AUR
publication is skipped even if `aur` is checked. Pushing a `v*` tag still publishes
a release automatically.

## Dev and stable installations

Dev builds preserve the identity used by the workflows on `main`:

- Product name: `lettuceai-dev`; desktop window title: `LettuceAI (Dev)`.
- Identifier and Android application ID: `dev.lettuceai.app`.
- Version: `0.1.<workflow run number>-dev.<short commit SHA>`.

Stable builds retain `com.lettuceai.app`. The two applications install separately
and use separate application data. Android keeps its committed Kotlin namespace;
the workflow changes the Gradle application ID, launcher label, and shortcut target
packages. Native project initialization is not rerun over the customized project.

## Packages

| Platform | Standard build | Outputs |
| --- | --- | --- |
| Linux x86-64 | CPU and Vulkan | `.deb`, `.AppImage`, portable `.tar.gz` |
| Windows x86-64 | CPU and Vulkan | NSIS `.exe`, `.msi` |
| macOS Apple Silicon and Intel | Metal | `.dmg`, `.app.tar.gz` |
| Android | ARM64, ARMv7, x86, x86-64 | Signed universal `.apk`, signed `.aab` |

Optional CUDA builds produce the same Linux and Windows package types and include
the CUDA user-space runtime libraries. They require an NVIDIA driver on the target
computer. The CI build uses explicit GPU architectures and does not run the local
CUDA wrapper that requires `nvidia-smi`.

The Linux `.tar.gz` is a distribution-neutral archive of the AppImage's contents,
including its launcher, libraries, and resources. Extract the complete archive and
run `./AppRun` inside its directory. It requires no package manager, installation,
or FUSE. Both standard and CUDA builds include this archive, with checksums.

Linux packages are built on Ubuntu 24.04. The portable archive retains the same
host glibc and graphics-driver requirements as the AppImage; extracting it does
not add support for older systems. Rust and frontend tool versions and
third-party action revisions are pinned in the workflows and composite actions.
CUDA builds cache registry and Git sources, without uploading their large compiled
target directories to the Actions cache.

Android uses the existing `ANDROID_KEY_BASE64`, `ANDROID_KEY_ALIAS`, and
`ANDROID_KEY_PASSWORD` repository secrets. Signing is checked before artifacts are
uploaded; missing secrets fail before expensive builds start. Android version codes
use the preparation time in Unix seconds so successive dev and stable releases can
update their respective installed apps.

macOS packages are ad-hoc signed and are not notarized. Windows installers are not
code-signed. iOS is excluded until its native project, ONNX libraries, and signing
configuration are available in this checkout.

## AUR publication

The `aur` checkbox controls AUR publication independently of GitHub publication.
`publish: false` also skips AUR. Tag-triggered stable releases update AUR by default.
Only packages backed by assets in the published release are updated; leaving CUDA
off leaves the CUDA AUR package at its existing version.

| Release channel | Standard package | CUDA package |
| --- | --- | --- |
| Stable | `lettuceai-bin` | `lettuceai-cuda-bin` |
| Dev | `lettuceai-dev-bin` | `lettuceai-cuda-dev-bin` |

The standard package includes CPU and Vulkan support. The existing
`lettuceai-vulkan-bin` AUR repository is not updated. Named stable prereleases such
as `3.0.0-beta.1` do not replace stable AUR packages.

Recipes download the exact published `.tar.gz` and use its release SHA-256 checksum.
They install the complete AppDir under `/opt/lettuceai` or `/opt/lettuceai-dev` and
provide separate commands, icons, and desktop entries. Stable and dev can coexist;
standard and CUDA variants within one channel conflict. Dev versions replace the
SemVer hyphen with an underscore for Arch's `pkgver` rules.

The locked `tauri-runtime-wry` 3.0.0-alpha.4 explicitly enables Tauri's `gtk3`
feature and uses Wry 0.57 with WebKitGTK 4.1 and libsoup 3. It does not use GTK 4
or WebKitGTK 6. The current feature set enables neither `linux-libappindicator`
nor `linux-libxdo`; their legacy runtime dependencies are omitted. ALSA, D-Bus,
Wayland, and X11 cover the app's audio, keyring, and window runtime. Vulkan packages
require the Vulkan loader, with a GPU driver optional. CUDA packages include CUDA
runtime libraries and depend on `nvidia-utils`, without requiring the CUDA toolkit.

Configure these repository secrets, using an unencrypted AUR SSH key authorized
for both existing stable packages and able to create the dev packages:

- `AUR_SSH_PRIVATE_KEY`
- `AUR_PACKAGER_NAME`
- `AUR_PACKAGER_EMAIL`

Set repository variable `AUR_SSH_KNOWN_HOSTS` to the verified `aur.archlinux.org`
known-hosts entry. SSH uses strict host-key verification. Existing AUR secret
names from the old workflow are retained.

The **Publish AUR packages** workflow can also run independently with a published
`release_tag`. Its `publish` checkbox defaults off, generating downloadable
`PKGBUILD` and `.SRCINFO` files for review. `.SRCINFO` is generated with Arch's
`makepkg`. To revise packaging for the same app version, increase `pkgrel`.

An AUR failure leaves the GitHub release published. Retry the failed AUR job or run
the standalone workflow for that tag. All candidate updates are checked before any
push. Retries skip unchanged recipes and older versions, and never force-push.
The two AUR repositories cannot be updated atomically; a push failure can leave one
updated until the retry completes the other.

## Failures and retries

Every selected target must succeed before publication. If Windows CUDA fails,
successful jobs retain their artifacts and no public release is created. Use
**Re-run failed jobs** within 30 days to retry failed builds and their downstream
publication job. Each target has its own artifact name; retries replace only that
target's artifact.

The publishing job verifies source commits, versions, and SHA-256 checksums before
creating a draft. It uploads the complete file set, verifies the uploaded assets,
and then publishes. An upload failure leaves a draft for the next retry; it does
not delete an existing release or tag. Retrying cannot overwrite a published release.

## Release notes

Changes include every commit since the most recent published release in the same
channel that is an ancestor of the source commit, including merge commits. Existing
`desktop-dev-…`, `android-dev-…`, and stable platform release tags can be used as the
initial baseline. Failed runs and drafts are not changelog baselines.

The first release without a matching baseline includes the complete history. Every
release also contains `CHANGES.md` with all commit links. If the full list exceeds
GitHub's release description limit, the description links to that complete file.
`build-info.json` records the source and selected targets; `SHA256SUMS.txt` covers
the release files.
