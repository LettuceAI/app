# lettuce-jobs

Durable operation state, scheduling, progress, cancellation, and recovery.

## Boundary

Contains no feature-specific business state.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.
