# lettuce-conversations

Unified direct and group conversations, messages, revisions, branches, and generation turns.

## Boundary

Provider execution and concrete persistence are injected ports.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.
