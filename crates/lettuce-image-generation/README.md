# lettuce-image-generation

Capability-aware remote and local image requests, Stable Diffusion runtime,
LoRA application, upscale, jobs, output ingestion, and provenance.

## Boundary

Remote orchestration and the local SD runtime remain separate internal modules.
Permanent outputs exist only after media validation.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.
