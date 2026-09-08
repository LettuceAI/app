# lettuce-transfer

Bounded external-format parsing, canonical documents, import plans, exports,
consistent encrypted backups, staged restore, and rollback.

## Boundary

Compatibility transfer and full backup remain separate internal modules.
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
