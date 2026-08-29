# lettuce-memory

Memory provenance, extraction, retrieval, consolidation, and vector-index ports.

## Boundary

Source truth is independent from rebuildable embeddings.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.

The verified direct/group dynamic-memory tool behavior and approved corrections
are pinned in
`fixtures/legacy-import/dynamic-memory-tool-scenarios-v1.json`. The first
implementation slice is the deterministic core memory-operation reducer and
atomic repository port. Provider continuation, companion-only
source/supersession behavior, category-repair generation, and UI events remain
outside that slice.
