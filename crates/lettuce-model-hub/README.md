# lettuce-model-hub

Everything about getting model files onto the device and trusting them afterwards: the Hugging Face client (request shapes, response parsing, pinning a repository revision), resumable verified downloads into confined install roots, the installed-artifact manifests for each bundled model family (embeddings, Thymos, Whisper, Kokoro), GGUF header parsing and runnability scoring, and the Sprout hardware report.

The crate does not load or run models, and it does no network I/O. It builds requests and parses responses; `lettuce-app` sends them through `lettuce-network`'s clients, streams the bytes into the stores here, and hands the verified paths to the runtimes. A finished download produces verified artifact facts and never calls a runtime directly. File access goes through `lettuce-platform`'s `ConfinedInstallStore`, so no store takes a native path as an operational argument and partial paths never leave the crate.

## Identity and verification

A model file is identified by where it came from, not by its name: repository, an immutable 40-hex commit, the file path, the byte size and a digest. Mutable branch names are never stored. Hugging Face lists an LFS SHA-256 for large files and a git blob id for small files kept in git; one of the two is required for every file an installer pins, and a listing with neither is refused. After download, LFS files are checked against their SHA-256 and git files with `verify_git_blob` (the git blob hash of the downloaded bytes). Installed manifests then record a BLAKE3 digest per file, and every runtime resolution re-checks size and digest before it exposes a path.

Files land below a per-revision directory, so installing a newer revision never overwrites files a running model is using.

## Hugging Face client

`hugging_face.rs` is the one place that knows Hugging Face hosts and paths (`HUGGING_FACE_ENDPOINT`). Requests are `HfRequest` values (path and query); the application executes them.

- Browser: `search_plan` turns a search into one or more list requests plus a merge rule. LLM mode filters for GGUF, with an extra unfiltered pass for `owner/name` queries; image mode merges the text-to-image and image-to-image lists, sorted and cut to the limit. There are requests and parsers for author models and overview (user, then organization), avatars (organization, then user), repository details and trees, model files sized from the tree (`model_info`), the README without front matter, and `whoami`. `access_error` gives the error text for unauthorized and gated responses.
- Quantization: `extract_quantization` names a GGUF file's quant, with Unsloth dynamic quants prefixed `UD-`. Importance-matrix quants (`i1-`, `imat`, `imatrix` in the name) are listed and flagged by `is_imatrix_quant`; only the importance-matrix data file itself is dropped. `is_mtp_asset` recognizes multi-token-prediction heads shipped next to a model.
- Pinning: `model_pin_request` (current head) and `model_revision_pin_request` (a fixed revision) ask for the file listing with LFS metadata. `parse_pin_listing` returns every listed file unvalidated; `pinned_files` requires the 40-hex commit and a digest per file and reports `git_blob_id` for non-LFS files. Each installer applies its own checks on top.
- URLs: `resolve_url` (branch or commit) and `pinned_resolve_url` (commit only) build a file's download URL from a validated `owner/name`, revision and path. Paths may have several segments (`onnx/model.onnx`, `split_files/vae/ae.safetensors`); every segment uses a strict character set, `.` and `..` are refused, and segments are percent-encoded exactly as the URL path-segment setter does. `strip_endpoint_prefix` removes a leading `https://huggingface.co/` from a pasted model URL.

## Pinned artifact store

`PinnedArtifactStore` (`pinned_artifact.rs`) installs any file whose size, and usually SHA-256, is known up front. It is the general download path used by the GGUF, image bundle, CivitAI, ONNX Runtime, local diffusion and embedding installers in `lettuce-app`, and by Thymos here.

1. `prepare(artifact)` returns either the installed file (after re-verifying it) or a `PinnedDownload` positioned after the bytes a previous attempt left. The partial file's name binds the source identity, destination, digest and size, so a partial resumes only for exactly the same artifact.
2. The caller appends chunks; the store refuses bytes past the declared size.
3. `finish` verifies the complete partial and renames it into place atomically. An installed file that fails verification, or an oversized partial, is replaced. Without a digest only the size is checked, for release assets that publish none.

Only one install at a time may write a given partial within the process: a second one fails with `Busy` ("this file is already being downloaded") instead of appending to the same file.

## Embedding models

`EmbeddingModelFamily` is `LettuceEmbV4` (`Zeolit/lettuce-emb-768d-v4`) or `LettuceEidosV5` (`Zeolit/lettuce-eidos-768d-v5`). Each family knows its repository files, its vector-space label (`v4`, `v5`), trained positions (2048, 4096), native dimensions (768), its ONNX output name, and whether it needs a calibration. Eidos raw cosines sit high for unrelated text, so its scores are only usable through the `calibration.json` it publishes; an Eidos manifest must carry that file and a v4 manifest must not.

`parse_embedding_pin` maps a `pinned_files` answer onto the family's files. Files land at `<family dir>/<revision>/<file>`. `EmbeddingInstallStore` keeps one `manifest.json` per family, written atomically after the files verify; `InstalledEmbeddingManifest::verify` hashes model, tokenizer and calibration before exposing paths. Removal deletes the family folder and any recorded file inside the root, never anything outside it.

`inspect_legacy_embedding_install` describes a legacy `v4-model.int8.onnx` and `v4-tokenizer.json` in place, with revision `legacy-import:<BLAKE3 prefix>`. `select_embedding_family` loads the preferred family when installed, else Eidos, else v4.

## Companion emotion model (Thymos)

The companion emotion classifier is Lettuce Thymos (`Zeolit/lettuce-thymos-26m-v1`): `onnx/model_quantized.onnx`, `tokenizer.json` and `labels.json`. `RemoteCompanionEmotionModel::from_pinned_files` takes them from a pin listing: the commit becomes the revision every file is fetched at, the two LFS files need size and SHA-256, and the git-stored `labels.json` needs size and git blob id. No revision or digest is compiled in. Files land below `<root>/revisions/<revision>/<path>` through `PinnedArtifactStore`; a `labels.json` that fails its blob check after download is removed so a retry fetches it again.

`CompanionEmotionInstallStore` manages the install:

- `lock` serializes admission, completion and removal within the process. Under it, `active-install.json` names the one admitted install job and its revision, so only one Thymos install runs at a time.
- `complete` refuses while another revision is the active install. It verifies the whole manifest, writes `installed.json` (revision plus each file's size and BLAKE3) through `installed.json.next` and a rename so a crash leaves the old or the new record, clears the active install, then removes every other revision's files and every partial below `.downloads/`. A failure in that last sweep leaves the new install valid and is reported as `cleanup_pending`.
- Verification rehashes every file and parses `labels.json` under the upstream `inference.py` rules: labels and thresholds of equal length, thresholds finite in `[0, 1]`, `max_length` present, a window of at least 4 with a stride below the content length. If `labels.json` names a `model_sha256`, the model's SHA-256 must match. A recorded path outside the managed layout is refused.
- Status is not installed, installed (files present at their recorded sizes) or damaged.
- Removal deletes the record, then the active-install record, then the three Thymos files of every `revisions/<revision>/` directory (found by layout, not by record, so a corrupt record never strands files) and every partial. A failed sweep is retried by the next removal or install, and nothing outside the layout is touched.

## Whisper

`RemoteWhisperModel::pinned` describes a downloadable Whisper model: model id derived from the file name, a 40-hex revision, byte size (at most 8 GiB) and SHA-256. English-only and quantized flags and the general, mobile and desktop recommendations are derived from the model id and re-checked by `validate`.

`WhisperInstallStore` downloads into one confined root. The partial's name is derived from the complete remote identity, survives a restart, is never treated as installed, and resumes only for the same revision, size and SHA-256. A complete partial must match the SHA-256 before an atomic rename; the `InstalledWhisperManifest` (model id, revision, size, BLAKE3, language and quantization facts, admission time) is then built from the final file. If the app crashed between the rename and the database write, a matching final file replays the recovery. Neither partial nor final path appears in the download result.

Managed removal takes a model identity, not a path, and requires the manifest to carry an immutable revision and the exact derived path under the install root. Missing bytes count as an earlier interrupted removal. Legacy manifests and external paths are refused and never deleted.

`inspect_legacy_whisper_models` discovers models in the old two-level `models/whisper/<variant>/ggml-<variant>.bin` layout, rejects symlinks, and hashes regular files in place without moving or deleting them (revision `legacy-import:...`). `WhisperModelRepository` is the durable catalog; `select_default_whisper_model` picks the first installed model in file name order when no model is selected.

## Kokoro

Kokoro TTS is split into inventory, model install and voices.

- `KokoroAssetStore` (`kokoro.rs`) inspects one confined root: model variants FP32, FP16 and Int8 (only Int8 is allowed on mobile, `kokoro_platform_allows_variant`), nested ONNX files before flat ones, `model_uint8.onnx` as the Int8 fallback, the config, tokenizer and tokenizer-config files, and the sorted, case-sensitive `voices/*.bin` ids. Every reported artifact is non-empty, bounded and BLAKE3-identified; symlinks and invalid files fail inspection.
- `KokoroInstallStore` (`kokoro_install.rs`) installs the four-file bundle of one variant from `onnx-community/Kokoro-82M-v1.0-ONNX` at a pinned revision (`KOKORO_SOURCE_REVISION`), each file with an exact size and SHA-256 (`pinned_kokoro_model`). Partials resume by complete artifact identity, verify before rename, and an already installed file is reused only after re-verification. Removing a model variant keeps the shared config and tokenizer files. `materialize_lexicon` reads an optional user-authored `lexicon.json` as a snapshot of at most 1 MiB, rereads to reject a file that changed meanwhile, and never creates, rewrites or deletes it.
- `KokoroVoiceInstallStore` (`kokoro_voice.rs`) installs voices: a `RemoteKokoroVoice` pins revision, `voices/<id>.bin` path, size and SHA-256, with ids limited to ASCII letters, digits, `_` and `-`. Each completed voice gets a small versioned descriptor sidecar; reopening rebuilds the offline voice catalog only from sidecars whose id, revision, path, size and digest still match the voice bytes. A missing or mismatched sidecar makes that voice unavailable for synthesis but never deletes its binary. `materialize` streams a voice through the size and SHA-256 check and returns its bytes to the speech runtime; the value has no serialization and its `Debug` shows only id and byte count. Managed removal verifies the pinned artifact and target before deleting a voice and its sidecar; a missing file counts as removed.

## GGUF runnability

`gguf_runnability.rs` reads a model's shape from a GGUF header and scores how well each file runs on a machine. `parse_gguf_meta` reads the header in two passes (architecture first; keys past the bytes read stay unset), and `gguf_meta_with_retry` reads 512 KiB first and 10 MiB when the essentials were missing. The scoring is a port of the runnability formulas, which are frozen: per-file scores at the app's default context and KV type (`runnability_scores`), a downloaded file's score with its GPU sidecars (`local_runnability`), and the recommendation (`build_recommendation`: per-file context limits, the best GPU and RAM contexts, the best file, context and KV type). A parity test pins every output against the legacy functions on the same inputs (`tests/fixtures/legacy_runnability.txt`).

## Sprout

`sprout_hardware` reads a Sprout `/specs` report (schema 1), the hardware probe that runs next to a remote Ollama server, into `RunnabilityHardware`: the largest free VRAM of a discrete GPU, else of the integrated GPUs, which then count as unified memory. `sprout_specs_url` builds the report URL.
