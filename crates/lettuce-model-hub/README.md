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

`EmbeddingModelFamily` is `LettuceEmbV4` or `LettuceEidosV5`. Each family names
its repository, repository files, vector-space label (`v4`/`v5`), trained
positions (2,048/4,096) and whether it needs a calibration. An Eidos manifest
must carry `calibration.json` and a v4 manifest must not; `verify` hashes it
like the model and tokenizer. `parse_embedding_pin` maps the shared
`pinned_files` result onto the family's files and refuses a file with neither
an LFS SHA-256 nor a git blob id. `pinned_files` now also reports the git blob
id (`HfPinnedFile::git_blob_id`) of a file stored without LFS, and
`verify_git_blob` (`pinned_artifact`) checks a downloaded file against it, for
any installer whose repository has plain JSON files. Files
land at `<family dir>/<revision>/<file>`, so a newer revision never replaces
files in use. `EmbeddingInstallStore` keeps one `manifest.json` per family,
written atomically after verifying the files; removal deletes the family folder
and any recorded file inside the root, never files outside it.
`inspect_legacy_embedding_install` describes the legacy `v4-model.int8.onnx`
and `v4-tokenizer.json` in place (revision `legacy-import:<BLAKE3 prefix>`).
`select_embedding_family` loads the preferred family when installed, else
Eidos, else v4.

The companion-emotion model is Lettuce Thymos
(`Zeolit/lettuce-thymos-26m-v1`). `RemoteCompanionEmotionModel::from_pinned_files` takes
`onnx/model_quantized.onnx`, `tokenizer.json` and `labels.json` from the shared
`model_pin_request` / `pinned_files` answer: its forty-character commit becomes
the revision every file is fetched at, the two LFS files must carry their size
and SHA-256, and the git-stored `labels.json` its size and git blob id; a listing
with neither digest for a file is refused. The blob id is checked with the
shared `verify_git_blob` once the download finishes (a mismatching file is
removed so a retry downloads it again), so every file is digest-verified. No
revision or digest is compiled in. Files land confined below
`<root>/revisions/<revision>/<remote path>` through `PinnedArtifactStore`.
`CompanionEmotionInstallStore::lock` serializes admission, completion and
removal within the process; under it `active-install.json` names the one
admitted install job and its pinned revision, so the application admits a
single Thymos install at a time. `complete` refuses (busy) while another
revision is the active install, writes `installed.json` (revision plus each
file's size and BLAKE3) only after the full manifest verification, staging it
as `installed.json.next` and renaming it over the old record so a crash leaves
either the old or the new record, clears the active install, then removes the
Thymos files of every other revision and every partial download below
`.downloads/`; a failure there leaves the new install valid and is reported as
`cleanup_pending`. Verification rehashes every file, parses `labels.json` under
the upstream `inference.py` rules (labels and thresholds of equal length,
thresholds finite in `[0, 1]`, `max_length` required, window at least 4 with a
stride below the content length) and, when `labels.json` names a
`model_sha256`, checks the model's SHA-256 against it. A recorded path outside
the managed layout is refused. Status reports not installed, installed (files
present at their recorded sizes) or damaged. Removal deletes the record first,
then the active-install record, the three Thymos files of every
`revisions/<revision>/` directory (by layout, not by record, so a corrupt or
out-of-layout record never strands files) and every partial download; a failed
sweep is retried by the next removal or install, and nothing outside the layout
is deleted. The legacy SamLowe triplet contract is deleted.

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

Kokoro asset inventory opens one composition-owned confined root and exposes no
operational path argument. It preserves the legacy desktop FP32, FP16 and Int8
variants, the mobile-only Int8 restriction, nested-before-flat ONNX lookup and
the `model_uint8.onnx` Int8 fallback. It reports the expected config, tokenizer
and tokenizer-config artifacts and discovers sorted, case-sensitive
`voices/*.bin` IDs. Every reported artifact is nonempty, bounded and BLAKE3
identified; symlinks and invalid artifacts fail inspection. Downloads,
phonemization and ONNX execution remain outside this inventory slice.

Kokoro model installation pins the legacy four-file bundle to immutable
Hugging Face revision `1939ad2a8e416c0acfeecc08a694d14ef25f2231`. The three
shared JSON files and the selected FP32, FP16 or Int8 ONNX file each retain an
exact byte size and SHA-256. Confined partial files resume by complete artifact
identity, verify before rename and replay already installed bytes only after
reverification. The pinned inventory follows the upstream
[Kokoro ONNX repository](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/tree/1939ad2a8e416c0acfeecc08a694d14ef25f2231).
Remote Kokoro voice descriptors preserve the same pinned revision, exact
`voices/<safe-id>.bin` path, byte size and SHA-256. Voice IDs retain the legacy
ASCII alphanumeric, underscore and hyphen rule. The descriptor does not install
or expose a native path.
Voice installation uses revision, ID, path, size and SHA-256 as its complete
partial identity. It commits only verified bytes below the managed `voices`
directory and rechecks every requested voice before reporting a batch installed.
Managed removal verifies the exact pinned artifact and confined target before
deleting a model variant or voice. Missing files replay as already removed;
model removal preserves the shared config and tokenizer artifacts.
The optional user-authored Kokoro `lexicon.json` is read only as a one MiB
confined snapshot. The materialized value exposes bounded bytes without its
native path or contents in debug output, rereads to reject a changing file and
never creates, rewrites or deletes the source.
Installed Kokoro voice bytes cross into the speech runtime only through a
purpose-specific materialization method. It validates the pinned descriptor and
streams the confined file through the same size and SHA-256 check that retains
the bounded bytes. The materialized value has no serialization and its debug
form exposes only the voice ID and byte count.
Each completed managed voice also has a bounded immutable versioned descriptor
sidecar. Reopen builds the offline descriptor catalog only from sidecars whose
ID, revision, path, size and SHA-256 still match their voice bytes. A missing,
malformed or mismatched sidecar makes that voice unavailable to verified
synthesis but never deletes or overwrites its binary. Explicit managed removal
deletes the verified voice and its matching sidecar.

`PinnedArtifactStore` generalizes the Kokoro install store for any artifact
with a known size and (usually) SHA-256: partial names bind the source
identity, destination, digest and size; downloads resume; complete files are
verified before an atomic rename; an installed file that fails verification
(or an oversized partial) is replaced. A missing digest checks the size
only, as legacy did for release assets without one.

## Hugging Face browser

`hugging_face` builds the browser's requests and reads its responses: model
search (GGUF-filtered, with an unfiltered pass for `owner/name` queries; image
mode merges text-to-image and image-to-image lists, sorted and cut to the
limit), author models and overview (user, then organization), avatars
(organization, then user), model files sized from the repository tree, the
README without front matter, whoami, and the old error texts for unauthorized
and gated responses. Quantization names follow the old list with Unsloth
dynamic quants `UD-` prefixed; corrected: `BF16` files were labeled `F16`,
`TQ1_0`/`TQ2_0`/`MXFP4` were unknown, and every file with `imatrix` in its
name was hidden. Importance-matrix quants (`i1-`, `imat`, `imatrix` parts) are
now listed and flagged `imatrix`; only the importance-matrix data file itself
is dropped.

Every Hugging Face host and path the app uses is built here. The pin requests
(`model_pin_request` for the current head, `model_revision_pin_request` for a
fixed revision) are read by `parse_pin_listing`, which returns every listed
file unvalidated; `pinned_files` and the Whisper and Kokoro catalogs apply
their own checks on top of it. `resolve_url` (a branch or commit) and
`pinned_resolve_url` (a 40-hex commit only) build a file's download URL from a
validated `owner/name`, revision and file path. Artifact paths may have several
segments (`onnx/model.onnx`, `split_files/vae/ae.safetensors`); each segment
keeps the strict character set and `.`/`..` are refused. Segments are
percent-encoded exactly as the URL path-segment setter does.
`strip_endpoint_prefix` removes a leading `https://huggingface.co/` from a
pasted model URL.

## Runnability

`gguf_runnability` reads the model shape from a GGUF header (two passes, the
architecture first; keys past the read bytes stay unset) and ports the frozen
runnability formulas: per-file scores at the app's default context and KV
type, a downloaded file's score with its GPU sidecars, and the recommendation
(per-file context limits, optimal GPU/RAM contexts, the best file, context and
KV type). A parity test pins every output against the legacy functions run on
the same fixtures (`tests/fixtures/legacy_runnability.txt`). Corrected: the
GPU-candidate pass paired files with the wrong file's context limits whenever
an earlier file had no size.

Sprout: `sprout_hardware` reads a Sprout `/specs` report (schema 1) into the
runnability hardware: the largest free VRAM of a discrete GPU, else of the
integrated GPUs, which then count as unified memory.

A pinned artifact's partial file is downloaded by one install at a time per
store: a second concurrent install of the same file fails with `Busy`
("this file is already being downloaded") instead of appending to it.
