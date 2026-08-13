# lettuce-transfer

Bounded external-format parsing, canonical documents, import plans, exports,
consistent encrypted backups, staged restore, and rollback.

## Boundary

Compatibility transfer and full backup remain separate internal modules.
External schemas never become internal domain schemas, and restore never
destructively replaces live state before complete verification.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.
