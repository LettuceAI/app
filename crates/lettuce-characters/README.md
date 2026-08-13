# lettuce-characters

Versioned characters, user personas, reusable groups, authored scenes,
starters, presentation references, defaults, and repository ports.

## Boundary

Character, persona, and group remain independent internal aggregates with
separate revisions and mutation APIs. This crate does not own conversations,
imports, prompt bodies, media bytes, or storage implementations.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.
