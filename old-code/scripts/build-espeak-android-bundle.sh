
#!/usr/bin/env bash
# Build the Kokoro Android eSpeak NG bundle.
#
# Mirrors the local steps used by hand:
#   1. git clone espeak-ng
#   2. cd android && ./gradlew :app:assembleRelease (Gradle installs the NDK + CMake it needs)
#   3. Pull libttsespeak.so for each Android ABI out of the resulting APK
#   4. Pull the generated espeak-ng-data out of assets/espeakdata.zip
#   5. Tar everything into the layout that src-tauri/build.rs expects
#
# Output layout inside the tarball:
#   jniLibs/arm64-v8a/libttsespeak.so
#   jniLibs/armeabi-v7a/libttsespeak.so
#   jniLibs/x86_64/libttsespeak.so
#   espeak-ng-data/...
#
# The same layout is accepted by KOKORO_ESPEAK_ANDROID_BUNDLE_PATH /
# KOKORO_ESPEAK_ANDROID_BUNDLE_URL in src-tauri/build.rs.
#
# Required tools on PATH: git, java (17), unzip, tar, sha256sum.
# Required env: ANDROID_SDK_ROOT (or ANDROID_HOME).
# Optional env:
#   ESPEAK_NG_REPO    (default: https://github.com/espeak-ng/espeak-ng.git)
#   ESPEAK_NG_REF     (default: master)
#   WORK_DIR          (default: /tmp/espeak-ng-android-build)
#   OUTPUT_BUNDLE     (default: /tmp/kokoro-espeak-android-bundle.tar.gz)
#   KEEP_WORK_DIR     (default: 0 — set to 1 to keep the cloned source tree)

set -euo pipefail

ESPEAK_NG_REPO="${ESPEAK_NG_REPO:-https://github.com/espeak-ng/espeak-ng.git}"
ESPEAK_NG_REF="${ESPEAK_NG_REF:-master}"
WORK_DIR="${WORK_DIR:-/tmp/espeak-ng-android-build}"
OUTPUT_BUNDLE="${OUTPUT_BUNDLE:-/tmp/kokoro-espeak-android-bundle.tar.gz}"
KEEP_WORK_DIR="${KEEP_WORK_DIR:-0}"

ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}"
if [[ -z "${ANDROID_SDK_ROOT}" ]]; then
  echo "error: ANDROID_SDK_ROOT (or ANDROID_HOME) must be set." >&2
  exit 1
fi
export ANDROID_SDK_ROOT
export ANDROID_HOME="${ANDROID_SDK_ROOT}"

for tool in git java unzip tar sha256sum; do
  if ! command -v "${tool}" >/dev/null 2>&1; then
    echo "error: required tool '${tool}' not found on PATH" >&2
    exit 1
  fi
done

log() { printf '\033[1;36m[espeak-bundle]\033[0m %s\n' "$*"; }

log "espeak-ng repo: ${ESPEAK_NG_REPO}"
log "espeak-ng ref:  ${ESPEAK_NG_REF}"
log "work dir:       ${WORK_DIR}"
log "output bundle:  ${OUTPUT_BUNDLE}"
log "android sdk:    ${ANDROID_SDK_ROOT}"

if [[ -d "${WORK_DIR}/.git" ]]; then
  log "Reusing existing clone at ${WORK_DIR}"
  git -C "${WORK_DIR}" fetch --depth=1 origin "${ESPEAK_NG_REF}"
  git -C "${WORK_DIR}" checkout --force FETCH_HEAD
else
  rm -rf "${WORK_DIR}"
  log "Cloning ${ESPEAK_NG_REPO}@${ESPEAK_NG_REF}"
  git clone --depth=1 --branch "${ESPEAK_NG_REF}" "${ESPEAK_NG_REPO}" "${WORK_DIR}" 2>/dev/null \
    || git clone "${ESPEAK_NG_REPO}" "${WORK_DIR}"
  git -C "${WORK_DIR}" checkout --force "${ESPEAK_NG_REF}"
fi

ANDROID_PROJECT_DIR="${WORK_DIR}/android"
if [[ ! -d "${ANDROID_PROJECT_DIR}" ]]; then
  echo "error: ${ANDROID_PROJECT_DIR} does not exist (espeak-ng repo layout changed?)" >&2
  exit 1
fi

log "Running ./gradlew assembleRelease (module name varies upstream; we assemble every release variant)"
pushd "${ANDROID_PROJECT_DIR}" >/dev/null
chmod +x ./gradlew
./gradlew --no-daemon assembleRelease
popd >/dev/null

# Locate the APK that Gradle just produced. Module name varies upstream
# (e.g. `espeak` vs `app`), and the path varies between AGP versions, so glob broadly
# and prefer one that actually contains the libttsespeak.so we need.
APK_PATH=""
while IFS= read -r candidate; do
  if unzip -l "${candidate}" 2>/dev/null | grep -q 'lib/arm64-v8a/libttsespeak\.so'; then
    APK_PATH="${candidate}"
    break
  fi
done < <(find "${ANDROID_PROJECT_DIR}" -type f -path '*/build/outputs/apk/*.apk' | sort)

if [[ -z "${APK_PATH}" ]]; then
  echo "error: no APK containing libttsespeak.so was produced under ${ANDROID_PROJECT_DIR}" >&2
  echo "Found APKs:" >&2
  find "${ANDROID_PROJECT_DIR}" -type f -path '*/build/outputs/apk/*.apk' >&2 || true
  exit 1
fi
log "Using APK: ${APK_PATH}"

STAGE_DIR="$(mktemp -d)"
trap 'rm -rf "${STAGE_DIR}"' EXIT

APK_EXTRACT_DIR="${STAGE_DIR}/apk"
BUNDLE_ROOT="${STAGE_DIR}/bundle"
mkdir -p "${APK_EXTRACT_DIR}" "${BUNDLE_ROOT}/jniLibs" "${BUNDLE_ROOT}/espeak-ng-data"

log "Extracting APK"
unzip -q "${APK_PATH}" -d "${APK_EXTRACT_DIR}"

log "Collecting libttsespeak.so for each ABI"
for abi in arm64-v8a armeabi-v7a x86 x86_64; do
  src="${APK_EXTRACT_DIR}/lib/${abi}/libttsespeak.so"
  if [[ ! -f "${src}" ]]; then
    echo "error: missing ${src} (release APK did not contain ${abi})" >&2
    exit 1
  fi
  mkdir -p "${BUNDLE_ROOT}/jniLibs/${abi}"
  cp "${src}" "${BUNDLE_ROOT}/jniLibs/${abi}/libttsespeak.so"
done

log "Extracting espeak-ng data"
DATA_ZIP="${APK_EXTRACT_DIR}/assets/espeakdata.zip"
DATA_SRC=""
if [[ -f "${DATA_ZIP}" ]]; then
  unzip -q "${DATA_ZIP}" -d "${BUNDLE_ROOT}/espeak-ng-data"
elif [[ -d "${APK_EXTRACT_DIR}/assets/espeak-ng-data" ]]; then
  DATA_SRC="${APK_EXTRACT_DIR}/assets/espeak-ng-data"
else
  # Recent espeak-ng Gradle setups leave the generated data tables under
  # android/.cxx/<variant>/<hash>/espeak-data/espeak-ng-data and never copy
  # them into the APK. Fall back to scanning the CMake output for any
  # phontab file (the espeak-ng marker file).
  log "APK has no bundled data; scanning CMake build output for espeak-ng-data"
  while IFS= read -r candidate; do
    if [[ -d "${candidate}" ]]; then
      DATA_SRC="${candidate}"
      break
    fi
  done < <(find "${ANDROID_PROJECT_DIR}" -type f -name phontab 2>/dev/null \
    | xargs -r -n1 dirname \
    | sort -u)
fi

if [[ -n "${DATA_SRC}" ]]; then
  log "Copying espeak-ng-data from ${DATA_SRC}"
  cp -R "${DATA_SRC}/." "${BUNDLE_ROOT}/espeak-ng-data/"
fi

# Some espeak-ng builds wrap the data in a top-level espeak-ng-data/ directory. Flatten it
# so the tarball always has the same shape regardless of upstream packaging.
if [[ -d "${BUNDLE_ROOT}/espeak-ng-data/espeak-ng-data" ]]; then
  inner="${BUNDLE_ROOT}/espeak-ng-data/espeak-ng-data"
  shopt -s dotglob
  mv "${inner}"/* "${BUNDLE_ROOT}/espeak-ng-data/"
  shopt -u dotglob
  rmdir "${inner}"
fi

if [[ ! -f "${BUNDLE_ROOT}/espeak-ng-data/phontab" ]]; then
  echo "error: bundled espeak-ng-data is missing phontab (no espeakdata.zip, no assets/espeak-ng-data, no phontab under .cxx/)" >&2
  exit 1
fi

mkdir -p "$(dirname "${OUTPUT_BUNDLE}")"
log "Writing tarball ${OUTPUT_BUNDLE}"
tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner \
  -C "${BUNDLE_ROOT}" -czf "${OUTPUT_BUNDLE}" jniLibs espeak-ng-data

SHA="$(sha256sum "${OUTPUT_BUNDLE}" | awk '{print $1}')"
SIZE="$(stat -c '%s' "${OUTPUT_BUNDLE}" 2>/dev/null || stat -f '%z' "${OUTPUT_BUNDLE}")"
log "Done."
log "Path:   ${OUTPUT_BUNDLE}"
log "Size:   ${SIZE} bytes"
log "SHA256: ${SHA}"

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  {
    echo "bundle_path=${OUTPUT_BUNDLE}"
    echo "bundle_sha256=${SHA}"
    echo "bundle_size=${SIZE}"
  } >> "${GITHUB_OUTPUT}"
fi

if [[ "${KEEP_WORK_DIR}" != "1" ]]; then
  log "Cleaning ${WORK_DIR} (set KEEP_WORK_DIR=1 to keep)"
  rm -rf "${WORK_DIR}"
fi
