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

Immutable snapshot and replay payloads are written through the private
`ConversationArtifactStore` boundary. `ProtectedArtifactBytes` is bounded,
redacted in `Debug`, not serializable, and zeroized on drop; only a store or
backup adapter should open it. Stores make artifact identity immutable and
deduplicated, and expose only verification plus explicit orphan cleanup;
backup exports reuse conversation-retained references rather than creating a
second retention identity. Ordinary repositories and IPC never carry raw
artifact bytes.

Generation attempts own their scheduler idempotency keys and optional job IDs.
Attaching a job is a separate CAS mutation, so retries and recovery children
cannot accidentally coalesce with an earlier attempt. Cancellation is a
two-step lifecycle: the repository commits `CancellationRequested`, the
application asks the job runtime to stop, and settlement commits the terminal
usage-linked `Cancelled` state. Checkpoints are operation-bound and must begin
at sequence one. Terminal outbox events carry the attempt, usage,
message/candidate references where applicable, and bounded memory revision
references so downstream consumers can be idempotent by turn ID without
reading conversation internals.

Job attachment is repository-wide, not merely turn-local: an attempt may be
attached once, and a non-null job ID may belong to only one attempt across all
conversations. The M8 schema must enforce this with a partial unique index on
`generation_attempts(job_id)`, while the domain reports separate
`JobAlreadyAttached` and `JobInUse` failures.

The repository exposes its same-database artifact verifier and must verify all
launch snapshot references before create and replay references before
finalization. Verification failures are typed repository errors and prevent
the transaction from staging; payloads staged before a failed mutation are
cleaned through the artifact store's orphan-cleanup contract.

Trusted encrypted backup/sync composition receives a separate
`ConversationArtifactTransferPort`. It streams chunks into a
`TrustedArtifactSink` and is deliberately not reachable through
`ConversationRepository::artifact_store`; ordinary repositories have no
artifact read-back API and no raw payload DTOs.

Launch documents carry a format version and protected snapshot references.
Inherited selections contain their fully resolved value, while mutable current
settings remain separate from immutable launch history. Branch history uses
parent links, depth/order and bounded validators; operation records bind CAS
mutations to request digests and idempotency keys.

Launch snapshots stay frozen for the conversation lifetime. Current participant
policy and settings are explicit mutable state; this contract does not derive
live values from cross-conversation sources.
