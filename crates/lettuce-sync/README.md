# lettuce-sync

Versioned domain changes, peer sessions, conflict policy, and blob negotiation.

## Boundary

Replication never exposes or copies raw SQLite state.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The canonical change boundary is active. It defines immutable change and device
identities, hybrid timestamps, bounded causal frontiers, entity identities and
distinct insert, update and delete contracts. Versioned canonical payload bytes
carry their own content hash, and a deterministic change fingerprint binds all
causal, entity, operation, revision and payload facts.

This deliberately replaces legacy raw SQLite changesets and schema-position
coupling at the domain boundary. Insert requires a payload without a base
revision, update requires both and delete requires a base revision without a
payload. Payloads retain the legacy eight-megabyte single-revision ceiling while
identifiers and causal frontiers now have explicit bounds.

The local journal port admits a validated change request under a stable
operation ID and returns the allocated immutable change. Exact retries replay;
changed operation reuse conflicts. The SQLite adapter owns device identity,
monotonic origin sequence and hybrid-clock allocation in the same transaction
as the journal row.

The first entity codec is the complete validated persona snapshot. It includes
authored fields, lifecycle, revision/timestamps and ordered media references,
while media bytes remain content-addressed elsewhere. Stable create, revise and
media-mutation operation identities let the SQLite persona repository append
the insert or update in the same transaction as its aggregate mutation. Media
operation identities bind the normalized mutation kind and intent. The update
base is the canonical pre-mutation snapshot hash.

Persona media replacement, attachment, detachment and reordering now journal
the complete resulting snapshot atomically and replay exact retries without a
second sequence. The revisioned application-default persona singleton has its
own versioned canonical state, entity identity and intent-bound set/clear
operations. Default selection changes share their SQLite transaction with one
canonical update and retain the prior singleton snapshot hash as the base.
Persona archive and restore use distinct lifecycle operation identities and the
same complete snapshot update contract. Archiving the selected default also
emits a distinct default-clear update in the same transaction, so both
revisions and both journal rows either commit together or remain unchanged.
Every other aggregate still needs explicit journal wiring. Other aggregate blob
families and legacy sync-state migration remain later slices. No legacy
database, source or user asset is read, rewritten or deleted by this crate.

The journal now exposes its local causal frontier, bounded outbound batches and
durable peer acknowledgements. Outbound reads cover every origin in the local
frontier, not only this device (backlog #19: a peer now relays changes it
received from a third device), start after the peer frontier for each origin,
retain each origin's order and merge origins by readiness, then hybrid
timestamp and device ID; every causal dependency must be satisfied and a
missing sequence fails closed. Caller limits are capped at 256 changes and 16 MiB of
payload. Peer acknowledgements only advance and are clamped to facts present in
the local frontier.

Incoming replication now has one bounded canonical batch boundary. A batch hash
binds ordered change fingerprints; the repository stages metadata and payloads
durably before apply. The first materializer accepts complete persona and
default snapshots, advances change/frontier evidence in the same transaction as
the aggregate, observes remote hybrid clocks and replays committed delivery
after restart. Unsupported schemas and causal gaps remain pending; a later
session re-sends the same changes in a new batch. Backlog #20: two cases used
to stay pending forever and now commit. An update for a persona this device
never received (created before journaling existed on its origin) adopts the
complete snapshot, and a default change whose persona is archived or missing
here is journaled with the local default kept and an unresolved conflict
recorded (winning side current). Choosing the other side of that conflict is
refused until the persona is restored; any later local default change
supersedes it. Conflicts whose current side predates the journal (the seeded
default) are superseded by a later change that observes the incoming side. Concurrent
persona changes retain both snapshots and deterministic winner evidence in an
immutable conflict record.

Local persona operation identities are scoped to the latest remote change that
won locally for the entity (backlog #21): a remote winner can move a persona or
default revision backwards, and revision-derived identities would then repeat
and turn the next local edit into a permanent stale-revision replay. Entities
never touched by sync keep their unscoped identities, and a losing remote
change leaves the scope unchanged, so an exact retry still replays.

Persona conflicts can be listed as at most 100 typed current/other candidates
and resolved by choosing either side. Resolution always creates a new local
canonical snapshot with a fresh aggregate revision, even when the current
candidate is retained, so the decision observes and propagates beyond both
concurrent changes. The original evidence remains durable; exact decisions
replay after restart, changed or stale decisions fail, and a later observed
resolution marks the corresponding remote conflict superseded.

Session hello negotiation now uses the durable local device identity and a
fresh session identity. It requires exact app and protocol versions plus a
fingerprint of the supported canonical payload schemas, replacing the legacy
dependency on positional SQLite schema equality. Device names and version
strings are bounded, authenticated transport identity must match the hello, and
both peers use the smaller validated batch limits. Pairing remains
session-scoped, matching the legacy UI; no persistent peer-trust model is
invented. The application layer now supplies secure pairing and the actual
exchange; frontend status flow remains later work.

The session, change, acknowledgement and persona-media frame values support the
application transport's bounded binary codec. Decoding alone does not grant
validity: the transport reconstructs every received hello, frontier, canonical
change, batch and catalog through the existing constructors before a
coordinator can observe it. Pairing, encryption, socket ownership and
cancellation remain in `lettuce-app`; this crate contains no network access or
trusted-peer persistence.

Canonical session frames carry either one validated identity-and-hash-bound
change batch or explicit quiescence, followed by an acknowledgement containing
the exact optional batch ID and causal frontier. Batch construction enforces the
negotiated count and byte limits before a transport can send or accept it.

Persona-referenced media now has one canonical asset snapshot containing the
logical asset metadata and its ready content descriptor. The persona repository
journals each referenced asset at most once, immediately before the persona
snapshot in the same transaction. Persona-independent media is not added to the
sync journal. Incoming media changes remain durably pending until the verified
logical asset is available, so a persona can never materialize with dangling
image references. Catalogs are limited to 256 distinct persona assets and blob
chunks to one MiB. Blob identity remains the BLAKE3 content hash; native paths
and bytes never enter canonical change payloads.

State-scanned aggregates (sync S2). Instead of wiring every mutation, a sync
session first calls `LocalChangeJournal::journal_current_state`: for each
scanned kind, in dependency order, the current canonical snapshot of every
entity is compared with the latest journaled one that became local state
(incoming changes that lost a conflict are skipped), and differences are
journaled as insert or update; journaled entities that no longer exist are
journaled as deletes in reverse order. Edits, imports and restores therefore
replicate with no per-mutation code, and because the scan runs before anything
is received, incoming changes always meet journaled local state. Deviations
from legacy's per-write capture: edits between sessions collapse into one
change, and its hybrid timestamp is the session's, not the edit's. Scanned
today: provider accounts (`provider_account.snapshot`, secret references travel
as opaque identifiers, secret values never leave the device) and model
profiles (`model_profile.snapshot`). Deletes follow legacy: a delete beats a
concurrent update on both sides (the discarded update is kept as losing
conflict evidence, so a later update in the same chain cannot resurrect the
entity); a delete this device must refuse (any foreign key here still
references it, checked in a savepoint) is only journaled, and the next scan
journals a fresh insert in dependency order so every device keeps it. A model
whose account was deleted here is journaled but not materialized; the next
scan journals its delete, so both devices converge. Applying a remote model
delete clears app defaults pointing at it locally (like a local delete); when
settings sync lands that side effect must be reconciled. Snapshot structs
reject unknown fields, so any field change must bump the schema version, and
serde_json uses `float_roundtrip` so materialized snapshots re-encode
identically. The persona conflict listing only lists persona conflicts; other
kinds keep their conflict evidence without a resolution surface yet.

Media without a catalog (sync S3a, protocol version 2). The media phase no
longer exchanges a catalog of every referenced asset (capped at 256, which
cannot hold a character library): each side fetches, by content hash, the
blobs of the media assets its pending incoming batches wait for (their
payloads already carry the full asset and blob metadata), then exchanges
`MediaDone` while still serving the peer's blob requests. The state scan
journals every ready asset referenced by personas, characters, their
presentation and scene assets once (insert only, same deterministic operation
as persona journaling); later local metadata such as retention stays
device-local, and an incoming asset insert is checked only against identity
and blob fields. The media phase fetches only what the latest pending batch
from the connected peer waits for (older pending batches are superseded by
the peer's resend); a blob the peer cannot serve is answered with
`BlobUnavailable` and skipped instead of ending the session. An aggregate
whose referenced assets are not journaled yet (their blobs are not ready) is
not journaled in that scan, so a receiver never waits on media that cannot
arrive.

Characters (sync S3b, `character.snapshot`): the complete `CharacterDetails`
(root, scenes, variants, starters) is a scanned aggregate after models. The
materializer updates the root row in place (never deleting it: conversations,
Souls and creation runs depend on it) and replaces scene, variant, starter,
media and presentation rows; a new companion character gets its initial Soul
exactly like a local create. Missing media assets keep the batch pending for
the media phase (their inserts precede the character in origin order). A
default model deleted on this device is cleared instead of blocking the
origin's later changes, and the next scan journals the cleared default.
Characters are never hard-deleted, so they journal no deletes.

Lorebooks and bindings (sync S4). Personas and the persona default are
scanned too (their explicit journaling stays; the scan journals what it
missed, such as the owner revision bump of a binding change or a seed default
that was never journaled, which now syncs as an insert). Lorebooks sync as
complete `LorebookDetails` (book row updated in place, entries replaced; a
missing icon asset waits for the media phase, and lorebook icons join the
media scan). Each character's and persona's ordered lorebook bindings are one
entity (`lorebook.bindings`); removing an owner's last binding journals a
delete, which clears that owner's bindings on the peer. Group bindings follow
with groups. Scan order: accounts, models, personas, persona default,
characters, lorebooks, character bindings, persona bindings.

Prompts (sync S5, `prompt.snapshot`): every prompt document syncs, built-ins
included, since built-ins are editable in place. Built-in document ids are now
derived from the built-in key (UUIDv5) and built-in entry ids from the
document id and entry key, so every device seeds the same identities and
references to built-ins (characters, starters, settings, derived prompts)
resolve across devices; two devices' independent seeds meet as one entity and
settle by last-writer-wins. Prompts are scanned in derivation order (a derived
prompt after its source) before characters. Entry revision and timestamps are
local bookkeeping outside the snapshot. A prompt whose built-in key is held by
a different local id (databases seeded before deterministic ids) is journaled
but not materialized.

Groups (sync S6, `group.snapshot`): reusable groups sync as complete
`GroupDetails` after characters (root updated in place, members, presentation
references and the starting scene replaced); a member model override deleted
on this device is cleared like a character default. Group lorebook bindings
join the binding entities, and group presentation, background and scene
assets join the media scan.

Application settings (sync S7, `app_settings.snapshot`): the settings row is
one singleton entity (preferences, default model and prompt selections and
the app-wide model settings layer), scanned after prompts. A selected model or
prompt missing on this device is cleared. Because a scan journals upserts
before deletes, a peer receives the cleared defaults before the model delete
that caused them, so its own clearing is a no-op.

Per-entity deferral. A journaled incoming change that waits for another
entity that is not here yet (its owner, lorebook, derivation source or media
asset) is settled inside a savepoint, rolled back and recorded in
`sync_deferred_changes` (a per-entity queue in journal order: a later change
waits behind an earlier deferred one) instead of holding the
batch: the batch commits, the origin's frontier advances and its later changes
still apply. Deferred changes are settled again after every batch (until no
progress) and after each media phase, whose pending media now comes from
deferred media changes. Deferred changes are not local state for the scan or
operation scoping. Batches stay pending only for causal gaps or unknown
schemas. Binding lists are ordered lists, so an emptied list journals an
update to `[]` instead of a delete (a concurrent bind is not lost to a
delete).

Reference and seed rules. A reference to a model or prompt that is missing
here is cleared only when nothing for it is deferred; a deferred one makes the
referencing aggregate wait. A group waits for its persona and member
characters. A deferred change that later fails for another reason stays
deferred instead of failing every later batch. An untouched seed snapshot
(settings or default persona still at revision 1, as on a freshly installed
device) always loses to an edited one, and an edited one always beats it. An
emptied binding list with a journaled history counts as present, so a
concurrent edit is detected as a conflict. Known limit: revision CAS on local
edits can be satisfied by a remote snapshot that reached the same revision
number; an editor must reload after a sync.
An entity with a deferred incoming change is skipped by the state scan: its
local state is transient until the deferred change settles, and journaling it
would override the origin's newer choice with a later local timestamp.

Conversations, part one (sync S8a). Launch snapshot artifacts are immutable
insert-only entities (`conversation.snapshot_artifact`, bytes base64-encoded;
an artifact beyond the payload limit is not synced). The conversation root
(`conversation.root`) carries what exists from creation on: the conversation
with participants and settings, the root branch, the memory binding (none,
own space, companion pool) and the initial scene/starter messages in their
creation form (first revision, visible, unpinned). Revisions, update times,
branch heads, the active branch and timeline ordinals stay device-local, so a
new message never rewrites the root. A new root creates the conversation with
its root branch, initial messages and origins, snapshot references, memory
binding and a create operation; an existing one takes title, lifecycle,
participants and settings. Messages, other branches and the active branch
follow in S8a-3.
