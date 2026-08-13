# lettuce-settings

Typed settings, validation, defaults, scope resolution, secret references, and
secure-store ports.

## Boundary

Persists through injected ports and never reads SQLite directly. Secret bytes
are scoped store values and never ordinary settings or serializable DTOs.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first slice is intentionally small: one opaque serializable `SecretRef`, a
bounded purpose vocabulary, redacted zeroizing `SecretValue`, metadata/status
types, and one object-safe asynchronous `SecretStore` port. The in-memory store
is a deterministic reference adapter for tests only; it does not encrypt,
authenticate, persist, or provide platform protection.

An OpenRouter adapter will use the boundary like this: the provider account owns
an opaque `SecretRef` with `ProviderApiKey { owner }`; a settings read returns
only the reference and `status(ref, purpose)`. The provider adapter calls
`load(ref, purpose)` for a bounded `SecretValue`, borrows it briefly while
constructing the authorization header, and then drops it. Initial creation uses
`put(record, value, None)`; rotation uses the same metadata plus
`Some(status.generation)` and the store computes the next generation. Stale
writes are rejected. A missing or unavailable secret is reported as a typed
state/error, never as an empty key or backend message.

Deletion is cleanup-idempotent: an absent reference returns `Missing` even
with an observed generation, while live entries validate purpose and reject a
stale observed generation.

This slice does not implement encryption, Android Keystore, desktop keyrings,
native secret entry, IPC, database persistence, ordinary settings sections, or
portable backup/sync vaults. Those belong to platform/application and
transfer/sync work once a real authenticated format exists; no placeholder vault
or ciphertext API is exposed here.
