# lettuce-app

Memory structured fallback and summary fallback preserve cache-read/reasoning
details when both responses supply them. Missing or overflowing detail sums stay
unknown. Input/output aggregation retains the existing fallback behavior.

The sole composition root, application workflows, desktop IPC boundary, and
optional local OpenAI-compatible host API.

`AppBackend::preflight_legacy_database` exposes the read-only legacy SQLite
inventory boundary before any import is attempted. Compatibility types live in
`lettuce-transfer`, while all source SQL remains in `lettuce-database`.
`AppBackend::plan_legacy_personas` exposes the bounded persona candidates and
their explicit default owner while leaving all source media unresolved.
`AppBackend::plan_legacy_lorebooks` exposes the bounded root and ordered-entry
plan needed to resolve later persona and character bindings.
`AppBackend::plan_legacy_provider_models` exposes the read-only provider-account
and model-profile plan. Credentials remain pending secret-store inputs represented
only by API-key presence and header names; no secret value or destination row is
written during planning.
`AppBackend::plan_legacy_prompts` reads the bounded legacy prompt library and
app-wide prompt selection. Import admission seals those source identities, and
the provider/model completion transaction creates validated imported documents,
preserves entry order and legacy execution flags, maps the valid app default,
and replays after reopen without duplicate prompts.
`AppBackend::plan_legacy_media` resolves planned persona and lorebook references
below an explicitly supplied legacy storage root plus legacy ASR voice audio,
which may be an external file selected by the user. The application adapter
validates and hashes bounded regular files without copying them or exposing
ambient filesystem access to domain crates. Admission seals each original voice
locator, safe logical key, content hash and stable destination assignment. The
media executor revalidates and hashes each sealed source immediately before
ingestion, stores voice examples as persistent `OtherAudio` assets, supports
mixed image/audio plans, and replays the immutable completion after reopen.

## Boundary

`prepare_staged_lorebook_documents` reads ready SourceDocument assets through
the existing protected media handles, enforces legacy per-source/total byte
limits, verifies content hashes, and prepares ordered bounded text/PDF excerpts.
It returns extracted snapshots only; raw bytes and filesystem paths are not
persisted in workflow state. Source assets retain their existing library or
temporary retention policy; project-owned source associations remain pending.

Staged lorebook settings selection combines explicit overrides, persisted
generator settings and packaged prompt IDs. Model-owned lorebook-generator
overrides use the existing operation parameter layer. Unset temperature/top-p
use legacy 0.3/1.0; unset output tokens use the generator setting (default 4096),
not ordinary chat output tokens. Other unset fields inherit model parameters.
Reasoning is disabled with effort/budget cleared as in legacy feature requests.
Staged, single-entry and keyword prompt conditions treat an explicit Disabled
mode as reasoning off.
`admit_configured` loads persisted settings,
the selected model/account and planner prompt through existing ports, resolves
the profile with the shared model resolver, applies legacy target-count defaults
and bounds, then uses normal durable planner admission. Missing model/prompt
references and incompatible profiles fail before job creation.
`start_configured_batch` reuses that selection/resolution path with the writer
prompt, then starts the durable draft batch. Configured refinement and coherence
admission use the same path with their own stage prompts. Each admission freezes
its resolved inputs. Explicit project model/prompt overrides are persisted;
follow-up selection uses operation override, project override, then live settings
and defaults. Planner admission replay uses its saved inputs/profile even after
settings or model changes, rejecting changed request inputs. Configured refinement
and coherence also persist per-operation overrides and replay before live
configuration resolution; changed feedback, ownership, time, safety or overrides
conflict. Configured writer batches persist profile, prompt identity/revision and
operation overrides atomically with batch start. Partial admission resumes from
that checkpoint without resolving current settings, including entries that have
no job yet. Resolved low-level callers cannot replace a frozen batch profile.
Older low-level batches without this checkpoint still use their resolved API.
Execution checkpoint replay checks job ownership and project cancellation but
does not require the live prompt revision: no new inference uses that prompt.
New admissions also persist the full validated prompt document in the existing
run JSON (and configured batch checkpoint before per-entry admission). First
inference uses that frozen document instead of the caller's live document;
prompt edits affect newly admitted work only. Older runs without a snapshot
still require the caller's exact admitted prompt revision. This deliberately
replaces legacy's execution-time live template lookup with stable durable inputs.
Automatic host
wiring remains pending.
Writer batches validate prompt and text modalities before changing the project
or creating jobs, so invalid inputs leave the draft checkpoint unchanged.

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
carried character/persona relationship, and the next character/persona
continuity episode in one transaction. The prior open episode is closed by the
new launch. Exact launch retries revalidate the initial-state request without
resetting relationship continuity or advancing the episode sequence. Group
companions and group growth scheduling remain deferred.

Single-entry lorebook generation now has a restart-safe admission boundary.
Application preparation resolves the direct conversation's character and
persona owners, reads the selected branch messages, conversation memory and
summary, and target lorebook through their existing ports, then copies the
legacy chronological message, durable memory, relative-time, existing-entry,
and `(none)` formatting into immutable prompt values. Missing or foreign
selected message and memory IDs fail closed instead of being silently omitted.
One interactive generic creation job and one immutable request-owned run bind
those inputs before provider dispatch. The claimed-job executor renders the
frozen prompt with the exact source/force instruction, tries the native tool
contract, and uses exactly one same-model JSON/XML fallback only when no usable
declared native result exists. Each decision, usage counters, and provider
diagnostics are checkpointed before job settlement, so restart replays the
durable entry-or-none result without dispatching again. Changed admission
replay conflicts, and neither preparation nor execution mutates the lorebook.
Keyword-draft preparation separately freezes the legacy title/content,
existing-keyword, and direction values with the active keyword prompt and
resolved text profile, then admits one restart-safe request-owned creation job.
Exact replay returns the same run and changed replay conflicts; provider
execution uses the exact required native contract and one same-profile
structured fallback, checkpoints usage and provider diagnostics, and settles
the generic job. Completed keyword drafts replay without dispatch and no
lorebook is mutated.
Staged lorebook admission accepts only already-extracted bounded excerpts,
freezes the resolved planner profile and active planner prompt, and persists one
stable project with a generic restart-safe creation job. Exact admission and
created-to-planning CAS replay without duplicating state; source extraction
remains a separate input boundary.
Staged admission requests can prepare pasted text or already-read UTF-8 TXT/MD
bytes through `with_sources`; validation and legacy excerpt truncation happen
before job creation, and raw source bodies are not retained in the request.
Writer batch requests derive identity from the stable plan ID and persisted
batch revision, and replay with the frozen batch start time. Restart after a
partial completion reuses unfinished runs; retrying failed drafts admits new
jobs while completed drafts remain unchanged.
Failed refinement closes its job while preserving the existing reviewed draft.
Late failures from older draft batches close only their job; they cannot mark
the newer retry's draft failed.
Explicit planner retry admits a new job only after the previous planner job
failed. The same project, sources, resolved profile and prompt are retained;
retry admission and checkpoint replay do not issue duplicate provider calls.
Claimed planner execution renders the frozen prompt with the exact legacy
brief, decimal target count, source-excerpt formatting, final instruction, and
required tool declaration. It makes one native request, accepts only typed tool
calls, durably checkpoints calls, usage, and provider diagnostics, then applies
the planning-to-outline-review CAS. Restart replays that checkpoint without
redispatch, and the planner job succeeds while the project waits for review.
Outline approval then performs one restart-safe project CAS that creates the
ordered pending drafts from the reviewed plans and enters drafting. It does not
dispatch writer inference or reopen the completed planner job.
The coordinator also saves pre-approval outline edits through project revision
CAS, preserving planner evidence and supporting exact edit replay.
The coordinator also exposes restart-safe stable-ID draft edits and approval
toggles as project revision operations without dispatching inference.
Refine admission freezes the selected draft, trimmed feedback, exact legacy
render values, resolved profile, and active refine prompt. The shared writer
executor checkpoints one typed attempt before applying the revised draft and
revision-history append through the project CAS.
The staged project coordinator also persists coherence proposals and applies
only the accepted stable change IDs before returning to drafts-ready. Coherence
provider admission and execution remain separate.
Coherence admission now freezes the exact legacy drafted-entry rendering,
resolved profile, active prompt revision, project revision, and generic job.
Claimed execution uses the required native tool contract, checkpoints one
attempt, then submits the proposals; restart replays both checkpoints without a
second provider request.
Final staged commit accepts a new destination/name or an existing destination
revision, and delegates one atomic apply to the creation repository. The
committed project retains the result receipt for exact retries.
Project cancellation also cancels its queued jobs and requests cancellation of
claimed jobs. All three staged executors check the durable project before and
after inference; dispatch settlement recognizes cancellation that races with
provider completion. The active host can additionally signal the job handle to
abort transport promptly; durable cancellation alone discards a late response.
Claimed staged-writer execution renders those frozen values, appends the exact
legacy final instruction, and makes one required native tool request. Its
checkpoint is durable before the matching pending draft CAS; restart replays
without redispatch, and the request-owned creation job settles normally.
Batch admission selects the first three pending/failed drafts in outline order,
marks them durably as drafting, and creates deterministic stable-ID writer
runs/jobs. An active batch replays without selecting later work; terminal
writer failure marks its draft failed, while transient job retry leaves it in
the active batch.
A pending stable plan can be prepared into one restart-safe writer admission.
Preparation copies the legacy full-outline, proposed-key, and relevant-excerpt
formatting into immutable values with the active writer prompt and resolved
profile, then binds one generic creation job. Foreign/non-pending plans and
changed request replay fail before provider execution.

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
name, then renders the legacy prompt-state block with the stored continuity
episode at the source message's effective clock through the existing typed
`companion_state` placeholder.
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
effects can now be rediscovered from SQLite in stable order. Automatic
admission waits for the resolved legacy message interval and takes the oldest
ready effect prefix; a user/assistant effect remains atomic when it crosses the
interval boundary. The batch's logical idempotency key is derived from its
ordered durable effect identities and frozen interval, so
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
again, and `done` preserves the existing short-circuit behavior. When a native
tool request errors or returns no calls, the coordinator copies the legacy
second request with tools disabled and the frozen JSON/XML format instruction,
parses its text into the same typed calls, aggregates both request usages, and
admits them through the same durable round path. Empty operations become an
explicit no-change `done` checkpoint. Before round zero, the runner renders the
editable dynamic-summary prompt over the frozen source window and prior
cumulative summary, requires `write_summary`, accepts valid same-response text,
and copies the legacy tool-disabled retry and validation. The validated summary,
token count, provider context, usage, and cursor commit atomically against the
memory root; recovery replays that checkpoint without provider I/O, and the
following memory-tool phase consumes its stored text. A settled background
round now appends its admitted native calls and typed results to the exact
durable request context, stops before provider I/O on `done`, or dispatches and
atomically admits the next bounded round with the frozen profile/tool contract.
An already admitted next round replays without provider I/O. Debounce/startup
wiring and binding admission to host startup/finalization remain later slices.
The composition root now exposes its SQLite database as the durable generic
`JobStore`. The background loop connects the
existing round executor and continuation coordinator until the durable `done`
result. It resumes from the latest admitted checkpoint, skips seed generation,
embedding, reduction, and provider dispatch for already settled/admitted work,
and keeps create IDs, token counts, and timestamps as explicit caller inputs.
Effect discovery also accepts the exact authored `auto`, `askFirst`, or
`manual` run mode. `manual` admits no automatic job; `askFirst` waits for the
existing interval gate and records/replays a durable conversation approval
without provider dispatch; `auto` preserves the existing oldest-prefix cadence
and clears any pending approval when it admits the job. The approval query
returns only a currently pending count. Skip clears that pending flag while
retaining the prompted-count baseline, and approval bypasses the ordinary gate
to freeze the newest interval-sized whole-effect suffix. Older unsummarized
effects remain attached for terminal settlement while only that suffix is sent
to summary and memory inference, matching the legacy forced-cycle cursor move.
The same coordinator exposes explicit direct/group trigger admission without
consulting the authored run mode or interval gate. Direct trigger/retry selects
the recent suffix; the legacy group trigger keeps the oldest cursor prefix.
Both accept a partial window when fewer than one interval of messages exists,
keep whole companion effects, and reuse the same idempotent job/claim path.
Direct retry may freeze an explicit model profile into that same recent-window
job. The run rejects a different resolved profile, and an opted-in default
change happens only on the first successful terminal settlement. Failed or
cancelled retries leave the configured dynamic-memory model unchanged. Group
retry intentionally has no model-override entry point, matching the legacy API.
The run freezes whether companion time awareness is enabled and uses the matching
legacy tool contract. In enabled runs, a known create source is retained and an
unknown or omitted ID falls back to the latest frozen user/assistant source. In
disabled runs, source attribution is cleared. Each source effective time is
frozen with the run; enabled transcript lines use the copied legacy
`[message:ID] role: <time>YYYY-MM-DD HH:MM</time> content` format, and created
memories persist the selected role, observed time, and `turn` precision.
Companion runs additionally freeze the legacy supersession capability: their
create schema exposes `supersedes`, valid active targets are linked during the
existing atomic reduction, and disabled direct/group execution clears that
field before reduction.
The existing eight-round and 64-call admission limits remain authoritative.
Terminal success now feeds the run's durable starting snapshot and current
memory snapshot into the existing companion effect coordinator, settles every
coalesced effect idempotently, then marks the attempt succeeded. Failure and
cancellation use the existing bounded effect summaries and attempt failure
codes. Effect-first ordering makes a crash retry converge without leaving a
succeeded attempt whose effects are still processing.
The same terminal read now exposes the ordered prefix of at most sixteen
nonblank memory items whose IDs were absent from the run's durable starting
snapshot. This is the legacy growth trigger/evidence boundary; it does not run
growth or alter memory settlement.
Successful direct-companion results can now admit one restart-policy
`CompanionGrowth` job keyed by the durable memory-run ID. Admission freezes the
successful attempt, resolved memory profile, current Soul revision, character
owner, and bounded fresh-memory evidence; empty and non-companion results
create no job. A claimed growth job renders the editable growth prompt with the
frozen legacy values, requires the exact `record_growth` tool, checkpoints the
first parsed provider proposal, and applies it through the existing Soul policy
and repository with one stable operation ID. Restart skips inference after the
checkpoint and exact Soul apply replays its receipt. The same SQLite job claim
then settles success, cancellation, terminal rejection, or a retryable provider
or storage failure without changing the already-settled chat or memory run.
Only a succeeded growth job that actually added facts can admit its separate
`CompanionConsolidation` follow-on, and only when the authoritative post-growth
Soul has at least twelve active changeable facts. Admission freezes the exact
profile, authored context, Soul revision, character owner, and deterministic
apply identity in SQLite; repeated admission loads the same run and its one
immutable parsed-proposal checkpoint. A claimed consolidation job renders the
editable legacy prompt from that frozen snapshot, requires the exact
`consolidate_soul` tool, checkpoints its first parsed proposal, and passes it
unchanged through the existing consolidation policy and Soul repository with
the run's stable operation ID. Restart skips inference after the checkpoint and
replays the same Soul receipt; the SQLite job then settles success,
cancellation, terminal rejection, or retryable provider/storage failure.
Explicit Soul-writer preview admission now freezes the selected primary and
optional fallback profiles, editable prompt identity/revision, exact legacy
prompt values, normalized starting draft, structured fallback format, and
request identity in SQLite before inference. Repeating the request ID returns
the stored run and its interactive restart-policy job, and ordered round
checkpoints are idempotent; admission does not mutate the character or durable
Soul. The concrete dispatcher claims and settles that job; its executor renders
the same editable prompt only at its frozen ID/revision, appends the exact
legacy authoring instruction, and requires the six-tool contract. It reduces
and persists every native round
before provider-neutral continuation, suppresses calls after `done`, resumes
incomplete checkpoints, and returns completed previews without provider I/O.
No-call responses use one tools-disabled JSON/XML fallback request with the
copied legacy instructions; a failed primary target may continue through the
frozen fallback profile. Cancellation, the eight-round hard cap, later
provider failures, and generic job settlement never silently promote an
incomplete draft. Character Soul mutation and frontend IPC remain outside this
preview workflow.
Direct companion context assembly now resolves scheduled notes from the
character-owned repository at the source message's immutable effective time.
The resulting block and `has_active_scheduled_note` condition are derived from
that authoritative active set, replacing caller-supplied scheduled-note text;
when an editable prompt does not consume `{{scheduled_notes}}`, the same legacy
block is inserted at in-chat depth zero. Roleplay and group contexts do not read
or inject companion scheduled notes.
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

The delete-after coordinator preserves the selected anchor message and applies
the existing branch-local tombstone policy to its suffix. It derives removed
IDs from the durable tombstone outbox record, restores memory and summary from
the earliest intersecting immutable run, invalidates removed companion effects,
and immediately admits retained effects from the rewound run suffix with a
restart-stable rebuild job key. Tombstone replay first checks the rewind receipt,
closing the crash gap between the conversation mutation and memory rollback.

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

Startup recovery is one explicit bounded application pass over expired durable
claims. It returns every recovered job with its persisted policy action and
current snapshot, including kinds whose executors are not wired yet, so a host
cannot silently consume unrelated work. `AppBackend` also exposes the existing
companion memory discovery/claim coordinator with the same SQLite database as
both effect repository and job store. Worker identity, resource admission,
runtime inputs, polling, and execution remain host-owned.

The adjacent `v1.snap` pins a canonical digest for every individual entry,
including its stable key, role, content, scheduling, conditions, payload, and
system-prompt flag. A change to one legacy field therefore identifies the
specific prompt entry instead of appearing only as a catalog-wide checksum.

`AppBackend::usage_costs` exposes `UsageCostCoordinator::capture_job` for
caller-triggered OpenRouter cost capture by job and dispatch ID. It reads
immutable response identity, resolves the same-revision enabled OpenRouter
account, fetches generation metadata and the actual model's endpoint prices,
then writes an immutable basis using the existing usage ledger. Replays return
before network/account lookup; another writer's completed basis wins a race.
Provider errors leave raw usage and cost state untouched for retry. Missing
account/identity/usage/native totals/auxiliary counts or ambiguous pricing
returns no cost. No host polling, automatic post-inference scheduling or cache
was added. The narrow app-owned billing port is implemented by RemoteProviders.

SQLite scenarios cover native/normalized disagreement, preserved raw response
amounts, routed endpoint selection, stored generation/price provenance, old
basis JSON, changed-account gating, missing/error/ambiguous lookup retry and
reopen with account deletion and no network replay.

Staged lorebook planner, writer/refinement and coherence execution now reuse
the existing job-dispatch usage ledger before provider invocation and before
response reduction/checkpointing. Every actual dispatch preserves its response
ID and optional usage, including invalid responses and project cancellation
during inference; returned transport failures remain explicit failed evidence.
Checkpoint replay creates no second dispatch record. Existing successful
checkpoint usage stays unchanged. The staged SQLite lifecycle scenario covers
all four stages, invalid planner usage, cancellation after response, independent
concurrent-writer failure evidence and replay without duplicated charges.

Legacy staged pipeline.rs called the provider without recording usage; recording
these dispatches corrects that accounting omission. Legacy single-entry primary
and fallback requests recorded usage before checking response success; their
dispatch-ledger integration is described below. No new schema,
worker, pricing formula or host scheduling was introduced.

Lorebook entry and keyword native/fallback executions also use job dispatch
evidence. The shared helper distinguishes evidence persistence failure from
provider failure. Entry/keyword stop on evidence failure without writing a
false failed-native checkpoint or dispatching a fallback; provider failures
retain the existing fallback policy. Primary/fallback IDs and optional usage
remain separate, and successful replay adds no dispatch. Legacy entry generation
recorded both requests before response validation; legacy keyword generation
omitted that recording, which this corrects.

The same distinction fixes Soul-writer alternate-model fallback after evidence
or run-persistence/replay-cleanup failure. Other companion/staged callers retain
their existing public provider-error mapping. Fault-injection scenarios prove
admission failure sends zero requests, settlement failure sends one, no false
checkpoint is written, and later retry preserves the pending evidence. Existing
entry tests also prove provider-error fallback, cancellation and replay.

Background companion memory summary, native tool requests, structured fallbacks
and continuation now retain each dispatch through the same job ledger. Responses
are saved before summary/tool validation or checkpoint aggregation; transport
failures and cancellation remain distinct. Summary/round totals keep their
existing meaning and are not additional billable dispatches. Evidence failures
stop fallback; a cancelled primary response or job cancellation during a
tool-less response also stops fallback. The job store
and evidence repository must share durable job ownership.

SQLite scenarios cover summary fallback totals versus separate raw responses,
failed memory primary/fallback calls, continuation and terminal replay, missing
usage, storage failure without fallback, and cancellation with retained response
evidence. Legacy memory/flow.rs recorded summary and memory responses before
validation; this restores that boundary. No schema or scheduler was added.

Creation-helper continuation rounds retain each actual provider dispatch in the
existing job usage ledger before response validation. Successful round totals
remain unchanged; rejected/cancelled responses retain raw usage and response
identity even without a round checkpoint. Evidence storage failure stops the
run without classifying it as a provider failure. The caller must supply a job
persisted in the same store as the usage repository. Completed replay adds no
dispatch. SQLite tests cover these boundaries and separate two-round evidence
from the existing aggregate. Legacy creation_helper/service.rs also accounted
for initial/continuation responses before accepting their generated content.

Conversation-owned dynamic-memory continuation also records each actual dispatch
under the generation attempt's attached durable job before validating the
response. Provider failures and rejected/cancelled responses survive a failed
loop. Its terminal UsagePort still records the existing whole-attempt aggregate,
including the externally supplied initial response; that aggregate overlaps
with dispatch evidence and must not be added to it as an extra charge. This
coordinator does not dispatch or fabricate evidence for that initial response.
The SQLite two-round scenario verifies retained failures, separate successful
response IDs/counters and unchanged terminal aggregate/replay.

`ConversationInitialInferenceCoordinator` supplies the initial provider
dispatch boundary for a running conversation generation attempt. It reloads the
durable turn, verifies the turn/attempt/job/request identities and requires the
request's model and context attributions to match the prepared turn.
Each dispatch is admitted as a durable pending checkpoint before the provider is
called, using the job usage event that retains the raw response evidence. The
provider response, an invalid response, a provider failure or a cancellation
observed after the response settles that checkpoint once; a later call with the
same request replays the stored outcome or error without another provider
request, including after the attempt has been finalized or the database was
reopened. Changed context, profile, tools or media grants conflict with the
admitted request. A concurrent caller, or a caller that finds a pending record
whose settlement did not complete, receives a pending error instead of a second
dispatch; the attempt then follows the existing interruption and recovery path
and the child attempt dispatches fresh under its own job while the parent record
and evidence are retained. Cancellation before admission produces no evidence.
Response interpretation, tool admission and terminal aggregation stay with their
existing coordinators.

`ConversationGenerationDispatchCoordinator` and `ConversationGenerationJobRunner`
compose those pieces into one claimed-job pipeline for a conversation attempt.
Admission creates or reuses the `ConversationGeneration` job keyed by
the attempt's job idempotency key and attaches it; claim starts the job and its
stage; the runner then stages Preparing, prepares the turn from the supplied
model and attributions, stages Running, dispatches through the initial
coordinator, admits and executes dynamic-memory tool rounds when the response
carries tool calls, and commits through the terminal coordinator with the usage
timestamp frozen to the dispatch checkpoint. Every mutation the runner owns uses
an operation token derived from conversation, turn, attempt, job and step, so a
re-run replays instead of conflicting; a finalized attempt replays its candidate
and usage event without touching the provider. Settlement maps the run outcome
onto the job store: success, cancellation through the two-phase turn cancel,
failure through `fail_generation`, and a pending dispatch through interrupt and
recover into a child attempt with its own job (linked as a child of the parent).
Non-success settlement records a real usage event for the attempt from the
complete ordered dispatch evidence (known counters only when every admitted
response reported them, otherwise an unavailable reason) so no attempt ever
references a fabricated usage id or sums the aggregate event as another charge.
A recovered child on an already prepared turn moves straight to Running and
never re-prepares. A re-claimed attempt classifies its durable tool tail, reads
all immutable per-round plans, rebuilds the request with every succeeded round,
and continues after the last result without applying those memory changes
again. Validated tails execute normally; running tails settle from their stored
plan. An interrupted parent's active planned tail is atomically cloned and
settled in its immediate child before continuation. Rejected, mixed, missing-plan
or otherwise unverified states still fail with `RecoveryUnavailable`. Turn-side
settlement errors schedule a job retry instead of leaving the claim running.
Before each Preparing or Running stage append, the runner reads the latest
durable checkpoint sequence for its attempt and allocates the next value. A
pre-existing streaming progress checkpoint and process reopen therefore do not
collide with runner-owned stages; the database still enforces contiguous
uniqueness and operation replay. Streaming progress emission and frontend
commands remain outside this runner. The prepared runner exposes one synchronous
command-facing execution operation for an already durable turn and attempt. It
admits the existing idempotent job, claims that exact job with caller-supplied
resource availability and a shared cancellation token, runs the prepared
pipeline, and settles through the same dispatcher. A succeeded job returns its
durable candidate and usage as an exact replay without another claim or provider
call; other terminal jobs and temporarily unclaimable jobs are distinct typed
outcomes. `AppBackend` also owns the inference runtime shared by these executions
and its provider runtime. Cancelling by durable job ID first records the job
cancellation and then signals the exact registered token. An unclaimed queued job records the
turn's cancelled-before-response usage, settles the turn and job immediately;
claimed work remains with the runner until its response evidence and cleanup are
settled. Completion unregisters the live token. Unknown and already-terminal IDs
return explicit idempotent outcomes. The caller remains responsible for creating the turn, supplying
dynamic-memory create seeds, and deciding when to invoke the operation; no
second scheduler or background loop is introduced.
The dispatcher can schedule the exact `BeginGeneration` returned by send,
continue, regenerate or retry. A single worker operation lists the oldest
highest-priority queued conversation-generation job, resolves its immutable
creation event back to the attached turn and attempt, and executes it through
the same prepared runner. One invocation handles at most one job and returns
`Idle` when none is queued, leaving polling cadence and host lifetime outside
the application workflow.
`PreparedConversationGenerationJobRunner`, exposed by `AppBackend`, now owns the
reconstructible input boundary for an ordinary direct or resolved group turn. It
first reads an admitted initial-dispatch request for the current attempt or its
interrupted parent. That durable request preserves the resolved profile, exact
provider-neutral context, tools and media grants across restart and live model or
context drift; recovery rewrites only the current attempt/job cancellation
identity and installs the caller's runtime stream sink. Dynamic execution is
reconstructed from the launch-frozen policy, current conversation-owned memory
space and immutable settled round plans. When no dispatch exists, the runner
loads the durable turn and branch ancestry, trims a finalized replay back to the
turn's original source message, resolves the selected participant's effective
settings and exact snapshotted live model/account, assembles provider-neutral
context, derives unique media grants from that context, and invokes the
claimed-job runner. Stored explicit decisions, including muted mention targets,
run directly. For a user-message send in a non-director group, a mention of an
enabled character in that message becomes the explicit speaker before any
heuristic, round-robin or LLM selection, so no speaker-selection call is made
and mute is bypassed as in legacy. Mentions resolve against each character's
current record: its display name (nickname, or name) first, then its real
name. Display names are cosmetic otherwise; the LLM speaker-selection prompt
and group transcript labels use real character names. A participant whose
character record no longer exists is ineligible for every selection path and
is labelled "Unknown". A director continuation uses its forced participant, and
group regeneration retains the original candidate author; neither path fabricates or
persists an automatic-selection rationale. An unresolved group turn fails with
`SpeakerUnavailable` before provider dispatch. Only
the stream sink and prompt runtime values remain caller-supplied; the sink stays
outside the durable initial-dispatch fingerprint, so replay may use a new sink
without another provider call. Context or model preparation failures map into
the existing run settlement categories. A dynamic direct turn resolves the
persisted global policy, requires tool-capable inference, loads its authoritative
conversation memory space and summary, embeds the current or enriched two-message
query, and selects current projections with the legacy threshold, cold-memory
penalty, category diversity, and smart recent/accessed fallbacks. Retrieved
memory text and a stable identity derived from the exact space revision enter
the context; the matching memory tool request, policy, and duplicate threshold
enter the runner. The first admitted tool round freezes that policy in its
existing durable preparation plan. Retrieval embedding unavailability preserves
the legacy behavior of continuing without retrieved keys. A nonempty selection
now atomically promotes selected cold items and records the legacy access count,
time and importance updates exactly once under the preparation attempt. The
resulting memory revision is the context attribution and tool-round input.
Terminal replay bypasses input reconstruction, so it does not repeat embedding
or access side effects; changed or stale retrieval input fails closed. Manual
mode now reads every active item from the conversation-owned memory root in
stored order and renders the legacy `- text` lines without summary, tools,
embedding, promotion, or access-count changes. Its exact root revision is the
turn attribution for send, continuation, and regeneration. Legacy manual memory
has no selected-revision concept, and the current snapshot field has no writer
or content authority, so a nonempty `selected_revision_ids` still fails closed
instead of inventing a second store. Direct launches freeze the global direct
policy, while group launches freeze the complete group override when present
and otherwise inherit the direct policy. Prepared
generation requires and uses those frozen values, so later global setting
changes cannot alter retrieval, access mutation, or tool-round policy for the
conversation. Older dynamic snapshots without the optional policy remain
readable but fail closed if generation tries to use them. Durable request-body
recovery remains a later slice.
For unresolved group turns using the frozen heuristic or round-robin policy,
the prepared runner now derives participation counts and the prior speaker from
the durable active-branch timeline, stages `SelectingSpeaker`, applies the
existing pure policy, and persists the decision through `ResolveGroupSpeaker`
before preparing context. Disabled participants are ineligible and muted
participants are excluded from automatic choice. Group invariants keep at least
one enabled unmuted member available; the pure policy retains its defensive
no-speaker error. The persisted selection and completed generation replay after
process reopen without selecting or dispatching again. Group launch freezes the
dedicated speaker-selection model when configured, otherwise the application
default, including its exact model/account revisions and protected artifact.
LLM selection uses that frozen identity when the live revision supports tools,
a bounded provider-neutral legacy-shaped participant/recent-message prompt, and a required
`select_next_speaker` tool whose enum contains only enabled unmuted participants.
The provider call is admitted under a separate immutable selection checkpoint
and job-usage record before dispatch. A valid call persists its participant and
bounded rationale; invalid output or non-cancellation provider failure retains
that usage and falls back to the existing heuristic. Cancellation stops instead
of starting fallback. The settled decision and subsequent generation replay
after reopen without repeating either dispatch. Clearing or changing the live
setting after launch does not change that conversation; a missing or unusable
frozen model falls back heuristically without provider usage. The legacy final
fallback to the first configured model is deliberately omitted because it had
no explicit selection provenance.
The selection and generation job-usage rows are raw dispatch evidence for the
same attempt. Their counters overlap the terminal `UsageLedger` aggregate and
must not be summed with it as independent charges. Required-tool output is
validated structurally; missing, malformed or foreign calls use the heuristic
fallback instead of accepting arbitrary response text as a speaker identity.
`PrepareGeneration` records the resolved model and prompt/lorebook/memory
attributions atomically before moving a preparing turn to ContextPrepared.
The existing speaker-resolution mutation continues to own group speaker choice.
Preparation also accepts the explicit director target or original regeneration
author without requiring a new selection decision; mentions remain explicit
decisions rather than automatic selector requests.
Exact operation replay preserves the stored preparation; changed input conflicts.
Recovery can reuse matching preparation under the child's attached job, while
changing the prepared model or attributions requires a new turn. Full automated
send/regenerate/retry orchestration and durable request-body replay remain later.
Generation finalization derives a regenerated candidate's persisted ordinal
from the prior candidate instead of trusting the provider-local response index;
this preserves dense alternatives while new-assistant turns still begin at zero.

The legacy import admission coordinator fingerprints the complete preflight inventory and provider/model/prompt/persona/lorebook/media plans, derives the closed source ID set, and admits it through the transfer-owned repository port. It allocates no IDs itself and performs no filesystem ingest, secret read, secret-store write, or domain creation; SQLite returns sealed account, profile, prompt, secret-reference, graph, and media mappings so retries and reopen use the same destination identities. The deterministic legacy llama.cpp account participates in the same assignment path. The legacy database and storage tree remain untouched and retained.

The provider-secret import coordinator requires the live read-only source set to
match every sealed secret assignment. It binds each value to the assigned
destination reference and destination account owner, writes only through
`SecretStore`, and records non-secret progress after each value. Retry and reopen
verify purpose, generation and exact secure-store value. An already-correct value
without a receipt is treated as a recoverable write-before-receipt crash; a
changed, missing or foreign value stops without overwrite or deletion.

The provider/model import coordinator verifies every assigned secure-store
reference, purpose and generation against its immutable completion receipt before
asking SQLite to materialize metadata. It recomputes the complete sealed plan
fingerprint, exposes no secret values to the transaction, and returns the same
provider/model receipt on replay after reopen. Overall import completion follows
only after both graph and provider/model receipts exist.

The legacy media import coordinator reopens only confined regular files under the read-only source root, rejects symlinks and path drift, and rechecks byte length plus BLAKE3 before ingest. It catalogs each file under the admission's assigned asset ID, records durable completion after the ready blob exists, and resumes partial progress by replaying completed objects. Shared bytes deduplicate to one blob while their logical asset identities remain distinct. It never removes or rewrites the legacy source.

The legacy graph execution coordinator recomputes the sealed plan fingerprint and asks the transfer repository to materialize it. SQLite creates lorebooks and their assigned entries, personas and assigned media links, ordered active-lorebook bindings, and the legacy default owner in one transaction only after all media receipts verify. Legacy entry display order becomes the contiguous stored order already established by preflight sorting. Exact retry returns the original completion receipt, including after reopen; any mismatch leaves no partial graph. Source cleanup is intentionally absent pending full migration verification and explicit user-approved deletion.

The legacy ASR import coordinator independently materializes every admitted
vocabulary, correction, ignored suggestion and voice example in one transaction.
It preserves validated learning metrics and timestamps, resolves optional links
through assigned typed IDs, and binds examples only to completed managed audio.
Its immutable receipt replays after reopen without overwriting live learning
data; graph import can proceed before voice audio is available.

The ASR learning transfer coordinator exports the four filtered learning record
classes as one bounded versioned document. Import validates every record and
link before allocating fresh IDs, verifies each ready managed audio asset
against its content and redacted provenance evidence, remaps voice links and
delegates one atomic batch to the speech repository. Missing or changed audio
and malformed later records leave earlier records uncommitted.

Legacy version-2 ASR JSON import validates every source identity, normalized
value, timestamp and optional link before opening audio. Relative voice paths
are confined to the selected document directory; absolute paths preserve the
old file-picker workflow, while symlinks and non-files are rejected. Each
distinct file is ingested once as managed library audio, then the complete
converted version-3 graph uses the same atomic learning import. Source JSON and
audio are never modified or removed.

The speech-transcription coordinator admits an idempotent interactive
`SpeechTranscribe` job, binds it to one immutable audio/model request, claims it
through the generic worker lifecycle, and coordinates managed audio decoding,
prompt construction, runtime execution, correction application and durable
settlement. Successful results replay after reopen without another runtime
call. Cooperative cancellation reaches the runtime token and settles through
job cleanup; unavailable audio, learning storage, models or runtimes schedule a
retry, while malformed inputs fail terminally. `AppBackend` owns one
process-lifetime `WhisperCppRuntime` backed by the installed-model repository so
verified contexts can be reused across coordinator calls. It also exposes the
SQLite-backed ASR learning library and a composed run method that supplies both
the real learning and Whisper adapters to a claimed transcription. Host
scheduling and microphone IPC remain later integration work.

The remote Whisper catalog resolves the current Hugging Face repository head to
an immutable commit before reading its file tree. It exposes only bounded model
metadata with coherent LFS size and SHA-256 evidence, retains the legacy
recommendation classifications, and rejects mutable or incomplete entries
before download admission.

The Whisper download coordinator admits the pinned remote identity as a durable
artifact-install job, reports bounded byte progress, preserves matching partial
bytes across restart, and cooperatively cancels before the irreversible install
stage. It accepts a resumed response only at the requested byte offset, safely
restarts when a server returns the complete object, verifies upstream SHA-256,
atomically installs, and admits the existing BLAKE3 manifest afterward. A crash
between rename and manifest admission recovers from the verified final file;
completed jobs replay without another network request or a public native path.

The Whisper-model coordinator scans the retained legacy model directory through
the bounded model-hub inspection contract, records each verified manifest, and
replays exact admissions after reopen without changing source bytes. Resolution
preserves the legacy first-filename default or accepts an explicit installed
model ID, then revalidates size and BLAKE3 before producing the speech request
descriptor and runtime-only artifact handle. The runtime independently repeats
that verification before loading native code. Managed removal validates model
ownership, clears every cached Whisper context before deleting bytes, then
conditionally deletes the exact immutable manifest. Missing retries are no-ops;
retained legacy files cannot enter this removal path.

The TTS configuration coordinator creates remote-provider credentials only in
the injected native secret store under `AudioApiKey` ownership, then persists
their reference through the speech repository. Key rotation uses secret
generation compare-and-swap. Provider metadata and user voices use repository
revision compare-and-swap, and provider kind cannot change during an update.
Provider deletion first validates the exact revision, atomically removes its
voice graph, and deletes the matching secret generation. A native-store failure
after metadata deletion returns an opaque retry receipt; callers cannot forge
its secret identity. Kokoro rejects credentials. Remote provider transports are
constructed with the host's current TLS policy. ElevenLabs and hosted Fish
configured-voice refresh load only the provider's scoped secret, reject partial
or invalid responses, and atomically replace that provider's bounded ordered
cache after the complete fetch succeeds. Preview caching remains a later TTS
slice.

ElevenLabs, hosted Fish and Gemini provider verification load the existing
provider and its exact `AudioApiKey` secret scope before calling the typed
verification transport. HTTP credential rejection returns `false`;
configuration, secret-store and transport failures remain distinguishable
errors. Gemini verification also requires the stored project and location. Fish
Speech verification supports both scoped-secret and unauthenticated servers.
OpenAI-compatible verification requires its stored base URL and scoped secret
and performs a real standard models probe instead of returning success locally.

The ElevenLabs voice-design coordinator admits only a stored ElevenLabs
provider, freezes its provider snapshot, trims and validates the active editor's
sample and description, and resolves only that provider's scoped `AudioApiKey`.
It passes cancellation into the shared remote runtime and sends decoded preview
bytes directly to the existing media sink. Each result retains the generated
voice ID, finite duration, detected MP3 MIME, content hash, size and temporary
managed asset identity; provider base64 and plaintext credentials never cross
that boundary. Voice-design preview requests are interactive and transient;
their audio assets carry an admitted expiry instead of adding another durable
job or request table. The same coordinator admits saved-voice creation from one
generated preview, resolves the same scoped secret, calls the remote runtime and
returns the validated provider voice ID. The active legacy flow then passes that
ID to the existing user-voice configuration save; it did not refresh the
provider cache, so this boundary does not add an unrelated refresh.

The TTS synthesis coordinator admits an idempotent interactive
`SpeechSynthesize` job and persists its provider, text, voice, prompt and output
policy before execution. It claims work through the generic job lifecycle,
loads the exact audio-provider secret when required, calls an injected runtime,
and admits returned bytes through the media boundary under a deterministic
asset identity. Preview output is temporary and retained chat audio is
persistent. Cancellation is checked before secret access, before dispatch and
after the provider response so late audio is not stored. Transient provider,
secret, media or repository failures requeue the same immutable request;
invalid inputs and audio fail terminally. Successful results replay after
reopen without another provider call. The application composition root exposes
one remote TTS runtime that routes the frozen provider kind through the five
completed HTTP adapters. Kokoro remains unavailable until its native runtime
is routed through the durable synthesis job.

The Kokoro asset inventory coordinator is constructed once from a host-resolved
managed root. Its operations accept only a variant and optional selected voice,
return model/config/tokenizer presence plus bounded content identities, and list
installed voices without exposing native paths. This replaces the legacy
caller-supplied asset-root commands. Missing assets remain a usable status;
unsupported variants, empty files, symlinks and inspection races fail. The
legacy automatic `system-kokoro` database row is not reproduced because local
providers already use the ordinary typed configuration boundary and no longer
persist an editable asset root.

Kokoro model bundles install through one durable interactive artifact job. The
application accepts a host-constructed confined install store, never an
operational asset path, and downloads the four pinned files through the central
ranged artifact client. Partial bytes and completed shared files survive retry;
success requires all four files to reverify, and completed jobs replay without
network access. Progress aggregates the bundle with bounded checkpoints.
Cancellation remains available until the first verified file commit, after
which the job finishes or retries the remaining bundle under its irreversible
stage.
The Kokoro voice catalog reads the same immutable repository revision through
the central JSON client. It accepts only bounded `voices/<safe-id>.bin` entries
with coherent LFS size and SHA-256 metadata, sorts and deduplicates IDs, and
merges exact installed flags from the managed inventory. Catalog reads do not
download, remove or expose native voice paths.
Selected Kokoro voices install as one stable-ID artifact job for either a
single voice or a batch such as the legacy starter pack. Selection is sorted
and deduplicated before admission. Each voice resumes from an identity-bound
confined partial, verifies pinned size and SHA-256 before atomic commit, and is
reverified on successful replay without network access. Progress covers the
whole batch and cancellation ends at the first verified voice commit.
Managed Kokoro removal accepts only the pinned model or voice identity, verifies
the current bytes and confined target before deletion, and treats missing bytes
as an idempotent replay. Removing a model variant preserves its shared JSON
assets. A later install receives a deterministic successor job instead of
replaying an earlier success whose artifact was explicitly removed.
Kokoro phonemization runs only after the complete pinned model bundle is
reverified through its confined install store. The application supplies the
purpose-specific eSpeak capability to the speech boundary and accepts no
operational model, tokenizer, config or executable path per request.
Kokoro voice blending resolves each normalized voice ID to one pinned remote
descriptor, revalidates and materializes its confined installed bytes, then
hands only bounded voice material to the speech boundary. Missing or ambiguous
descriptors and changed assets fail before style selection; native paths do not
cross the coordinator. Completed installs persist an immutable bounded
descriptor sidecar, so persisted voice selections can resolve and reverify
their blend after restart without fetching the remote catalog. Sidecar-free or
invalid retained binaries remain untouched and unavailable to this verified
offline path.
The Kokoro native synthesis coordinator revalidates the complete pinned model
bundle, selects its exact model artifact and runs cancellation-aware ONNX chunk
inference into a bounded WAV. It consumes already validated phonemization and
voice styles. The application TTS runtime now routes persisted Kokoro requests
to this native path while every other provider keeps the existing remote
runtime. It preserves the legacy model override, single or camel-case JSON voice
blend document and positive speed from the voice prompt, resolves only verified
offline voice descriptors, runs blocking native work outside the async worker
and returns the WAV through the existing durable media settlement. Missing
installable assets and runtime libraries retry; malformed requests and changed
artifacts fail before audio ingestion. The optional legacy `lexicon.json` is
read as a bounded pathless snapshot, parsed in its flat or global-plus-language
shape and merged for the primary normalized voice before eSpeak runs. Missing
files preserve the empty lexicon; malformed, oversized, unsafe or changing files
fail without rewriting or deleting user data. Android eSpeak hosting remains a
later platform slice.

The application sync hello coordinator initializes the database-owned device
identity before any change exists and builds a bounded current-protocol hello
with a caller-owned fresh session ID. Device identity survives reopen while
session identity rotates. Transport code will supply its authenticated peer
identity to the sync-domain negotiator; the application does not persist the
legacy one-time PIN or invent a trusted-peer registry.

The sync exchange coordinator runs the canonical change phase over a narrow
already-authenticated transport. It negotiates hello compatibility, exchanges
frontiers, sends and durably stages/applies bounded batches in both directions,
records monotonic peer acknowledgements and finishes only after both sides are
quiescent. An acknowledgement must name and causally cover the sent batch.
Cancellation is available to every transport await and checked before each
storage boundary. Unsupported input remains staged and returns a pending outcome
without acknowledgement, so an upgraded retry can resume without data loss.
Sockets, secure session pairing and status events remain later.

The persona-media sync coordinator exchanges only the bounded catalog of assets
currently referenced by personas and pulls missing content through a typed
authenticated transport. A caller first stages the canonical change batch; a
media dependency returns `Pending`. The media coordinator then resumes and
verifies only catalog entries that exactly match those staged canonical media
facts, commits their logical assets, and the same batch replay materializes the
persona without redispatch or data loss. Cancellation is
checked around every transport wait and mutation boundary.

The production loopback/LAN sync transport now binds a caller-selected socket,
creates or accepts a six-digit session PIN and returns the sync transport traits
only after both peers prove the PIN over fresh challenges. The proof binds both
ephemeral connection roles and durable device identities; the resulting
ChaCha20-Poly1305 session uses direction-separated monotonic nonces. Every typed
frame is length-prefixed, capped at 20 MiB before allocation, decoded with the
same bound and reconstructed through current domain validators. Host/client
ordering prevents two large change frames from filling both socket buffers, and
a bounded early-frame queue lets one peer serve media chunks while entering the
next phase. Cancellation covers accept, connect and every read/write; disconnect
leaves the database batch and confined media partial available for a fresh
session. A pending persona-media batch therefore reconnects, transfers one
shared blob for two logical assets and then converges through the existing
coordinators. Listener discovery, durable peer trust and frontend status events
remain separate frontend work; coordinator completion and errors are the
backend status boundary.

The provider backup coordinator reads one consistent provider/model/prompt
snapshot, derives the exact referenced secret inventory, and loads only those
values from the injected native `SecretStore`. It checks each secret generation
before and after the read, rejects missing or rotating values, and seals metadata
and secrets as separate authenticated sections. Secret plaintext is held in
zeroizing buffers and never appears in public backup metadata, errors or Debug
output.

That coordinator now includes global settings plus audio providers and user
voices in the same database snapshot. Its derived secret inventory covers both
provider credentials and every referenced `AudioApiKey`; either family must be
complete and stable before encryption begins.

The same export now carries the complete authored persona, lorebook, character
and group graph plus all logical media and blob metadata. A file-backed scenario
reopens that graph before export and verifies its default persona, roots and
ordered ownership documents. It also reads every unique ready blob through the
confined media store in bounded chunks and lets the transfer layer verify size
and BLAKE3 before encryption. Shared image assets produce one media section;
missing ready content fails the export. The same snapshot includes the complete
canonical ASR learning library across every language and scope, including voice
examples and their links to the already deduplicated audio sections. Restore
remains a separate application slice. The export also seals the complete visible
conversation history shared by direct and group chats, including archived
roots, policy and settings, branches, exact message order, all revisions and
candidates, initial origins and media references. A file-backed reopen scenario
preserves an empty starter message, ordinary starter content and a Director-mode
group that requires no selected speaker, and rejects a broken parent link. The
coordinator derives the complete unique protected snapshot and replay inventory
from that graph and streams each payload through
`ConversationArtifactTransferPort`. It enforces the envelope entry and aggregate
byte budgets before retaining the bytes, then requires the port descriptor,
size and BLAKE3 to match the graph reference. The file-backed scenario decrypts
and verifies the exact starter snapshot payload.

The same export includes the conversation-owned generation runtime: turns,
attempts, checkpoint timestamps, speaker and initial inference dispatches,
preparation attribution and tool executions. Its file-backed scenario advances a
turn through three durable stages, settles a tool result, fails the attempt, then
reopens and verifies the exact terminal state. Conversation UsageLedger and cost
bodies remain a later full-backup section while their IDs stay intact.

The export now also includes every generic scheduler job, its ordered lifecycle
events, and immutable job inference evidence with provider response identity,
reported counters and any existing cost basis. A file-backed scenario preserves
a claimed, progressed and retry-scheduled job plus its settled inference record
across reopen and rejects a corrupt event sequence.

The same file-backed export preserves conversation UsageLedger evidence for both
known provider counters with its captured pricing basis and unavailable counters
with their exact reason. Matching job inference is identified as overlapping
audit evidence rather than an additional charge, including after database reopen.

Conversation operation replay records and the complete immutable outbox journal
are exported from the same reopened snapshot. The scenario verifies contiguous
event order, terminal usage linkage, retained operations that emitted no event,
and rejection of a duplicated sequence.

The export also retains normalized companion relationship and session state,
ordered signals, continuity episodes and immutable replacement receipts. Its
file-backed scenario applies a real state transition, reopens the database,
checks exact owner, signal, episode and receipt identity, and rejects a corrupt
episode index. The companion calculation itself is unchanged.
