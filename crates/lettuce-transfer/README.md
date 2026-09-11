# lettuce-transfer

Bounded external-format parsing, canonical documents, import plans, exports,
consistent encrypted backups, staged restore, and rollback.

## Boundary

Compatibility transfer and full backup remain separate internal modules.

Backup format version 2 seals a bounded, versioned whole-profile envelope with
Argon2id-derived keys and independently generated XChaCha20-Poly1305 nonces for
every section. The manifest binds each safe logical entry name, schema,
plaintext size and BLAKE3 content hash into the authenticated data. Inspection
is deterministic and exposes metadata only; opening returns no section unless
the password, complete manifest and every encrypted section authenticate and
match their declared size and hash. Duplicate or unsafe names, duplicate
nonces, oversized inventories, truncation and trailing data reject the whole
envelope.

Backup reception uses confined resumable partial files and commits to a new
path only after the complete envelope and its expected hash verify. An existing
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

`data/dynamic-memory.json` preserves the restart graph for both visible-turn
preparation and background extraction. It contains ask-first approval state,
exact immutable preparation documents and digests, frozen runs and message
sources, retry attempts, ordered inference rounds and calls, background results,
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
are hashed once and retain every typed owner and ordinal use. Missing,
ambiguous, unsafe, oversized or over-limit references reject the whole media
plan. Conversation attachments, generated images and other later-domain media
stay attached to the source inventory and are not misclassified as orphaned by
this authored-only slice.

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
The planner validates authored references, root/parent topology, message
ancestry, selected-variant ownership, JSON shapes, counters and finite sampling
values before any write. Equal message timestamps and unordered legacy variant
queries are reported as ordering loss. Legacy rows contain no current
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
planner rejects duplicate identities, orphaned group/member/model/prompt/
lorebook/branch links, message cycles, invalid selected variants and malformed
nested scene, usage, MTP or attachment JSON. Participation and variant queries
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
All model, prompt, audio-voice, chat-character, scene and lorebook links are
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
notices, and malformed or orphaned graphs reject before any mutation.

Legacy reusable group profiles now join that same read-only plan. Ordered and
muted members, explicit or inherited persona selection, archived state, chat and
memory modes, all five speaker policies, per-member model overrides, group
prompt selections, lorebook order and policy, raw appearance, starting-scene
variants and both unresolved background locators remain intact. Director and
Director Action are stored policies and require no persisted selected speaker;
the legacy UI supplied that choice per send or continuation. Current group and
member validation rejects duplicate or missing members, all-muted and undersized
groups, orphan overrides and invalid selections before restore writes. Group
lorebooks use the canonical ordered binding document. Group sessions remain in
the attached inventory for the later conversation/runtime conversion slice.
