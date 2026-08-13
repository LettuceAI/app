# lettuce-media

Content-addressed user media, validation, derivatives, serving, retention, and repair.

## Boundary

Exposes asset IDs, never managed native paths.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.
