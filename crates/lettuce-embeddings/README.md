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

Embedding and classification are CPU work behind a job: callers declare model-load, disk-read and CPU resources, and the runtime checks cancellation before tokenizing, before execution, during ONNX graph execution and before returning output. Every target uses ONNX Runtime's default CPU execution provider, including macOS and iOS.

## Memory projections

`MemoryEmbeddingRepository` is the port for the stored vectors of memory items; `lettuce-database` implements it and `lettuce-app` drives it. A ready `MemoryEmbeddingProjection` carries the memory id, the exact source text it was computed from, the vector-space label, the dimensions, finite values and an update time. `put_ready` and `put_reembedded` store a vector only while the memory still has exactly that text; otherwise the write is `Superseded`. When embedding fails, `mark_repair_needed` records a typed repair state instead, so writing a memory never depends on ONNX being available. `list_ready` and `list_repairs` read by space, label and dimensions.

A full backup keeps this cache byte for byte, including repair-needed and stale rows. Export never runs the embedding runtime and does not touch similarity or selection.

## ONNX Runtime library

Every ONNX consumer (embeddings, the emotion classifier, Kokoro in `lettuce-speech`) uses the same shared library, and `ort_runtime.rs` owns where it comes from and how it is initialized.

The Rust binding is `ort` 2.0.0-rc.13 with C API 22. The managed runtime is 1.28.0 on Windows, Linux, Android and Apple Silicon on macOS 14 or newer. Intel Mac and Apple Silicon on macOS Ventura use 1.23.2, whose dylib declares macOS 13.4 as its deployment target. iOS links ONNX Runtime statically. Embedding and emotion sessions on the newer runtime cap intra-op thread spinning at 1 ms with an exponential backoff cap of 8.

### Finding and installing it

`resolve_installed_onnx_runtime` checks the compatible `ORT_DYLIB_PATH` override, bundled resources and the versioned download directory in that order. Older unversioned downloads are bypassed. The installer fetches the SHA-256-pinned native CPU archive from the LettuceAI release mirror, with the original PyPI file as a fallback. It extracts the native library and any required siblings, writing the main library last. Linux uses the stable local filename `libonnxruntime.so`. macOS checks architecture and dylib type and signs the installed library ad hoc as needed.

### Initializing it

`initialize_process_onnx_runtime` commits one process-wide environment. A library load failure is retryable until `ort` pins a library. Kokoro receives evidence that this environment already exists from the composition root. On macOS, the host drains active and cached sessions before the `ort` exit hook releases the environment.
