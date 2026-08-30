# lettuce-creation

Resumable AI-assisted authoring plus remote catalog discovery, inspection, and
import preparation.

## Boundary

Creation and discovery remain separate internal modules and commit only through
normal transfer, media, and authored-domain use cases.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The verified creation-helper progressive-edit and finalization behavior is
pinned in
`fixtures/legacy-import/creation-helper-tool-scenarios-v1.json`. The first
implementation slice is active: versioned new/edit targets, durable user turns,
immutable draft proposal revisions, bounded pure target-specific text, scene,
and lorebook-entry operations, ordered typed outcomes, and explicit
awaiting-review/awaiting-confirmation stages are owned here. Failed operations
do not roll back later valid operations in the same provider order. The
repository port persists input before inference and advances proposal lineage
with workflow revision CAS; exact retries return the immutable stored records.

The SQLite implementation lives in `lettuce-database`. No operation in this
slice writes character, persona, or lorebook aggregates. Final authored-domain
apply, provider continuation, image/media leases, administrative tools, and
frontend events remain later slices.

Single-entry, keyword, and staged lorebook-generation behavior is pinned in
`fixtures/legacy-import/lorebook-generation-tool-scenarios-v1.json`. Generation
owns durable reviewed proposals; `lettuce-context` remains the only lorebook
domain owner and final apply must use its expected-revision use cases atomically.
Process-local job state, permissive free-text tool inference, positional draft
identity, partial commit, and cancel-by-deletion are not preserved.
