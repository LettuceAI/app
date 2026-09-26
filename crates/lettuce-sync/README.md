# lettuce-sync

Device-to-device sync. Two paired LettuceAI installs exchange domain changes directly, with no server in between. This crate defines what a change is, how devices order and acknowledge changes, how a session is negotiated, the payload schema of every synced entity, and the repository traits the rest of the system implements.

The crate has no network access, no SQLite and no stored list of trusted peers:

- `lettuce-app` (`src/sync/`) owns pairing, encryption, the socket, the binary frame codec, cancellation and the session coordinators.
- `lettuce-database` (`src/sync/` and the catalog adapters) owns the journal tables, the state scan and materializing incoming changes into domain rows.

Because the rules are split across those crates but defined by this one's contracts, this README describes the whole sync design.

Replication never copies raw SQLite state. Every change carries a complete, versioned domain snapshot of one entity, so peers never depend on each other's table layout.

## Changes

A `CanonicalChange` is one immutable fact about one entity:

- `id`, `origin_device` and `origin_sequence`: every device numbers its own changes 1, 2, 3, … with no gaps.
- `timestamp`: a `HybridTimestamp` (wall time and counter) that orders changes across devices.
- `base_frontier`: the `CausalFrontier` the origin had seen when it wrote the change, a map from device to the highest sequence seen (at most 1024 devices). `change.observes(other)` is true when the change was written with `other` already known.
- `entity`: a `SyncEntity`, a kind such as `character` and an id.
- `operation` with its contract: `Insert` carries a payload and no base revision, `Update` carries both, `Delete` carries a base revision and no payload. The base revision is the content hash of the snapshot the change was written against.
- `payload`: a `CanonicalPayload`, which is a schema name (for example `character.snapshot`), a schema version, the JSON bytes and their BLAKE3 content hash. One payload holds at most 256 MiB.
- `fingerprint`: a hash over all of the above. Peers acknowledge fingerprints, so a stored change can never be edited afterwards.

Snapshot structs reject unknown fields, so any field change needs a new schema version, and JSON is encoded with `float_roundtrip` so a materialized snapshot re-encodes to the same bytes. The kind, schema and version constants for every entity, and the payload builders for the explicitly journaled ones, live in `models.rs`, `persona.rs` and `media.rs`. `current_sync_schema_fingerprint` hashes the full list of schemas and versions a build supports.

## The journal

`LocalChangeJournal` is the device's append-only log of canonical changes, its own and the ones it received.

`record_local_change` admits a change under a stable `OperationId`. Retrying the same operation returns the same change; reusing the operation id for different input is a conflict. The SQLite adapter allocates the device identity, the next origin sequence and the hybrid clock in the same transaction as the journal row.

`outbound_changes(peer_frontier, …)` returns what a peer has not seen. It covers every origin in the local frontier, not only this device, so a device relays changes it received from a third one. For each origin it starts after the peer's sequence and keeps that origin's order; origins are merged by readiness, then hybrid timestamp, then device id. Every causal dependency of a returned change must already be satisfied, and a missing sequence fails closed. A batch holds at most 256 changes and 16 MiB of payload; a single larger change goes alone.

`record_peer_acknowledgement` stores how far a peer has confirmed. It only moves forward and is clamped to what exists in the local frontier.

### Producing changes: the state scan

Most entities are not journaled by the code that edits them. At the start of each session, `journal_current_state` compares every synced entity's current snapshot with the latest journaled snapshot that became local state (incoming changes that lost a conflict do not count) and journals the difference as an insert, update or delete. Edits, imports and restores therefore replicate with no per-mutation code, and several edits between two sessions collapse into one change. Because the scan runs before anything is received, incoming changes always meet journaled local state.

Kinds are scanned in dependency order (`SCANNED_CODECS` in `lettuce-database/src/sync/sync_adapter.rs`): provider accounts, model profiles, personas, the persona default, prompts (a derived prompt after its source), app settings, characters, groups, lorebooks, character, persona and group bindings, conversation roots, branches, messages, memory items, summaries and cursors, companion Souls, notes, relationships and sessions, then the plain-row kinds. Deletes are journaled in reverse order. Media assets and launch snapshot artifacts are journaled ahead of the aggregates that reference them.

A scanned change is stamped with the content's own last change time, not the session time: the latest `updated_at` its snapshot records; for a memory item its creation, last access or supersession; for a Soul the latest fact validity, creation or supersession; for a relationship its last interaction. A snapshot with no timestamp falls back to the session time. A stamp never falls behind a change already journaled for the entity (it takes the next counter instead), so an edit always supersedes the state it was based on.

Deletes get the time this device deleted the entity. Delete triggers on the synced tables, messages, branches and memory items included, write it to `sync_deleted_entities`, and a purge records its own time inside its transaction first. Each scan clears that table except for entities it skipped (a deferred incoming change or a pending re-journal), which a later scan still needs; a journal restart clears it after carrying its deletes. Summaries and bindings, and deletions no scan has journaled yet, fall back to the session time.

The scan skips an entity when:

- it has a deferred incoming change: its local state is transient until that change settles, and journaling it would override the origin's newer choice with a later local timestamp;
- its id is not a valid sync entity id or its snapshot cannot be decoded, so one bad row never stops the rest;
- its snapshot exceeds the payload limit, which also records a `not_synced` notice (once, until dismissed); it resolves on its own once the entity fits;
- it references media whose assets are not journaled yet because their blobs are not ready, so a receiver never waits for media that cannot arrive;
- it is a message with a generation still running on it.

Message scans only read conversations whose messages changed. Triggers on every table a message snapshot is built from (messages, revisions, candidates, initial origins, turns, attempts, media references, usage events) count writes per conversation in `sync_conversation_marks`, whatever code path made them. A conversation is marked scanned only when none of its messages had to be skipped. A conversation root reads only its initial messages. A restored database has no marks and is scanned whole once.

A restored database starts with a new device identity and an empty journal. It rejoins as a new device whose state meets peers as concurrent inserts, never as deletes.

### Explicitly journaled personas

Personas and the default persona are journaled by their mutations as well, in the same transaction as the aggregate change. The persona snapshot holds the authored fields, lifecycle, revision, timestamps and ordered media references. Create, revise, media attach, detach, replace and reorder, archive and restore each have their own deterministic operation id (media operations bind the normalized mutation and its intent), and each writes the complete resulting snapshot, with the canonical pre-mutation snapshot hash as the update base. Default selection is a revisioned singleton with its own set and clear operations. Archiving the selected default also writes a default-clear update in the same transaction, so both revisions and both journal rows commit together or not at all. Each referenced media asset is journaled at most once, right before the persona snapshot. The scan still covers personas and picks up what mutations do not journal, such as the owner revision bump of a binding change or a seed default that was never journaled (it syncs as an insert).

Local persona operation ids are scoped to the latest remote change that won locally for the entity. A remote winner can move a persona or default revision backwards, and ids derived from the revision alone would then repeat and turn the next local edit into a stale replay. Entities never touched by sync keep unscoped ids, and a losing remote change leaves the scope alone, so an exact retry still replays.

## A session, step by step

1. Pairing and transport (`lettuce-app`). Devices pair for one session with a PIN; there is no persistent peer trust. The channel is PAKE-authenticated and ChaCha20-Poly1305-encrypted. Frames use a bounded binary codec, and decoding alone grants nothing: every hello, frontier, change and batch is rebuilt through this crate's validating constructors before a coordinator sees it.

2. Hello. Each side sends a `SyncHello`: app version, `SYNC_PROTOCOL_VERSION`, schema fingerprint, durable device id, bounded device name, a fresh session id and its transfer limits. `negotiate_sync_session` requires that the device id matches the authenticated transport identity, that device and session ids differ between the peers, and that app version, protocol version and schema fingerprint are equal. Both sides then use the smaller of each limit.

3. Scan. Each side runs `journal_current_state`.

4. Frontiers. The sides exchange their causal frontiers.

5. Change loop. In each round both sides send a `SyncChangeFrame`: either a `Batch` (`SyncChangeBatch`, with an id and a hash over the ordered change fingerprints, checked against the negotiated count and byte limits), or `Quiescent` with the sender's frontier. The receiver stages the batch, applies it and answers with a `SyncBatchAcknowledgement` carrying the batch id it received and its new frontier. The acknowledgement must name the batch that was sent and must move the peer frontier past it. The loop ends when both sides are quiescent in the same round. If a batch stays pending (below), the exchange stops there and the peer resends it next session.

6. Media phase. Each side fetches the blobs its pending incoming changes wait for (see Media).

7. Secret phase. Each side fetches the API keys it lacks (see Secrets).

`SyncExchangeCoordinator`, `SyncMediaCoordinator` and `SyncSecretCoordinator` in `lettuce-app` run steps 2 to 7 over an authenticated transport.

## Applying incoming changes

`IncomingChangeRepository::stage_incoming_batch` stores the batch's metadata and payloads durably, checked against the declared hash. `apply_incoming_batch` then applies it in one transaction: the aggregate rows, the journal rows and the frontier move together, remote hybrid clocks are observed, and a committed batch replays as committed after a restart. A batch stays `Pending` only for a causal gap or an unknown schema; a later session resends the same changes in a new batch. A committed batch drops its staged copies (the changes live on in the journal), so a received payload is stored once; staged changes of batches that never committed stay for the retry.

Each snapshot change is settled against the entity's current local snapshot:

- same content: nothing to do;
- a clean insert, or an update whose base is the current snapshot: apply;
- the entity is absent here (for example it was created on its origin before that device journaled anything): adopt the snapshot;
- otherwise the two edits were concurrent. The winner is the change with the greater `(timestamp, origin device, origin sequence, change id)`, and both snapshots are kept as conflict evidence with the winning side recorded.

An untouched seed snapshot (settings or the default persona still at revision 1, as on a freshly installed device, a received companion's first Soul, a relationship with no interactions) always loses to an edited one, and an edited one always beats it. An emptied binding list with a journaled history counts as present, so a concurrent edit is still detected as a conflict.

### Why last writer wins by edit time

The comparison uses the hybrid timestamp, and scanned changes carry the time the content was last changed rather than the time it was synced. A newer edit on one device therefore beats an older edit that happens to reach the peer later, and an older version brought back by a restore does not override newer work. Ties on time fall to device, sequence and id, so every device picks the same winner without talking to the others.

### Deletes

A delete wins over a concurrent update on both sides. The discarded update is kept as losing conflict evidence, so a later update in the same chain cannot bring the entity back. A delete this device has to refuse because a foreign key here still references the entity (checked in a savepoint) is only journaled; the next scan journals a fresh insert in dependency order, so every device keeps the entity.

### Deferral

A change that waits for another entity that is not here yet (its owner, lorebook, derivation source or media asset) does not hold up its batch. It is settled inside a savepoint, rolled back and recorded in `sync_deferred_changes`, a per-entity queue in journal order where a later change waits behind an earlier one. The batch commits, the origin's frontier advances and its later changes still apply. Deferred changes are settled again after every batch until no more progress is made, and after each media phase. They do not count as local state for the scan or for operation scoping. A deferred change that later fails for another reason stays deferred instead of failing every later batch. A received change for an entity that cannot be encoded here is deferred with a `not_synced` notice instead of failing the batch.

References follow two rules. A reference to a model or prompt missing here is cleared only when nothing for it is deferred; if something is deferred, the referencing aggregate waits. A group waits for its persona and member characters.

Revision CAS on local edits can be satisfied by a remote snapshot that happened to reach the same revision number, so an editor must reload after a sync.

## Conflicts

Every concurrent edit leaves a conflict record with both snapshots and the winner. Evidence stays until resolution.

Personas and the default persona have a resolution surface. `PersonaConflictRepository` lists unresolved conflicts, newest first, with typed current and other candidates, and resolves one by choosing a side. Resolution always writes a new local snapshot with a fresh revision, even when the current side is kept, so the decision observes and propagates past both concurrent changes. The original evidence stays; an exact decision replays after a restart, a changed or stale one fails, and a later observed resolution marks the matching remote conflict superseded.

A default-persona change whose persona is archived or missing here is journaled with the local default kept and an unresolved conflict recorded (current side winning). Choosing the other side is refused until the persona is restored, and any later local default change supersedes the conflict. A conflict whose current side predates the journal (the seeded default) is superseded by a later change that observes the incoming side.

Other kinds keep their conflict evidence without a resolution surface.

### Concurrent replies

Two devices answering the same message is common, and neither reply should be lost. A synced message that does not extend its branch head is checked against the local path. When a path message answers the same parent, the lower message id keeps the path and the other chain is copied into a fork branch:

- the copy is flattened to what each message shows (one revision, content-derived id, no turns);
- the fork branch is named after the chain's first message (UUIDv5), and copy ids derive from the fork and the original, so every device builds the same fork;
- originals stay where they were, off the path, and nothing is dropped;
- a message that continues a moved chain is copied after its parent's nearest copy, and a merged original refreshes its copies (content-derived revisions, flags, tombstones stay);
- nothing moves while a generation runs in the conversation; the change waits.

A device records a notice in the device-local `sync_conversation_forks` table only when one side is its own message (`holds_local` says whether the fork holds its former path). `ConversationForkRepository` lists and resolves them. The user's choices (keep both, make mine main, make theirs main) only decide which branch this device shows, through normal branch selection.

Two concurrent first messages of an empty conversation have no message to fork from, since a branch always starts at a message. The lower first message keeps the conversation, and the other chain is copied the same way into a conversation of its own, derived from the source conversation and the chain's first message (same participants, settings and memory binding, no initial messages, titled as a branch), with the same notice pointing at it. Later messages on that chain follow their copies there.

A third device replying inside a chain that later loses can leave devices with different sub-forks; the originals are always kept.

## What syncs

| Kind | Schema | Notes |
| --- | --- | --- |
| provider account | `provider_account.snapshot` | secret references only |
| model profile | `model_profile.snapshot` | local model files stay device-local |
| persona, default | `persona.snapshot`, `persona.default` | also journaled explicitly |
| media asset | `media.asset` | insert-only |
| character | `character.snapshot` | root, scenes, variants, starters |
| lorebook | `lorebook.snapshot` | book and entries |
| lorebook bindings | `lorebook.bindings` | one entity per character, persona or group |
| prompt | `prompt.snapshot` | built-ins included |
| group | `group.snapshot` | members, presentation, starting scene |
| app settings | `app_settings.snapshot` | singleton |
| launch snapshot | `conversation.snapshot_artifact` | insert-only |
| conversation | `conversation.root` | |
| message | `conversation.message` | |
| branch | `conversation.branch` | insert-only |
| memory | `memory.item`, `memory.summary`, `memory.cursor` | |
| companion | `companion.soul`, `companion.relationship`, `companion.session`, `companion.scheduled_note` | |
| usage | `usage.cost_basis`, `usage.job_inference`, `usage.job_cost_basis`, `usage.legacy_record` | row codec |
| audio and ASR | `audio.provider`, `audio.user_voice`, `asr.*` | row codec |
| playground | `playground.history`, `playground.history_image` | row codec |
| Creation Helper | `creation.workflow` | |

### Accounts and models

Provider accounts sync as the complete row; secret references travel as opaque ids and secret values never leave the device. For llama.cpp and stable-diffusion.cpp profiles, the files the runtime loads (model file in `external_model_id`, projector, MTP draft model, text encoder, VAE, vision encoder, base LoRAs) and the installed runtime build (release, asset, backend) are device-local. The exchanged form replaces the model file with `UNPICKED_LOCAL_MODEL_FILE` and drops the rest; a merge into an existing profile keeps this device's files, and a profile first received from another device shows its model file as still to pick. Everything else in the profile (name, parameters, GPU settings, capabilities) syncs. A model whose account was deleted here is journaled but not materialized; the next scan journals its delete, so both devices converge. Applying a remote model delete clears app defaults pointing at it, as a local delete does.

### Characters, lorebooks, prompts, groups, settings

A character is the complete `CharacterDetails`. The materializer updates the root row in place, because conversations, Souls and creation runs depend on it, and replaces scene, variant, starter, media and presentation rows. A new companion character gets its initial Soul exactly as a local create would. Missing media keep the change deferred until the media phase (asset inserts precede the character in origin order). A default model deleted on this device is cleared instead of blocking the origin's later changes, and the next scan journals the cleared default.

Lorebooks sync as `LorebookDetails` (book row updated in place, entries replaced). Each owner's ordered binding list is one entity, and removing an owner's last binding journals a delete, which clears that owner's bindings on the peer. An emptied list journals an update to `[]` instead of a delete, so a concurrent bind is not lost to a delete.

Every prompt document syncs, built-ins included, because built-ins are editable in place. Built-in document ids are UUIDv5 of the built-in key and entry ids derive from the document id and entry key, so every device seeds the same identities, references to built-ins resolve across devices, and two independent seeds meet as one entity settled by last writer wins. Entry revisions and timestamps are local bookkeeping outside the snapshot. A prompt whose built-in key is held by a different local id is journaled but not materialized; this happens on databases seeded before built-in ids were deterministic.

Groups sync as `GroupDetails` (root updated in place; members, presentation references and starting scene replaced). A member model override deleted on this device is cleared like a character default.

App settings are one singleton: preferences, default model and prompt selections and the app-wide model settings layer. A selected model or prompt missing on this device is cleared. Since a scan journals upserts before deletes, a peer receives the cleared defaults before the model delete that caused them, and its own clearing is a no-op.

### Conversations

Launch snapshot artifacts are insert-only, with bytes base64-encoded. An artifact beyond the payload limit records a `not_synced` notice.

The conversation root holds what exists from creation on: the conversation with participants and settings, the root branch, the memory binding (none, own space, companion pool) and the initial scene and starter messages in their creation form (first revision, visible, unpinned). Revisions, update times, branch heads, the active branch and timeline ordinals stay device-local, so a new message never rewrites the root. A new root creates the conversation with its root branch, initial messages and origins, snapshot references, memory binding and a create operation; an existing one takes title, lifecycle, participants and settings.

A message (entity id `<conversation>:<message>`) carries every revision and candidate, the terminal turns that produced the candidates and those turns' settled usage events. Timeline ordinals, message revisions and update times, provider replay artifacts (device-local provider caches) and the retry link to a failed turn that produced nothing stay on the device that wrote them; runtime records (dispatches, tools, checkpoints) are not exchanged. Revision sequences and candidate ordinals are numbered per device, since concurrent edits or regenerations would otherwise collide, so the exchanged form orders both by creation time and a merge appends new ones after the local maximum. Merging unions revisions, candidates and turns, takes the render pointer, author and flags from the incoming snapshot, never lifts a tombstone, and marks media references active or historical as the snapshot says. A new message waits for its conversation, branch, parent, participants, media, prompt documents and lorebooks, takes the next local timeline ordinal, and becomes its branch head only when the local head is its parent (or the branch is empty and it follows the fork point). A concurrent message stays off the path and goes through the fork rule above. Initial messages are only created by their root.

Forked branches (`<conversation>:<branch>`) are insert-only, since nothing changes a branch after creation. Head, revision and update time stay local (the head follows the messages that arrive on it), and a branch waits for its fork message. Which branch a conversation shows is device-local.

### Memory

Each device picks its own memory space ids, so memory is exchanged under the space's owner: `conversation:<id>` for a conversation's own space, `pool:<character>` for a companion character's shared pool. Every item is its own deletable entity (`<owner>/<memory id>`), so a retrieval journals only the items it touched, concurrent additions on two devices are both kept and deletions propagate. Short ids and ordinals are numbered per device: a new item takes a free ordinal and keeps its derived short id unless it is taken. Items are written one row at a time. A full space makes an incoming new item wait until a deletion frees room. An item id that already belongs to another space here is refused.

The summary is one entity per owner (space id blanked) and waits for its source messages. The dynamic-memory cursor is the summary window, which counts path messages, so the owner conversation continues where the other device stopped. The other conversations of a companion pool read their cursor from local runs, so each pool conversation's cursor is exchanged too (`memory.cursor`, kept in `memory_synced_cursors`, only raised by sync, last writer wins). Only a device's own run cursor is sent, so a received cursor is never echoed. The local cursor is the larger of the local run cursor and the received one, and a local rewind clears the received one.

Runs, attempts, retrieval accesses and ask-first approvals stay device-local. Embedding projections are rebuilt on each device: retrieval embeds every memory without a current vector first, skipping superseded ones and continuing past failures.

### Companions

A character's Soul (its facts; revision local), the relationship a character keeps with one persona (`<character>:<persona key>`), a companion conversation's session state (emotional state, active signals, initial hash, the continuity episode's start and end) and scheduled notes (deletable) are whole snapshots, last writer wins. A session waits for its conversation and relationship. An episode's index and predecessor are numbered per device, as two devices can open sessions at once, so they are not exchanged: the predecessor is the latest episode that started before it, and a new local launch closes the episode that started last. A Soul whose facts are all authored and none superseded, and a relationship with no interactions, are seeds, so a grown Soul or a used relationship replaces them. Apply receipts, turn effects and growth, consolidation and writer runs stay on the device that ran them.

### Plain rows

Audio providers, user voices, ASR learning data (vocabulary terms, corrections, ignored suggestions, voice examples), usage cost bases, job inference usage evidence and its cost bases (memory, creation, companion and generation jobs), imported usage records (key `<run>:<source>`) and the playground history go through one generic row codec (`lettuce-database/src/sync/row_sync_adapter.rs`). The payload is the row's columns keyed by name, without the local revision. An update bumps the local revision; tables with a revision-step trigger never move `updated_at` backwards, the others take the incoming value exactly. Deletes propagate. A row waits for a required parent (a voice's provider, an example's audio asset, a cost basis's event) and clears an optional reference that is gone. Immutable tables are insert-only with no delete: an existing row is kept and a different incoming one refused. Two devices that stored the same natural row under different ids (the same ignored suggestion, the same imported playground entry) keep the lower id. Jobs themselves stay local; usage evidence keeps its job id as a plain value.

A playground entry is journaled once its generation has settled; a pending one stays on the device running it. Its images (`<entry>:<ordinal>`) are insert-only and wait for their entry and media.

A Creation Helper session is one snapshot of the workflow with its proposal chain and user turns (the workflow revision stays local). Turns and proposals are append-only, so a merge adds the missing ones in turn order and moves the workflow to the synced current proposal. A session already applied on this device never changes, and one where both devices added a different turn at the same position is kept as it is here, with the other version as conflict evidence. Inference attempts, rounds and apply receipts stay local.

### What stays local

Image generation, speech synthesis and transcription records belong to a job row of the device that ran them and are not exchanged; generated images reach other devices through the playground history and messages. Pricing caches, retrieval accesses, memory and companion runs, conversation runtime records and which branch is shown are also device-local.

## Media

Media bytes never enter a change. An asset is journaled once as an insert-only `media.asset` change with its logical metadata and ready content descriptor, and its blob is identified by BLAKE3 content hash; native paths never travel. The scan journals every ready asset referenced by personas, characters, their presentation and scene assets, lorebook icons, groups (presentation, background, scene), messages, voice examples and playground images, with the same deterministic operation as persona journaling. Media nothing synced references is not journaled. Later local metadata such as retention stays device-local, and an incoming asset insert is checked only against its identity and blob fields.

An incoming `media.asset` change whose blob is not here stays deferred, and so does every change that references the asset, so a persona or character never materializes with a dangling image. The payload already carries the full asset and blob metadata, so no catalog is exchanged. In the media phase each side takes up to `MAX_SYNC_MEDIA_ASSETS` (256) deferred asset changes (`MediaSyncRepository::pending_media`) and asks the peer for their blobs by content hash, in chunks of at most 1 MiB, while it keeps serving the peer's requests until both sides send `MediaDone`. A blob the peer cannot serve is answered with `BlobUnavailable` and skipped instead of ending the session. Deferred changes are then settled again. Media a purge leaves unused is collected on each device separately.

## Secrets

API keys and secret headers of provider accounts and audio providers never enter the journal, which is plain SQLite. After the change exchange, each side sends an inventory of every secret its synced records reference, with the version its value was last set at (`SyncSecretEntry`, no count limit). The version comes from the device-local `sync_secret_versions` table: reference, secret-store generation, set-at time and device. A changed local generation gets a new version at the next session, stamped with the time the native store wrote the value, and later than any version the device had.

Each side fetches the values it lacks or holds an older version of and writes them into its native secret store under the generation it saw when the phase began, so a key changed locally in the meantime is kept. A peer version more than a day ahead of the local clock is refused, so a broken clock cannot pin a key. There are no tombstones: a value this device lost is fetched again rather than deleted elsewhere, because a wiped keychain must not wipe the other devices. A value nothing references any more (its provider was deleted) is removed from the store; a provider row that still mentions the reference in any column keeps it, even when its secret fields cannot be decoded. A secret the store cannot read or write is skipped before its value is fetched and retried next session.

Values travel only inside encrypted frames, are zeroized after use and never printed. A side serves only the secrets it listed and only during the phase, and an unrequested value ends the session. Versions are not carried across a restore.

## Hard deletes

A purged conversation (direct or group) or character is absent at the next scan, which journals a delete of its `conversation.root` or `character.snapshot` entity after the deletes of its memory items, summaries and scheduled notes. Messages, branches, Souls, relationships and sessions journal nothing and go with their owner on the peer.

A received delete is decided against everything the device holds of the entity and what it owns: a journaled change of an owned entity that the delete's causal frontier does not include; the current content of every owned entity against its latest journaled content, so edits made after the scan, even during the exchange, count; and a generation still running. Untouched seeds do not count.

- If the device holds such changes, the entity stays, and everything it owns (launch snapshots, referenced media, the root, branches, messages in timeline order, memory, companion state, bindings, notes) is journaled again as fresh inserts that observe the delete. The deleting device gets it back whole from whichever device it syncs with next; a peer that still holds the same content takes them as no-ops. A `kept_unsent_local_changes` notice is recorded when some of those changes are this device's own. While a snapshot or referenced asset is missing, a blob is not ready or content cannot be encoded, nothing is sent: the entity waits in `purge_rejournals` (retried before every scan, which journals nothing it owns meanwhile) with a `rejournal_incomplete` notice. After five attempts it is sent without what is still missing, and a `rejournal_dropped` notice names each asset or entity left out.
- Otherwise the delete wins like any snapshot delete and is queued in `purge_queue`. The queue runs right after the batch commits and after every scan, decides each delete again first (the user may have written while it waited), then performs the same purge as a local delete. A purge that is busy (a memory run or companion effect in flight) stays queued while the scan skips the entity. One that fails eight times for another reason is dropped with a `dropped_after_failures` notice, and the next scan journals the entity again so every device keeps it.

A character delete also removes the character from every group on the receiving device, as it does locally, so both devices end with the same group content.

## Schema changes restart the journal

The journal records the schema fingerprint it was written under (`sync_journal_format`). Journaled payloads are immutable and bound into fingerprints peers have acknowledged, so a build with a different fingerprint never relays them. When the database opens, and again before each scan, a mismatch empties the journal tables (changes, frontiers, peer acknowledgements, staged batches, conflicts, deferred changes and conversation marks) and takes a new sync device identity; the hybrid clock carries on. The next scan journals the current state as inserts stamped with each entity's own change time, like a restored database, and peers on the same build start over the same way and settle the concurrent inserts by last writer wins. Domain rows, fork notices, secret versions and received memory cursors are kept.

A restart never undoes a delete. Before the tables are emptied, every delete that is the latest state of its entity here (the entity is gone, or its received delete still waits in the purge queue) is journaled again under the new identity with its original stamp and no causal dependencies. That empty frontier is what marks a carried delete, and the journal refuses any other delete without causal dependencies (a scanned delete always has one: the change it deletes), so the marker cannot be forged locally. A delete this device refused, even one whose entity still waits in `purge_rejournals`, is never carried. A received carried delete observes nothing, so it is decided only against changes stamped after it: content a peer journaled again at its own restart is older and loses, deterministically, while an edit made after the delete still keeps the entity. Queued purges and pending re-journals are pending work and stay (a queued purge points at its carried delete).

Conflicts still waiting for the user move to `sync_carried_conflicts` with both sides and a `conflict_carried` notice. One whose side is an untouched seed, or whose sides are equal, loses nothing and is not carried.

## Size and growth

Entities travel as whole snapshots, so the payload limit bounds one entity, not one row. At 256 MiB it holds the largest lorebook (64 MiB authored) and the largest protected launch artifact (128 MiB, base64-encoded). Batches keep a 16 MiB payload budget, a larger change travels in a batch of its own, and frames carry up to the payload limit plus 16 MiB. The limit is also a memory cost: encoding, hashing, framing and encrypting one change at the limit holds several copies at once (snapshot, JSON, frame, ciphertext), about a gigabyte in the worst case on either side. Real entities stay far below it.

The change journal is never compacted. A peer receives every origin's changes in contiguous sequence (a gap is a causal dependency and keeps the batch pending), and devices pair per session with no list of known peers, so a device that has never synced may need the whole history from sequence one. Dropping even changes every known peer has acknowledged would strand the next device to pair. A schema change restarts the journal, which leaves one snapshot per entity.
