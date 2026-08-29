# lettuce-companions

Relationship events, projections, growth policies, milestones, and scheduled effects.

## Boundary

Does not read chat storage or invoke models directly.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.

The verified companion growth, consolidation, and Soul-writer behavior is
pinned in `fixtures/legacy-import/companion-tool-scenarios-v1.json`. The first
implementation slice is the pure typed Soul/growth policy: category mutability,
confidence and weight gates, semantic-slot supersession, locked-fact
protection, and atomic consolidation change sets. Provider calls, prompt
rendering, job coordination, database adapters, and frontend events remain
outside that slice.
