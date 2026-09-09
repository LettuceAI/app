# lettuce-model-hub

Remote model discovery, durable verified downloads, compatibility, installed
artifact manifests, leases, installation, and removal planning.

## Boundary

Does not load or execute models. Download completion produces verified artifact
facts and never calls a runtime module directly.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first installed-artifact contract verifies the Lettuce embedding v4 model
and tokenizer as regular bounded files against an immutable source identity,
byte size, and BLAKE3 digest before exposing runtime paths. The legacy download
used an unpinned Hugging Face `main` URL; new installs must persist an immutable
revision and verified hashes instead of treating filenames as identity.
The audited v4 upstream revision is
`8fe12dc548f75865bfb120593fd5a514e9186ca0`; its model config declares 2,048
trained positions and 768 native dimensions.

The companion-emotion installed contract separately verifies the exact model,
tokenizer, and config triplet used by the GoEmotions auxiliary classifier. It
also requires an immutable source revision before exposing paths to the runtime;
model loading and config interpretation remain owned by `lettuce-embeddings`.

Installed Whisper models now have a separate immutable manifest with model ID,
source revision, byte size, BLAKE3 digest, language/quantization facts and
admission time. Retained legacy discovery is bounded to the old two-level
`models/whisper/<variant>/ggml-<variant>.bin` layout, rejects symbolic links and
hashes regular files without moving or deleting them. The durable catalog keeps
legacy filename sorting, so an omitted selection resolves to the same first
installed model used by chat and group chat. Every runtime resolution rechecks
the file size and digest before exposing its internal verified path.

Remote Whisper catalog entries require an immutable forty-character repository
revision plus coherent LFS byte size and SHA-256 evidence. Model IDs,
English-only and quantization detection, and the legacy general/mobile/desktop
recommendation sets remain unchanged. Mutable branch names and files without
complete LFS metadata are rejected. Hugging Face documents repository-tree file
metadata and LFS SHA-256 identities in its
[Hub API](https://huggingface.co/docs/huggingface_hub/en/package_reference/hf_api).
The old unpinned Hugging Face `main` URL is not copied.

Pinned Whisper downloads use one confined install root and a stable partial
name derived from the complete remote identity. Partial bytes survive process
restart, are never treated as installed, and resume only for the same revision,
size and SHA-256. A complete partial must match the upstream SHA-256 before an
atomic rename. The installed manifest is then built from the final file with
the existing BLAKE3 identity; a matching final file replays recovery after a
rename-before-database crash. Neither partial nor final native paths enter the
public download result.

Managed removal accepts a model identity rather than a path. It requires the
persisted manifest to carry an immutable remote revision and the exact derived
path under the selected managed install root. Missing bytes are an idempotent
recovery case for a prior interrupted removal. Retained legacy manifests and
external paths are refused and never deleted.
