# lettuce-database

Epoch-2 SQLite schema, migrations, maintenance, and repository adapters.

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
either every expected revision advances or none do. Dynamic-memory settlement
extends that transaction to the memory-space CAS, so authoritative memory and
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
provenance/value pairing, and branch/message topology.
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
The foundation settings row keeps the dynamic-memory model as a typed model
profile foreign key beside the application default. Its narrow CAS update is
idempotent, and model/profile graph deletion clears either selection in the
same transaction before removing the referenced profile.
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
immutable row binds the interactive job and freezes both resolved profiles, prompt identity/revision, exact
prompt values, normalized starting draft, fallback format, and creation time;
ordered round checkpoints are appended separately and exact checkpoint replay
is idempotent. This preview store does not mutate character-owned Soul state.
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

Secrets are never stored here. Provider rows contain opaque `SecretRef` values
only. Blob registration validates SQLite-representable metadata and preserves
the first immutable metadata record for a content hash; physical `BlobState`
remains separate operational state and is not changed by deduplication.
Logical assets store only versioned, redacted provenance and retention; asset
mutations use revision CAS. Library pagination uses an opaque
`(updated_at, id)` keyset cursor. It is deliberately non-snapshot pagination:
rows added or updated between page requests may move relative to a prior page.
