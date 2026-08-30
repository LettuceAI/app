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

The native proposal-tool contract is also active. Drafting exposes only the
versioned mutators valid for the current target plus `show_preview`; review
exposes only `request_confirmation`, and confirmation exposes no tools. Exact
declaration/version checks reject undeclared or malformed calls before
reduction. Valid calls reduce in provider order into one proposal and one typed
result per call, including operation errors without stopping later calls.
Application-generated scene and lorebook-entry IDs are deterministic across an
exact proposal retry, and the repository CAS commits the proposal before any
later provider-continuation work.

Creation turns also own durable inference attempts before provider dispatch.
Each attempt pins its immutable base and planned proposal identities, retry
parent, target/stage-specific tool request, job identity, exact resolved-profile
fingerprint, ordinal, lifecycle, and failure state. Retry children must keep the
profile binding and use a distinct job. Native calls are admitted atomically in
provider order before reduction, including their exact definition version,
provider identity, arguments, raw arguments, and protected replay reference.
Exact retries return the stored evidence; stale bases, changed retries,
cross-turn owners, undeclared tools, version drift, reused jobs, profile drift,
and duplicate identities fail closed.

Each attempt additionally checkpoints up to eight immutable provider-response
rounds. Round evidence preserves mixed visible text/reasoning, candidate replay,
and the exact contiguous call range, including text-only terminal responses.
This makes batch ownership and continuation recovery explicit instead of
guessing round boundaries from timestamps or a flat call list.

Remote provider dispatch and recursive continuation are active through the
`lettuce-app` coordinator. User-turn/first-attempt admission is one atomic port
operation bound to the exact workflow revision; it never exposes a durable turn
without its dispatch attempt. The explicit interrupted-parent recovery contract
remains a later slice.

Single-entry, keyword, and staged lorebook-generation behavior is pinned in
`fixtures/legacy-import/lorebook-generation-tool-scenarios-v1.json`. Generation
owns durable reviewed proposals; `lettuce-context` remains the only lorebook
domain owner and final apply must use its expected-revision use cases atomically.
Process-local job state, permissive free-text tool inference, positional draft
identity, partial commit, and cancel-by-deletion are not preserved.
