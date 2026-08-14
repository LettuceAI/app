# lettuce-conversations

Unified direct and group conversations, messages, revisions, branches, and generation turns.

## Boundary

Provider execution and concrete persistence are injected ports.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The crate now exposes the persistence-ready V1 domain contracts: one tagged
direct/group aggregate, immutable launch snapshots, participants, branches,
messages with immutable revisions/candidates, and the durable generation-turn
lifecycle. Repository methods are synchronous and transaction-shaped; runtime
dependencies are separate asynchronous ports. No database schema, provider
adapter, migration, frontend, or engine client belongs here.

All authored documents and message parts are bounded and closed under
`serde(deny_unknown_fields)`. Provider replay artifacts are versioned opaque
references (artifact ID, digest, schema, codec, retention and size), with no
provider payload bytes in conversation rows or public DTOs. Streaming text is
held only in a non-serializable sequenced UI envelope; durable checkpoints and
outbox events contain stages, counters and typed IDs only.

Launch documents carry a format version and protected snapshot references.
Inherited selections contain their fully resolved value, while mutable current
settings remain separate from immutable launch history. Branch history uses
parent links, depth/order and bounded validators; operation records bind CAS
mutations to request digests and idempotency keys.
