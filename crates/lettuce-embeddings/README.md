# lettuce-embeddings

The ONNX side of the app: the text embedding runtime (Lettuce v4 and Eidos v5), similarity calibration, the companion emotion classifier (Lettuce Thymos), the memory embedding projection port, and the provisioning and one-time process initialization of the ONNX Runtime library itself.

Model files arrive already verified from `lettuce-model-hub` (`VerifiedEmbeddingArtifacts`, the verified Thymos artifacts); this crate never downloads a model. The application decides when to embed, what to store and how to rank. Vectors are derived data: versioned by model, rebuildable from memory text, and never part of the memory model itself.

## Structure

- `onnx.rs`: `OnnxEmbeddingRuntime`, `EmbeddingDimensions`, `EmbeddingVector`, `effective_max_sequence_length`.
- `calibration.rs`: `SimilarityCalibration`.
- `emotion.rs`: `OnnxEmotionClassifier`.
- `projection.rs`: `MemoryEmbeddingProjection`, `MemoryEmbeddingRepair` and the `MemoryEmbeddingRepository` port.
- `ort_runtime.rs`: finding, unpacking and initializing the ONNX Runtime shared library.

## Embedding runtime

`OnnxEmbeddingRuntime::load` opens a verified model, tokenizer and (for Eidos) calibration. Both families run through the same code; the family decides the details:

| | v4 (`Zeolit/lettuce-emb-768d-v4`) | Eidos (`Zeolit/lettuce-eidos-768d-v5`) |
| --- | --- | --- |
| Inputs | `input_ids`, `attention_mask`, `token_type_ids` if the model declares it | `input_ids`, `attention_mask` |
| Output | the first float output | the output named `embedding`, CLS-pooled and L2-normalized in the graph |
| Trained positions | 2048 | 4096 |
| Scores | raw cosine | calibrated |

`embed` encodes with special tokens, and returns an `EmbeddingVector` of 64, 128, 256, 512 or 768 dimensions (Matryoshka). `EmbeddingDimensions::from_preference` uses one of those values as given and anything else, or nothing, as 768. Truncated vectors are sliced and L2-normalized again; full 768-dimensional output is kept as the model produced it. `count_tokens` counts without a cap. Input text is limited to 1 MiB.

Sequence length is explicit. A published `tokenizer.json` can carry its own truncation and padding (the shipped v4 tokenizer silently truncated at 128 tokens), so the runtime removes both and truncates itself: the text, never the special tokens, is cut so the whole sequence fits `effective_max_sequence_length`, the smaller of the model's trained positions and the user's embedding token setting (clamped to 512..4096, unset means 4096).

Every vector carries its family's vector-space label (`v4` or `v5`), not the download revision. `cosine_similarity` returns `None` for vectors of different labels or lengths, so vectors compare only within one model and dimension, and imported legacy v4 vectors match any installed v4.

### Calibration

`SimilarityCalibration` turns a raw cosine into the score that thresholds apply to. v4 is `RawCosine`, because its thresholds were tuned on raw cosine. Eidos raw cosines sit high even for unrelated text, so it requires the `calibration.json` it publishes: per dimension `shown = clamp(a * cosine + b, 0, 1)`, plus a `default_threshold` and a `fallback_threshold`. Loading fails unless there is a positive-slope map for all five dimensions and `0 < fallback <= default <= 1`. `retrieval_threshold` returns the configured threshold for raw cosine; for a calibration it returns the default threshold, or the fallback when no candidate reaches the default.

## Emotion classifier

`OnnxEmotionClassifier` runs Lettuce Thymos (`Zeolit/lettuce-thymos-26m-v1`, int8 ONNX) for companion mode. Label order, per-class thresholds and window length all come from the verified `labels.json`, never from code. `classify` follows the upstream `inference.py`:

1. Trim the text; blank text returns no classification. Input is capped at 1 MiB.
2. Tokenize without special tokens and split into windows of `max_length` tokens (96) including each window's own `<s>` and `</s>`, with stride `max(1, (max_length - 2) * 3 / 4)` (70), stopping at the window that reaches the last token. An empty encoding becomes one `<unk>`.
3. Right-pad every window with `<pad>` to the longest window of the text and run them eight at a time, each batch at that shared width, with `input_ids` and `attention_mask`.
4. Read the `probabilities` output by name. It is already sigmoided and is never passed through a sigmoid again. Each label keeps its maximum across windows.
5. Return the scores sorted descending, each with its label's threshold; confidence is the maximum of the top three.

Loading checks that `probabilities` is a float output with as many columns as `labels.json` has labels (a dynamic width is checked with one probe window), so a mismatched model and label file fail at load rather than on the first turn. A non-finite or out-of-range probability or a wrong output shape fails closed.

## Running inference

Embedding and classification are CPU work behind a job: callers declare model-load, disk-read and CPU resources, and the runtime checks cancellation before tokenizing, before execution, during ONNX graph execution and before returning output. Apple targets try the CoreML execution provider and fall back to CPU with a log line; other targets use the CPU.

## Memory projections

`MemoryEmbeddingRepository` is the port for the stored vectors of memory items; `lettuce-database` implements it and `lettuce-app` drives it. A ready `MemoryEmbeddingProjection` carries the memory id, the exact source text it was computed from, the vector-space label, the dimensions, finite values and an update time. `put_ready` and `put_reembedded` store a vector only while the memory still has exactly that text; otherwise the write is `Superseded`. When embedding fails, `mark_repair_needed` records a typed repair state instead, so writing a memory never depends on ONNX being available. `list_ready` and `list_repairs` read by space, label and dimensions.

A full backup keeps this cache byte for byte, including repair-needed and stale rows. Export never runs the embedding runtime and does not touch similarity or selection.

## ONNX Runtime library

Every ONNX consumer (embeddings, the emotion classifier, Kokoro in `lettuce-speech`) uses the same shared library, and `ort_runtime.rs` owns where it comes from and how it is initialized.

`ONNX_RUNTIME_VERSION` is 1.22.0 because `ort` 2.0.0-rc.10 with `load-dynamic` binds C API 22; a compile-time assertion keeps the two in step. `ort` loads the library dynamically on every target except iOS, which links it statically and compiles the dynamic paths out.

### Finding it

`resolve_installed_onnx_runtime` checks, in order: a non-empty `ORT_DYLIB_PATH` (on Windows `onnxruntime_providers_shared.dll` must sit next to it, on macOS it must be an `MH_DYLIB` of this architecture per `lipo` and `otool`), the bundled resource names, then the downloaded library. Every check requires a non-empty regular file, never mere existence. A zero-byte or unusable downloaded library is deleted so the next download replaces it; override and bundled files are never touched.

### Installing it

`onnx_runtime_archives` names the release archive per platform (Windows x64 zip, Linux x64 tarball, macOS arm64 or x86_64 tarball with universal2 as fallback). `install_onnx_runtime_archive` unpacks it:

- It reads only the entries it keeps: siblings with the library's extension, or on Linux the library's own link family. Windows keeps every DLL of `lib/` and requires `onnxruntime_providers_shared.dll`; Linux saves `libonnxruntime.so.1.22.0` as `libonnxruntime.so`; macOS keeps every dylib of `lib/`.
- Symbolic and hard links in the tarball become full copies of their target, with chains resolved. Siblings whose links cannot be resolved or are empty are skipped; only the library itself is required. Empty entries and dangling links for the library fail.
- Files are written under a temporary name first and the library is written last, so an interrupted unpack never leaves a zero-byte library or a usable library without its siblings. A failed write removes its temporary file, and a cancellation check runs between entries.
- Downloaded macOS dylibs, old and new, are re-signed ad hoc (quarantine attribute removed, `codesign --force --sign -`) when `codesign -d` shows them unsigned or signed with a real team identifier. Failures are only logged.

### Initializing it

`initialize_process_onnx_runtime` is the only initializer, and the composition root calls it before anything else uses ONNX. It preloads the library before `ort` sees its path (on macOS also the shared and CoreML provider dylibs; on Android the bundled `libonnxruntime.so`), so a missing or broken file fails with a retryable error. The `ort` environment is then committed inside `catch_unwind`, because `ort` panics when loading fails.

From that point a failure is permanent for the process and returned to every later call: `init_from` has pinned the path, and `ort`'s `OnceLock` marks a failed setup as done, so a retry could report success over an uninitialized environment. After the first success every call reuses the environment, and `committed_onnx_runtime` reports it. Kokoro never initializes the runtime itself; it requires the `OnnxRuntimeCommitted` evidence the composition root hands out after this initializer succeeded.
