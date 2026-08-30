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
