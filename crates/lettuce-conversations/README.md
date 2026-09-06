# lettuce-conversations

Initial provider dispatch has a durable checkpoint contract. `InitialInferenceBinding`
fingerprints the prepared request (turn, attempt, operation, resolved profile,
context, job, media grants and tools) without the stream sink, so a reattached
caller replays the same dispatch. `InitialInferenceRepository` admits one pending
dispatch per attempt, settles it once with either the exact `InferenceOutcome`
or a typed `PortError`, and returns the stored record for exact replay. Changed
requests conflict, a pending record is reported as pending rather than
redispatched, and a settled response keeps its conversation-retained replay
references coherent. Response interpretation and tool admission remain separate.

Generation preparation uses `PrepareGeneration` through the existing repository
and manager. It binds the resolved model and context attributions to the turn,
checks attached attempt/job ownership and revisions, and advances ContextPrepared
atomically. Operation replay is idempotent; different preparation conflicts.
Existing group speaker resolution remains separate. Explicit director targets and
original regeneration authors can prepare without another selection step; mention
decisions need no automatic selection. The database verifies attached model
artifact provenance. Preparation stores provenance,
not the full provider request or resolved generation parameters.

InferenceUsage retains optional provider-reported cost with a validated finite,
nonnegative value type. JSON round trips preserve the existing f64 precision;
unknown amounts remain distinct from zero. Invalid numeric values cannot enter
the type, so usage records retain their equality contract.

Unified direct and group conversations, messages, revisions, branches, and generation turns.

## Boundary

Provider execution and concrete persistence are injected ports.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

Tool calling uses the same boundary. This crate owns bounded provider-neutral
definitions, request selection, proposed calls, transcript call/result values,
and the attempt-scoped `ToolExecution` lifecycle. It does not own handlers or
provider JSON. Application code persists a provider proposal before execution;
message parts keep only the resulting execution ID. Terminal executions cannot
regress, and provider call IDs are separate from the stable application ID so
providers without native IDs do not force fabricated wire identities.
`ConversationManager` validates a provider call set against the exact declared
request, assigns stable wire ordinals and handler versions, and persists the
whole set atomically before any handler starts. Undeclared calls, named-choice
mismatches, duplicate provider IDs, and over-limit sets fail before storage.
Later continuation rounds append after prior executions with an expected-next
ordinal, so a recovery worker cannot interleave a second copy of the round.

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

Provider-native signed replay uses the separate
`ProviderReplayArtifactPort`. It can stage, verify, and materialize exact
protected replay bytes for a trusted provider adapter without adding read-back
to ordinary conversation repositories or serializable contracts.

Generation attempts own their scheduler idempotency keys and optional job IDs.
Attaching a job is a separate CAS mutation, so retries and recovery children
cannot accidentally coalesce with an earlier attempt. Cancellation is a
two-step lifecycle: the repository commits `CancellationRequested`, the
application asks the job runtime to stop, and settlement commits the terminal
usage-linked `Cancelled` state. Checkpoints are operation-bound and must begin
at sequence one. The repository exposes the latest sequence for an exact
turn/attempt so checkpoint producers can append after durable streaming events,
including after restart. Runtime-owned stage checkpoints advance the named
attempt from created to preparing to running in the same transaction as the turn
stage, so persisted attempt state matches the job state consumed by tool
workflows.
Terminal outbox events carry the attempt, usage,
message/candidate references where applicable, and bounded memory revision
references so downstream consumers can be idempotent by turn ID without
reading conversation internals.

Job attachment is repository-wide, not merely turn-local: an attempt may be
attached once, and a non-null job ID may belong to only one attempt across all
conversations. The final conversation schema must enforce this with a partial unique index on
`generation_attempts(job_id)`, while the domain reports separate
`JobAlreadyAttached` and `JobInUse` failures.

Conversation creation receives a non-cloneable `PreparedConversationLaunch`.
The application planner validates the complete plan, traverses both launch
settings and initial-message origins, and hands over exactly one validated
draft for every distinct protected snapshot reference. The repository consumes
that bundle in one adapter transaction; it does not perform a separate
pre-verification that could race with staging. The same-database artifact store
remains available on the full repository for replay finalization and trusted
retention workflows, with orphan cleanup for staged payloads.

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

Conversation creation also carries a bounded, versioned initial timeline. It
materializes the selected scene and direct-chat starter messages as ordinary
immutable message revisions with stable IDs, while retaining normalized source
attribution to the selected protected scene or starter snapshot. `Scene` is a
distinct authorless message role: request assembly must not silently convert it
to a provider `system`, `user`, or `assistant` message. Scene content is supplied
to prompting through the scene-context path instead.

An origin row proves which immutable launch snapshot was selected; it does not
decrypt that snapshot or independently prove that arbitrary text came from it.
The application composition layer must therefore build
`PreparedConversationLaunch` through the conversation launch planner from the
same validated authored scene/starter graph used to create the protected
artifacts. IPC callers do not construct initial timelines, and the database
creator only persists and revalidates the prepared plan atomically.

Tool execution is currently a horizontal contract only. Remote OpenAI-envelope,
Anthropic, Gemini, and Ollama adapters have family-specific declaration,
transcript, buffered-response, and streaming codecs. The application
coordinator and legacy memory/creation/companion/lorebook handler migrations are
separate usable slices; arbitrary shell, filesystem, plugin, and general-chat
tools are not implied by this contract.

The direct/group dynamic-memory scenario audit is pinned in
`fixtures/legacy-import/dynamic-memory-tool-scenarios-v1.json`. Conversation
ownership ends at durable call admission and execution lifecycle: memory
operation semantics and atomic memory-space changes belong to `lettuce-memory`,
while continuation and recovery orchestration belong to `lettuce-app` and
durable jobs.
Dynamic launch memory snapshots optionally retain the exact retrieval and
mutation policy values used by generation. The optional field keeps older
snapshot documents readable; new dynamic launches populate it, while manual and
disabled modes reject it.

One admitted handler round can transition through the repository's typed batch
CAS. The batch requires one conversation/turn/attempt owner and unique execution
IDs; a stale revision, invalid transition, or storage failure rolls back every
execution rather than exposing a partially running or terminal round.

Launch snapshots stay frozen for the conversation lifetime. Current participant
policy and settings are explicit mutable state; this contract does not derive
live values from cross-conversation sources.

InferenceOutcome carries optional provider_response_id independently of the
HTTP provider_request_id. OpenAI buffered/SSE adapters populate it for billing
lookups; it is not an internal attempt or candidate identity. Durable companion
dispatch responses retain it; conversation/creation/memory checkpoints do not
yet persist this additional field.

Provider-backed group speaker selection has a separate immutable checkpoint
contract from the initial generation dispatch. Its request binding covers the
conversation, turn, attempt, job, resolved profile, prompt context and required
tool request. Admission returns the usage-event identity for the actual provider
call; settlement stores only the validated final speaker decision. A pending
record blocks redispatch, while a settled record can replay the decision without
retaining a second copy of the provider response.
An LLM group launch may retain a dedicated speaker-selection model snapshot.
The optional field is backward-compatible for existing snapshot documents and
is valid only with the LLM policy. Its protected model artifact freezes the
model and provider-account revisions independently from the responding member's
generation model.
