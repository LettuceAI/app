# lettuce-app

The sole composition root, application workflows, desktop IPC boundary, and
optional local OpenAI-compatible host API.

## Boundary

May wire all crates; no crate may depend on it. Host API handlers reuse the
model and inference services without constructing product conversations.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The composition root initializes storage and built-in prompts, plans direct and
group conversation launches, assembles provider-neutral context, and can build
a reusable remote-provider runtime. Provider composition requires the host to
supply its real `SecretStore` and current TLS policy; the app never substitutes
an in-memory credential store in production. The runtime loads provider
accounts through `lettuce-database` repositories and exposes catalog, model
discovery, and key verification through `lettuce-contracts` DTOs. It also owns
and exposes the matching `InferenceRuntime`, so later generation workflows can
register bounded stream receivers and cancellation tokens against the same
runtime used by the provider adapters.

Direct companion launch is now a usable storage-initialization path. The
planner freezes the validated authored companion defaults and effective
persona scope in a non-serializable prepared launch, and the database commits
the conversation graph, artifacts, operation/outbox, fresh session emotion,
and carried character/persona relationship in one transaction. Exact launch
retries revalidate the initial-state request without resetting relationship
continuity. Group companions and growth scheduling remain deferred.

Direct user sends can now pass through `CompanionTurnCoordinator`. It detects a
companion from its normalized runtime state, resolves the stored direct
character/persona owner, reads the current authored companion config like the
legacy completion flow, classifies ordered user text through a narrow engine
capability, applies the copied pure signal and turn math, then submits the
atomic prepared companion send. Blank, missing, and unavailable classification
use the legacy neutral update; cancellation stops before persistence. Exact
send replay bypasses classification and never reapplies state. Roleplay and
group sends retain the ordinary conversation repository path.

Direct companion launch now selects the authored nested companion prompt
template after any explicit starter template and otherwise falls back to the
bundled companion prompt; a missing or archived inherited companion template
also falls back without borrowing the ordinary direct-chat prompt. Context
assembly reads the current authored Soul/prompting config, character-owned
Soul state, conversation/persona-scoped runtime state, and current persona
name, then renders the legacy prompt-state block at the source message's
effective clock through the existing typed `companion_state` placeholder.
Roleplay and group assembly do not read companion state. Missing or corrupt
companion state fails assembly closed, and the composition root exposes the
fully wired assembler over the shared database ports.

For dynamic-memory companion sends, the coordinator also derives the legacy
turn-effect seed from the already computed transition. Storage keeps that seed
with the admitted turn and creates the visible processing effect atomically
with assistant finalization; companion continuation uses the legacy zero-delta
seed without a user-message owner, and exact replay cannot duplicate it. Typed
ready/failed settlement is available for post-turn dynamic-memory coordination,
and the terminal coordinator now assigns authoritative added/updated memory IDs
to each coalesced effect through its exact user/assistant source window. It
copies the legacy largest-delta summary selection, percent rounding, wording,
pluralization, and three-part cap; no-op success is ready, terminal failures use
bounded stable reasons, and exact settlement replay is idempotent. Processing
effects can now be rediscovered from SQLite in stable order and coalesced per
conversation into background `MemoryExtraction` jobs. The batch's logical
idempotency key is derived from its ordered durable effect identities, so
duplicate discovery reuses one runtime job and a fresh in-memory job store after
restart rebuilds the same logical batch. Empty discovery is the
non-companion/dynamic-memory-disabled no-op. The application bridge maps that
logical batch deterministically to one durable background memory run, resolves
the conversation-owned memory space, and freezes ordered visible user/assistant
messages with each message's exact active revision or candidate plus the
resolved inference profile. Same-job rediscovery replays the processing attempt;
a replacement runtime job restart-recovers it into one child instead of creating
a duplicate run. The first-round inference coordinator materializes those exact
revision/candidate bodies, renders the existing editable dynamic-memory prompt
with the copied legacy budget variables and runtime-input wording, dispatches
the run's frozen profile and required tool contract, and atomically admits the
single provider tool candidate as round zero. A committed round replays without
provider I/O; cancellation settles before admission, and rejected, empty,
mixed-content, undeclared, or inconsistent signed-replay outcomes do not become
durable calls. One admitted background round now reuses the existing create
preparation and typed reducer, then atomically settles its ordered results with
the memory CAS; exact retry returns those results without embedding or reducing
again, and `done` preserves the existing short-circuit behavior. Summary
generation and structured text fallback remain deferred. A settled background
round now appends its admitted native calls and typed results to the exact
durable request context, stops before provider I/O on `done`, or dispatches and
atomically admits the next bounded round with the frozen profile/tool contract.
An already admitted next round replays without provider I/O. Debounce/startup
wiring and binding admission to host startup/finalization remain later slices;
the generic job store itself is still only an in-memory reference store. The background loop now connects the
existing round executor and continuation coordinator until the durable `done`
result. It resumes from the latest admitted checkpoint, skips seed generation,
embedding, reduction, and provider dispatch for already settled/admitted work,
and keeps create IDs, token counts, and timestamps as explicit caller inputs.
The run freezes whether companion time awareness is enabled and uses the matching
legacy tool contract. In enabled runs, a known create source is retained and an
unknown or omitted ID falls back to the latest frozen user/assistant source. In
disabled runs, source attribution is cleared. Freezing source effective times,
timestamped transcript rendering, and observed time/role persistence remain the
next temporal-memory slice.
The existing eight-round and 64-call admission limits remain authoritative.
Terminal success now feeds the run's durable starting snapshot and current
memory snapshot into the existing companion effect coordinator, settles every
coalesced effect idempotently, then marks the attempt succeeded. Failure and
cancellation use the existing bounded effect summaries and attempt failure
codes. Effect-first ordering makes a crash retry converge without leaving a
succeeded attempt whose effects are still processing.
The claimed-job runner composes the existing run admission/recovery,
first-round inference, bounded execution/continuation loop, and terminal
settlement into one application path. Prompt/profile/policy inputs and stable
create seeds stay caller-owned. First-round and loop failures are classified
through the same terminal coordinator before returning their typed error.
One shared startup/post-finalization bridge now rediscovers those durable
processing effects, reuses their deterministic per-conversation admission,
claims each exact job with the ordinary lease/resource lifecycle, starts it,
and returns the admission, claim, and cancellation handle needed by that
runner. Repeated dispatch while a conversation job is active is a no-op, and a
fresh job store rebuilds and claims the same logical effect batch after
restart. Host-specific runtime inputs and terminal job progress/completion
remain outside this bridge.
The matching settlement boundary keeps runtime lifecycle evidence on that same
claim: success records complete progress and the conversation outcome,
provider/tool failures use bounded job errors after the runner has settled the
attempt and effects, and cancellation follows request, cleanup, and terminal
steps. Admission or terminal-settlement errors return the existing job to its
retry queue so processing effects are not stranded behind a terminal
idempotency record.
Conversation launch now creates the authoritative normalized memory space in
the same transaction for every resolved manual/dynamic memory policy, and the
memory repository resolves it directly from the conversation identity. This
removes an external `MemorySpaceId` input from the pending post-turn worker.

The first direct/group dynamic-memory handler path accepts an already admitted
and running ordered tool round, validates the exact v1 feature contract, joins
precomputed create metadata, reduces it against one stored memory-space
snapshot, and compare-and-applies at most one change. It returns ordered typed
provider-neutral outputs for the conversation coordinator to settle durably;
provider continuation remains later coordinator work.

The composition layer can also verify an installed embedding manifest through
`lettuce-model-hub`, load one serialized ONNX embedding runtime, and execute
request-scoped inference with a `lettuce-jobs` cancellation token. Hosts remain
responsible for scheduling this synchronous CPU/model-load work on the job
worker rather than a UI or async-runtime thread.

Dynamic-memory create preparation requires a claimed job carrying model-load,
disk-read, and CPU resources. It embeds candidates at 128 dimensions, compares
only live exact-revision projections, supplies qualified duplicate evidence to
the reducer, and persists a projection only after the memory CAS confirms the
item survived. ONNX unavailability does not discard authoritative memory: the
create proceeds without semantic evidence and leaves rebuildable repair state.
Cancellation still stops preparation instead of degrading to an unembedded
write. The production preparation entry point persists one immutable versioned
plan before returning: exact execution order, create seeds and semantic
evidence, source text, embedding source/dimensions, policy, memory revision,
and attempt/job ownership are all bound together.

The durable coordinator starts every validated execution with one batch CAS.
Production settlement reloads the immutable plan and exact running executions;
caller-supplied policies or preparations cannot bypass that boundary. It uses
the planned memory revision and policy, reconstructs create preparation and
repair metadata without rerunning ONNX or semantic search, then reduces once.
A same-process ready projection may be reused only when every identity, source,
dimension, and preparation field matches the plan; otherwise recovery records
repair work instead of trusting caller data.
For a successful handler round, the optional memory-space CAS and every exact
typed terminal output commit in one SQLite transaction; a stale execution or
memory revision rolls back both sides. Handler-level failure can likewise fail
the whole running round atomically without mutating memory. The earlier
reducer-only application helper is test-private so production callers cannot
accidentally bypass settlement.

Recovery verifies that the supplied job handle owns the generation attempt,
then loads the attempt's durable ordered executions. Fully settled rounds are
returned for exact replay and uniformly validated rounds may enter the atomic
start path. Running/interrupted rounds become restart-eligible only after the
database revalidates their immutable plan against the attached job, exact tool
arguments/order, create source text, and unchanged memory revision. Semantic
duplicate evidence is replayed from that plan rather than recomputed against
newer mutable state. Missing, stale, or mixed recovery state fails closed.
An interrupted attempt is never reopened: after conversation recovery creates
and starts its immediate child attempt with a distinct job, the dynamic-memory
coordinator asks storage to atomically remap the verified parent calls and
preparations into new child execution IDs. Exact retries return the same child
round; parent evidence and terminal execution rows remain untouched.

The first provider-continuation intake replays one exact settled round into the
provider-neutral context under the same running attempt and attached job. A
persisted `done` result stops without another request; otherwise the coordinator
enforces four rounds and 64 total calls, runs the existing inference port, and
either returns one usable text candidate or atomically admits the next declared
dynamic-memory call set with stable ordinals. It rejects stale durable counts,
mixed content/tool responses, multiple candidates, cancellation at each network
boundary. Signed provider replay must use one coherent candidate/call artifact
identity; rejected or cancelled pre-admission outcomes clean staged orphans,
while successful call admission retains the artifact through durable tool rows.
The terminal coordinator now aggregates every provider response, records one
immutable usage event with the exact resolved model/account revisions, and
finalizes a usable text candidate through the conversation manager. Exact
retries reuse both usage and finalization identities. A persisted `done` result
is exposed as a distinct successful derived-memory terminal and never
fabricates an assistant message.

Continuation rounds now have an explicit executor that validates the admitted
job before mutation, snapshots the current memory revision, atomically starts
the exact validated call set, prepares/persists caller-supplied create seeds
through the configured embedding engine, and settles through the plan-bound
handler. The continuation coordinator can repeatedly invoke that injected
boundary after parsing and durably validating each newly admitted call set,
while carrying exact replay context and preserving every provider outcome for
terminal usage accounting. Seed IDs and token counts remain explicit inputs
rather than hidden globals. A full SQLite scenario composes two tool rounds,
two immutable preparation plans, one authoritative memory mutation, exact
provider replay ordering, aggregated usage, and idempotent finalization.

Native creation inference now has its own bounded application coordinator. It
builds each request from the durable turn, base proposal, attempt-owned tool
contract, and resolved provider profile; dispatches through the shared
`InferencePort`; and admits mixed assistant text/reasoning plus native calls as
immutable creation rounds. Each admitted round preserves usage, provider finish
metadata, request identity, and replay evidence. Recovery reconstructs the
cumulative proposal and exact call/result continuation without redispatching
completed rounds. A text-only response stops without fabricating a proposal;
tool-driven completion appends exactly one planned proposal; cancellation and
provider failures settle the attempt; and eight non-terminal rounds fail with
a durable round-limit code. Host commands and frontend surfaces remain a later
slice.

Dispatch is additionally bound to the attempt's unique claimed job and a
BLAKE3 fingerprint of the complete resolved inference profile. A stale caller,
reused retry job, or profile drift is rejected before the attempt starts or any
provider request is sent.

The creation admission use case records one user turn and its first bound
attempt in a single repository transaction against the exact workflow revision
and base proposal. Exact request replay returns the original pair; changed
identity or content, stale workflow state, and a job collision roll back without
leaving a turn that has no attempt.

Interrupted creation recovery is an atomic parent/child boundary. It preserves
the parent's partial rounds and calls for audit, settles that running parent as
interrupted, and creates an empty immediate child with the same base/profile
binding, a new planned proposal, and a distinct job. Partial provider rounds are
not transplanted into a new request. Successful continuation settlement now
commits the exact reduced proposal, workflow advance, and attempt success in one
SQLite transaction, closing the proposal-committed/running-attempt crash window.
Confirmed new- and existing-persona commands are also exposed through
application use cases and delegate to one creation-owned atomic apply port;
callers do not coordinate persona mutation and receipt persistence themselves.
The confirmed new-character use case follows the same boundary and delegates
the complete character graph plus receipt transaction to the creation apply
port.
Confirmed new-lorebook finalization is exposed through the same application
boundary and delegates its ordered aggregate plus receipt commit atomically.
The revision-pinned existing-lorebook use case uses that boundary as well, so
callers never coordinate root CAS, entry reconciliation, and receipt storage.
Confirmed existing-character finalization follows the same rule for profile,
scene reconciliation, dependency checks, root CAS, and receipt persistence.

## Bundled prompts

The app owns the closed, versioned built-in prompt catalog in
`resources/built-in-prompts/v1.json`. It is embedded with `include_str!`, so
bootstrap and reset work from the application binary on desktop and Android
without depending on installation paths or writable files.

`BuiltInPromptService` reconciles unedited seeds during startup and supports
explicit reset of one or every protected prompt. Consumers keep the returned
`BuiltInPromptIds` instead of hard-coding database UUIDs. The stable legacy
catalog keys and entry keys are compatibility identities; user-facing names
and content remain editable, while purpose and lifecycle protection do not.

`AppBackend::open` is the supported production database-open path. It opens the
caller-provided SQLite path, applies database migrations, bootstraps the
catalog, and returns the database together with the complete typed prompt ID
set. This prevents a host from accidentally serving an initialized database
without the required app prompts. `AppBackend::open_in_memory` provides the
same invariant for tests.

The adjacent `v1.snap` pins a canonical digest for every individual entry,
including its stable key, role, content, scheduling, conditions, payload, and
system-prompt flag. A change to one legacy field therefore identifies the
specific prompt entry instead of appearing only as a catalog-wide checksum.
