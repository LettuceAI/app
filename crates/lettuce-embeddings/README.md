# lettuce-embeddings

Embedding preprocessing/runtime/index interfaces plus auxiliary analysis.

## Boundary

Vectors are derived, versioned, and rebuildable.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first ONNX slice loads a model-hub-verified Lettuce embedding v4 model and
tokenizer, encodes with special tokens, supplies `input_ids`, `attention_mask`,
and model-declared `token_type_ids`, reads the first float output, and supports
the audited 64/128/256/512/768 Matryoshka dimensions. `from_preference` keeps
the legacy v4 rule: one of those values is used as-is, anything else or no
preference means 768. Truncated dimensions are
L2-normalized; native 768-dimensional output preserves the model result.

The v4 base config is limited to 2,048 trained positions. Legacy settings
allowed 4,096 while the shipped tokenizer JSON silently truncated at 128. The
runtime now drops any truncation and padding a published `tokenizer.json`
carries and truncates explicitly: the text (never the special tokens) is cut so
the whole sequence fits `min(model positions, embeddingMaxTokens)`, where the
setting is clamped to 512..=4,096 and unset means 4,096
(`effective_max_sequence_length`). A unit test proves a 300-token text reaches
the session tensors with 302 tokens through a tokenizer whose JSON truncates at
128. Token counting is no longer capped either.

Lettuce Eidos (`Zeolit/lettuce-eidos-768d-v5`, ModernBERT, int8
`onnx/model_quantized.onnx`) loads through the same runtime: `input_ids` and
`attention_mask` only, the output named `embedding` (CLS pooled and L2
normalized in the graph; v4 keeps its first output), 4,096 trained positions,
no prompts. Truncated Matryoshka dimensions are sliced and re-normalized for
both families. Every vector carries its family's vector-space label (`v4` or
`v5`, `EmbeddingModelFamily::vector_space`) rather than the download revision,
so vectors compare only within one model and dimension and imported legacy `v4`
vectors match any installed v4.

`SimilarityCalibration` turns a raw cosine into the score thresholds apply to.
v4 stays `RawCosine` (its thresholds were tuned on raw cosine). Eidos requires
its published `calibration.json`: per dimension `shown = clamp(a * cosine + b,
0, 1)`, plus `default_threshold` and `fallback_threshold`; the file must carry a
positive-slope map for 64/128/256/512/768 and `0 < fallback <= default <= 1`,
or loading fails. `retrieval_threshold` returns the configured threshold for raw
cosine and, for a published calibration, its default threshold, or its
fallback when no candidate reaches the default.

The companion emotion runtime runs Lettuce Thymos
(`Zeolit/lettuce-thymos-26m-v1`, int8 ONNX) from model-hub-verified model,
tokenizer and `labels.json` artifacts. The label order, per-class thresholds and
window length come from the verified `labels.json`, never from code. Text is
trimmed (blank returns no classification), tokenized without special tokens and
split into windows as the upstream `inference.py` does: `max_length` (96) tokens
per window including the window's own `<s>`/`</s>`, stride
`max(1, (max_length - 2) * 3 / 4)` (70), stopping at the window that reaches the
last token, an empty encoding becoming one `<unk>`. Windows are right-padded
with `<pad>` to the longest window of the whole text and run eight at a time,
each run keeping that one shared width, with `input_ids` and `attention_mask`.
Loading checks that the model's `probabilities` output is float with as many
columns as `labels.json` has labels (a dynamic width is checked with one probe
window), so a mismatched pair fails at load rather than on the first turn. The `probabilities` output is read by name and is already
sigmoided; it is never passed through a sigmoid again. Each label keeps its
maximum across windows. Scores are sorted descending, each carrying its label's
threshold, and confidence stays legacy's top-three maximum. A non-finite or
out-of-range probability or a wrong output shape fails closed. The legacy
SamLowe `roberta-base-go_emotions-onnx` path (`config.json` `id2label`, 512-token
truncation, sigmoid over logits, fixed thresholds) is deleted. A one-megabyte
input cap remains a safety correction over legacy. The ignored live test's
reference probabilities were taken with ONNX Runtime 1.22 (the version this
build targets); ONNX Runtime 1.23 and later differ by up to about 1.4e-3 on its
long text.

Inference declares model-load, disk-read, and CPU job resources and cooperates
with cancellation before tokenization, before execution, during ONNX graph
execution, and before publishing output. Apple targets attempt CoreML with a
logged CPU fallback; the actual legacy Android/non-Apple path remains CPU.
The legacy companion NER and router (NLI) models are not ported (approved
removal); retrieval ranking remains outside this slice.

Memory projections are rebuildable derived data behind the domain-owned
`MemoryEmbeddingRepository` port. Ready rows carry exact source text, immutable
model revision, declared dimensions, finite vectors, and update time; failed
generation is represented as typed repair-needed state rather than making an
authoritative memory write depend on ONNX availability.

Full backup preserves the projection cache byte-for-byte, including ready
vectors, repair-needed state and stale rows retained after memory changes. Export
does not invoke the embedding runtime or alter similarity and selection math.

ONNX Runtime provisioning (`ort_runtime`): `ONNX_RUNTIME_VERSION` is 1.22.0
because `ort` =2.0.0-rc.10 with `load-dynamic` binds C API 22, and a
compile-time assertion keeps the two in step. `resolve_installed_onnx_runtime`
keeps legacy's order: a non-empty `ORT_DYLIB_PATH` (Windows also needs
`onnxruntime_providers_shared.dll`, macOS an `MH_DYLIB` of this architecture
per `lipo`/`otool`), then the bundled resource names, then the downloaded
library. A zero-byte or unusable downloaded library is deleted so the next
download replaces it; override and bundled files are never touched. Every
check requires a non-empty regular file, never mere existence.
`install_onnx_runtime_archive` unpacks the release archive: Windows keeps
every DLL of `lib/`, Linux saves `libonnxruntime.so.1.22.0` as
`libonnxruntime.so`, macOS keeps every dylib of `lib/`. Symbolic and hard
links in the tarball become full copies of their target (chains resolved),
empty entries and dangling links fail, and files are written under a
temporary name first, so no zero-byte library is ever left behind. Downloaded
macOS dylibs, existing and new, are re-signed ad hoc (quarantine attribute
removed, `codesign --force --sign -`) when `codesign -d` shows them unsigned
or signed with a real `TeamIdentifier`; failures are only logged.

`initialize_process_onnx_runtime` is the process's ONNX Runtime
initializer; embeddings and the emotion classifier go through it, and the
composition root calls it first. Kokoro in `lettuce-speech` never commits:
it requires the `OnnxRuntimeCommitted` evidence the composition root hands
out after this initializer succeeded. The library is preloaded
before `ort` sees its path (macOS also preloads the shared and CoreML
provider dylibs), so a missing or broken file fails with a retryable error.
The `ort` environment is committed inside `catch_unwind` because `ort`
panics when loading fails. A failure from that point on is permanent for the
process and returned to every later call: `init_from` has pinned the path,
and `ort`'s `OnceLock` marks a failed setup as done, so a retry could report
success over an uninitialized environment. After the first success every
other call reuses that environment, and `committed_onnx_runtime` reports it.
Android preloads the bundled `libonnxruntime.so`. `ort` is built with
`load-dynamic` on every target but iOS, which links ONNX Runtime statically
as legacy did; there the dynamic paths are compiled out.

Extraction reads only the entries it keeps (siblings with the archive's
extension, or the library's own link family on Linux), skips siblings whose
links cannot be resolved (or are empty) and only requires the library
itself, which it writes last so an interrupted unpack never leaves a usable
library without its siblings; Windows also requires
`onnxruntime_providers_shared.dll`. Extraction polls a cancellation check
between archive entries and removes its temporary file when a write fails.
