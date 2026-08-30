# lettuce-contracts

Versioned IPC requests, responses, events, and generated frontend bindings.

## Boundary

Transport types only; domain crates must not depend on this crate.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first provider contracts are active: the provider catalog and capability
metadata, account-scoped model discovery, and key-verification results. These
owned serde DTOs keep domain/provider implementation types out of the IPC and
future generated frontend boundary.

Prompt-cache catalog metadata includes the exact typed retention choices for
each configurable provider; frontend code does not infer provider wire values.
Provider descriptors separately expose native tool translation, structured
output, signed tool replay, and reasoning-with-tools support so callers do not
infer unsafe feature combinations from a protocol name or a model capability.
