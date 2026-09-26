# lettuce-jobs

The lifecycle of long-running work: job identity and state, claims and leases, stages and progress, cancellation, retries, parent and child jobs, crash recovery, retention, and an event history per job.

The crate knows nothing about what a job does. It has no Tauri, SQLite, provider, filesystem or feature-specific payloads. A job points at its input and its result through typed `OutcomeRef`s whose records belong to the domain crates; this crate owns only the lifecycle truth. Executors, feature checkpoints and follow-up work live in `lettuce-app`, and durable storage in `lettuce-database`.

## Structure

- `model.rs`: the vocabulary. `JobSpec` (also `NewJob`) is a submission: `JobKind`, `JobSubject`, optional `IdempotencyKey`, optional parent, `CorrelationId`, input `OutcomeRef`, `JobPriority`, `RecoveryPolicy`, `CancellationPolicy` and the `ResourceClass`es it needs. `JobSnapshot` is the current state of one job. `JobState` holds the transition table.
- `store.rs`: the `JobStore` trait, `InMemoryJobStore` (the one implementation of the lifecycle rules), `JobMutation`, `StoredJobRecord`, `Clock`/`SystemClock`/`FakeClock`, and the claim, recovery and retention results.
- `events.rs`: `JobEvent` and `JobEventEnvelope`, the per-job history.
- `handle.rs`: `JobHandle` and `CancellationToken` for executors.
- `recovery.rs`, `retention.rs`, `registry.rs`, `scheduler.rs`: `RecoveryAction`, `RetentionPolicy`, the per-kind `JobRegistry` of policies, and `ResourceClaim`.

## Kinds and subjects

`JobKind` names the operation: artifact install and verify, runtime prepare, model load, memory extraction and consolidation, companion growth, consolidation and Soul writer, conversation generation, vector index build, creation run, image generation, media transform, transfer import and export, backup export and restore, sync session, speech transcription and synthesis, embedding benchmark, maintenance.

Kinds are split finely on purpose. Companion Soul growth runs after a successful memory job and must fail or retry without touching that completed job, so it has its own kind. Companion consolidation follows a successful growth job, targets the character's Soul, and likewise must not change the growth job when it skips, fails or retries, so it is separate from generic memory consolidation. The feature input and checkpoints of these jobs belong to the application. `ConversationGeneration` is one direct conversation generation attempt.

`JobRegistry` maps each kind to a policy (recovery, cancellation, resources) through a private numeric key. New kinds get a new key at the end, and a test asserts every kind maps to a distinct key, so inserting a variant in the middle of the enum cannot make two kinds share a policy.

The crate checks only that a job's resource classes are non-empty and distinct. Which resources a kind needs is decided where the kind is registered, so a remote image executor can declare network access instead of the GPU without a default baked in here.

A `JobSubject` is a `SubjectKind` (conversation, group, memory space, artifact install, backup and so on), a `SubjectId` and an optional display `SafeLabel`.

## Safe text

Events and snapshots cross into the UI, so nothing in them can carry arbitrary JSON, provider bodies, paths, credentials or base64. `SafeLabel` is at most 128 characters without control characters, `/`, `\`, `:` or `?`. `SubjectId` and `IdempotencyKey` are at most 128 ASCII characters from a small alphabet. `SubjectId` and `IdempotencyKey` print as redacted in `Debug`; `SafeLabel` prints only its length. These are bounded, caller-attested fields, not scrubbers: callers put machine-facing stage names, error codes and translation keys in them, never prompts, credentials, paths or provider bodies. `JobError` pairs a `JobErrorCode` with a retryable flag, a `SafeLabel` message and an optional retry delay.

## Lifecycle

The legal transitions (`JobState::can_transition_to`):

| From | To |
| --- | --- |
| `Queued` | `Claimed`, `CancellationRequested`, `Interrupted` |
| `Claimed` | `Running`, `CancellationRequested`, `Queued`, `Interrupted` |
| `Running` | `CancellationRequested`, `CleaningUp`, `Succeeded`, `Failed`, `Queued`, `Interrupted` |
| `CancellationRequested` | `CleaningUp`, `Succeeded`, `Failed`, `Interrupted` |
| `CleaningUp` | `Cancelled`, `Failed`, `Interrupted` |

Terminal states are `Succeeded`, `Failed`, `Cancelled` and `Interrupted`, and a terminal job and its events never change again.

Every change goes through `append_and_transition(JobMutation)`, which updates the snapshot and appends the matching event under one lock, so the snapshot always equals the replay of its events. Mutations after the claim carry the `ClaimRef` (job, worker, attempt, lease id) and fail with `StaleLease` if it is not the job's current claim or `LeaseExpired` if the time is past the lease. Timestamps may not run backwards (`TimestampRegression`).

1. Create. `create_or_get` validates the spec (at least one resource class, no duplicates) and inserts the job as `Queued` with `Created` and `Queued` events. With an idempotency key, the same key and the same submission return the existing job; the same key with a different submission is `IdempotencyConflict`. A child names a non-terminal parent and is attached to it as a required child.
2. Claim. `claim_next` picks the highest-priority queued job whose resources the caller's `ResourceAvailability` allows, oldest first within a priority. `claim(id, ...)` claims one exact job with the same lease and resource checks and leaves the rest of the queue alone. A claim increments the attempt (the first claim is attempt 1) and issues a new lease. The `Claimed` event records the input, policies and resources the worker received.
3. Run. `Start` moves to `Running`. `StageChanged` sets a new `StageSnapshot` and resets progress. `Progress` merges with the previous snapshot (omitted dimensions are kept), must not exceed a declared total, must not change a total, and must not go backwards within a stage. `Checkpoint` records an `OutcomeRef` the executor can resume from.
4. Heartbeat. `heartbeat` extends the lease. The boundary is inclusive: a heartbeat or mutation at the expiry instant is accepted, one after it is not. A heartbeat never shortens the stored expiry.
5. Settle. `Succeed` needs every required child to have succeeded. `PartiallySucceed` ends in `Succeeded` with warnings. `Fail` stores the `JobError`. `RetryScheduled` returns a claimed or running job to `Queued`; the next claim increments the attempt once.

Snapshots keep the idempotency key, so a scheduler can tell whether the active job for a subject is exactly the logical batch it would submit or whether new work has arrived, without reading the stored spec.

## Cancellation

`CancellationPolicy` is `Cooperative`, `UntilIrreversibleStage` or `NotCancellable`. A request records the reason (user, shutdown, parent, timeout, recovery) and moves the job to `CancellationRequested`. It is rejected for `NotCancellable` jobs and, for `UntilIrreversibleStage`, once a claimed or running job has entered a stage marked irreversible. A repeated request is a no-op.

A running job then calls `RequestCleanup` and `FinishCancellation`, so cleanup is always a recorded state. A job that finishes its work just as the stop arrives may still settle as `Succeeded` or `Failed`, which keeps its real outcome. A queued job that was never claimed is closed with `finish_queued_cancellation` (cleanup and cancelled in one step).

In process, `JobHandle` carries a cloneable `CancellationToken` with an atomic `is_cancelled` check and an async `cancelled()` notification, so an executor can abort blocked I/O without polling. A host can build the handle around an existing token (`with_cancellation`) when a command's lifetime should cancel the executor; the job id stays explicit and the store still owns the durable state.

## Recovery

`expired_claims(now, limit)` releases claims whose lease has passed. What happens depends on where the job was:

- `CleaningUp`: it ends `Interrupted`. Cleanup may already have had side effects, so the job is never requeued and the store does not claim that cleanup finished.
- `CancellationRequested`: it goes through cleanup to `Interrupted`.
- otherwise the persisted `RecoveryPolicy` decides: `Resume` and `Restart` requeue it, `Compensate` and `MarkInterrupted` end it `Interrupted`.

Each `ExpiredClaim` returns the recovery policy; `recovery_action()` turns it into a `RecoveryAction`. `Compensate` is a request for follow-up work in the application, not a statement that it happened.

`orphaned_claims` does the same for every claim regardless of its lease. It runs at startup, when no worker of the previous process can still be alive, and settles each job at `now` or at its last update if that is later. `lettuce-app`'s startup job recovery calls it.

## Retention

`prune(RetentionPolicy, now)` removes terminal jobs older than `keep_terminal_for` (default 30 days; `None` keeps everything). A whole old terminal parent and child graph is removed together, but any job linked to a parent or child that is not itself prunable (not terminal, too new, or outside the set) is kept, and optional links count as links. Pruning removes lifecycle records only, never a domain outcome or anything an `OutcomeRef` names.

## Events

Each job has a gapless event sequence starting at 1, every envelope carrying the job id, sequence, time and correlation id. `events_since(id, after, limit)` pages through it (limit 1 to 1000), and `list` pages snapshots filtered by state, kind or subject, ordered by creation time, with a `PageRequest`. Events are typed references and counters only; there is no field for a path or raw payload.

## Durable storage

`InMemoryJobStore` is both the test store (deterministic with `FakeClock`) and the rule engine for the durable one. `StoredJobRecord` is the persistence-neutral aggregate (spec, snapshot, events). The SQLite adapter in `lettuce-database` loads the records an operation touches, rebuilds a store with `restore_working_set`, applies the operation there, and writes back what changed, so there is one implementation of the lifecycle rules. `restore_working_set` accepts a trailing window of each job's events and links to jobs outside the set; `restore` requires full histories and a closed graph and is used for backup restore. Both check that snapshot and spec agree and that the event sequence is contiguous.
