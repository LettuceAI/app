# lettuce-database

The SQLite storage of the app: the schema and its migrations, and one adapter per repository port that the domain crates define. It is the only crate that depends on SQLite.

Domain crates own their rules and describe persistence as synchronous repository traits (`ConversationRepository`, `MemoryRepository`, `JobStore`, `LocalChangeJournal` and the rest, about ninety in all). This crate implements all of them on a single `Database` value and keeps the storage representation private: callers get validated domain values, never a connection, a row type or SQL. It does not decide policy. It does, however, enforce the invariants that must hold on disk regardless of which code path writes, with CAS checks in the adapters and triggers in the schema.

- `lettuce-app` opens the `Database`, passes it to services as the repository for each port, and runs every call on its database worker thread, not on the UI or async runtime threads.
- `lettuce-transfer` defines the backup and legacy-import documents that the backup, restore and legacy adapters read and write.
- `lettuce-sync` defines the journal and snapshot contracts the sync adapters implement; its README describes the whole sync design, including the parts that live here.

## Structure

| Path | What it holds |
| --- | --- |
| `lib.rs` | `Database`, migrations, settings, provider accounts, model profiles, media blobs and assets |
| `conversation/` | Creator, mutation kernel and mutations, queries, artifacts, dispatch checkpoints, tools, companion state, chat import, historical writer |
| `memory/` | Memory spaces, summaries, embeddings, dynamic-memory runs and rewinds, consolidation runs, scheduled notes |
| `companion/` | Soul state, growth runs, Soul-writer runs |
| `catalog/` | Characters, personas, groups, prompts, creation helper workflows, character and persona files |
| `lorebook/` | Lorebooks and bindings, entry and keyword runs, staged lorebook projects |
| `media/` | Transcriptions, ASR learning, TTS configuration and syntheses, Whisper manifests, image generations, LoRA library, playground history |
| `models/` | Local llama.cpp reports and metrics, model lookup, model path relocation, device-local carry-over |
| `job_adapter.rs`, `usage_adapter.rs` | The durable job store, the usage ledger and cost bases |
| `sync/` | Journal, state scan, incoming changes, conflicts, per-domain snapshot codecs, secret versions |
| `backup/` | Backup export source, restore admission, restore writer |
| `legacy/` | Legacy database preflight and documents, import admission and stages, backup of import evidence |
| `purge/` | Hard delete of conversations and characters, media garbage collection |

## The Database handle

`Database` owns one `rusqlite` connection behind a mutex. Opening it:

1. refuses a file that already holds a migration this build does not know (`NewerSchema`), before any pragma touches it;
2. enables foreign keys and a five-second busy timeout, and for files WAL with `synchronous=NORMAL`;
3. applies pending migrations in one immediate transaction, checking the FNV-1a checksum of every applied one (`MigrationChecksum` on mismatch);
4. seeds the settings singleton;
5. restarts the sync journal if its schema fingerprint changed (see lettuce-sync).

A single serialized connection keeps the transaction story simple: every adapter method opens its own transaction, usually `BEGIN IMMEDIATE` so reads that decide a write cannot race with another writer. Possession of `Database` is a trusted composition capability. Ordinary repository traits expose artifact references only; the ports that can read protected bytes (trusted transfer, provider replay) are separate traits the app wires deliberately.

If foreign key enforcement cannot be restored after a purge, the handle refuses every later call with `ForeignKeysLost` until it is reopened.

## Migrations

Each migration belongs to one domain and holds all of that domain's tables, triggers and indexes:

| # | Domain |
| --- | --- |
| 1 | Foundation: settings, device settings and UI state, provider accounts, model profiles, jobs and job events, media blobs, legacy import runs and evidence, purge bookkeeping |
| 2 | Media assets, legacy media completions, media GC candidates |
| 3 | Characters, media, scenes, variants, starters |
| 4 | Personas and the default persona |
| 5 | Groups, members, starting scenes |
| 6 | Prompt documents, lorebooks, owner bindings, legacy graph and provider results |
| 7 | Stable keys for built-in prompt entries |
| 8 | Conversations: participants, settings, branches, messages, revisions, candidates, turns, attempts, tools, checkpoints, artifacts, media refs, operations, outbox, dispatch checkpoints |
| 9 | Memory spaces and items, summaries, embeddings, retrieval receipts, dynamic-memory approvals and runs |
| 10 | Usage events, job inference usage, cost bases, legacy usage records |
| 11 | Creation helper workflows, lorebook runs and staged projects, apply receipts |
| 12 | Companion Soul, growth, consolidation and Soul-writer runs, scheduled notes |
| 13 | Companion relationships, sessions, continuity, turn effects, suffix rewinds, memory pools |
| 14 | Speech transcriptions |
| 15 | Installed Whisper models |
| 16 | ASR vocabulary, corrections, ignored suggestions, voice examples |
| 17 | Audio providers, user voices, discovered voices |
| 18 | Speech syntheses |
| 19 | Sync journal, frontiers, incoming batches, conflicts, deferred changes, marks, forks |
| 20 | Backup restore admissions |
| 21 | llama.cpp runtime reports and generation metrics |
| 22 | Image generations, LoRA library, playground history |

The schema has not shipped, so a domain's tables change inside that domain's migration instead of through follow-up migrations. The checksum check makes a database created from an earlier draft fail loudly on open rather than drift.

## Storage conventions

The same few patterns run through every adapter:

- Normalized rows for anything queried or constrained; versioned JSON envelopes (`{format_version, value}`, unknown fields rejected) for values that are meant to be extensible, such as prompt conditions, settings payloads, run documents and outbox events. When a row has both, the scalar columns are projections of the document and are checked against it on every write and read.
- Aggregates change under revision compare-and-swap. A stale revision is a conflict, never a partial write.
- Operations that may be retried carry an operation id or request hash. An exact retry returns the stored result; the same id with different input conflicts.
- Evidence is immutable. Usage events, dispatch checkpoints, run documents, apply receipts, import evidence and journal rows are insert-only, and triggers reject `UPDATE` and `DELETE` on them. The only way such rows go is a purge (below).
- Triggers restate the invariants the domain already checks: legal state transitions, topology, ownership, settle-once. A trigger abort that names a known invariant maps to `Conflict`; anything else is a storage fault.
- Library pages use an opaque `(updated_at, id)` keyset cursor. This is not snapshot pagination: rows changed between requests may move relative to an earlier page.
- Secrets are never stored. Provider rows hold opaque `SecretRef` values and secret owner ids; the values live in the native secret store behind `lettuce_settings::SecretStore`.

## Settings, providers and models

`app_settings` is a singleton row with a versioned payload plus typed foreign-key columns for the selections that must follow a deleted row: the default model and prompt, the dynamic-memory model and the group speaker model. Selection setters (`set_default_prompt_document` and friends) are narrow CAS updates under the settings revision; a missing target fails as `InvalidData` through the foreign key. Deleting a model profile clears every foreign-key route to it and the model selections inside the payload (help me reply, lorebook generator, image features) in the same transaction. Device settings and UI state are separate rows that never sync.

Provider accounts store a strictly decoded, versioned `ProviderConfig` (only the fields of its variant are accepted) and model profiles their versioned config. Global model settings are stored as NULL when empty.

## Authored catalog

Characters, personas, groups, prompt documents and lorebooks are each stored as one aggregate: the root row, ordered children and media associations are read and written together, every read rebuilds and validates the complete value, and the adapter owns child identity, ordering, revisions and timestamps. Prompt and lorebook JSON (conditions, payloads, provenance) is strictly versioned. Lorebook bindings to characters, personas and groups use the owner's revision as their CAS token and follow the latest lorebook revision; an archived book stays readable through its bindings. Personas keep a singleton default under its own revision.

Built-in prompt entries carry a nullable stable key with a partial per-prompt unique index and strict non-blank, bounded checks. Reconciling the built-in catalog matches entries by key, so ids and entry history survive catalog refreshes, and ordinary user drafts cannot forge a key.

Group tables never mention conversations; a group's members, presentation assets and optional starting scene form the aggregate.

## Media

Media has two layers. `media_blobs` is the physical, content-addressed catalog: registration validates that the metadata fits SQLite, the first metadata registered for a hash is kept, and deduplication never changes the separate operational `BlobState`. `media_assets` is the logical catalog: each asset names a ready blob, a kind, a retention class and redacted, versioned provenance, and changes under revision CAS. Rows that reference media do so through typed media-ref tables with foreign keys, so the schema knows who holds an asset.

## Conversations

The conversation schema (migration 8) stores the `lettuce-conversations` aggregate fully normalized. Durable history is restrict-owned, and composite foreign keys keep every child scoped to its conversation and turn. Global id indexes serve recovery lookups without weakening that ownership. Triggers enforce target and input coherence of turns, retry-source ownership, group speaker ownership, pairing of each setting's value with its provenance, branch and message topology, contiguous checkpoint sequences, settle-once attempts, and immutable operations and outbox events.

### Creation

`conversation_creator` consumes a `PreparedConversationLaunch` in one transaction: it stages the snapshot artifacts, inserts the conversation, participants, settings, root branch, the initial scene and starter messages with their origin rows, the memory space (or pool membership) when memory is on, the create operation and the `conversation_created` outbox event. Origin rows are attribution; storage never interprets artifact bytes. The companion launch adapter seeds the companion session and relationship rows and the continuity episode in the same transaction, so a seed failure rolls back the whole launch, and replay checks the same frozen initial-state hash.

### The mutation kernel

Every other conversation mutation runs through `run_mutation` in `conversation_mutation_kernel.rs`, which fixes the order:

1. Open an immediate transaction.
2. Look up the operation record for the conversation, operation kind and idempotency key. A hit with the same request digest replays: the stored operation and outbox rows are returned and the value is rehydrated from current state, without running the body. A hit with a different digest is a conflict.
3. Run the body: CAS the conversation revision, require an active conversation (except for restore), stage the writes and describe the outbox events.
4. Insert the operation record, whose scalar result columns are projections of the stored reference.
5. Allocate contiguous per-conversation outbox sequences and append the events.
6. Commit.

The generation-begin family adds its own rule: a conversation holds at most one live turn, so a second begin while one is unsettled conflicts. Turn idempotency keys are prefixed with the operation kind, since send, continue, regenerate and retry share one unique index. `append_event` is shaped differently: it carries no conversation revision, so it checks the lifecycle, appends the checkpoint and leaves the revision and outbox alone. `latest_checkpoint_sequence` returns the highest stored sequence for an exact turn and attempt (none before the first append), and the insert trigger remains the authority on contiguity. Every turn status change is checked against the domain transition table before SQL runs, and again by the trigger.

Some mutations carry more:

- Preparation writes the turn's model, prompt and memory fields and its ordered lorebook attribution rows. It checks job ownership, the preparing state and revisions first, verifies the model artifact's stored reference, digest and attachment to the conversation (missing, foreign or forged artifacts cannot become provenance), and rolls everything back if one attribution insert fails. Group preparation takes the speaker from an explicit director target, a stored mention or automatic decision, or the original author of a regenerated reply, and fails before writing when a multi-character turn has no speaker. Recovery may reuse a matching preparation under a child attempt.
- Prepared companion sends and continuations reuse the send and continue transactions: the user message, turn, attempt, companion session and relationship revisions, operation and outbox commit together, and a stale state CAS or hook failure rolls the send back. Only dynamic-memory companion sends create the hidden turn-owned effect seed (relationship columns, three emotion-delta vectors, ordered signal changes); continuations use the zero-delta seed. Finalization creates the processing effect in the same transaction; failure or cancellation removes an unconsumed seed, and interrupted recovery keeps it for the same turn.
- In a companion direct chat, new user and assistant messages take their effective time from the conversation's companion clock. A finalized turn event reports the stored message time, so a regenerated reply keeps its original time.

### Dispatch checkpoints and tools

`generation_initial_dispatches` holds one row per running prepared attempt, bound to its job, the request fingerprint and the job usage event that keeps the raw dispatch evidence. The row stores the versioned provider-neutral request without the stream sink; reads recompute its fingerprint, and an exact attempt-owned lookup serves restart and child recovery without touching mutable preparation inputs. Admission needs a running turn and attempt with a resolved model, the attached job, matching prepared model and attributions, an existing job and no tool executions yet. Settlement is the only permitted update: it writes the versioned result once, requires settled job usage that agrees with it (including the provider response id), verifies conversation-retained replay artifacts and records them in `generation_initial_replay_refs` so orphan cleanup keeps them. Exact settlement replays; changed results conflict; SQL updates or deletes of admitted rows, including replacing the request, are rejected.

`generation_speaker_dispatches` does the same for LLM speaker selection: admission needs the attempt preparing at `SelectingSpeaker`, settlement needs terminal job inference evidence and an enabled, unmuted chosen participant, and the row keeps the fingerprint, the usage event id and the decision, so a reopened process replays the selection without another provider call.

`tool_executions` rows live under the exact conversation, turn and attempt that requested them. Request identity and input are immutable, provider call ids are unique within an attempt, state changes use revision CAS and terminal states cannot regress. A multi-call response is inserted in one immediate transaction, so an ordinal or provider-id collision cannot leave half a round; a later round compare-and-appends at the expected ordinal. Batch transitions require one attempt owner and advance every expected revision or none.

### Artifacts

Snapshot and provider replay bytes live in private artifact tables (`conversation_snapshot_artifacts`, `conversation_replay_artifacts`). Conversation rows store references, and the artifact store verifies immutable metadata and payload digests before any read or trusted transfer. The provider replay port materializes only fully matched references for exact provider-native continuation.

### Reads

`conversation_query` is read-only and snapshot-consistent: it owns the SQL projections, cursor decoding and the conversion of rows into validated domain values. `LiveTurnReader` lists unsettled turns across conversations for job recovery.

### Historical conversations

`conversation_history_writer` inserts a finished conversation with its own ids and timestamps, for backup restore and legacy import. Rows go in the order the migration 8 triggers require; each turn walks the shortest legal path of the transition graph instead of being inserted terminal; the create operation and its outbox event come last so later live mutations see a normal conversation. Legacy import generates the create operation; restore writes the exported operations and events exactly. The writer also creates the conversation's own memory space (or restores it with its id, revision, items, summary and embedding projections, after the messages the summary cites), creates or joins its companion pool, creates the companion session state and writes its continuity episode. Turns targeting one message are inserted in input order. Candidate media refs are written active.

## Memory

Migration 9 stores `lettuce-memory`'s model: revisioned memory spaces with typed, ordered items (six-digit `short_id` unique per space, source attribution with role, observed time and `turn` precision, supersession links), the cumulative summary with its ordered source-message cursor, and the dynamic-memory run graph.

### Own spaces and companion pools

A conversation with memory has its own space (`conversation_memory_spaces` with `pooled = 0`). A companion character also has one shared pool (`companion_memory_pools`), and each of its conversations has a `pooled = 1` row for it. The character's `share_memory_across_chats` toggle picks which space a conversation uses; `memory_adapter::active_space_id_in` reads it on every use, and flipping it copies and deletes nothing. A character leaving companion mode stops sharing. Backups export a pool once with its members in `shared_conversation_ids` and each own space separately.

The summary row is per space and records the conversation that wrote it. `summary_cursor` gives each conversation its own cursor within one read transaction: its own summary window; when another pool member wrote the summary, the end of its latest settled run that no later rewind of its own invalidated (only runs with a succeeded attempt count); with no summary, 0. A run's summary checkpoint becomes the space's summary when its attempt succeeds or only its tools phase fails, not when it is cancelled; a later rewind of the conversation or a newer checkpoint that already wrote the summary skips the write. When the current summary came from a run without a succeeded attempt, the owner's cursor stays at its latest succeeded run, so a retry reprocesses the window.

### Writes

The item set changes only through `compare_and_apply`: one immediate transaction compares the space revision, replaces the complete item set and bumps the revision once, so a stale or failed memory round cannot partly alter a snapshot. Change sets and synced items keep the stored token count of an item whose text they leave unchanged, so a recount needs no revision bump and an older snapshot cannot restore the old count.

Retrieval access has its own immutable receipt per generation attempt: the transaction checks conversation, attempt, space, revision and every selected active memory, promotes cold items and bumps access counts, records which items were cold before, and advances the memory root once. A matching retry returns the recorded revision without touching the items again.

### Embeddings

Embedding projections are rebuildable derived data: ready vectors as little-endian f32 BLOBs, or repair-needed rows without one. Reads join the exact live memory id and text, so a stale projection cannot affect similarity, and unchanged projections survive an item-set CAS without their BLOBs being rewritten. Repair queries also produce work for live items that have no projection, which closes the window between committing memory and storing its derived data. `put_ready` writes a vector only while the memory still has the embedded text (`Superseded` otherwise), and `put_reembedded` also stores the recounted token count under the same check.

### Runs

A dynamic-memory run is not a conversation turn and creates no turn or message. Its immutable row binds the conversation memory space and freezes the complete starting snapshot, ordered source messages with roles, effective times and the revision or candidate each rendered, the summary interval and half-open window, the resolved inference profile, time awareness, whether supersession is on, the structured fallback format and the tool contract. Admission applies the optional cycle-start change through the same memory CAS in the transaction that inserts the run, then requires the stored space to equal the run's `starting_memory`; a stale change conflicts and inserts no run.

Attempts move through `created`, `processing` and a terminal state by CAS. Rounds and calls keep their exact provider-neutral request context, bounded usage and protected replay references. Recovery interrupts the parent and copies its round and call evidence and settled results into one processing child. Background tool outcomes commit per round in the same transaction as the memory CAS; a stale revision rolls back without results and an exact retry returns the original settlement. The single pre-round summary checkpoint stores the summary, token count, request context, usage, provider request id and root-revision transition in the same transaction as the summary cursor CAS. Ownership guards keep a foreign conversation, space or source message out of a run.

The ask-first state stores the prompt baseline and pending or skipped state per conversation: repeating discovery at the same unsummarized count is a no-op, skipping clears pending but keeps the baseline, a later full interval advances the prompt count, and admitting an automatic or approved forced job clears the row.

### Rewinds

`dynamic_memory_rewind_adapter` undoes, latest first, only the tool results of the conversation's first invalid run and its later runs (`undo_runs`), then restores the summary without a second root revision bump, clears pending approval, invalidates the named companion effects and records an immutable receipt for exact retries. Decay, retrieval access, user edits and, in a pool, the other members' memories stay; a pool keeps its summary while an own space takes the prior run's summary. A hard-deleted memory it restores gets a fresh short id when its old one has been taken. Rewinds, the prior-summary search and the run cursor look only at runs of the space the rewind touches, so own-space runs and pool runs never undo each other. Effect invalidation is an overlay, so terminal effect evidence is never rewritten.

## Companions

- Soul (migration 12): character-scoped state keyed by `(character_id, scope)`, where an empty scope is the shared Soul and a conversation id is that conversation's own. Facts, ordered source memory ids and supersession evidence are normalized, not stored as a JSON authority. `SoulChangeSet`s apply under revision CAS with an immutable receipt. Creating a companion character seeds its Soul root and authored facts in the character transaction; roleplay characters get none. `apply_sharing_change_in` runs the share-Soul-growth transitions inside the character defaults write, and `seed_conversation_soul_in` seeds a conversation Soul when sharing is off.
- Runs: one growth run per durable job and memory run, freezing the memory attempt, profile, prompt inputs, evidence, Soul snapshot and a deterministic apply id, with one immutable reduced-proposal checkpoint; one consolidation run per succeeded growth job, freezing the post-growth Soul revision; Soul-writer preview runs by request id with ordered primary and fallback round checkpoints, which never touch Soul state.
- State (migration 13): four emotion vectors and ordered driver and active signals per conversation; relationship axes, tension, stability, interaction count and last interaction per companion character and persona (or default) scope. A new conversation in the same scope starts with fresh emotion and the existing relationship. Updates CAS both scopes and record an immutable request-hash receipt.
- Continuity: a fresh direct companion launch closes the prior open episode for the character and persona and links the next one; replay leaves the sequence unchanged.
- Turn effects: drafts become processing effects at finalization and settle as ready or failed with typed memory ids, the source-message window and bounded summary data. Terminal effects are immutable.
- Scheduled notes: character-scoped, companion owners only, listed in `available_at`, id order, idempotent delete.

## Creation helper and lorebook runs

Migration 11 stores creation-helper workflows: the target and its expected authored revision, user turns before inference, and an immutable ordered proposal lineage, with stage and current-proposal changes under revision CAS. A workflow may leave the confirmation stage again, but once an apply receipt exists it is closed: triggers and the adapter refuse further turns, attempts, recoveries, proposals and settlements. Apply is refused while an attempt on the current proposal is created or running. Tool calls must match a declared name and version, except undeclared tools, which are admitted at version 1 and answered as unknown.

Apply is the helper's only write into authored domains, and each target reuses that domain's own insert or CAS path in the same transaction as an immutable receipt: new and existing personas, new characters (root, scenes and receipt together), existing characters (profile and scenes under one root revision, unreviewed graph kept, dependent scene removal refused), new lorebooks and existing lorebooks (reviewed entry ids reconciled, hidden policy fields kept). Receipts are per workflow, so later workflows can revise the same entity with fresh CAS tokens.

Lorebook generation stores immutable single-entry runs bound to one job and the conversation, lorebook, character or persona, model and prompt revisions it used. Staged lorebook projects keep their state on one project row changed by CAS, with frozen planner, writer, refinement and coherence runs stored as immutable documents: the planner attempt is persisted before the planning-to-outline CAS so a restart never redispatches it; outline approval, replacement, draft batches, per-plan success and failure, manual edits, approvals, refinement, coherence proposals and accepted changes are all project CAS operations with exact replay; parallel writer completions from one batch may each advance the project revision without invalidating each other, and a writer from an older batch is rejected; each coherence review cycle keeps its own request and job evidence; an explicit planner retry creates a new job and archives the previous attempt in the same transaction, keeping the old job; cancellation and its job events commit together (queued work settles at once, claimed work keeps its lease for cleanup or expiry recovery), and late writer results cannot change a terminal project; final apply reuses the lorebook aggregate writers and returns the same entry ids on retry, even after later edits to the book. Staged source documents are retained by project and source identity: admission writes their asset references with the project and requires ready assets, foreign keys protect those assets even when their retention class is temporary, and loading checks the references against the excerpt provenance in the run document.

## Jobs and usage

`JobStore` persists versioned job specifications, snapshots and an ordered event table. Each mutation loads the durable aggregates into the single lifecycle reducer from `lettuce-jobs`, applies it under `BEGIN IMMEDIATE` and writes only the changed jobs and events. Idempotency, leases, cancellation, retries, progress, pagination, recovery and retention behave the same across restarts and concurrent handles, with no second state machine. Pruning keeps terminal jobs that a speech transcription, synthesis or image generation still binds, and their ancestors.

The usage ledger (migration 10) is append-only. `usage_events` derive conversation ownership from the generation attempt (checked by trigger at insert, not by foreign key, so usage outlives a purged conversation) and store known counters or one explicit unavailable reason with immutable model and provider revisions. Optional counters (cached input, reasoning, cache write, web search) and the provider-reported cost are nullable, checked, non-negative values; raw counters are never clamped, and the cost is a finite REAL at full f64 precision. `job_inference_usage` admits each job-owned dispatch and settles its versioned result once. `usage_costs` and `job_usage_costs` keep immutable, versioned cost bases (prices, provenance, calculation inputs, optional OpenRouter evidence) separate from raw usage, and costs are recomputed with the basis version's calculator, never with current prices. All of it survives job retention cleanup.

## Speech, images and local models

- Transcriptions (14) and syntheses (18) are admitted against their job and settle once; a synthesis must name its exact output asset with matching provenance and retention. `SpeechCacheRepository` looks up reusable syntheses and treats a blob as cached only when no asset of it is referenced from any other foreign key column, discovered from the schema.
- ASR learning (16) keeps authored text apart from normalized lookup columns and never truncates it, reproduces scope and language filters and runtime order, stores ignored suggestions under a null-safe unique identity with a repeat count, and removes matching ignored rows when a correction is saved. A learning batch imports in one transaction with fresh ids and remapped links, or not at all. Voice examples reference audio assets with `RESTRICT`, and term and correction links use `SET NULL`.
- TTS (17) cross-checks provider kinds and versioned configs on every read; credential columns hold only scoped secret references and owner ids. Provider and voice updates use revision CAS and keep their creation time. Voices require a provider and go with it, and the deleted provider record is returned so secret cleanup can run separately. The discovered voice cache keeps the provider's response order and label map, is replaced in one transaction and is left intact on failure.
- Whisper manifests (15) are immutable and kept apart from user media; the model hub may delete the exact manifest after its bytes are gone, and retained legacy models are refused before that call.
- Image generations (22) bind one `image_generate` job; success can name only `generated_image` assets whose provenance names that job, and output links keep those assets alive. Terminal rows may be deleted from the playground history, pending rows may not.
- Local llama.cpp state (21): runtime reports per model file and generation metrics. The metrics list keeps the newest 500; a row that falls out of the list or is cleared but belongs to a message's generation stays as `message_stats_only` (summary kept, samples dropped), and is found by `llm_generation_metric_for_message` through the message's candidate attempts. Triggers delete such rows when their candidate is deleted or message tombstoned. Neither table is backed up or synced.

## Sync

Migration 19 holds the sync journal: the local device identity, origin sequence and hybrid clock, the causal frontier, immutable change rows and frontier snapshots, peer frontiers, immutable incoming batches, conflicts, deferred changes, secret versions, conversation marks and fork notices, carried conflicts and deleted-entity stamps. The adapters in `sync/` implement the journal, the state scan (`SCANNED_CODECS`, in dependency order), incoming application and conflict resolution, and one snapshot codec per domain (conversations, memory under `conversation:<id>` or `pool:<character>` owners, companion state, plain rows, secrets). Persona and default-persona mutations also journal in the same transaction as their aggregate write. The lettuce-sync README describes the design.

## Backup and restore

Export (`backup_adapter`) reads everything a backup holds in one deferred transaction, through the same strict decoders the live adapters use: providers, models, prompts and selections, settings, audio providers and voices, personas, lorebooks, characters and groups with bindings (archived roots included), media metadata, ASR data, the full conversation graph in timeline order with turns, attempts, checkpoints, dispatches and tool state, jobs with inference evidence and costs, usage events and bases, operations and outbox events, companion state and effects, suffix rewinds, memory spaces with every embedding projection (stale rows and exact BLOB bytes included) and the dynamic-memory run graph. Scalar projections are checked, global limits apply while reading, and dangling references are rejected before encryption. Nothing is recalculated during export. Secret values and media bytes stay outside SQLite and are added by the app.

Restore admission (migration 20) records one immutable row per restore operation binding the source hash, plan fingerprint, staging receipt fingerprint and inventory counts; the same file can be admitted again under a new operation.

`restore_writer` fills an empty database in one transaction with deferred foreign keys, in dependency order:

1. Media blobs and assets, providers, models, prompts, audio providers and voices, personas, lorebooks, characters and groups with their bindings, exactly as exported (ids, revisions, timestamps, states), then settings, selections and ASR records. Blobs that were `missing` are inserted `ready` so their assets pass the ready-blob check, then marked `missing` again.
2. Protected snapshot and replay artifacts from their verified bytes.
3. Every conversation through the history writer in exact mode: exported operations and events, replay references, speaker, lorebook and memory attribution, failed or interrupted turns without candidates, shared pools, usage events and bases. Per attempt the writer restores its speaker dispatch while the turn is selecting a speaker, its initial dispatch with replay references and tool executions while the attempt runs (a tool walks from `requested` along the path its revision implies), and its checkpoints. Media refs that were historical at backup time are flipped back after insertion.
4. Work in progress at backup time is settled as interrupted through `lettuce_transfer::settle_in_flight_generation`: unfinished attempts get a derived interrupted usage event, unfinished turns become interrupted, turns without attempts are left out, running tools are interrupted and requested or validated ones cancelled.
5. Jobs and events through the job store's validation (in-flight jobs keep their state, so lease expiry recovers them as after a restart), job usage and bases; speech transcriptions and syntheses inserted pending and settled through their settle-once triggers.
6. Memory retrieval accesses, companion relationships, sessions, episodes and receipts, Soul states (replacing the rows character insertion seeded), scheduled notes, pool ownership, dynamic-memory approvals and runs. Each run source is inserted while its message briefly renders the source the run recorded; messages tombstoned at backup time are written hidden and tombstoned after the runs. Open background work stays open for its restored job. Companion turn effects go through draft, processing and children before settling; suffix rewinds and invalidations come last.
7. Legacy import evidence, restored run by run by walking its status guard, and imported legacy usage records.

Secrets, media bytes and the cutover to the new file belong to the app's restore coordinator. `carry_device_local_state_from` then attaches the previous file and copies what a backup never carries: Whisper manifests, the models folder (unless the new file names one), the LoRA library, generation metrics, the shell's install state and device settings (unless the new file has them), app usage days (keeping the larger active time per day), and the discovered voices and llama.cpp reports of providers and models that exist in the new file. A LoRA path in both files keeps the more recently updated row; a metrics row is skipped when the new file holds its id or the same generation under another id. The sync identity, journal, frontiers and conflicts stay behind, so a restored database joins sync as a new device.

## Legacy import

The legacy boundary reads the old app's `app.db` read-only and never modifies or deletes it.

1. Preflight (`legacy_database_preflight`) opens the file read-only, checks it is one of `LEGACY_DATABASE_SCHEMA_VERSIONS` (92 to 96, which share the table layout the reader expects), and returns a bounded typed inventory.
2. Planning reads each domain completely, in stable order and without row-count limits, and fails on malformed data instead of dropping fields: personas, lorebooks with entries, ASR learning, providers and models (exposing only API-key presence and header names, types and lengths, so no secret value crosses the reader), characters, groups, sessions and the rest. `read_legacy_database_documents` reproduces the 22 documents the legacy backup exporter writes, with its exact column lists, ordering, defaults and JSON, so a live legacy database and a legacy backup archive feed one shared planner.
3. Admission seals a run in one immediate transaction: immutable source schema, inventory and plan fingerprints, stable destination id assignments (insertable only while admitting), secret owner ids and opaque references for pending keys and headers, and every skip. A run whose source fingerprint matches a run that has not failed replays that admission; a different plan for the same source conflicts; a partial unique index stops two live runs from admitting one source. A failed run can be retried under a new id.
4. Secrets and media: the read-only secret adapter loads one exact value on demand into `SecretValue`, and each secret or media object gets an immutable completion receipt (no value or digest for secrets; path, size, hash and destination asset for media, checked against a ready blob). The first media receipt moves the run to `importing`.
5. Materialization reuses the ordinary aggregate insert paths inside immediate transactions: personas and lorebooks once every media receipt exists, then providers, models and prompts (secret references rebuilt only from sealed assignments and completion rows), then the stages. Each stage writes one immutable `legacy_import_stage_results` row per `(run, stage)`, requires a `partial` run bound to a source fingerprint and its prerequisite receipts, rolls back entirely on a destination collision, and replays its receipt.

The stages are characters, groups, audio, settings, direct conversations, group conversations, usage records, creation helper, images and LLM metrics. Characters, groups, scenes, variants, starters, audio providers and voices take ids derived from their legacy ids within the run's `LegacyIdScope`, so a different legacy source that reuses the same ids imports alongside and a replay derives the same ids; models, prompts, lorebooks and media are remapped through the sealed assignments. Conversation stages write every session through the history writer, restore memory, pools, companion state, Soul facts and scheduled notes, write per-message speed stats as `message_stats_only` metric rows, and fork unresolved legacy message conflicts into branches flagged in `sync_conversation_forks`. `complete_legacy_import_run` moves a `partial` run to `completed` only when every stage has a result row.

Legacy records that the old app itself ignored are skipped instead of aborting the import, and every skip is sealed with the run in `legacy_import_skips` and hashed into the plan fingerprint. Values it read leniently fall back to the value it used and are recorded as `legacy_value` skips keyed `<table>.<field>:<row id>`. Other malformed records abort. Legacy rows with no destination (`companion_turn_effects` beyond those tied to an imported turn, `sync_v2_conflicts`) are kept verbatim as `legacy_import_preserved_rows`, with BLOBs as `{"hex"}`, non-UTF-8 text as `{"text_hex"}` and non-finite REALs as `{"real"}`, and travel with the run in v2 backups.

## Hard delete

`purge_conversation` and `purge_character` delete an entity with every row that only it owns, in one immediate transaction:

- A conversation takes its participants, settings, branches, messages, revisions, candidates, turns, attempts, tools, checkpoints, dispatches, operations, outbox, snapshot and media references, sync marks and fork notices, its own memory space with items, projections, summary and runs, its companion session state, effects, receipts and continuity episode (a later episode is relinked to the deleted one's predecessor), and its growth, consolidation and lorebook entry runs. A shared pool stays, minus this conversation's runs, retrieval accesses and summary. Usage events and cost bases stay.
- A character takes its direct conversations, its pool, Soul, facts, receipts, relationships, sessions, episodes, scheduled notes, runs, scenes, starters, media links and lorebook bindings. Groups that list it keep their other members' order and mute state; the group revision moves while its update time stays, so two devices removing the same character reach identical content. A group left with fewer than two members or only muted ones stays valid and gets a `group_below_two_members` notice. Group conversations stay readable and keep running with the members left. Creation apply receipts keep the character id as a plain value, and replaying an apply whose character is gone answers `NotFound`.
- An entity with a live generation, memory run or companion effect is refused as `Busy`.

Branches and messages restrict each other, so a purge runs with foreign key enforcement off and checks every foreign key into the touched tables before committing (`PurgeError::Integrity` rolls back). Enforcement is then restored and verified. Append-only tables keep their delete guards; each guard allows a delete only while the owner (conversation, character or memory space) is listed in `purge_authorizations`, which the purge fills and empties inside its own transaction. Launch snapshots and provider replays the deleted rows named go when nothing else references them, and every UUID in a deleted row's text that is a non-library asset id is queued in `media_gc_candidates`.

`collect_media_garbage` takes the queue. A candidate is kept and forgotten when anything still names it: a foreign key (a legacy media completion only while its run is open), a sync change still waiting to apply, conflict evidence, an unfinished job or its events, or the text of any other table except bookkeeping (media catalog, purge and sync journals, legacy import evidence, finished jobs, replay caches). Library media is always kept. The rest are deleted; a blob none of whose assets is used leaves the catalog (deleted, or marked `missing` while an asset row or completion names it), and the released objects are returned so their files are deleted after the commit. `media_object_retained` answers the orphan sweep for a blob in any state, and `media_objects_in_file` reads the blobs another database file catalogs.

Deletes received through sync wait in `purge_queue` with the change that carried them; `run_queued_purges` decides each one again before purging. Entities kept and waiting to be sent back whole are in `purge_rejournals`. A busy entry stays queued without counting; one that fails eight times for another reason is dropped with a `dropped_after_failures` notice. `purge_notices` lists open notices and `dismiss_purge_notice` closes one.
