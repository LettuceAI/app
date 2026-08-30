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
`lettuce-creation` port without writing character, persona, or lorebook tables.
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

Secrets are never stored here. Provider rows contain opaque `SecretRef` values
only. Blob registration validates SQLite-representable metadata and preserves
the first immutable metadata record for a content hash; physical `BlobState`
remains separate operational state and is not changed by deduplication.
Logical assets store only versioned, redacted provenance and retention; asset
mutations use revision CAS. Library pagination uses an opaque
`(updated_at, id)` keyset cursor. It is deliberately non-snapshot pagination:
rows added or updated between page requests may move relative to a prior page.
