# lettuce-providers

Remote provider catalogs, request translation, stream parsing, and error mapping.

## Boundary

Provider-native DTOs remain private adapters.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.
