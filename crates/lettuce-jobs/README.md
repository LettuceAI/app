# lettuce-jobs

Durable operation state, scheduling, progress, cancellation, and recovery.

## Boundary

Contains no feature-specific business state.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Slice 0 status

The crate now provides the domain-independent foundation:

- validated opaque job, lease, worker, correlation, event-sequence and
  idempotency identities;
- typed subjects, outcome references, safe labels/errors, progress and stages;
- a checked lifecycle state machine with durable cancellation and cleanup
  semantics;
- an atomic, thread-safe in-memory `JobStore` reference implementation and a
  persistence-neutral aggregate bridge used by the SQLite adapter, with
  idempotent create-or-get, event cursors, pagination, next-job and exact-job
  claims, heartbeats and expired-lease recovery; and
- policy, recovery, retention, handle and resource-admission vocabulary for
  later database/application adapters.

Snapshots retain the optional caller idempotency key so application schedulers
can distinguish an exact active logical batch from newly arrived work for the
same subject without opening the store's private specification record.
Exact-job claims use the same lease and resource checks as ordinary queue
claims while leaving unrelated queued work untouched.

Request handles expose a cloneable cancellation token with both an atomic
instant check and an async notification, allowing executors to interrupt
blocked I/O without polling.

The in-memory store is deterministic when constructed with `FakeClock`; the
SQLite implementation lives in `lettuce-database` and restores every durable
aggregate through this same lifecycle reducer. Events have no
structural slot for arbitrary JSON, provider response bodies, local paths,
credentials or base64 payloads. `SafeLabel` and `SubjectId` are bounded,
caller-attested text fields—not secret scrubbers—and their `Debug` output is
redacted. Adapters should supply machine-facing stage/error/translation keys
and must never place prompts, credentials, paths or provider bodies in these
fields. Domain crates own the records named by typed `OutcomeRef` values; this
crate owns only lifecycle truth. Slice 0 validates resource snapshots for
nonempty, deduplicated classes; kind/subject compatibility is intentionally
registration-owned so a remote image executor can declare network rather than
GPU admission without a universal default baked into this crate.

`CompanionGrowth` is a distinct durable kind because Soul growth is derived
after memory success and must fail or retry without changing the completed
memory job. Its feature input and checkpoints remain application-owned.

Lease timestamps use an inclusive boundary: a mutation or heartbeat at the
expiry instant is accepted, while a timestamp after expiry is rejected. A
heartbeat never shortens the authoritative snapshot expiry. The first claim
is attempt 1; retries return to `Queued` and the next claim increments once.
Lease scavenging follows the persisted recovery policy and treats a requested
cancellation as cleanup/recovery work rather than illegally moving it back to
`Queued`. If a lease expires after cleanup has started, the store records
`Interrupted` and does not claim that cleanup or domain compensation completed;
`RecoveryAction::Compensate` tells the application to perform that follow-up.
Retention with `keep_terminal_for: None` retains records indefinitely. With a
finite retention window, an old terminal parent/child graph may be pruned
together, while any node referenced by a nonterminal, too-new, or otherwise
external parent/child is retained. Optional links count as references too.
