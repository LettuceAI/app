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
