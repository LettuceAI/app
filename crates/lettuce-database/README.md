# lettuce-database

Migration 17 persists TTS audio-provider metadata and user voice profiles.
Provider kinds and versioned configurations are cross-checked on every read;
credential rows contain only scoped native-secret references and owner IDs.
Provider and voice updates use revision compare-and-swap while retaining their
creation timestamps. Foreign keys require every voice to have a provider and
delete dependent voices atomically when that provider is removed. The deleted
provider record is returned through the domain port so native-secret cleanup
can run separately without storing plaintext in SQLite.

Migration 17 also stores bounded discovered TTS voices by provider and response
ordinal. Refresh replacement runs in one immediate transaction, preserves the
provider response order and versioned label map, and cascades with provider
deletion. Failed or invalid replacement leaves the prior rebuildable cache
intact.

The usage ledger also reads the terminal usage event by turn and attempt
(`get_for_attempt`) through the existing event query; it adds no schema.

Initial generation dispatch is checkpointed in the original conversation
migration: `generation_initial_dispatches` holds one pending row per running
prepared attempt, bound to its job, request fingerprint and the job usage event
that will retain the raw dispatch evidence. The same immutable row stores the
versioned provider-neutral request without its runtime stream sink; reads
recompute its fingerprint, and an exact attempt-owned lookup supports restart
and interrupted-child recovery without consulting mutable preparation inputs.
Admission requires a running turn
and attempt with a resolved model, the attached job, matching prepared model and
attributions, an existing job and no tool executions yet. Settlement is the only
permitted update, writes the versioned result once, requires settled job usage
that agrees with the result, verifies conversation-retained replay artifacts and
records them in `generation_initial_replay_refs` so orphan cleanup keeps them.
Reads revalidate stored replay references and usage agreement; exact settlement
replay returns the stored record, changed results conflict, and SQL update or
delete of admitted rows, including request replacement, is rejected.

Conversation preparation now writes the existing turn model/prompt/memory fields
and ordered lorebook attribution rows in the mutation-kernel transaction. Job
ownership, preparing state and revisions are checked before writes; a failed
attribution insert rolls back the entire preparation. Exact replay verifies the
stored values, and recovery may reuse matching preparation under a child attempt.
The original conversation migration includes the preparation operation kind.

Preparation verifies the model artifact's stored reference, digest, and attachment
to the conversation using the existing snapshot verifier. Missing, foreign, or
forged artifacts cannot become turn provenance. Group preparation reuses the
candidate-author resolver: an explicit director target, a persisted mention or
automatic decision, or the original regeneration author supplies the identity.
It does not require an automatic selector or a SelectingSpeaker checkpoint.
Unresolved multi-character turns fail before preparation writes.

A file-backed lifecycle scenario covers direct, automatic group, explicit mention,
and director continuation preparation; ordered prompt/lorebook entry attribution;
reopen; interruption and child-job reuse; changed child rejection without writes;
and child finalization. Group regeneration also prepares and finalizes without
reselecting the original author. Historical attribution revisions are retained
without consulting current authored entries. This is repository recovery coverage,
not host scheduling or response checkpoint replay.

Optional cached-input and reasoning token evidence survives usage ledger,
creation inference-round, dynamic-memory inference-round and summary-checkpoint
round trips. Original migrations 9/10/11 store nullable nonnegative counters.
Raw counters are not clamped; billing applies its existing clamping policy.
Cache-write and web-search counts follow the same nullable, checked persistence
path in the original migrations, including summary and inference checkpoints.
Optional provider-reported cost follows that same path as a checked finite,
nonnegative REAL, retaining the legacy f64 precision without rounding to cents.

Migration 10 also retains immutable per-event usage-cost bases through
`UsageCostLedger`: versioned prices, provenance and calculation inputs are
stored separately from raw usage evidence. Exact retries are idempotent;
changed bases conflict and SQL updates/deletes are rejected. Costs are derived
with the basis version's calculator rather than current provider prices.

Job-owned inference usage in migration 10 admits each dispatch against an existing
job and settles its versioned result once. Pending and settled evidence survives
database reopen and job retention cleanup. SQL guards reject evidence mutation
and deletion; exact admission/settlement replay remains idempotent.

`UsageCostLedger::record_job_cost` and `get_job_cost` attach the existing
version-1 pricing basis to known response usage in `job_inference_usage`.
Migration 10 stores it in `job_usage_costs` with an evidence FK and immutable
update/delete guards. Job cleanup retains both evidence and costs. The same
domain validation checks model/provider identity and all known counters and
reported amounts; unavailable dispatch usage is rejected. The file-backed
usage regression verifies cost replay after reopen and job deletion, conflicting
bases, invalid evidence, and unchanged raw usage. Prices remain caller supplied.

Staged lorebook source documents are retained by project/source identity in
migration 11. Admission writes their asset references and the project in one
transaction, requiring ready source-document assets. Foreign keys protect the
assets even when their original retention class is temporary; references live
as long as the saved project, including cancelled and committed projects.
Loading verifies the reference rows against excerpt provenance in the run JSON.

Epoch-2 SQLite schema, migrations, maintenance, and repository adapters.

The legacy migration boundary can open an old `app.db` read-only, require the
actual version-92 schema roots, and return a bounded typed import inventory.
It performs no source migration or destination writes during preflight.

Persona planning reads the complete version-92 persona shape in stable creation
order, enforces the 10,000-record transfer bound, and fails on malformed JSON,
IDs, crop, LoRA, default or timestamp data rather than dropping fields.

Lorebook planning reads roots and their entries together, validates exact
legacy enum and boolean encodings, and rejects malformed keywords, timestamps
and orphan entries. External world-info fields already discarded by the legacy
importer are not fabricated as version-92 database fields.

ASR learning planning reads all four final version-92 learning tables through
the same read-only connection. It retains every authored value, normalized
value, metric, timestamp and voice-audio locator in stable source-ID order,
validates optional term and correction links within the complete plan, and
enforces per-table plus aggregate limits before returning any output. Planning
does not ingest audio or mutate either database.

Legacy import admission persists immutable assignments for all four ASR row
kinds alongside media assignments. Source integer IDs map to typed destination
UUIDs, replay returns the original mapping after reopen, and the foundation
schema accepts these assignment kinds only while the run is admitting. This
does not write live ASR rows or mark their audio complete. A later immediate
transaction resolves those assignments into all four live ASR tables only after
each voice asset has an immutable media completion. The ASR result receipt is
immutable and exact retries replay it after reopen; invalid timestamps, missing
audio, dangling links or destination collisions roll back every ASR row.

Provider/model planning reads the final version-92 settings, credential and model
tables through a read-only connection. It applies the legacy credential resolution
order, validates defaults and parent bindings, maps custom protocol configuration
and model scopes/settings into typed candidates, and enforces separate account and
profile bounds. SQL exposes only API-key presence plus header names, types and
lengths, so secret values do not cross the source-reader boundary. Unmapped field names
identify later embedded-runtime, image and feature-policy work without copying their
values out of the source. File-backed tests compare the database bytes before and
after planning and cover mapped fields, defaults, stable order, malformed rows,
orphans and both limits.

The version-92 plaintext `api_key` column is authoritative because migration 7
backfilled it from the former secrets table. The obsolete `api_key_ref` is not
treated as secret evidence or copied. Legacy llama.cpp models keep their runtime
behavior through a deterministic secret-free built-in account instead of becoming
orphans.

## Boundary

The only crate allowed to depend on SQLite libraries.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Foundation schema

Migration 1 creates only migration bookkeeping plus four usable roots:
global settings, provider accounts, model profiles, and content-addressed media
blobs. Migration 2 adds the logical media-asset catalog, retaining the blob
catalog as the physical metadata boundary. Later feature tables arrive with
their owning vertical slices.

Migration 3 adds authored character graphs and starter-owned content. Migration
4 adds personas and their media/default associations. Migration 5 adds
reusable group profiles, ordered members, typed image associations, and the
optional complete group-owned starting scene graph. Migration 6 adds the
provider-neutral prompt/lorebook documents, ordered entries, and typed
character/persona/group lorebook bindings. Prompt/lorebook JSON is strictly
versioned and aggregate revisions use CAS; bindings follow the latest
lorebook revision while archived references remain readable.
Migration 7 adds nullable stable keys for built-in prompt entries, with a
partial per-prompt uniqueness index and strict nonblank/bounded storage checks.
Built-in reconciliation matches keyed entries so IDs and entry history survive
catalog refreshes, while ordinary user drafts cannot forge catalog keys.
Migration 8 adds the normalized conversation ownership graph: participants and
settings, durable branches/messages/turns, revisions/candidates, typed media
associations, generation attempts/checkpoints, operation/outbox records, and
usage references. It also stores bounded tool executions under the exact
conversation/turn/attempt that requested them. Tool request identity and input
are immutable, provider call IDs are unique within an attempt, state changes use
revision CAS, and terminal states cannot regress. Provider replay references use
the existing protected replay-artifact store rather than leaking opaque replay
data into ordinary rows. Multi-call provider responses are inserted in one
immediate transaction, so an ordinal or provider-ID collision cannot leave a
partially durable handler round. Each later tool round compare-and-appends at
the expected attempt ordinal. Multi-execution lifecycle transitions use the
same immediate-transaction rule: all executions share one attempt owner and
either every expected revision advances or none do.
The conversation repository can read the latest checkpoint sequence for an
exact turn/attempt, returning no sequence before the first append and preserving
that result across reopen. The existing contiguous insert trigger remains the
authority for sequence uniqueness.
Dynamic-memory settlement extends that transaction to the memory-space CAS, so
authoritative memory and
terminal tool outputs cannot survive independently. Durable history is
restrict-owned and composite foreign keys
keep every child scoped to its conversation and turn. Snapshot and provider
replay bytes are held in separate private artifact tables; ordinary conversation
rows store references and the artifact store verifies immutable metadata and
payload digests before reads or trusted transfer. A separate narrow provider
replay port materializes only fully matched protected references for exact
provider-native continuation; raw bytes remain outside repositories and DTOs.
The database currently exposes
an internal normalized create/read slice while the complete conversation
repository mutation port is still being implemented.
Initial scene and starter messages use normalized source rows tied to both the
conversation message and its selected snapshot artifact. Those rows are durable
attribution, not a license for storage to interpret protected artifact bytes;
the application launch planner materializes the validated authored content
before the eventual atomic creator writes it.
Migration 8 is the final normalized conversation schema. Generation turns
persist a typed target, retry source, provider request overrides, and
forced-speaker identity directly; there is no legacy runtime-contract column
or follow-up migration. Runtime triggers enforce target/input coherence,
retry terminal-source ownership, group speaker ownership, settings
provenance/value pairing, and branch/message topology. The conversation
adapter checks every generation-turn status change against the domain transition
table before issuing SQL, while the matching migration trigger remains a
storage-level invariant.
Global ID indexes support recovery lookups without weakening conversation-local
composite ownership. Outbox consumer leases, usage ledgers, and memory join
tables are intentionally outside this migration.
Migration 9 adds revisioned memory spaces and strictly typed ordered memory
items. The adapter creates spaces atomically and replaces a complete item set
under one immediate-transaction revision comparison, so a stale or failed
dynamic-memory round cannot partially alter the stored snapshot. The same
migration stores rebuildable ready/repair-needed embedding projections as
little-endian float BLOBs. Projection reads join exact live memory ID and text;
stale derived rows cannot affect similarity, while unchanged projections
survive the complete item-set CAS without rewriting their BLOBs. Repair queries
also synthesize work for live items with no matching projection, closing the
crash window between authoritative memory commit and derived-data persistence.
Round settlement can also require an exact memory revision when the reducer
produces no memory change, so concurrent memory drift cannot commit terminal
tool outputs derived from stale prepared evidence.
Migration 9 also stores one immutable versioned dynamic-memory preparation plan
per provider round, keyed by generation attempt and first execution ordinal.
Its digest and relational projections are checked on every read, while the
adapter verifies the attached job, exact ordered durable round slice and create
arguments, and unchanged memory revision. Multiple rounds in one attempt retain
separate evidence; an exact insert retry returns its stored plan, while changed
bytes or mutable dependencies conflict before recovery can restart the handler.
Ordered history reads verify every plan digest, relational identity and exact
execution slice. Settled older plans may name an earlier memory revision, so
history accepts only revisions at or behind the current root while the latest
active-plan read continues to require exact equality.
Interrupted recovery validates the existing conversation child-attempt link and
attached child job, then clones the exact parent call payloads under new child
execution IDs, advances them to running, remaps create preparations, and inserts
the child plan in one immediate transaction. Existing partial child state fails
closed; an exact committed retry hydrates the same rows.
Migration 9 also owns background dynamic-memory runs that are intentionally not
conversation generation turns. The immutable run binds the normalized
conversation memory space plus its complete starting snapshot, ordered
source-message roles, effective times, and active immutable revision/candidate sources,
full resolved inference profile, frozen time-awareness flag, and matching tool
contract. The same memory root owns a normalized cumulative summary, token
count, and ordered source-message cursor; immutable runs freeze the selected
summary interval and half-open window. Memory items preserve coherent optional source role, observed time,
and legacy `turn` precision alongside source-message attribution. The same
rows preserve superseded-by/time and ordered supersedes IDs, while the immutable
run freezes whether its tool contract enables companion supersession. It also
stores the selected JSON/XML structured fallback format so recovery cannot
silently change the parser or retry request.
One immutable pre-round summary checkpoint stores the validated cumulative
summary, token count, exact provider-neutral request context, usage, provider
request ID, and root-revision transition in the same immediate transaction as
the summary cursor CAS.
Migration 9 stores the conversation-owned `askFirst` prompt baseline and
pending/skipped state. Repeating discovery at the same unsummarized count is a
no-op; skip clears pending but retains the baseline, and a later full interval
advances the durable prompt count. Automatic or approved forced job admission
clears the row.
The foundation settings row keeps the dynamic-memory and group speaker models
as typed model-profile foreign keys beside the application default. Their
narrow CAS updates are idempotent, and model/profile graph deletion clears all
matching routes in the same transaction before removing the referenced profile.
CAS attempts provide
created/processing/succeeded/failed/cancelled/interrupted state; ordered rounds
and calls retain their exact provider-neutral request context, bounded usage,
and protected provider-replay references.
Admission and retry are idempotent, recovery interrupts the parent and clones
its exact round/call evidence plus any settled result checkpoints into one
processing child, and SQL ownership
guards prevent a foreign conversation, memory space, or source message from
entering the run. Background tool outcomes are immutable per admitted round and
commit in the same transaction as the memory revision CAS; stale revisions roll
back without results, while exact retries return the original settlement. The
SQLite scenarios also prove this lifecycle creates no conversation turn or
message.
Possession of `Database` is a trusted application-composition capability: ordinary
conversation repositories and DTOs expose artifact references only and cannot
export protected bytes. Trusted transfer remains a separate composition-only
capability.

Migration 10 owns the append-only usage ledger. The adapter derives conversation
ownership from the durable generation attempt, records known token counters or
one explicit unavailable reason with immutable model/provider revisions, and
implements the conversation `UsagePort`. Exact retries for one attempt return
the original usage ID; changed evidence conflicts, and SQL triggers reject
updates or deletes.
Migration 11 owns creation-helper proposal state. It persists the target and its
expected authored revision, user turns before inference, and an immutable
ordered proposal lineage. Workflow stage/current-proposal changes use revision
CAS and ownership/lineage triggers; exact retries are idempotent, while stale
base proposals and changed identities conflict. The adapter implements the
`lettuce-creation` port. Confirmed new-persona apply is the first deliberate
authored-domain write: new and existing persona applies reuse the persona create
and revise paths and commit the persona plus an immutable
workflow/proposal/destination receipt in one transaction. Receipt identity is
per workflow, allowing later distinct workflows to revise the same persona
through fresh authored CAS tokens. Other creation targets still cannot write
character, persona, or lorebook tables.
Confirmed new-character apply likewise reuses the complete character-plan
insert transaction and records a separate strongly referenced immutable
character receipt. Character root, scenes, and receipt commit together; a
failed graph insert or duplicate destination leaves neither partial children
nor a receipt.
Confirmed new-lorebook apply shares the complete lorebook aggregate insert
transaction and adds a strongly referenced immutable lorebook receipt. Root,
ordered entries, and receipt are one commit; exact retries never regenerate
entry identities.
Existing-lorebook apply reuses the complete aggregate CAS writer and the same
typed receipt table. It reconciles reviewed entry IDs in one transaction,
preserves hidden policy fields for retained entries, permits sequential
workflows against later root revisions, and rolls back root, children, and
receipt together on any collision or validation failure.
Existing-character apply similarly uses one character-owned profile/scene CAS
transaction and the immutable character receipt. The adapter preserves the
unreviewed graph, rejects dependent scene removal, and commits profile, ordered
scenes, one root revision bump, and receipt together.
Migration 12 owns character-scoped companion Soul state. Facts, ordered source
memory IDs, and supersession evidence are normalized rather than mirrored into
a JSON authority. The domain-owned adapter applies one typed `SoulChangeSet`
under immediate-transaction revision CAS, rewrites the bounded state, advances
the root, and inserts an immutable operation receipt atomically. Exact retries
return the original receipt; changed operation reuse, stale revisions, foreign
owners, malformed rows, and partial writes fail closed.
The same unshipped migration now stores one immutable companion growth run per
durable job and memory run. It freezes the successful memory attempt, profile,
prompt inputs, bounded evidence, Soul snapshot, and deterministic apply ID;
the adapter admits one immutable reduced-proposal checkpoint before Soul apply
and replays it exactly across restart.
Migration 12 also stores one consolidation run per succeeded growth job. The
row freezes the authoritative post-growth Soul revision and companion prompt
inputs plus a deterministic apply ID; its typed adjustment/retirement proposal
checkpoint is immutable and independently replayable before the later Soul
apply worker runs.
Migration 12 also stores explicit Soul-writer preview runs by request ID. The
immutable row binds the interactive job and freezes both resolved profiles,
prompt identity/revision, exact prompt values, normalized starting draft,
fallback format, and creation time. Ordered primary/fallback round checkpoints
are appended separately and exact checkpoint replay is idempotent, so a later
provider failure retains only durable incomplete work for retry. This preview
store does not mutate character-owned Soul state.
Migration 12 also owns normalized character-scoped scheduled notes with the
legacy recurrence, availability, expiry, enabled, and recurrence-window fields.
The SQLite adapter rejects non-companion owners, lists in legacy
`available_at`/ID order, normalizes authored label/content on upsert, and keeps
delete idempotent.
Creating a companion character now seeds its initial character-owned Soul root
and normalized authored facts inside the same character aggregate transaction.
An authored-fact failure rolls back the character and Soul rows together;
roleplay character creation writes no Soul root. Exact initial-state retries
through the Soul repository return the existing typed state only when state
and creation timestamp match.
Migration 13 owns normalized companion runtime state. Four emotional vectors
and ordered driver/active signals are conversation-local; relationship axes,
tension, stability, interaction count, and last interaction are keyed by the
companion character plus persona/default scope. Creating another conversation
for the same scope keeps its fresh emotion but hydrates the existing
relationship, regardless of dynamic-memory sharing. Atomic dual-revision CAS
updates both scopes and records an immutable request hash/receipt; stale writes,
changed retries, corrupt rows, and partial vector/signal writes fail closed.
The companion prepared-launch adapter now seeds those rows inside the existing
conversation creation transaction. A state-seed failure rolls back the entire
conversation launch, and replay validates the same frozen initial-state hash
before returning the already committed conversation.
Migration 13 also stores the legacy character/persona continuity sequence.
Each fresh direct companion launch closes only the prior open episode and links
the next index to it in that same transaction; exact launch replay leaves the
sequence unchanged, and a different persona starts its own sequence.

Migration 11 also stores immutable single-entry lorebook-generation runs. Each
request is bound to one generic creation job plus the real conversation,
lorebook, character/persona, model profile, and prompt revision; the complete
frozen request remains versioned JSON and exact replay rejects any drift.
Staged lorebook planner runs use the same migration to persist the frozen
project/profile/prompt boundary and its single immutable provider attempt.
Attempt persistence precedes the atomic planning-to-outline-review CAS, so a
restart cannot redispatch a completed planner request.
The following outline approval is another exact-replay CAS on the same staged
project row, atomically storing the ordered legacy-initialized pending drafts
and the drafting stage.
Migration 11 also stores immutable per-plan staged writer runs. Scalar
project/job/plan/model/prompt projections are checked against versioned JSON,
and exact request replay returns the original run while changed replay
conflicts.
Writer attempts are stored inside the immutable writer-run document before the
separate stable-plan project CAS. Exact checkpoint and draft settlement replay
without another provider request.
Draft-batch start, independent stable-plan success/failure, and drafts-ready
gating use project-row CAS updates. Parallel completions may advance the root
revision independently without invalidating another run from the same frozen
batch.
Stable-plan manual draft edits and approval toggles use the same project-row
revision CAS. Exact operation retries return the stored project, while stale or
changed retries conflict.
Refinement reuses the immutable staged-writer run table for its frozen input and
attempt, then commits the revised draft plus one history record through a strict
project-revision CAS. Exact settlement replay is restart-safe.
Coherence proposal submission and accepted-change application are project-row
CAS operations as well. The application receipt retains the exact accepted-ID
list so changed stale retries cannot masquerade as exact replay.
Coherence admissions and attempts are retained in the versioned staged-project
document before the separate proposal CAS. Multiple review cycles keep their
own immutable request/job evidence instead of overwriting an earlier run.
Prepared companion sends reuse the conversation send transaction: the user
message, generation turn/attempt, companion session and relationship revisions,
operation, and outbox commit together. A stale state CAS or hook failure rolls
the entire send back, while exact operation replay returns the existing turn
without applying the state transition twice.
Migration 13 also normalizes the legacy companion turn-effect seed as
relationship columns, three emotion-delta vectors, and ordered signal changes.
Only dynamic-memory companion sends create that hidden turn-owned seed.
Companion continuations use the same boundary with the legacy zero-delta seed
and nullable user-message reference.
Assistant finalization creates the processing effect in the same transaction;
failure/cancellation removes an unconsumed seed, while interrupted recovery
keeps it with the same turn. Ready/failed settlement stores typed memory IDs,
the exact source-message window, and bounded summary data behind the
companion-owned repository port. Terminal effects are immutable.
Migration 9 stores one immutable dynamic-memory retrieval-access receipt per
generation attempt. Its transaction verifies conversation, attempt, space,
revision and every selected active memory before applying the legacy cold
promotion and access-count updates, then advances the memory root once. A
matching retry returns the recorded resulting revision without touching the
items again; changed input and stale selections roll back as conflicts.
Migration 13 adds immutable delete-after rewind receipts and companion-effect
invalidation rows. The adapter uses an admitted run's starting snapshot and an
earlier summary checkpoint as the rollback authority, applies one memory CAS,
restores the summary without a second root revision bump, clears pending
approval, and records the exact retry result atomically. Effect invalidation is
an overlay, so migration 13's terminal effect evidence is never rewritten.
The versioned operation `result_json` and outbox `event_json` envelopes are the
canonical payloads; scalar columns are routing/index projections. The future
full repository must validate projection equality on every write and hydrate.
Usage rows are references owned by the external `UsagePort`, not copied usage
records.

Migration 8 also stores one immutable provider-backed speaker-selection
dispatch per generation attempt. Admission requires the attached attempt to be
preparing at `SelectingSpeaker`; settlement requires the linked job inference
evidence to be terminal and the selected participant to be enabled and unmuted.
The row retains the exact request fingerprint, usage-event identity and final
decision, so process reopen replays selection without another provider call.

Sessions, conversation assembly/resolution, starter-link normalization,
import/export, hard purge, sync/backup, FTS, and legacy text-column retrofits
remain outside this slice. Existing prompt/lorebook text columns are not
silently migrated or overwritten.

`Database` owns a serialized `rusqlite` connection, enables foreign keys and a
bounded busy timeout, and uses WAL for persistent files. Repository traits stay
in their domain crates; this crate contains their SQLite implementations. The
application is responsible for running these synchronous operations on its
database worker rather than a UI or async-runtime thread.

The generic `JobStore` persists versioned specifications, snapshots, and an
ordered event table here. Each mutation loads the durable aggregates into the
single lifecycle reducer in `lettuce-jobs`, applies it under `BEGIN IMMEDIATE`,
and writes only changed jobs and events before commit. This preserves the
reference store's idempotency, lease, cancellation, retry, progress, pagination,
recovery, and retention behavior across process restart and concurrent database
handles without creating a second scheduler state machine.

Migration 14 stores admitted ASR transcription requests and their single
immutable successful result. Each row is bound to a `speech_transcribe` job, a
speech-request identity, and a ready audio asset; request/model/audio bindings
cannot change after admission. A result can settle once and remains available
after process reopen so job recovery can finish without transcribing twice.

Migration 18 stores one immutable TTS synthesis request and successful result
against a `speech_synthesize` job. Settlement requires the request's exact
synthesized-speech asset, producing-job provenance, and admitted temporary or
persistent retention. Scalar request and result projections are validated
against versioned documents on every read. Repeated admission and settlement
must be exact, and completed evidence cannot be updated or deleted.

Migration 15 stores immutable installed-Whisper manifests separately from user
media. Scalar model/path/size/hash projections must match the versioned
manifest, and repeated admission is exact. Updates remain disabled. The
model-hub removal workflow may conditionally delete the exact stored manifest
only after its managed bytes are gone; retained legacy models are refused
before this repository call.

Staged lorebook final apply reuses the existing lorebook aggregate insert/replace
functions inside the project transaction. The committed project stores its
request and result receipt in the versioned run; exact retries return the same
entry identities, including after subsequent changes to the authored book.
Staged project cancellation and its job cancellation events commit atomically,
using the existing job reducer. Queued work settles immediately; claimed work
keeps its lease for worker cleanup or startup expiry recovery. Late writer
admission/checkpoints cannot mutate terminal projects.
Pre-approval outline replacement uses the same staged project CAS, reindexes
the submitted order, and retains the immutable planner attempt.
Batch settlement rejects a writer from an older batch after a failed draft is
selected again. Terminal projects cannot reopen an unfinished batch.
Draft failure settlement checks the writer's admitted project revision against
the current batch, just like successful settlement.
Explicit planner retry creates the new generic job and archives the previous
planner attempt in the same transaction as the project revision/job binding.
The old job remains stored; retries with the same request identity are deduplicated.

Secrets are never stored here. Provider rows contain opaque `SecretRef` values
only. Blob registration validates SQLite-representable metadata and preserves
the first immutable metadata record for a content hash; physical `BlobState`
remains separate operational state and is not changed by deduplication.
Logical assets store only versioned, redacted provenance and retention; asset
mutations use revision CAS. Library pagination uses an opaque
`(updated_at, id)` keyset cursor. It is deliberately non-snapshot pagination:
rows added or updated between page requests may move relative to a prior page.

Job dispatch result JSON retains optional provider response IDs with serde
default compatibility. The file-backed usage scenario rejects changed-ID
settlement and verifies the identity survives reopen/job cleanup. No migration
is needed for this addition to the existing versioned response JSON.

Job-result replay compares decoded versioned values, so an older stored response
without the optional ID remains an exact replay of an absent-ID response. The
regression exercises the old bytes directly without rewriting the evidence.

Optional OpenRouter generation/endpoint evidence lives inside existing cost
basis JSON. The usage domain validates response-ID binding, endpoint/price
agreement and native billing counts while raw dispatch responses remain
immutable. Existing get/record methods persist and replay the enriched basis;
no schema change is needed. App file-backed tests cover differing raw/native
counts and cost amounts surviving reopen without a second lookup.

Migration 1 also stores sealed legacy import admissions and their stable destination ID assignments. Admission is one immediate transaction: the source schema, inventory and plan fingerprints are immutable, assignments can only be inserted while the run is being admitted, and a rollback leaves neither the run nor a partial mapping. Exact replay survives reopen; a changed binding or source set conflicts. Provider accounts additionally receive stable secret-owner IDs, and pending API keys and ordered header names receive opaque secret references. The assignment schema has no secret-value column and admission does not create provider, model, persona, lorebook, entry, asset, or blob rows. Graph materialization leaves a provider-bearing run importing so later provider transfer can complete the same sealed run.

The read-only legacy provider-secret adapter lists only planned API-key/header
metadata, then loads one exact value into `SecretValue` on demand. It ignores the
obsolete pre-v7 API-key reference and never opens the source writable. Migration
1 records immutable per-secret completion generations tied to the sealed
assignment. These receipts contain no value or digest and cannot be inserted for
an unassigned reference.

Migration 2 stores immutable per-object legacy media completion receipts. Each receipt must match the sealed path, size, hash and destination asset, and SQLite verifies that the asset points to the recorded ready content-addressed blob. The first receipt advances the run from admitted to importing in the same transaction. A failed later object leaves earlier receipts replayable without marking the run complete.

Legacy persona and lorebook materialization reuses the aggregate insert paths inside one immediate transaction after every assigned media object has a verified receipt. It preserves assigned root and entry IDs, authored fields and timestamps, legacy lorebook behavior, ordered entry and persona-binding semantics, and the default-persona singleton through its initial revision CAS. The immutable result receipt and completed run state commit with the graph. Exact retry survives reopen; missing receipts, changed plans, destination collisions, binding failures and default conflicts roll back all writes from that attempt. The legacy source database and storage tree are never mutated or removed and remain retained until separate verification and explicit user approval.

Provider/model/prompt materialization rebuilds account secret references only from the
sealed assignments and verified non-secret completion rows. One immediate
transaction validates and inserts assigned provider accounts before their mapped
model profiles, inserts validated imported prompt documents and ordered entries,
and activates mapped model and app-wide prompt defaults. A dangling legacy app
prompt selection remains sealed evidence and falls back instead of inventing a
target. The same transaction writes an immutable result receipt. A collision, orphan,
invalid mapped configuration, missing or mismatched secret receipt, or authored
default rolls back every provider/model/default write while preserving completed
media, graph and secret evidence. The run reaches completed only when both graph
and provider/model/prompt receipts exist. Model-level prompt references, deprecated
system prompts and unsupported field names remain sealed in the plan fingerprint
and retained source rather than becoming unused live columns.

Migration 16 stores ASR vocabulary and correction rules behind the
speech-owned repository port. Authored text and categories remain separate from
normalized lookup columns, and no save path truncates them. Queries reproduce
the legacy scope/language filters and runtime order. A file-backed scenario
proves prompt construction, correction order, CRUD and reopen behavior. The
same migration stores ignored edit suggestions with a null-safe unique identity,
counts repeated ignores, and removes matching ignored rows atomically when a
correction is saved. Chat and group edit scenarios verify suppression, repeated
acceptance, scope promotion and two reopen cycles. Legacy ASR row transfer is
materialized through the same validated records and preserves every counter.

Migration 16 also stores ASR voice examples through a composite foreign key to
an audio-kind media asset. Vocabulary and correction links use `SET NULL`, while
the audio asset remains retained by `RESTRICT`. The adapter validates normalized
text on every read, orders by creation time and ID, and rejects missing or
non-audio assets. File-backed coverage proves create, update, edit-derived
suggestion, link clearing, deletion and two reopen cycles. Legacy ASR row
transfer resolves assigned managed audio assets, so native paths are never
copied into live records.

The ASR repository lists ignored suggestions with the same scope/language
semantics as the other learning records and imports a validated learning batch
in one immediate transaction. Fresh IDs and remapped links are inserted only if
all referenced managed audio exists; any constraint failure rolls back the
complete batch.

Migration 19 starts the canonical sync journal. It keeps one durable local
device identity, monotonic origin sequence and hybrid clock, the current causal
frontier, immutable change rows and their immutable frontier snapshots. The
sync-owned repository port allocates identity and clock facts in the same
immediate transaction as a validated canonical change. Exact operation replay
returns the stored change; changed reuse conflicts, and a failed insert rolls
back the device state and frontier. Aggregate coverage is added explicitly at
each repository transaction rather than through arbitrary SQL capture.
The same port can initialize and read the stable device identity before the
first change, leaving the frontier empty and the first later change at sequence
one. This supports session hello creation and survives reopen.

Persona repository create, authored revision and all four media mutations are
atomic journal call sites. Their canonical payload is the complete
post-mutation aggregate, including ordered media references but no media bytes;
revision updates bind the pre-mutation snapshot hash as their base. Aggregate
and journal writes share one immediate transaction, so a missing or duplicate
media reference, stale CAS or later SQL failure rolls back both. Exact retries
return the stored persona without allocating another sequence. Persona default
set and clear append a canonical singleton update in the same immediate
transaction as their existing CAS write. Exact retries preserve the first
timestamp and sequence; missing or archived targets, stale revisions and either
singleton or journal failures leave both sides unchanged. Persona archive and
restore journal the complete lifecycle transition. A selected persona archive
also journals the coupled default clear; failure of either change rolls back
both aggregate writes and both sequence allocations. Restore does not reselect
the persona, matching the existing lifecycle contract.

Migration 19 also stores durable monotonic peer frontiers. The sync adapter
exports the local canonical journal in bounded causal-ready batches, rejects a
gap instead of silently skipping it, clamps acknowledgements to the local
frontier and preserves them across reopen. Network sessions, incoming apply and
conflict handling were deliberately left to the following storage slice.

Migration 19 now also stores immutable incoming batches and conflict evidence.
Persona/default batches are staged before application, then supported snapshots,
remote changes, causal frontiers, conflict evidence and remote-clock observation
commit atomically. Exact committed delivery replays across reopen. Unknown
schemas, origin gaps and missing materialization dependencies stay pending with
their original bytes. The adapter never deletes pending input or either side of
a conflict. Bounded conflict reads decode and validate both typed candidates.
Choosing current or other atomically applies a fresh revision, appends a local
canonical change and records the immutable resolution; exact replay survives
reopen. Replicated changes that causally dominate both candidates close the
corresponding remote conflict as superseded. Session hello identity now consumes
the durable device ID through the sync port. Peer transport, authentication and
blob transfer remain later sync work.

Encrypted backup export reads provider accounts, model profiles, all prompt
documents with ordered entries and their current global selections in one
deferred SQLite transaction. The adapter returns typed non-secret domain data
through the transfer-owned source port; native secret values remain behind the
application's `SecretStore`.

The same snapshot transaction includes the exact global settings payload and
all audio providers and user voices. TTS ownership and voice-to-provider links
are decoded through the existing validated row readers before the graph leaves
the database boundary.

That transaction also reads every authored persona, lorebook, character and
group aggregate through the existing aggregate decoders, including archived
roots, ordered children and every owner-lorebook binding. It includes all media
asset and blob metadata in stable ID order. The transfer graph rejects dangling
model, prompt, voice, lorebook and media references before encryption; SQLite
does not read media files or secret values.

The same snapshot reads every ASR vocabulary term, correction, ignored
suggestion and managed voice example without language or scope filtering. Voice
examples keep their term, correction and audio asset links. Per-table and total
transfer limits are enforced before the graph leaves the database boundary.

The deferred backup transaction also reads the unified direct and group
conversation graph through the existing validated aggregate and message row
decoders. It includes archived roots, all branches and selected heads, every
message in exact timeline order, every immutable revision and candidate, and
initial-origin and media references. Global bounds apply while rows are read;
runtime generation, tool, job, usage, memory and protected artifact payloads are
reserved for their own backup sections.

Conversation backup also hydrates all generation turns and attempts through the
normal strict turn reader, then reads contiguous checkpoint timestamps, speaker
and initial dispatch checkpoints, and exact tool execution state in the same
deferred transaction. Preparation model, prompt, lorebook and memory attribution
is already part of each hydrated turn. No SQL or raw database document crosses
the transfer boundary.

The same deferred snapshot reconstructs every durable job through the existing
strict job-store loader, then reads ordered immutable inference evidence and
optional job cost bases independently because that evidence survives job
retention. Scalar identity columns must agree with their typed documents, and
global job, event and dispatch limits are checked before the graph leaves SQLite.

Conversation UsageLedger events and their optional captured cost bases join that
same deferred backup snapshot through the existing strict usage decoder. Historic
events remain valid when their provider account or model profile has since been
deleted. The adapter derives exact overlap IDs from retained job inference
evidence, rejects orphan cost rows, and does not calculate or combine charges.

The same transaction reads all conversation operation records and immutable
outbox events through their existing typed documents and relational validators.
It preserves operations that emit no event, verifies scalar result projections,
event timestamps, ownership and current foreign-key targets, and applies bounded
global limits before the journal leaves SQLite. Consumer delivery state is not
fabricated because migration 8 stores no lease or acknowledgement table.

Normalized companion relationships, session emotion vectors, ordered driver and
active signals, continuity episodes and immutable state-apply receipts are read
in the same deferred backup transaction. The adapter preserves initial-state and
replacement hashes, persona-key projections, revisions and row timestamps while
reusing the state adapter's strict vector and signal readers.

That transaction also reads all companion turn effects through the existing
strict effect hydrator and preserves the immutable dynamic-memory suffix-rewind
rows with their storage-only request digest and revision projections. Resulting
memory and summary snapshots are decoded as versioned domain values, and ordered
effect invalidations remain attached to their rewind operation. The database
does not interpret these rows as current memory state or recalculate companion
effects during export.

Authoritative conversation memory spaces now leave the same snapshot through
the existing strict memory and summary hydrators. The adapter includes every
ordered item and summary source cursor, then decodes immutable retrieval-access
rows with their exact attempt owner and before/after revisions.

The same snapshot exports every memory embedding projection row, including
stale derived rows that current retrieval deliberately ignores. Ready vectors
retain their exact BLOB bytes; repair-needed rows retain their null-vector
state. Space/item IDs, source revision and text, dimensions and update time are
preserved without regenerating embeddings. Dynamic-memory run tables remain for
their own bounded document.

Dynamic-memory backup now reads pending approvals, every immutable visible-turn
preparation document, and the complete background run graph in the same deferred
transaction. Existing strict hydrators validate frozen run sources, retry
attempts, ordered inference rounds and calls, background settlements and summary
checkpoints before they leave SQLite. Storage-only preparation and settlement
digests remain explicit, including exact preparation JSON, so restart evidence
is not reconstructed during export.
