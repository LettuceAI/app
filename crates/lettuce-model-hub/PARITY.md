# lettuce-model-hub: legacy parity notes

Facts about how `lettuce-model-hub` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- The runnability formulas are frozen ports of legacy's; `tests/fixtures/legacy_runnability.txt` pins every output against the legacy functions run on the same fixtures.
- Whisper model ids, English-only and quantization detection, and the general, mobile and desktop recommendation sets are legacy's. The durable catalog keeps legacy's file name sorting, so an omitted selection resolves to the same first installed model chat and group chat used.
- Legacy Whisper discovery is bounded to legacy's two-level `models/whisper/<variant>/ggml-<variant>.bin` layout.
- Kokoro inventory keeps legacy's desktop FP32, FP16 and Int8 variants, the mobile-only Int8 restriction, nested-before-flat ONNX lookup and the `model_uint8.onnx` Int8 fallback. The install pins legacy's four-file bundle. Voice ids keep legacy's ASCII alphanumeric, underscore and hyphen rule.
- `PinnedArtifactStore` checks only the size when no digest is known, as legacy did for release assets without one.
- The Hugging Face browser keeps legacy's search behavior, author and avatar lookup order, README handling and error texts for unauthorized and gated responses. Quantization names follow legacy's list.
- `inspect_legacy_embedding_install` reads legacy's `v4-model.int8.onnx` and `v4-tokenizer.json` in place.
- The download planner's formulas lived in the legacy frontend (`HuggingFaceBrowserPage.tsx` 163-420 `KV_BPV`, `maxContextForBpv`, `computeGpuOptimalContext`, `computeRamMaxContext`, `calcScore`; 650-666 and 701-704 the layer counts; 868-1000 and 1170-1185 the detail report; 1969-2004 the mixed layers; 3598-3627 the upgrade suggestion; 3746-3770 the file switch default; 505-515, 543-648 and 668-676 the KV offload, headroom, run and speed statuses and the offload a layer count means). They are ported verbatim, including their bytes-per-value table and score that differ from the runnability score's, and `tests/fixtures/legacy_planner.json` pins every output against the legacy functions run with bun on the same inputs.

## Deliberate differences from legacy

- Legacy downloaded the embedding model from an unpinned Hugging Face `main` URL and treated file names as identity. New installs persist an immutable revision and verified hashes. The audited v4 upstream revision is `8fe12dc548f75865bfb120593fd5a514e9186ca0`; its config declares 2048 trained positions and 768 native dimensions.
- Legacy's unpinned Hugging Face `main` URLs for Whisper are not copied; remote Whisper entries need an immutable revision and complete LFS size and SHA-256. Mutable branch names and files without LFS metadata are rejected. Hugging Face documents the tree metadata and LFS identities in its [Hub API](https://huggingface.co/docs/huggingface_hub/en/package_reference/hf_api).
- The legacy SamLowe emotion triplet contract is deleted; Thymos replaces it.
- Quantization naming: legacy labeled `BF16` files `F16`, did not know `TQ1_0`, `TQ2_0` and `MXFP4`, and hid every file with `imatrix` in its name. Importance-matrix quants are now listed and flagged; only the importance-matrix data file is dropped. The runnability score keeps the full-precision quality for `UD-BF16` files, which legacy scored as `F16`.
- Legacy's GPU-candidate pass paired files with the wrong file's context limits whenever an earlier file had no size. Corrected.
- A second concurrent install of the same pinned file fails with `Busy` instead of appending to the same partial.
- Legacy's errors were strings the UI matched (`HfTokenMenu.tsx` 15-17); refusals, missing repositories, rate limits and unreachable hosts are now typed `HfFailure`s, with legacy's texts kept as the message.
- Image bundle installs are not deduplicated here: `lettuce-app` returns the job of a running install whose files are exactly the wanted ones and refuses (`Busy`) a bundle that shares any other file or a hash, as legacy refused any duplicate (`old-code/src-tauri/src/hf_browser/image_bundle.rs:891-895`); this crate's `PinnedArtifactStore` stays per-file.
- Legacy reported every failed token check, including an unreachable Hugging Face, as an invalid token (`image_bundle.rs` 116-121); only a 401 is invalid now, anything else is unknown.
- Legacy sized every file 0 when the file tree request failed (`hf_browser/mod.rs` 2371-2375), which made every file unrunnable in the UI; `model_info` now needs the tree and the browse call fails instead.

## History

- The Kokoro bundle is pinned to revision `1939ad2a8e416c0acfeecc08a694d14ef25f2231` of the upstream [Kokoro ONNX repository](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/tree/1939ad2a8e416c0acfeecc08a694d14ef25f2231).
- Downloads, phonemization and ONNX execution were outside the first Kokoro inventory slice; installs and voices have since been added here, and phonemization and execution live in `lettuce-speech`.
- `pinned_files` gained `git_blob_id` for non-LFS files, and `verify_git_blob` checks them, for installers whose repositories keep plain JSON files in git.

- Kokoro catalog requests resolve the current repository commit, replacing the fixed install revision used by the earlier rewrite. New model and voice files live under revision directories; an atomic installed descriptor selects verified content, so a later upstream revision cannot overwrite retained bytes. The original known-revision flat voice layout remains readable. Legacy queried available voices from the repository (old-code/src-tauri/src/tts_manager/kokoro/download.rs:127-150); the new descriptors require immutable revision and SHA-256 evidence.
- All Hugging Face JSON catalog sends now enter `hugging_face.rs` through `lettuce-network`, including Whisper, Kokoro, embeddings and the shared browser. Plain Kokoro JSON metadata is verified against Git blob identity before computing SHA-256; model LFS digests remain upstream-provided.
