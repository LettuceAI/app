# lettuce-embeddings: legacy parity notes

Facts about how `lettuce-embeddings` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- `EmbeddingDimensions::from_preference` keeps legacy's v4 rule: 64, 128, 256, 512 or 768 are used as given, anything else or no preference means 768.
- Thymos confidence stays legacy's top-three maximum.
- On Apple targets CoreML is tried with a logged CPU fallback; the legacy Android and non-Apple path remains CPU.
- `resolve_installed_onnx_runtime` keeps legacy's search order: `ORT_DYLIB_PATH`, bundled resources, then the downloaded library.
- iOS links ONNX Runtime statically, as legacy did.

## Deliberate differences from legacy

- Legacy settings allowed 4096 embedding tokens while the shipped v4 tokenizer JSON silently truncated at 128. The runtime now strips the tokenizer's truncation and padding and truncates the text explicitly to `min(model positions, embeddingMaxTokens)`. A unit test shows a 300-token text reaching the session tensors as 302 tokens through a tokenizer whose JSON truncates at 128. Token counting is no longer capped either.
- The legacy SamLowe `roberta-base-go_emotions-onnx` path (`config.json` `id2label`, 512-token truncation, sigmoid over logits, fixed thresholds) is deleted; Thymos replaces it.
- The emotion classifier's 1 MiB input cap is a safety correction over legacy.
- Legacy could leave a zero-byte ONNX Runtime library behind (tar symlink entries extracted as empty files). Links now become full copies, files are written under temporary names, and every check requires a non-empty regular file.

## Approved removals

- The legacy companion NER and router (NLI) models are not ported.

## Notes

- The ignored live Thymos test's reference probabilities were taken with ONNX Runtime 1.22, the version this build targets. ONNX Runtime 1.23 and later differ by up to about 1.4e-3 on its long text.
- Retrieval ranking was outside the first embedding slice; similarity selection lives in `lettuce-app`.
