# lettuce-settings

Typed settings, validation, defaults, scope resolution, secret references, and
secure-store ports.

## Boundary

Persists through injected ports and never reads SQLite directly. Secret bytes
are write-only scoped values and never ordinary settings or serializable DTOs.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.
