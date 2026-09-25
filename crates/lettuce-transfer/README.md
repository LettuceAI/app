# lettuce-transfer

Bounded external-format parsing, canonical documents, import plans, exports,
consistent encrypted backups, staged restore, and rollback.

## Boundary

Compatibility transfer and full backup remain separate internal modules.

Backup format version 2 is a streamed file: a plaintext header with only the
magic and the Argon2id parameters and salt (written at RFC 9106's 64 MiB, t=3,
p=4; read within a bounded range so later bumps stay readable), then every
section encrypted with XChaCha20-Poly1305 in 1 MiB chunks (random 19-byte
per-section nonce prefix, chunk counter and last-chunk flag; the header hash
and section index are the associated data), then the encrypted manifest
(names, schemas, sizes, BLAKE3 hashes) and a plaintext footer with its nonce
and length. Nothing about the content is visible without the password (backlog
#17: the earlier plaintext manifest exposed section hashes and sizes, and media
section names are image hashes). `BackupWriter` appends sections from any
reader and `BackupReader` opens a seekable source and decrypts one section at a
time into memory or a sink, so a section never has to be held whole to be
written or restored. Reordered, swapped, truncated or extended data, duplicate
or unsafe names and duplicate nonce prefixes reject the backup.
`verify_backup_frame` checks the plaintext frame without the password.

Export is planned by `plan_provider_backup_export` (validated graph, secret
and data sections, then the ready media blobs and conversation artifacts in
order); `verify_backup_media` and `verify_backup_artifact` check one object
against its size and hash so the app appends it without copying and drops it
before loading the next. Section limits are 2 GiB per section and 1 TiB in total (backlog #15:
the earlier 512 MiB total blocked large libraries; legacy had no limit). The
version-1 decoder keeps only its 512 MiB per-entry limit
(`MAX_LEGACY_BACKUP_ENTRY_BYTES`).

`decode_provider_backup_restore_plan` takes any seekable `BackupSource` (the
received archive file), hashes it by streaming, decodes the data sections,
secrets and conversation artifacts, and keeps media as `BackupMediaEntry`
(hash, size, section) read one at a time through `read_media` from the
authenticated reader the plan owns. Media ciphertext is authenticated when
staging reads it (a damaged blob fails staging with `Source`, before anything
reaches the media root or a database); installation then reads the verified
staged copy (`read_staged_media`), so the source is decrypted once per blob and
restore memory no longer grows with the media library.

`LegacyBackupMedia` carries each legacy file's size and BLAKE3 hash and reads
its bytes on demand through `read` (re-checked against both): the live legacy
database inventory keeps only the file path (`from_file` hashes it by
streaming) and `decode_legacy_backup_inventory` reads a version-1 archive from
a seekable `BackupSource`, decrypting each media entry once for its size and
hash and again on `read` (a version-1 entry is one AEAD message, so each entry
is still limited to 512 MiB, entry names are checked before anything is
decrypted, and the decrypted documents held for planning are limited to 512 MiB
together; the archive and media totals are not). A live file that changed
after the inventory fails its import with `SourceChanged`. The live database
inventory no longer rejects a large or unreferenced file or a library over
512 MiB; the media plan still limits each referenced object to 64 MiB (the
media store limit) and the legacy media total is now the backup total. The
live inventory source hash binds each file's size and content hash instead of
its bytes.

Backup reception uses confined resumable partial files and commits to a new
path only after the file's expected hash (streamed) and frame verify. An existing
backup is replayed when identical and never replaced. This corrects the legacy
backup format's fast password hash, reused encryption nonce and unbound entry
inventory. The uncompressed envelope also avoids archive traversal and
decompression expansion at this boundary. Domain-row and secret export,
profile restore and cutover remain later composed slices.

Files with the version-2 envelope header use the current verifier. A legacy ZIP
backup has no current envelope version and is classified as backup version 1
for the compatibility reader. Unknown bytes are rejected instead of being
guessed as legacy. Version 1 import will convert into the current staged restore
plan; new exports never write the legacy format.

A backup is complete only when it contains every user-owned profile domain,
referenced media object and portable secret needed to recreate that profile.
API keys and other secret-store values are required backup content; they remain
isolated in authenticated encrypted sections and never appear in public
metadata, logs or durable receipts.

The version-2 configuration snapshot now includes the exact global settings
payload, provider accounts, model profiles, prompt documents, audio providers
and user voices. It validates all selections and ownership links as one graph.
Both provider credentials and audio API keys are derived from opaque references
and must be supplied as the exact scoped inventory before the sections can be
sealed.

The same consistent snapshot contains every authored persona and its default
selection, lorebook with ordered entries, character with ordered scenes,
variants and starters, and reusable group with ordered members and its starting
scene. Character, persona and group lorebook bindings remain separate ordered
documents. Archived roots are retained. Model, prompt, voice, lorebook and media
references must all resolve inside the snapshot before sealing.

All logical media assets and physical blob metadata are included once by typed
identity, including records shared by several authored roots. The metadata binds
kind, content hash, byte size and lifecycle state. Every ready content-addressed
blob is read and hash-checked against that snapshot, then stored once in its own
authenticated, content-hash-addressed section. Assets that share a blob do not
duplicate its bytes. Missing, shortened, changed or oversized ready content
rejects the complete export; non-ready metadata remains represented without
invented content.

The versioned ASR learning document is a bounded JSON-ready interchange format
for vocabulary, corrections, ignored suggestions and managed voice examples.
It preserves metrics and relationships without exporting native audio paths;
version 3 carries each audio asset's logical kind, origin, redacted provenance,
content hash, byte size, MIME type and duration. The application validates and
remaps the complete graph before atomic import. Full backup version 2 now writes
the canonical version-3 document as its own authenticated section and verifies
each voice audio reference against the complete media snapshot.

The conversation-history section stores the complete visible durable graph for
direct and group chats in one bounded versioned document. It retains archived
roots, participants, policy and settings, every branch and selected head, every
message with its timeline ordinal, all revisions and generated candidates,
initial origins and media references. Validation rejects duplicate identities,
broken ownership, parent, fork, head, author, render-source and media links.
Every protected launch, current-setting and candidate-model snapshot referenced
by that graph, plus every conversation-retained provider replay artifact on a
revision or candidate, is included once in an authenticated identity-addressed
section. Descriptor, size and BLAKE3 must match the visible graph exactly.
Generation attempts, checkpoints, tool executions, jobs, usage, companion
runtime state and memories remain separate later sections of the full backup.

The conversation-runtime document now retains every turn, ordered attempt,
durable stage/progress checkpoint with its original timestamp, speaker and
initial inference dispatch checkpoint, preparation attribution embedded in the
turn, and every tool execution with exact request and terminal result. It is
bounded independently and validates ownership against the visible history,
including candidate, message, job, replay-artifact and contiguous checkpoint
links. Generic job records and usage event/cost bodies remain separate later
sections; their typed IDs are retained here.

The legacy version-2 ASR JSON schema remains a separate camelCase compatibility
document. Its nullable IDs, counters, timestamps and native audio locators are
parsed without making that external format part of the speech domain.
External schemas never become internal domain schemas, and restore never
destructively replaces live state before complete verification.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

A read-only legacy SQLite preflight contract now reports the supported schema
version and bounded counts for importable root records. SQLite inspection stays
in `lettuce-database`; this crate owns the compatibility result and typed
failures. Record mapping, media discovery and import writes remain later slices.

The persona plan preserves legacy identity, authored text, crop, LoRA
recommendation, lorebook references, timestamps and the single default owner.
Avatar and design images remain typed unresolved locators until media import.

The lorebook plan preserves root detection policy and unresolved avatar plus
every persisted entry field in deterministic order. It bounds roots, total
entries and entries per book before any destination mutation.

The ASR learning plan preserves vocabulary, correction, ignored-suggestion and
voice-example rows in stable legacy ID order. Authored and normalized text,
counters, confidence, timestamps, language, scope and optional learning links
remain exact. Voice audio paths remain unresolved media evidence. Separate
per-table and aggregate bounds apply before any plan is returned, and malformed
or orphaned rows reject the whole read-only plan.

Provider/model planning preserves stable legacy account and profile identities,
protocol, endpoint and custom wire configuration, effective streaming/TLS state,
model routing, modalities, supported generation parameters, prompt references and
both legacy defaults in deterministic bounded lists. API keys and header values
never enter the plan: it contains only pending secret kinds and header names for a
later secret-store transfer. Unsupported provider/model configuration field names
remain explicit deferred inputs whose values stay in the retained source database.
The legacy built-in llama.cpp credential becomes one deterministic synthetic
account only when a llama.cpp model needs it; it carries no secret or endpoint.
Legacy `dynamicMemory`/`groupDynamicMemory` settings map every field the current
settings hold, including `enabled`, the summary interval (0 becomes 1, as the
legacy cycle used), run mode (unknown values run as `auto`, like legacy),
decay, delete confidence, hard-delete ratio and recursive loops; the advanced
`dynamicMemoryStructuredFallbackFormat` (`json`/`xml`) sets both the direct and
the group settings, any other value is malformed; the summarizer and manager
prompt template ids are retained as source ids (`dynamic_memory_prompt_source_ids`)
and, like the lorebook generator prompt ids, checked against the backed-up
prompt templates but not yet materialized into settings on import;
`dynamicMemoryLlamaSamplerOverwriteEnabled` maps to the same-named setting
(default true); the `helpMeReply*` keys map to `help_me_reply` (a zero history
count is a lossy notice, an unknown style is malformed, the model id and the
two prompt template ids are retained and orphan-checked like the other
selections); the image feature keys map to `image_generation` (an unknown
scene mode is malformed; avatar, scene and creation-helper model ids must name
image-output models and the scene writer a text+image-input, text-output
model, else they are skipped as missing or incompatible references); only
unknown keys remain unsupported notices. Version-2 backup graphs reject a
settings document whose dynamic-memory prompt overrides name a prompt outside
the graph.

Legacy import admission now seals provider-account and model-profile source IDs
alongside the existing graph and media plan. Each account receives a stable
destination account ID and secret-owner ID, each model receives a stable
destination profile ID, and every pending API key or named header receives an
opaque `SecretRef`. These assignments contain names and identities only; secret
values remain in the retained legacy database until the later secret-transfer
step verifies and writes them through `SecretStore`.

Admission also seals the complete ASR learning plan and assigns stable typed
destination IDs to every vocabulary, correction, ignored-suggestion and voice
example row. Voice audio joins the same bounded, content-hashed media plan with
its original locator retained as evidence and a safe logical import key. Exact
retries preserve all assignments; any ASR content, link, locator or media change
conflicts before destination writes. Voice audio is now ingested through the
same immutable media completion protocol with an audio asset kind. ASR
materialization resolves every assigned row and optional link, requires those
audio completions, and returns one immutable aggregate receipt.

Provider secret transfer uses a narrow source port that yields temporary
zeroizing values only for the sealed API-key and header assignments. Durable
completion receipts contain the destination reference, secure-store generation
and completion time, never plaintext or a reusable value digest. Source-set
changes conflict before writes, and retries verify an already-present value so a
crash between the secure-store write and SQLite receipt does not rotate it.

Provider/model/prompt materialization has a separate receipt from graph materialization.
It consumes the sealed provider/model plan only after every assigned secret has a
matching completion, maps all source account/profile/default identities to their
assigned destinations, and replays without duplicate rows. The prompt plan
preserves ordered structured entries, condensation, purpose and timestamps;
content-only templates become the single system entry the legacy runtime created.
The app-wide prompt selection maps to its assigned imported document. Undefined
legacy templates are corrected to direct-chat documents because undefined is not
an operational purpose. Model prompt references and deprecated system-prompt text
remain fingerprinted evidence and do not create parallel live model fields.

The composed media plan deduplicates persona avatar, persona design-reference
and lorebook avatar uses by relative legacy file, retaining bounded byte size
and BLAKE3 evidence for later ingestion. Missing, unsafe, ambiguous and
oversized sources are typed failures.

Persona-and-lorebook imports now have durable admission and execution contracts. One run binds the supported source schema, an inventory fingerprint, the complete persona/lorebook/media plan fingerprint, and stable destination IDs for every persona, lorebook, entry, and logical media asset. Exact retries replay those assignments; changed source IDs or plan content conflict. Media completion receipts retain the assigned asset, ready blob, byte length, hash, and original completion time. Materialization accepts only the sealed plan and returns an immutable receipt for the atomically created graph.

The legacy database and storage tree are read-only migration sources. A completed import must retain them until migration verification succeeds and the user explicitly approves deletion; cleanup is never part of admission or automatic import execution.

The jobs backup document retains each durable scheduler spec, current lifecycle
snapshot and contiguous event history. Immutable inference dispatch evidence
and any attached cost basis retain their job identity independently so evidence
survives normal scheduler retention. Validation reuses the job-store restore
contract and never aggregates these records with conversation UsageLedger events.

The conversation-usage document retains every terminal attempt's immutable
UsageLedger event, exact reported or unavailable counters, and optional captured
cost basis. It records matching job-inference evidence IDs as overlap metadata,
so restore and reporting can preserve both audit trails without treating them as
independent charges. Validation requires every terminal runtime usage reference
to resolve and rejects changed ownership, invalid pricing inputs, orphan costs,
or incomplete overlap metadata.

The conversation-outbox document retains every idempotency operation record and
the complete immutable per-conversation event journal. Operations without an
outbox event remain included because checkpoint and preparation replay also
depend on them. Validation requires contiguous event sequences, exact operation
links and timestamps, and valid history, runtime and usage references. The
legacy UI emitted transient Tauri events; the current schema has no durable
consumer lease or delivery state to export.

The companion-state document preserves persona-scoped relationship state,
conversation-owned emotion vectors and ordered signals, continuity episodes and
immutable replacement receipts. Storage-only initial-state and replacement
hashes plus row revisions and timestamps remain explicit, so restore can retain
launch and update idempotency. Validation binds each session and episode to its
authored character, optional persona and direct conversation without changing
any companion-state formula.

The companion-effect document preserves every processing, ready, failed and
invalidated turn effect with its exact transition seed, terminal result, source
window and referenced memory IDs. Immutable suffix-rewind receipts retain their
request digest, memory revision boundary, resulting memory and summary snapshots,
and ordered effect invalidations. Validation binds effects to their turn and
user/assistant messages, checks the stored effect shape, and requires every
invalidated effect to resolve through exactly one rewind. Memory IDs, memory-space
ownership and dynamic-memory run identities remain cross-document dependencies
for the following backup sections; they are not discarded or fabricated here.

The memory document preserves every conversation-owned memory-space revision,
its ordered authoritative typed items, optional cumulative summary and immutable
retrieval-access receipts. Item access counters, cold/pinned state, importance,
temporal source attribution and supersession metadata remain exact. Validation
binds spaces and summary/item sources to conversation history, accesses to their
generation attempts and revision transitions, and rewind/effect references to
the matching conversation space when the referenced memory still exists.
Historical memory IDs may be absent after an authorized hard delete; the backup
does not fabricate tombstones. Durable dynamic-memory execution records remain a
separate later document.

`data/memory-projections.json` preserves every stored ready and repair-needed
embedding projection, including stale rows retained after memory edits or
deletion. Ready vectors encode their exact little-endian SQLite bytes as hex alongside the
source revision, dimensions, source text and update time. Validation requires a
backed-up owning space, a unique storage identity and the same finite-vector
shape accepted by the runtime. It does not require a live matching memory item,
because retrieval already excludes stale rows through its item/text join.

`data/dynamic-memory.json` preserves the background extraction restart graph.
It contains ask-first approval state, frozen runs and message sources, retry attempts, ordered inference rounds and calls, background results,
summary checkpoints, usage details, replay references and timestamps. Validation
binds these records to backed-up conversations, attempts, jobs and memory spaces;
export performs no provider, tool, embedding or memory work.

The version-2 restore decoder is read-only. It authenticates the complete
envelope, requires the exact fixed section names and schemas, decodes every
current document, validates the complete cross-domain graph, and inventories
every expected secret, ready media blob and protected conversation artifact.
Missing, duplicate, unknown, malformed or mismatched content rejects the entire
plan before any database, secret store or filesystem mutation. A legacy ZIP is
routed to the version-1 compatibility decoder. The source backup and existing
database remain untouched; a later staged materialization and verified cutover
must retain them until the user explicitly approves deletion.

Decoded version-2 binary payloads can now be materialized under a caller-owned
isolated restore workspace. The workspace resumes only an exact partial prefix,
uses confined no-follow paths and no-replace commits, then rereads and verifies
the size and BLAKE3 hash of every media blob and protected conversation artifact.
A versioned receipt binds the staged inventory to the encrypted source archive;
exact retries replay while divergent bytes or receipts fail closed. Secret
values never enter the workspace or receipt. This remains pre-cutover staging:
live database, secret and media roots are not opened or changed.

`ProviderBackupRestoreWriter` is the port that writes a decoded version-2 graph
into an empty database exactly as it was exported (user decision 2026-09-14: a
backup replaces the current data in a fresh database and the previous database
file is kept). A target that already holds data is refused.

Unversioned legacy ZIP backups are now opened by a separate read-only
compatibility inventory decoder. It reproduces the legacy BLAKE3 password KDF
and shared XChaCha nonce only to authenticate existing encrypted archives, then
returns bounded zeroizing bytes for the known optional JSON documents and known
media roots. New output never uses that cryptography. Duplicate or unsafe names,
plaintext payloads, unknown entries, unsupported internal manifest versions,
bad base64 parameters, wrong passwords and oversized archives reject before
conversion or writes. The decoder accepts bytes so desktop files and Android
`content://` provider streams share the same format path without treating a URI
as a native path or deleting its source.

Authored version-1 media locators now resolve directly against those retained
archive bytes before any ingest. Persona and character avatars use their exact
owner directories; lorebook icons, design references and character/group/scene
backgrounds use the legacy image-reference convention. Shared archive objects
are hashed once and retain every typed owner and ordinal use. A reference whose
file is absent is cleared and recorded as a `MissingMediaFile` skip, like the
legacy SQLite import: persona avatar (with its crop) and design references,
lorebook avatars, and `CharacterMedia`/`GroupMedia` slots keyed
`<id>:avatar|background|design:<locator>|scene:<scene id>`. Ambiguous, unsafe,
oversized or over-limit references still reject the whole media plan. Conversation attachments, generated images and other later-domain media
stay attached to the source inventory and are not misclassified as orphaned by
this authored-only slice.

Legacy character and group JSON columns now become typed domain values instead
of raw JSON:
- **Companion.** `companion` becomes a `CompanionSoulConfig`:
  - keys the rewrite has no field for (`memory`, `timeAwareness`, `context`, stray nested keys) are dropped and recorded;
  - missing affect, regulation and relationship values take the legacy defaults;
  - authored facts follow legacy storage normalization: default policy by category, slot from category, `authored` kind, deterministic ids for blank ids;
  - facts with an unknown category, blank value or invalid validity are dropped and recorded;
  - `prompting.promptTemplateId` is kept as a prompt source id for remapping;
  - a companion value on a roleplay character is recorded and dropped.
- **Voice.** A `voice_config` user voice resolves to the imported `VoiceProfileId`; a missing voice records `VoiceReference`/`MissingUserVoice`. A provider voice is kept verbatim as an unresolved legacy locator.
- **Chat appearance.** `chat_appearance` becomes the complete appearance legacy displayed: defaults, then the legacy global `advancedSettings.chatAppearance`, then the sparse override.
  - Keys are applied one at a time with dependency retries; a key that cannot be represented keeps the inherited value and is recorded.
  - Widget nodes convert camelCase keys and `12h`/`24h`; library and upload images become unresolved legacy tokens.

References the rewrite would refuse to launch with are cleared and recorded as
`IncompatibleReference`:
- a character direct, group, companion or starter prompt, or a group profile prompt, whose purpose differs from the one the launch planner looks up;
- a character default model or group member override that is not a chat model.
- an app default, dynamic-memory, speaker, lorebook-generator or help-me-reply model that is not a chat model, and a lorebook-generator, dynamic-memory or help-me-reply prompt of another purpose (legacy loaded any template by id; the feature now uses its built-in document). Import admission accepts these settings skips, which previously made a legacy database with a stale feature reference fail admission.

A missing prompt still stays referenced and resolves to the app default like
legacy. Character lorebook rows follow the legacy restore rule: only enabled
`character_lorebooks` rows count, they replace the character's list only when
the character has one, and rows naming a deleted character or lorebook are
recorded instead of aborting. A design reference that repeats the background or
another reference is dropped and recorded, since one image cannot hold two
character media slots.

The version-1 ASR backup document now maps into the same bounded read-only plan
used by SQLite migration. It preserves the exported authored and normalized
text, language, scope, category, priority, confidence, approval, counters and
timestamps, while assigning deterministic plan-local IDs from retained archive
order because the legacy exporter omitted database IDs. Duplicate semantic
rows, invalid normalization, metrics, flags or timestamps reject before any
write. The legacy full-backup path never exported managed voice examples,
their audio or learning relationships, so the plan reports that absence and
does not fabricate them. All decrypted documents and archive media remain
attached; this slice neither writes learning rows nor ingests audio.

Legacy usage history is also decoded as a bounded read-only compatibility
plan. It retains request, session, character, model and provider labels, the
operation and finish reason, every exported token counter, success/error state,
the three stored monetary totals and all metadata. Known metadata counters and
cost fields are validated from their string representation without running a
pricing formula. These rows predate current turn, attempt, job and immutable
dispatch identities, so they are marked historical-only and cannot enter the
current aggregate ledger as new charges. Duplicate IDs or metadata keys,
negative counters, invalid timestamps and non-finite costs reject the plan.
Finite historical values remain exact, including values produced by old
calculator behavior, while current pricing and offloader formulas remain
unchanged. The source inventory stays attached and no usage row is written.

The legacy model-pricing cache is retained separately as bounded historical
evidence. Each entry keeps its exact model key, original JSON text and cache
timestamp; known camelCase price fields must parse as finite nonnegative USD
values, while unknown fields remain visible as conversion notices. A null
payload still preserves the legacy cached absence. These entries lack the
current provider account, endpoint, revision and capture evidence required by
the immutable cost basis, so they are historical-only and never populate the
live provider cache. No cost is recalculated and the version-1 pricing formula,
runtime calculator and smart-offloader outputs remain unchanged.

Direct legacy sessions now have a bounded read-only compatibility graph. It
retains each root and derived session, archived state, character/persona/scene
and prompt selections, author and generation overrides, voice policy, raw
companion and dynamic-memory documents, and every message and variant in the
order present in the decrypted backup. Message visibility, parent links,
selected variants, usage metrics, MTP statistics, reasoning, memory and
lorebook references, attachments and effective timestamps remain explicit.
The planner validates authored references, message ancestry, selected-variant
ownership, JSON shapes, counters and finite sampling values before any write.
Branch links are repaired rather than validated: legacy deleted a session row
without touching its branches and "Branch to character" linked a session to
another character's root, so a parent or root that is missing, belongs to
another character or group, or closes a loop is cleared, a branch message
missing from the session is cleared, the root becomes the topmost surviving
ancestor, and each change is recorded as a `session_link` skip. The same repair
runs for group sessions. Equal message timestamps and unordered legacy variant
queries are reported as ordering loss. References legacy never cleaned up are
cleared and recorded in the plan's `skipped` list instead of rejecting the
backup: deleted override lorebooks (legacy looked each up and found nothing),
a scene the character no longer has (legacy used an empty scene), a message's
deleted model (only the debug view read it) and a missing selected variant,
which falls back to the last variant like the legacy chat view. A missing
character or persona still aborts because legacy's foreign keys made those
impossible and its restore enforced them. Legacy rows contain no current
generation attempts or protected model/provider snapshots, so those identities
are reported absent instead of fabricated. Group sessions and attachment bytes
remain attached for their following slices; no conversation or media store is
changed.

Legacy group sessions now extend that compatibility graph with the frozen
member and mute order, optional reusable-group link, persona, chat, memory and
speaker policies, per-member model overrides, prompt selections, starting
scene, branch topology, participation state and the complete message/variant
history. Speaker attribution, selection reasoning, Gemini content, usage JSON,
attachments and dynamic-memory documents remain exact retained evidence.
Director and Director Action still require no persisted selected speaker: the
legacy send path selected explicit mentions and the continue path accepted an
explicit member per request, including muted members, while automatic selection
excluded muted members. Session snapshots may therefore retain an all-muted
cast even though newly authored reusable groups require an active member. The
planner rejects duplicate identities, orphaned persona and muted links (a
missing reusable group link is cleared and recorded like legacy's foreign key
set it to null), message cycles and malformed nested scene, usage, MTP or
attachment JSON. A deleted session prompt is kept and recorded: legacy treated
it as an explicit choice and went straight to the app group template instead of
the character's group or direct prompt, so the materializer must resolve a
missing explicit prompt to the app default rather than inherit. References
legacy never cleaned up are otherwise cleared and recorded in the plan's
`skipped` list: deleted lorebooks, model overrides for
non-members or deleted models (legacy fell back to the character's model),
deleted message and variant models, a missing selected message variant (falls
back to the last variant like the legacy group chat view) and a starting scene
snapshot's missing selected variant (nulled in the snapshot; legacy used only
the scene content). Members and message speakers whose character was
deleted are kept and recorded as missing characters: the user decided that a
deleted character stays in a group chat as an ignored "Unknown" participant,
and legacy likewise kept those messages and skipped the id during speaker
selection. Participation rows and message speakers for a character removed from
the session (legacy removal left both behind) are kept and recorded the same
way; the conversation writer must give such a speaker its own ignored
participant because only members become participants. Participation and variant queries
had no stable database order, and messages with equal timestamp and turn-number
keys cannot recover their relative order; those losses are reported while
archive array order remains as an ordinal. Finite historical API costs remain
exact even when legacy pricing produced a negative value; no current pricing
formula is run. The legacy document has no current generation attempts,
protected snapshots or selected branch head, so none are fabricated.
Conversion remains read-only and does not ingest attachment or background
bytes.

Legacy companion scheduled notes now map into the existing companion-owned
note and recurrence types as another bounded read-only plan. Archive order,
identity, authored companion owner, label and content bytes, availability,
end-exclusive expiry input, recurrence window, enabled state and timestamps
remain explicit. The compatibility boundary rejects duplicate IDs, missing or
non-companion owners, unknown recurrence values, negative times and windows,
expiry at or before availability and reversed write timestamps. It does not
normalize archived text or evaluate schedules during conversion. Runtime
activation, local calendar handling, effective-time selection, rendering order
and prompt limits remain owned by the existing companion and application paths,
so the legacy scheduling semantics and clock source are unchanged. No note or
companion state is written.

Legacy companion shared memory now has a bounded read-only compatibility plan.
It retains each companion-owned manual-memory array, embedded dynamic-memory
projection document, summary and token count, tool-event history, progress and
error state, raw Soul growth, persona-keyed relationship state, continuity
episodes and timestamps exactly as exported. Companion owners, persona keys,
direct-session episode links, episode chains and nested JSON shapes must resolve
before any mutation. Fully specified Soul facts can seed one validated initial
Soul snapshot; incomplete legacy facts remain evidence because the old importer
generated missing IDs and timestamps. Relationship values can seed initial
conversation snapshots only for matching continuity episodes. Manual memories,
embedded projections and tool events remain retained evidence because the
legacy document cannot supply current memory-space revisions, immutable access
receipts or execution history. The old importer's table-wide deletes and silent
Soul normalization are deliberately not reproduced, and this slice writes no
companion or memory state.

The standalone legacy memory-embedding document now extends that read-only
plan. It retains the stable owner order, direct/group/companion-shared owner
kind, exact canonical JSON and every memory vector, score, counter, timestamp,
category, temporal attribution, supersession field and entity anchor. Each
standalone owner must resolve to the already planned conversation or companion,
and its canonical JSON must agree byte-for-byte with the copy embedded by the
legacy exporter. Duplicate owners or memory IDs, orphaned supersession links,
unsupported vector dimensions, mismatched declared dimensions, non-finite or
out-of-range scores and invalid timestamps reject. Equal creation timestamps
are reported because the old normalized query supplied no tie breaker. A fully
compatible item with a supported retained vector can seed an initial memory
item and projection after its new space is created; compatible items with no
usable vector require projection rebuild. Other records remain exact evidence
instead of losing legacy-only entity/fact metadata or inventing a historical
memory-space revision. The legacy importer's table-wide delete and permissive
malformed-JSON-to-empty behavior are not reproduced, and no memory row is
written.

Legacy creation-helper sessions now have a bounded read-only compatibility
plan. It retains the exporter’s recency order, goal and status, byte-exact
nested session and uploaded-image JSON, drafts and draft history, chat messages,
tool calls/results, block order, create/edit target and timestamps. Outer row
identity, goal, status and timestamps must agree with the nested session. Edit
targets resolve through the authored graph, draft scene links remain closed,
and cached image asset IDs resolve unambiguously to retained archive images.
Older inline base64 and current asset-backed uploads are both preserved and
hashed independently, including the valid case where both copies exist.
Duplicate identities, malformed tool/block graphs, unsafe or missing image
references and oversized nested state reject; tied recency timestamps report
the legacy query’s missing order tie breaker. A pristine active create session
whose draft fits the current target can serve as initial draft input only. No
legacy row is classified as a durable workflow record because it has no project
revision, proposal identity or lineage; conversational helper history also
lacks turn/attempt identities and inference checkpoints. The old importer’s
table-wide delete is not reproduced, and this slice writes no creation or media
state.

The complete version-1 compatibility graph now has one source-bound seal. The
seal runs the existing configuration, authored, media, ASR, usage, pricing,
direct/group conversation, companion-memory, projection and creation-session
planners in their dependency order. Its coverage lists all 23 known document
kinds as present or absent and inventories every retained media object with its
root, normalized segments, byte count and BLAKE3 hash. The stable fingerprint
binds that coverage, the archive identity and every sorted conversion notice;
duplicate document or media identities reject before planning. This is a
reconciliation boundary only. It does not admit an import, ingest bytes, write
current state or remove the source backup.

Every retained version-1 media object can now be staged through the same
confined restore workspace used by current backups. Content-addressed blobs use
exact-prefix resume, no-replace commit and full reread verification. A separate
versioned legacy receipt binds the source archive hash and compatibility seal to
each original media root/path, byte count and content hash. Exact staging
replays; a changed partial prefix, installed blob, seal or source receipt fails
closed. Secrets, live database rows and the media library remain untouched, and
the backup source is never removed.

`rebind_provider_backup_secrets` moves every provider API key, secret header and
audio API key reference of a decoded graph to a fresh `SecretRef` and rebinds the
decoded secret values to them, so a restore writes new store entries instead of
overwriting secrets the previous database file still references.

A planned legacy source now serves the import stages without touching its
origin. `LegacyBackupConfigurationPlan` implements `LegacyProviderSecretSource`
over the provider API keys and secret headers it decoded (the same deterministic
references its secret list uses), `LegacyBackupCompatibilityPlan::media_bytes`
returns the retained bytes of a planned media object by archive path, and
`database_inventory` derives the source counts a legacy import admission
validates. Together they let a version-1 backup run the same import chain as a
live legacy database.

Current version-2 and legacy version-1 staging receipts can now produce the same
validated restore-admission request. It binds the source version/hash, plan
fingerprint, exact staging-receipt fingerprint and document, media, secret and
protected-artifact counts. A current plan is a pure decode of its source, so its
fingerprint is derived from the source hash; a legacy plan uses its compatibility
seal. Current receipts must exactly match their decoded plan; legacy receipts
must match the revalidated compatibility seal and complete media coverage.
Admission does not reread the workspace, so cutover must verify staged bytes
itself. The transfer-owned repository port records admission only and
carries no secret value or live-domain payload.

The first version-1 conversion slice turns legacy settings, provider accounts,
models, prompt templates, audio providers, user voices, portable credentials
and chat templates into one bounded read-only plan. It preserves the established
provider-routing order, model scope fallbacks, supported generation parameters,
prompt content fallback and TTS provider defaults. Provider headers, inline or
legacy-table API keys and audio API keys become scoped zeroizing secret values;
machine-specific Kokoro asset roots never become canonical configuration.
Defaults and every absent, unsupported or lossy field are reported explicitly.
Provider `config` and `headers` and model `advanced_model_settings` that are not
valid JSON or have the wrong shape fall back exactly like the SQLite import
(empty config, no headers, empty settings) through the shared
`lenient_legacy_json`, recorded as `legacy_value` skips in the plan; legacy
restore wrote them back raw and its settings reader omitted them. Model scopes
that are not a JSON array become text-only and non-string scope items are
dropped and recorded, matching legacy restore normalization; JSON `null` counts
as absent. A prompt type that is missing becomes direct chat like legacy
restore's "undefined"; a non-string or unknown type also becomes direct chat and
is recorded as an unknown legacy value. `legacy_prompt_purpose` is the legacy
prompt store's exact type table (camelCase names plus its snake_case lorebook
aliases, no runtime text) and is shared with the SQLite import. Prompt entries that are missing or null
are empty; a string that is not a JSON array, or any other non-array value, is
empty and recorded (the content entry then applies); a valid array the new
types cannot read still aborts. Settings and model references legacy never
cleaned up after a delete are cleared and recorded instead of rejecting the
backup: the default provider and model, the summarisation, group speaker,
lorebook generator and help-me-reply models (model references; legacy failed
those features until another model was chosen, so clearing is a recorded
correction), and the default prompt, every advanced prompt template id and a
model's prompt template (prompt references; legacy fell back to the default).
All other model, prompt, audio-voice, chat-character, scene and lorebook links are
checked when their owning legacy document is present. The complete decrypted
source inventory remains attached to the plan, including converted documents,
so later slices can recover fields that do not yet have a canonical owner without
reopening, deleting or rewriting the backup.

The authored-profile conversion slice adds bounded persona, lorebook and
character candidates without writing live state. It preserves the default
persona, every authored text and provenance field, legacy presentation and
companion payloads, ordered rules, scenes and variants, chat starters, LoRA
recommendations, timestamps and unresolved media locators. Existing persona,
lorebook, character-profile, scene-document, starter-message and ordered
lorebook-binding contracts validate the parts they own. Character and persona
lorebook order remains explicit; the optional compatibility binding document
also retains disabled rows instead of repeating the legacy importer’s silent
drop. Defaults, selected variants, starter ownership, model, prompt, scene and
lorebook links must resolve inside the complete retained plan. Missing documents,
unknown fields and normalized legacy binding order remain visible as conversion
notices, and malformed or orphaned graphs reject before any mutation. Persona and
lorebook values legacy read leniently follow the SQLite import exactly and are
recorded as skips: persona design reference or lorebook id lists that are not
string arrays become empty, non-UUID and missing or repeated persona lorebook
ids are pruned by `reconcile_legacy_persona_lorebooks`, unknown detection or
match modes become recent message window or literal, malformed keywords become
empty and regex keywords the legacy matcher could never compile are pruned by
`reconcile_legacy_lorebook_keywords` (both reconcile functions now live here and
are shared with the SQLite import). Non-string JSON values in these string
fields still abort. Character and starter references legacy never cleaned up
are cleared or pruned and recorded in the authored and configuration plans'
`skipped` lists: a character's default scene, default model (legacy failed with
"model not found"; clearing is a recorded correction), default
starter and missing or non-UUID active lorebook ids (repeats are dropped; legacy
injected them twice in direct chats), a scene's selected variant (legacy fell
back to the scene content), and a starter's scene (when the characters document
is present) and override lorebook ids. A character's deleted prompts (direct and
group) and a starter's deleted prompt are kept and recorded instead, because legacy treated
them as explicit choices and fell back to the app template rather than
inheriting. Character and starter lorebook
links are pruned the same way whether or not the lorebooks document exists, so
both planners agree. A JSON `null` override means no override silently and
malformed override JSON means no override with a recorded value, like the
legacy session reader. A starter whose character is missing
still aborts because legacy's foreign key cascade made that impossible.

Legacy reusable group profiles now join that same read-only plan. Ordered and
muted members, explicit or inherited persona selection, archived state, chat and
memory modes, all five speaker policies, per-member model overrides, group
prompt selections, lorebook order and policy, raw appearance, starting-scene
variants and both unresolved background locators remain intact. Director and
Director Action are stored policies and require no persisted selected speaker;
the legacy UI supplied that choice per send or continuation. Current group and
member validation rejects duplicate members, all-muted and undersized groups and
invalid selections before restore writes. References legacy never cleaned up are
pruned and recorded in the authored plan's `skipped` list instead: members whose
character is gone (legacy speaker selection skipped them), muted ids and model
overrides for non-members, overrides whose model is gone (legacy fell back), a
deleted persona (legacy's foreign key set it to null, so it inherits), stale
group lorebook ids (stale group prompt ids are kept and recorded because
sessions copied them as explicit choices), and a starting scene's missing selected variant
(legacy group prompts used only the scene content). A group with fewer than two
members or with every member muted, before or after pruning, is skipped and
recorded (`group_profile`/`undersized_group`, user decision 2026-09-13). A group
session saved before sessions linked a reusable group gets the group the legacy
v42 migration created for it: a group under the session id built from the
session's cast, mode, scene, background, lorebooks and prompts, which the
session then links to. A session linking a deleted group stays recorded as
`GroupReference`/`MissingGroup`. Group
lorebooks use the canonical ordered binding document. Group sessions remain in
the attached inventory for the later conversation/runtime conversion slice.

`legacy_model_parameters` (legacy_backup_model_settings.rs) is the single mapper
for a legacy model's `advanced_model_settings`, shared by the backup v1 planner
and the live legacy database preflight. It moves every legacy key into the typed
model settings: common sampling, reasoning, prompt caching, `forceSendThinkingState`,
Ollama and OpenRouter keys into `ChatParameterProfile`; every `llama*` key into
`LlamaCppSettings` (sampler keys into its `LlamaSamplerSettings`); every `sd*` and
`sdcpp*` key into `StableDiffusionSettings`; each of the nine
`featureGenerationSettings` slots into its `FeatureGenerationParameters` (present
fields `Set`, absent ones inherit, matching legacy `feature_model_overrides`). A
value of the wrong type or outside the legacy editor's range is left out with a
Lossy notice instead of aborting the import (legacy accepted
`ollamaRepeatPenalty: 0`, which the new profile rejects), keys legacy never defined
get an Unsupported notice, null values are ignored, and the backend-owned
`llamaLastRuntimeReport` diagnostics are dropped with a Lossy notice.

Direct sessions now carry `generation_settings.model_settings`: the session's
`advanced_model_settings` through the same mapper when it parses as an object,
otherwise the flat temperature/top_p/max_output_tokens/penalty/top_k columns
(legacy `build_session_advanced_model_settings`). Legacy read fewer fields from
a session (and from the app layer) than from a model: prompt caching and the
OpenRouter pin only from the model, image generation settings and feature slots
never, and the app layer never the thinking state, top-k, frequency and
presence penalties or reasoning; `legacy_settings_layer` leaves those out with
Lossy notices. A session's prompt cache TTL only filled in for a model whose
caching was on without a TTL; imported model caching always carries its
retention, so the session TTL is dropped too. The thinking state was model-first
in legacy, which request resolution must keep when it reads the layers. The direct conversation importer writes the layer into the conversation's
current settings; group conversations reject a model settings layer (legacy group
sessions had none).

The legacy app `settings.advanced_model_settings` goes through the same mapper
into `LegacyBackupSettingsCandidate.model_settings` (replacing the former
Unsupported notice); the settings import stage writes it as the global model
settings, and backup version 2 carries it in `BackupGlobalSettings.model_settings`.

Legacy `image_loras` (2026-09-22) is read only from the live legacy database
(archives never carried it) as the `ImageLoras` document and planned beside
the compatibility chain (`plan.images`, sealed by its content hash and
re-derived by `verify_seal`). Rows keep their paths relative to the LoRA root;
keywords are normalized like legacy read them back (trimmed, case-insensitive
unique, at most 32); a sha256 that is not 64 hex characters, unknown sources,
negative sizes and unparseable keyword JSON are replaced and recorded as
legacy-value skips. The `images` import stage writes them, keeping whichever
row of a path changed last.

Legacy chat attachments (2026-09-22; previously dropped): every persisted
attachment of direct messages, group messages and group variants
(`sessions/<owner>/<session>/<file>`) is planned as a `MessageAttachment` media
use, one use per file however many rows repeat it. Bytes are sniffed at
planning: images become `message_image`, audio `message_audio`; the legacy
filename (a generated image's prompt) becomes the asset's source label, cut to
256 scalars without control characters. Missing, unsafe, oversized or
unsupported files (legacy's raw `.webp` fallbacks, AAC/AIFF audio), inline-only
data and attachments past the reference limit are recorded, never fatal.
Conversation import appends them as `MediaAsset` attachment parts after the
text; the rendered variant also gets the message's attachments, deduplicated
by id; media on non-rendered revisions and candidates is historical.

Legacy `data:` images and session backgrounds (2026-09-22; previously
fatal or dropped): legacy stored group and group-session backgrounds (and
sometimes other images) as `data:image/...;base64,` URLs. Every such value in
an image reference or session background is decoded into in-memory
`inline/<blake3 of the URL text>` media before planning; other URLs legacy
displayed as-is (`http(s)`, `blob:`, `asset:`, `tauri:`) and undecodable or
non-image data are missing media and recorded, where they used to abort the
whole import as unsafe. A session's own background is planned as a
`ConversationBackground` use: a direct session's `background_image_path`, a
group session's `config_overrides.backgroundImagePath` (legacy resolved every
other group session value from its group, which the conversation now follows
live). A linked session reads each value from its `config_overrides`, else
from the group row (2.2.1 repair encodings included); a starting scene other
than the group's becomes `starting_scene_override` (its background dropped as
`Lossy`, since legacy group chats never showed a scene background; an
unmappable scene is `Lossy` and follows the group), a model override for a
member the group row does not list is recorded `Lossy`, session model ids go through the same canonical model id
mapping as the group, and `lorebooks_overridden` says whether the session chose
its own lorebooks. Conversation import sets the image, or `Hidden` when the value was
empty or its image could not be imported, as legacy then showed none.

Old playground history (2026-09-22; `playground_generations`, live database
only) is planned beside the LoRA library as `plan.images.playground` and
written by the `images` stage into the app's own `playground_history` /
`playground_history_images` tables as `origin = imported` entries (ids derived
from the source id, `params_json` and status kept verbatim, the old image asset
ids kept as `source_asset_id`). Each image file (`images/<assetId>.<ext>`) is
planned as a `PlaygroundImage` generated-image asset; missing, ambiguous or
unusable files and lossy `images_json` entries are recorded and the image
keeps a null asset. Playground history travels in its own v2 backup section.

Scene image protocol (2026-09-23): legacy user prompt templates of the direct
or companion purpose that copied `entry_scene_image_protocol` / `_local` get
`lettuce_context::legacy_scene_protocol_conditions`. Both the backup and the
live-database converter add the condition, so they keep legacy's ID filter. A
configured `sceneGenerationModelId` that is missing or cannot output images is
cleared and recorded, and scene generation is turned off: legacy generated no
scenes with it, where the rewrite would otherwise fall back to the first image
model.

Character cards (2026-09-23, `character_card`): Character Card V1/V2/V3 read
from JSON or a PNG's `ccv3`/`chara`/`ccv2` text chunk (tEXt, zTXt, iTXt; raw or
base64 JSON; CRCs unchecked) into a `CharacterCardDraft`, and V2/V3 written
from a `CharacterCardSource`, with the old app's structs, field order, trimming,
definition layout (`[Personality]`, `[Scenario]`, `<example_dialogue>`; system
prompt and post-history instructions dropped on import) and detection order
(UEC, V3, V2, V1, then the legacy package reader). `export_definition` strips
old prompt sections and falls back to the description; a database export
passes no embedded lorebook, a conversion may. Scene ids and materialization
belong to the import use case.

Entity packages (2026-09-23, `entity_package`, crate `unified-entity-card`
0.2.0 like the old app): every character file reads into a `CharacterPackage`
(the old app's export package, same serde layout): UEC v1, UEC v2 (read through
its v1 downgrade, v2 scene variants expanded into scenes), Character Cards (a
new scene id per greeting) or the pre-UEC package JSON. `build_character_uec`,
`build_persona_uec` (database export) and `build_persona_package_uec`
(conversion) write v2 cards (v1 built, then upgraded: other scenes merged into
the picked scene's variants, `_ID:` prompt templates, asset locators). Error
texts match the old app's. Correction: a v2 import keeps nickname, creator,
creator notes (and multilingual), source and the embedded lorebook, and a
persona keeps its nickname; the old app lost them in the downgrade. The
workspace serde_json has `float_roundtrip`, so a rare float may print a
different last digit than the old app on re-export.

Lorebook files (`lorebook_transfer`): SillyTavern World Info read and written
with the old app's layout (entries keyed "1", "2", … in lexical order; import
uses `keys`, else `key`, ignores secondary keys, skips empty content, orders by
`insertion_order`, then `displayIndex - 1`, then the map key - 1, and matches
literally) and the USC 1.0 lorebook card written (no lorebook id or match mode
per entry; the old app had no USC lorebook reader).

Chat transcripts (`chat_jsonl`): SillyTavern JSONL written and read like the
old app: a compact header (`user_name`, `character_name`, `create_date`,
`chat_metadata`, `{group: true}` for groups), then one compact line per message
with blank content left out (`send_date` in UTC RFC 3339 with milliseconds,
`swipes`/`swipe_id` for alternatives; direct chats mark the selected variant,
else the shown content, else prepend it; groups show the selected or first
variant). Reading detects the header, takes `mes|content|text|message`, times
from `send_date|createdAt|timestamp|time` (seconds below 1e10), and treats more
than one named assistant speaker as a group chat.

Character files (`character_file_plan`): a package from any character file is
planned as a new character through the backup's row mapper
(`map_character_row`, `map_lorebooks`) with fresh ids: bundled lorebooks get
new ids and literal matching, active lorebook ids are remapped to them,
references to models, prompts, lorebooks and user voices are kept only when
they exist in this app (`CharacterFileReferences`). `CharacterFilePlan::import`
turns it into one `CharacterFileImport` (lorebooks, character plan, bindings in
active-lorebook order, companion scheduled notes) once its images are stored;
an image that was not stored is left off. Scheduled notes follow legacy (trim,
empty content skipped, times clamped at 0, recurrence normalized, missing
created/updated times become now) and are kept only for companion characters,
like the backup import. Corrections: legacy's insert misaligned a placeholder
and always dropped `default_model_id` (kept when the model exists); legacy
overwrote `source` with `["lettuceai"]` (the file's source is kept); legacy
stored scene background strings raw (data URLs become assets, other strings are
dropped). `lorebook_details_from_candidate` is shared with the legacy import
writer. A companion's shared memory becomes `CharacterFileCompanionMemory`:
its memory texts (trimmed, blank or oversized ones recorded) a memory pool of
written memories the companion's chats share, its soul growth the character's
soul facts when they parse exactly, and its relationship states rows for the
default persona and for personas that exist here (others recorded); the
memory summary is recorded as lossy because a pool summary belongs to a chat,
and tool events and run status are not carried. Bundled lorebook entries
keep their packaged match mode (legacy forced literal, so a regex entry stopped
matching after a round trip).

Character export (`character_export`): `character_package` writes a stored
character as the old app's package (definition stripped of legacy card prompt
sections or else the description, active scenes and variants in order, starters
as chat templates, enabled bound lorebooks in binding order, scheduled notes,
mode and memory type always present, companion config in the legacy
`CompanionConfig` shape with `memory.sharedAcrossSessions` and `timeAwareness`,
user voices as `{source: "user", userVoiceId}`, provider voices as their stored
legacy JSON). `export_character_file` writes UEC, Chara Card V3 or V2 from it;
V1 and legacy JSON are refused with legacy's texts. Differences from legacy:
scene backgrounds are exported as data URLs instead of local image ids, crops
are written from their shortest decimal form, a default scene that is archived
is left out, empty source/tag lists are omitted, lorebook avatars are not
exported, and the deprecated character system prompt is not stored so it is
never exported. A companion's shared memory is exported from its pool (active
memories only), its character soul facts and its relationship states, without
a summary or tool events.

Memory backup: `MemoryBackup.unbound_pools` carries companion pool spaces no
conversation is bound to (a pool from a character file before its first chat,
or one whose chats were all deleted); they validate as pools and restore
before conversations with their items and projections. Corrected: such a pool
made the whole backup fail.

Soul growth (backup, legacy database and character files): each fact is
repaired the way legacy normalized it before storing (blank id, policy and
slot filled, confidence and weight clamped to 0..1, a zero evidence count
taken from its sources, zero creation and validity times set to the state's
time); only a fact that still cannot be represented is dropped and recorded.
Corrected: one bad fact used to drop the whole soul growth.

Prompt files (`prompt_transfer`): a template is written as a USC 1.0
`system_prompt_template` card (legacy field order, empty `content`, entries in
the legacy `SystemPromptEntry` shape) or as a SillyTavern prompt-manager preset
(entries with `{{scenario}}`/`{{personality}}` rewritten to `{{scene}}`/
`{{char.desc}}`, legacy position codes, the eight SillyTavern markers, one
prompt order with character id 100001, and the preset-level texts legacy wrote,
kept in `resources/sillytavern-preset.json` because they are SillyTavern's
prompts, not this app's). Reading follows the old import page: a USC card needs
a non-blank name, a prompt type this app knows (else it becomes a direct chat
prompt, as `undefined` could not run; legacy's page accepted only 17 types, the
rest are kept so this app's own cards round-trip), entries without content are
dropped, depths clamp at 0 and message/turn counts at 1, image slots keep the
five legacy slots, the scene image protocol entries get their scene protocol
gate from their legacy id, an interval entry without a turn count (legacy never
fired it) is kept disabled, and content without entries becomes one system
entry; a preset takes the prompt order block matching the
most identifiers (then the longest), skips markers and blank prompts, lets the
order's `enabled` win, marks nothing as a system prompt, drops conditions and
the chat background slot, and is named after the file. The SillyTavern preset
JSON is written with sorted keys (legacy kept insertion order).

Chat template files (`chat_template_transfer`): a starter is written as a USC
1.0 `chat_template` card or the old app's `{version: 1, kind: "chat_template",
template}` JSON; reading accepts a USC card, that JSON or a bare template
object with a non-blank name and keeps user and assistant messages with string
content. References are resolved by the caller.

Model files (`model_transfer`): a model profile is written as the old app's
model JSON or a USC 1.0 `model_profile` card, with `advancedModelSettings`
rebuilt by `legacy_advanced_model_settings`, the reverse of
`legacy_model_parameters` for every value legacy could hold (reading it back
gives the same settings; rewrite-only values such as split KV types, cleared
feature overrides and feature-level context/reasoning/caching/Ollama resource
overrides have no legacy key and are not written; a pinned OpenRouter provider
is written with its id as its name, which legacy required). A reasoning
budget under 1024 or a repeat penalty over 2.0, which the domain allows but
legacy's editor did not, is written and reads back as lossy under the legacy
rules. A model file's `llamaLastRuntimeReport` is ignored like legacy's upsert
ignored it. The 2.2.5 DFlash keys (`llamaDflashEnabled`,
`llamaDflashDraftTokens` 1-15, `llamaDflashMinProbability` 0-1,
`llamaDflashModelPath`) read and write like the MTP ones. Reading takes a USC
card or a model object: name and provider id required and trimmed, label and
display name default to them, scopes keep text/image/audio in that order
(text when none), and advanced settings are taken when they are an object.
Legacy's model prompt template and deprecated system prompt are not carried
(the rewrite's models have no prompt).

Sprout: an old Ollama credential's `sproutEnabled`/`sproutUrl`/`sproutApiKey`
config keys become its `ProviderConfig::Ollama` Sprout settings in both the
legacy database and the legacy backup paths, and the plain-text key becomes a
`SproutApiKey` secret (pending source kind `provider_sprout_api_key`); a URL
that is not http(s) is left out with a lossy notice. Provider backups list and
rebind the Sprout secret with the account's other secrets.

Legacy llama sampler orders import as the old runtime read them: names are
trimmed and lowercased, aliases accepted (`topk`, `typ_p`, `temperature`,
`adaptive`, ...), unknown entries skipped and repeats dropped. Before, one
unknown entry dropped the whole order, which lost every order saved by 2.2.5
(whose default includes `adaptive_p`). `llamaAdaptiveTarget`,
`llamaAdaptiveDecay` (also in feature slots) and `forceGemma4Reasoning` map to
typed settings.

Legacy `llm_generation_metrics` (2026-09-24) is read only from the live legacy
database (archives never carried it; old backups have no metrics to import) as
the `LlmGenerationMetrics` document, and planned beside the compatibility chain
(`plan.llm_metrics`, sealed by its content hash and re-derived by
`verify_seal`). Installs without the table import no metrics and installs from
before the `message_id` column no message links. Each row keeps its id,
`created_at`, `model_name` (as the model path) and legacy message id; a
summary that is not a JSON object becomes `{}` and samples that are not an
array `[]` (how legacy read them back), recorded as legacy-value skips, as are
rows without a usable id or integer timestamp. Only the newest 500 by
`created_at` are kept, as both apps retain; dropping older rows is a `Lossy`
notice. The `llm_metrics` import stage receives each row with its imported
message id (`LegacyIdScope::source` of the legacy direct or group message id).
