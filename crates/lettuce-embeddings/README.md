# lettuce-embeddings

Embedding preprocessing/runtime/index interfaces plus auxiliary analysis.

## Boundary

Vectors are derived, versioned, and rebuildable.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first ONNX slice loads a model-hub-verified Lettuce embedding v4 model and
tokenizer, encodes with special tokens, supplies `input_ids`, `attention_mask`,
and model-declared `token_type_ids`, reads the first float output, and supports
the audited 64/128/256/512/768 Matryoshka dimensions. Truncated dimensions are
L2-normalized; native 768-dimensional output preserves the model result.

The v4 base config is limited to 2,048 trained positions. Legacy settings
allowed 4,096 while the shipped tokenizer JSON silently truncated at 128; the
adapter deliberately overrides tokenizer truncation to the verified manifest's
maximum, bounded at the real 2,048-position model capability.

The companion emotion auxiliary runtime loads only model-hub-verified model,
tokenizer, and config artifacts. It copies the legacy GoEmotions path exactly:
numeric `id2label` sorting with the 28-label fallback, special-token encoding,
512-token truncation, `input_ids` plus `attention_mask`, sigmoid scoring,
descending ordering, unknown-index labels, and top-three maximum confidence.
Blank text returns no classification. Installed artifact/config/tensor errors
fail closed; the application can preserve legacy's neutral fallback when no
verified installation is available. A one-megabyte pre-tokenization input cap
is a deliberate safety correction over legacy's unbounded allocation.

Inference declares model-load, disk-read, and CPU job resources and cooperates
with cancellation before tokenization, before execution, during ONNX graph
execution, and before publishing output. Apple targets attempt CoreML with a
logged CPU fallback; the actual legacy Android/non-Apple path remains CPU. NER,
router models, download UI, and retrieval ranking remain outside this slice.

Memory projections are rebuildable derived data behind the domain-owned
`MemoryEmbeddingRepository` port. Ready rows carry exact source text, immutable
model revision, declared dimensions, finite vectors, and update time; failed
generation is represented as typed repair-needed state rather than making an
authoritative memory write depend on ONNX availability.
