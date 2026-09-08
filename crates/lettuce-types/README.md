# lettuce-types

Shared IDs, timestamps, revisions, pagination, hashes, and stable application errors.

## Boundary

No internal LettuceAI dependencies and no I/O.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

ASR vocabulary and correction records use distinct UUID-backed IDs so learning
data never exposes SQLite row identities.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.
