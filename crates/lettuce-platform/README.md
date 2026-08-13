# lettuce-platform

Operating-system capabilities, application lifecycle, managed paths, confined
filesystem operations, updater integration, and user-intent ports.

## Boundary

Does not expose a global Tauri/shell handle or arbitrary application root path.
Filesystem consumers receive purpose-specific capabilities.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.
