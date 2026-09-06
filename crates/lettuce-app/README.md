# lettuce-app

Memory structured fallback and summary fallback preserve cache-read/reasoning
details when both responses supply them. Missing or overflowing detail sums stay
unknown. Input/output aggregation retains the existing fallback behavior.

The sole composition root, application workflows, desktop IPC boundary, and
optional local OpenAI-compatible host API.

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
compose those pieces into one claimed-job pipeline for a direct conversation
attempt. Admission creates or reuses the `ConversationGeneration` job keyed by
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
uniqueness and operation replay. Automatic scheduling, streaming progress
emission, group speaker selection and frontend commands remain outside this
runner.
`PreparedConversationGenerationJobRunner`, exposed by `AppBackend`, now owns the
reconstructible input boundary for an ordinary direct turn. It loads the durable
turn and branch ancestry, trims a finalized replay back to the turn's original
source message, resolves effective settings and the exact snapshotted live
model/account, assembles provider-neutral context, derives unique media grants
from that context, and invokes the claimed-job runner with tools disabled. Only
the stream sink and prompt runtime values remain caller-supplied; the sink stays
outside the durable initial-dispatch fingerprint, so replay may use a new sink
without another provider call. Context or model preparation failures map into
the existing run settlement categories. Dynamic memory and selected manual
memory still require a later input-retrieval slice because the launch snapshot
does not contain the execution policy or selected memory text; this boundary
fails explicitly instead of inventing those values.
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
