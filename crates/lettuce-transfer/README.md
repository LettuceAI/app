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
