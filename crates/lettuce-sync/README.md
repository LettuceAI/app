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
payload. Identifiers and causal frontiers have explicit bounds.

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
missing sequence fails closed. Caller limits are capped at 256 changes and a
16 MiB payload budget. Peer acknowledgements only advance and are clamped to facts present in
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

Payload schema changes. The journal records the schema fingerprint it was
written under (`sync_journal_format`). Journaled payloads are immutable and
bound into fingerprints peers have acknowledged, so a build whose fingerprint
differs never relays them: when the database opens (and again before each
scan) it empties every journal table (changes, frontiers, peer
acknowledgements, staged batches, conflicts, deferred changes, conversation
marks, queued purges and re-journals), takes a new sync device identity while
its hybrid clock carries on, and the next scan journals the current state as
inserts stamped with each entity's own change time, exactly like a restored
database. Peers on the same build start over the same way and settle the
concurrent inserts by last writer wins. Domain rows, fork notices, secret
versions and received memory cursors are kept, so nothing a device holds is
lost; a delete that had not reached a peer yet is undone there instead.

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
journaled as deletes in reverse order. Edits and imports therefore replicate
with no per-mutation code; a restored database starts with a new device
identity and an empty journal, so it rejoins as a new device whose state meets
peers as concurrent inserts, never as deletes. A scanned insert or update
carries its content's latest change time rather than the session time: the latest `updated_at` its snapshot records, or for a
memory item its creation, last access or supersession, for a Soul the latest
fact validity, creation or supersession, and for a relationship its last
interaction. Last-writer-wins therefore keeps a peer's newer edit over an older
one that synced later, and over an older restored version; a snapshot without
any timestamp falls back to the session time. A stamp never falls behind a
change already journaled for the entity (it then takes the next counter after
it), so an edit always supersedes the state it was based on.
Because the scan runs before anything
is received, incoming changes always meet journaled local state. Deviations
from legacy's per-write capture: edits between sessions collapse into one
change. Scanned
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
A deleted character journals a delete (see Hard deletes below).

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
an artifact beyond the payload limit records a `not_synced` notice). The conversation root
(`conversation.root`) carries what exists from creation on: the conversation
with participants and settings, the root branch, the memory binding (none,
own space, companion pool) and the initial scene/starter messages in their
creation form (first revision, visible, unpinned). Revisions, update times,
branch heads, the active branch and timeline ordinals stay device-local, so a
new message never rewrites the root. A new root creates the conversation with
its root branch, initial messages and origins, snapshot references, memory
binding and a create operation; an existing one takes title, lifecycle,
participants and settings.

Messages (`conversation.message`, entity id `<conversation>:<message>`) carry
every revision and candidate, the terminal turns that produced the
candidates and those turns' settled usage events. Timeline ordinals, message
revisions and update times, provider replay artifacts (device-local provider
caches) and the retry link to a failed turn that produced nothing stay on the
device that wrote them; runtime records (dispatches, tools, checkpoints) are
not exchanged. Revision sequences and candidate ordinals are numbered by each
device (concurrent edits or regenerations would otherwise collide), so the
exchanged form orders both by creation time and a merge appends new ones after
the local maximum. A message with a running turn on it is skipped until it
settles, and one whose snapshot exceeds the payload limit records a
`not_synced` notice.
Merging unions revisions, candidates and turns, takes the render pointer,
author and flags from the incoming snapshot, never lifts a tombstone and sets
media references active or historical as the snapshot says. A new message
waits for its conversation, branch, parent, participants, media, prompt
documents and lorebooks, takes the next local timeline ordinal and becomes its branch head
only when the local head is its parent (or the branch is empty and it
follows the fork point); a concurrent message stays off the path until the
fork rule (S8b). Initial messages are only created by their root.

Forked branches (`conversation.branch`, entity id `<conversation>:<branch>`)
are insert-only: no mutation changes a branch after creation. The exchanged
form leaves the head, revision and update time local (the head follows the
messages that arrive on the branch), and a branch waits for its fork message.
Which branch a conversation shows stays device-local, so each device keeps
viewing the branch its user chose.

Concurrent replies (sync S8b). A synced message that does not extend its
branch head is checked against the local path: when a path message answers
the same parent, the lower message id keeps the path and the other chain is
copied, flattened to what each message shows (one revision, content-derived
id, no turns), into a fork branch named after the chain's first message
(UUIDv5), with copy ids derived from the fork and the original, so every
device builds the same fork. Originals stay where they were (off the path),
nothing is dropped, and a message that continues a moved chain is copied
after its parent's nearest copy. A merged original refreshes its copies
(content-derived revisions, flags, tombstones stay). Nothing moves while a
generation runs in the conversation (the change waits). A device records a
notice in the device-local `sync_conversation_forks` table only when one side
is its own message (`holds_local` says whether the fork holds its former
path); `ConversationForkRepository` lists and resolves
them. The user's choices (keep both, make mine main, make theirs main) only
decide which branch the device shows, through the normal branch selection,
before the notice is resolved. Two concurrent first messages of an empty
conversation have no fork point and stay as they are. Known limit: a third device
replying inside a chain that later loses (nested concurrency) can leave the
devices with different sub-forks; the originals are always kept.

Memory (sync S9a). Each device picks its own memory space ids, so memory is
exchanged under the space's owner: `conversation:<id>` for a conversation's
own space, `pool:<character>` for a companion character's shared pool. Every
item is its own entity (`memory.item`, `<owner>/<memory id>`, deletable): a
retrieval journals only the items it touched, concurrent additions on two
devices are both kept and deletions propagate. Short ids and ordinals are
numbered by each device (a new item takes a free ordinal and keeps its
derived short id unless it is taken); items are written one row at a time. A
full space makes an incoming new item wait until a deletion frees room. An
item id that already belongs to another space here is refused.
The summary is one entity per owner (`memory.summary`, space id blanked,
waits for its source messages). The dynamic-memory cursor is the summary
window, which counts path messages, so the summary's owner conversation
continues where the other device stopped. The other conversations of a
companion pool read their cursor from local runs, so each pool conversation's
cursor is exchanged too (`memory.cursor`, only ever raised by sync, kept in
`memory_synced_cursors`, last writer wins; only a device's own run cursor
is exchanged, so a received cursor is never echoed; the local cursor is the
larger of the local run cursor and the received one, and a local rewind clears
the received one). Runs, attempts,
retrieval accesses and ask-first approvals stay device-local, and embedding
projections are rebuilt by each device (retrieval embeds every memory
without a current vector first, like legacy, skipping superseded ones and
continuing past failures).

Companions (sync S9b). A character's Soul (`companion.soul`, the facts;
revision local), the relationship a character keeps with one persona
(`companion.relationship`, `<character>:<persona key>`), a companion
conversation's session state (`companion.session`: emotional state, active
signals, initial hash and the continuity episode's start and end) and
scheduled notes (`companion.scheduled_note`, deletable) are exchanged as
whole snapshots, last writer wins. A session waits for its conversation and
relationship. An episode's index and predecessor are numbered by each device
(two devices can open sessions at once), so they are not exchanged; the
predecessor is the latest episode that started before it, and a new local
launch closes the episode that started last. A soul whose facts are all
authored and none superseded and a relationship with no interactions are
seeds (what launching or receiving a character creates on its own), so a
grown soul or a used relationship replaces them. Apply
receipts, turn effects, growth/consolidation/writer runs stay on the device
that ran them.

Message scans read only conversations whose messages changed since the last
scan: triggers on every table a message snapshot is built from (messages,
revisions, candidates, initial origins, turns, attempts, media references,
usage events) count writes per conversation in the device-local
`sync_conversation_marks`, whatever path made them, and a conversation is
marked scanned only when none of its messages had to be skipped (deferred or
mid-generation). A root reads only its initial messages. A restored database
has no marks and is scanned whole once.

The scan skips an entity whose id is not a valid sync entity id or whose
snapshot cannot be decoded, so one bad row never stops journaling for
everything else; one beyond the payload limit also records a `not_synced`
notice.

Plain rows (sync S10). Audio providers, user voices and ASR learning data
(vocabulary terms, corrections, ignored suggestions, voice examples) are
exchanged row by row through one generic row codec (`row_sync_adapter.rs`):
the payload is the row's columns keyed by name, without the local revision;
an update bumps the local revision and never moves `updated_at` back; deletes
propagate. A row waits for a required parent (a voice's provider, an
example's audio asset) and clears an optional reference that is gone. Two
devices that ignored the same suggestion under different ids keep the lower
id (the table's natural identity is unique). Tables with a revision step
trigger never move `updated_at` backwards; the others take the incoming value
exactly. Immutable rows are insert-only and have no delete; a different
existing immutable row is kept and the incoming one refused. API keys stay in each device's
secret store: the secret reference travels, the key does not. Media used by
messages and voice examples is journaled like other referenced media (the
referenced-media scan missed message media before, so messages with images
never became ready to journal).

Local model files (sync S13, user decision 2026-09-21). For profiles of
llama.cpp and stable-diffusion.cpp accounts the files the runtime loads (the
model file in `external_model_id`, projector, MTP draft model, text encoder,
VAE, vision encoder, base LoRAs) and the installed runtime build
(release/asset/backend) are device-local: the exchanged form replaces the
model file with `UNPICKED_LOCAL_MODEL_FILE` and drops the others, a merge into
an existing profile keeps this device's files, and a profile first received
from another device shows its model file still to pick. Everything else in
the profile (name, parameters, GPU settings, capabilities) syncs.

Secrets (sync S12, protocol version 3). API keys and secret headers of
provider accounts and audio providers never enter the change journal (it is
SQLite). After the change exchange a session runs a secret phase over the
PAKE-authenticated, ChaCha20-Poly1305-encrypted channel: each side lists the
secrets its synced records reference with the version its value was last set
at (`sync_secret_versions`, device-local: reference, secret-store generation,
set-at time and device; a changed local generation gets a new version at the
next session, stamped with the time the native store wrote the value), fetches the values it lacks or holds an older version of and
writes them into its native secret store under the generation it saw when
the phase began (a key changed meanwhile is kept). A value changed locally
gets a version later than any it had; a peer version more than a day ahead
of the local clock is refused. A value this device lost is fetched again
rather than deleted elsewhere (no tombstones: a wiped keychain must not wipe
the other devices), and a value nothing here references any more (its
provider was deleted) is removed from the store; a provider row that still
mentions the reference in any column keeps it, even when its secret fields
cannot be decoded. Inventories list every referenced secret, without a count
limit. Values travel only inside
encrypted frames, are zeroized after use and never printed; a side serves
only secrets it listed, only during the phase, and an unrequested value ends
the session. A secret the store cannot read or write is skipped (before its
value is fetched) and retried next session. Versions are not carried across
a restore.

Usage (sync S11). Cost bases of conversation usage events (`usage.cost_basis`,
waits for its event), job inference usage evidence and its cost bases
(memory, creation, companion and generation jobs) and imported legacy usage
records (key `<run>:<source>`) go through the same row codec; the immutable
tables are insert-only (an existing row is kept). Jobs themselves stay local,
the evidence keeps its job id as a plain value.

Hard deletes (protocol version 4). A purged conversation (direct or group) or
character is absent at the next scan, which journals a delete of its
`conversation.root` or `character.snapshot` entity after the deletes of its
memory items, summaries and scheduled notes (reverse scan order); messages,
branches, Souls, relationships and sessions journal nothing and go with their
owner on the peer. A received delete is decided against everything the
device holds of the entity and what it owns: a journaled change of an owned
entity the delete's causal frontier does not include, the current content of
every owned entity against its latest journaled content (so edits made after
the session's scan, even during the exchange, count), and a generation still
running. Untouched seeds such as a received companion's first Soul do not
count. When the device holds such changes, the entity stays and everything it
owns (launch snapshots, referenced media, the root, branches, messages in
timeline order, memory, companion state, bindings, notes) is journaled again
as fresh inserts that observe the delete, so the deleting device receives it
back whole whichever device it syncs with next; a peer that still holds the
same content takes them as no-ops. A `kept_unsent_local_changes` notice is
recorded when some of those changes are the device's own. While a snapshot or
referenced asset is missing, a media blob is not ready or content cannot be
encoded, nothing is sent: the entity waits in `purge_rejournals` (retried
before every scan, which journals nothing it owns meanwhile) with a
`rejournal_incomplete` notice. After five attempts it is sent without what is
still missing, and a `rejournal_dropped` notice names each asset or entity
left out. Otherwise the delete wins over a concurrent edit like any snapshot delete and
is queued in `purge_queue`; the queue runs right after the batch commits and
after every scan, deciding each delete again first (the user may have written
while it waited), then performing the same purge as a local delete. A queued
purge that is busy (a memory run or companion effect in flight) stays queued
while the scan skips its entity; one that fails eight times for another
reason is dropped with a `dropped_after_failures` notice and the next scan
journals the entity again, so every device keeps it. A character delete also
takes the character out of every group on the receiving device, as it does
locally, and both devices reach identical group content. Payloads are
unchanged (the schema fingerprint stays); the protocol version marks peers
that accept these deletes. The media a purge leaves unused are collected on
each device separately.

Large entities (protocol version 5). Aggregates travel as whole snapshots, so
the canonical payload limit bounds one entity rather than one row: 256 MiB,
which holds the largest lorebook (64 MiB authored) and the largest protected
launch artifact (128 MiB, base64 encoded). Batches keep a 16 MiB payload
budget, and a change larger than the budget travels in a batch of its own
(frames carry up to the payload limit plus 16 MiB). An entity that still
cannot be encoded is never skipped silently: the scan records a
`not_synced` notice for it (once until dismissed) and journals everything
else, and a received change for it is deferred with the same notice instead
of failing the batch; both resolve on their own once it fits again.
