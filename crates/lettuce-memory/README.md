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
Semantic duplicate hints are qualified typed evidence: stable target ID,
embedding source revision, supported dimension count, cosine score, and applied
threshold. The reducer rejects unqualified evidence and still verifies that the
target exists in its current snapshot.

The SQLite `MemoryRepository` implementation and first admitted-round
application handler are now wired through `lettuce-database` and `lettuce-app`.
The domain-owned cross-aggregate commit port carries one optional memory change
plus a complete terminal tool-transition batch. Infrastructure must commit both
or neither; orchestration and provider continuation remain application-owned.
The domain also owns an immutable dynamic-memory preparation-plan port. A plan
captures its attempt/job and memory revision, policy and duplicate threshold,
ordered execution IDs, and every create seed, exact source text, embedding
source/dimensions, and qualified semantic evidence. Exact retries are
idempotent; changed identity conflicts so interrupted recovery cannot silently
recompute evidence against a different snapshot.
The same port exposes one atomic interrupted-parent to immediate-child recovery
operation. It returns newly identified running child executions plus their
remapped immutable plan, while preserving exact call arguments and prepared
semantic evidence from the terminal parent.
