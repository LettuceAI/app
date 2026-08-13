# lettuce-models

Provider connections, model profiles, capability evidence, and selection.

## Boundary

Does not execute inference or install model files.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Foundation contracts

`ProviderAccount` owns a stable connection identity, protocol, endpoint,
enabled state, typed non-secret configuration, and opaque secret references.
Custom header values are never serializable strings; each is a `SecretRef`.

`ModelProfile` always points to an explicit provider account. There is no
provider-label, sole-account, or first-account fallback. Repository ports are
synchronous because the SQLite adapter is synchronous; application composition
places calls on its database worker.
