# lettuce-memory

Memory provenance, extraction, retrieval, consolidation, and vector-index ports.

## Boundary

Source truth is independent from rebuildable embeddings.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first direct/group dynamic-memory slice now provides versioned required
tool declarations, strict typed arguments and outcomes, a deterministic ordered
reducer over revisioned memory-space snapshots, and an atomic compare-and-apply
repository port. It preserves duplicate suppression, low-confidence demotion,
cycle-start hard-delete limits, pin protection, hot-token budgeting, and
capacity trimming. The verified scenarios are pinned in
`fixtures/legacy-import/dynamic-memory-tool-scenarios-v1.json`.

The new contract deliberately replaces ambiguous legacy text or six-digit
delete targets with stable memory UUIDs, reports missing targets explicitly,
and applies one validated change set instead of mutating session projections in
place. Provider continuation, prompt rendering, companion-only source and
supersession, provider-driven category repair, repository SQL, ONNX embedding
inference, and UI events remain later slices. ONNX runtime ownership belongs in
`lettuce-embeddings`; this crate consumes typed duplicate evidence through the
application coordinator.
