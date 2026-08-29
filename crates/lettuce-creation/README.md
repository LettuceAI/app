# lettuce-creation

Resumable AI-assisted authoring plus remote catalog discovery, inspection, and
import preparation.

## Boundary

Creation and discovery remain separate internal modules and commit only through
normal transfer, media, and authored-domain use cases.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.

The verified creation-helper progressive-edit and finalization behavior is
pinned in
`fixtures/legacy-import/creation-helper-tool-scenarios-v1.json`. The first
implementation slice is a durable proposal workflow with immutable draft
revisions, pure target-specific text/scene operations, and explicit review and
confirmation stages. Direct authored-domain writes, image/media work, provider
continuation, administrative tools, and frontend events remain outside that
slice.

Single-entry, keyword, and staged lorebook-generation behavior is pinned in
`fixtures/legacy-import/lorebook-generation-tool-scenarios-v1.json`. Generation
owns durable reviewed proposals; `lettuce-context` remains the only lorebook
domain owner and final apply must use its expected-revision use cases atomically.
Process-local job state, permissive free-text tool inference, positional draft
identity, partial commit, and cancel-by-deletion are not preserved.
