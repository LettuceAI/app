# lettuce-transfer

Everything that moves data in or out of the app: the encrypted full backup and its restore, import from the legacy app (a legacy database or a legacy backup archive), and the single-item file formats (character cards and packages, lorebooks, chat transcripts, prompts, chat templates, model files).

The crate reads, validates and plans; it does not own live state. SQLite reads and writes are in `lettuce-database` (backup source, restore writer, legacy preflight and import adapter), the media store in `lettuce-media`, secrets in `lettuce-settings`, and orchestration, jobs and commands in `lettuce-app`. Formats are defined here against the domain types of the other crates, and every legacy or third-party format is parsed into its own compatibility types first.

## Principles

- External schemas never become domain schemas. A legacy or third-party document is decoded into a compatibility plan and only then mapped onto domain types through the domains' own validation.
- Nothing destructive happens before everything is verified. Decoding, planning and staging are read-only toward live data; a restore writes a fresh database and keeps the old one; a legacy database or backup is a read-only source that is kept until migration has been verified and the user approves deleting it.
- No record count is capped anywhere in export, validation, restore or import. Only bytes are bounded, far above any real library, as malformed-input guards.
- Secrets travel only inside authenticated encrypted sections and never appear in public metadata, logs or receipts.
- Every step that can be retried is idempotent: sealed plans, assignments and receipts make an exact retry replay and a changed retry conflict.

## Modules

| Module | Contents |
| --- | --- |
| `backup/backup.rs` | The version 2 envelope: `BackupWriter`, `BackupReader`, `verify_backup_frame`, `BackupArchiveStore` |
| `backup/backup_graph.rs` | `ProviderBackupGraph`, export planning, validation, secret rebinding |
| `backup/*_backup.rs` | One document per runtime domain (conversations, runtime, outbox, jobs, usage, companion state and effects, memory, projections, dynamic memory, playground) |
| `restore/` | `decode_provider_backup_restore_plan`, the staging workspace, restore admission |
| `legacy/` | Legacy backup decoding, the compatibility planners and seal, legacy import contracts |
| `lib.rs` | Legacy import types shared by the database and backup paths, the ASR learning documents |
| `files/` | Single-item file formats |

## Backup format

A version 2 backup is one streamed file:

1. A plaintext header with only the magic and the Argon2id parameters and salt. New backups use RFC 9106's second recommended setting (64 MiB, t=3, p=4); readers accept a bounded range so later increases stay readable.
2. Every section encrypted with XChaCha20-Poly1305 in 1 MiB chunks: a random 19-byte nonce prefix per section, then a chunk counter and a last-chunk flag. The associated data binds the header hash and the section index.
3. The encrypted manifest: section names, schemas, sizes and BLAKE3 hashes.
4. A plaintext footer with the manifest's nonce and length.

Nothing about the content is visible without the password, including section names, which for media contain content hashes. Reordered, swapped, truncated or extended data, duplicate or unsafe names and a repeated nonce prefix all reject the backup. `BackupWriter` appends sections from any reader, and `BackupReader` opens a seekable source and decrypts one section at a time into memory or a sink, so no section has to be held whole. `verify_backup_frame` checks the plaintext frame without the password, and `detect_backup_format` tells a version 2 file from a legacy ZIP (version 1); anything else is rejected rather than guessed.

Limits: 2 GiB per section and 1 TiB in total. A data document larger than `BACKUP_DATA_PART_BYTES` (1 GiB) continues in `<name>.part<N>` sections that restore joins before decoding, so the per-section limit never limits the library. The envelope refuses more than 2 097 152 entries (media blobs plus artifacts) or a manifest over 1 GiB. The whole graph is still built in memory during export and restore.

`BackupArchiveStore` receives a backup into confined resumable partial files and commits it to a new path only after the expected hash (computed by streaming) and the frame verify. An identical existing backup is reused and never replaced. The format has no compression and no archive paths, so there is no traversal or decompression bomb to defend against.

## What a backup contains

A backup is complete: it holds every user-owned domain, every referenced media object and every portable secret needed to recreate the profile.

| Section | Contents |
| --- | --- |
| `data/provider-graph.json` | `ProviderBackupGraph`: global settings (including the app-wide model settings layer), provider accounts, model profiles, prompt documents, feature selections, audio providers and user voices; the authored profile (personas and the default, lorebooks with entries, characters with scenes, variants and starters, groups with members and starting scene, the three lorebook binding lists, media assets and blob metadata); creation-helper and lorebook generation records; legacy import audit records; playground history; device state (trusted certificates, embedding choice, app usage days) |
| `secrets/provider-secrets.json` | Provider API keys and secret headers, audio API keys, Sprout keys, and the app-wide tokens, keyed by reference |
| `data/asr-learning.json` | The ASR learning document, version 3 |
| `data/conversation-history.json` | Direct and group conversations: participants, policy, settings, branches and heads, messages with timeline ordinals, revisions, candidates, origins, media references |
| `data/conversation-runtime.json` | Generation turns, attempts, stage and progress checkpoints, speaker and dispatch checkpoints, preparation attribution, tool executions |
| `data/jobs.json` | Job specs, snapshots and event histories, job dispatch usage and job cost bases |
| `data/conversation-usage.json` | Terminal usage events, counters, cost bases, and the ids of overlapping job dispatches |
| `data/conversation-outbox.json` | Operation records and the per-conversation event journal |
| `data/companion-state.json` | Relationship and emotional state, signals, continuity episodes, replacement receipts |
| `data/companion-effects.json` | Turn effects and suffix-rewind receipts |
| `data/memory.json` | Memory spaces, items, summaries, retrieval receipts, and pools no conversation is bound to |
| `data/memory-projections.json` | Stored embedding projections, ready and repair-needed |
| `data/dynamic-memory.json` | Dynamic memory approvals, runs, attempts, rounds, results and summary checkpoints |
| `media/blobs/<hash>` | One section per ready media blob |
| `conversation/snapshots/<id>`, `conversation/replays/<id>` | Protected conversation snapshots and provider replay artifacts |

Every document is versioned and validated on its own and against the others. Selections and ownership links must resolve inside the graph (models, prompts, voices, lorebooks, media, participants, messages, attempts, jobs, memory spaces). Each document keeps exact ids, revisions, timestamps, receipts and storage-only hashes, so idempotency survives a restore. A few rules worth knowing:

- Media: every asset and blob record is included once, even when shared by several roots, and each ready blob is stored once in its own content-addressed section. Non-ready blobs keep their metadata with no invented content.
- Artifacts: every protected snapshot and provider replay artifact the conversation graph references is included once; its descriptor, size and BLAKE3 must match the graph.
- Usage: conversation usage events and job dispatch evidence are separate audit trails. The usage document records which dispatches a terminal event overlaps with, so reports never count both.
- Outbox: operations without an event are kept, because checkpoint and preparation replay depend on them. There is no delivery or lease state to export.
- Memory: historical memory ids may be absent after an authorized hard delete; the backup does not invent tombstones. Projections keep ready vectors as hex of their exact little-endian SQLite bytes and include stale rows, since retrieval already excludes those.
- In-flight work: `settle_in_flight_generation` settles generation that was running when the backup was taken, the way a restore would see it: a turn that never started an attempt is left out, unfinished attempts are interrupted with an interrupted usage event, unfinished turns become interrupted, running tools are interrupted and requested or validated tools are cancelled.

Export performs no provider, tool, embedding or memory work and does not touch similarity or pricing.

## Export

1. `lettuce-database` reads one consistent `ProviderBackupGraph` (`ProviderBackupSource`), and `lettuce-app` supplies the exact scoped secret inventory: every provider credential and audio key the graph references, derived from its opaque references.
2. `plan_provider_backup_export` validates the graph (`canonicalize_and_validate`) and returns the data and secret sections, then the ready media blobs and conversation artifacts in order.
3. The application streams each blob and artifact into the writer, checking it first with `verify_backup_media` or `verify_backup_artifact` against its recorded size and hash, and drops it before loading the next. Missing, shortened, changed or oversized content rejects the export. `pin_media_objects` in `lettuce-media` keeps the named objects from being deleted meanwhile.

## Restore

1. Decode. `decode_provider_backup_restore_plan` takes any seekable `BackupSource`, hashes it by streaming, authenticates the envelope, requires the exact fixed section names and schemas, decodes every document, validates the whole cross-domain graph and inventories every expected secret, media blob and artifact. Media stays in the archive as `BackupMediaEntry` (hash, size, section) and is read one at a time with `read_media`. Anything missing, duplicate, unknown, malformed or mismatched rejects the plan before any database, secret store or file is touched. A legacy ZIP goes to the version 1 decoder instead.
2. Stage. The payloads are written into an isolated restore workspace (`restore_workspace.rs`) with confined no-follow paths, exact-prefix resume and no-replace commits; every blob and artifact is re-read and checked for size and BLAKE3. Media ciphertext is authenticated as it is staged, so a damaged blob fails here, and installation later reads the verified staged copy (`read_staged_media`), decrypting each blob once. A versioned receipt binds the staged inventory to the source archive; exact retries replay and divergent bytes fail. Secret values never enter the workspace. Live database, secret and media roots are not opened.
3. Admit. `current_backup_restore_admission` (and `legacy_backup_restore_admission` for version 1) builds one validated admission request binding the source version and hash, the plan fingerprint (derived from the source hash for a version 2 plan, from the compatibility seal for a legacy one), the staging receipt fingerprint and the document, media, secret and artifact counts. `BackupRestoreAdmissionRepository` records it; it carries no secret value or live payload. Admission does not re-read the workspace, so cutover verifies staged bytes itself.
4. Rebind secrets. `rebind_provider_backup_secrets` moves every provider key, secret header, Sprout key and audio key reference to a fresh `SecretRef` and rebinds the decoded values to it, so the restore writes new store entries instead of overwriting secrets the previous database still references. A reference the backup listed as missing gets a fresh reference with no value. App tokens keep their fixed references.
5. Write. `ProviderBackupRestoreWriter` writes the graph into an empty database exactly as it was exported, keeping ids, revisions, timestamps and states; a target that holds data is refused. The application then cuts over to the new database file and keeps the previous one.

## Legacy import

The legacy app's data arrives from two sources that share one pipeline: a live legacy SQLite database (schema versions 92 to 96, `LEGACY_DATABASE_SCHEMA_VERSIONS`) and a legacy backup archive (version 1, an encrypted ZIP).

### Reading the sources

- Database. `lettuce-database` runs a read-only preflight that reports the schema version and counts of importable roots, and inventories media files by path. `LegacyBackupMedia::from_file` hashes a file by streaming; its bytes are read on demand and re-checked against size and hash, and a file that changed after the inventory fails with `SourceChanged`.
- Archive. `decode_legacy_backup_inventory` opens a version 1 archive from a seekable source. It reproduces the legacy password KDF and XChaCha usage only to authenticate existing archives (new output never uses them), checks entry names before decrypting anything, and returns bounded zeroizing bytes for the known JSON documents and media roots. Each media entry is decrypted once for its size and hash and again when read. Duplicate or unsafe names, plaintext payloads, unknown entries, unsupported manifest versions, bad parameters, wrong passwords and oversized archives reject before anything is converted. It works on bytes, so desktop files and Android `content://` streams share one path.

### Planning

`plan_legacy_backup_compatibility` runs the planners in dependency order over the decoded documents: configuration (settings, providers, models, prompts, audio providers and voices, credentials, chat templates), authored profiles (personas, lorebooks, characters, groups, bindings), media, ASR learning, usage history, pricing cache, direct and group sessions, scheduled notes, companion shared memory, memory embeddings and creation-helper sessions. The database path feeds the same planners and additionally plans the LoRA library, playground history and LLM generation metrics, which only live databases had.

The result is sealed once: the seal lists all 23 known document kinds as present or absent, inventories every retained media object (root, path segments, size, BLAKE3), and fingerprints that coverage, the source identity and every sorted conversion notice. The seal is a reconciliation boundary only; it admits nothing and writes nothing.

Conversion rules are the same everywhere:

- A value legacy read leniently is read the same way. Malformed JSON falls back exactly as legacy did (`lenient_legacy_json`) and is recorded as a `legacy_value` skip.
- A reference legacy never cleaned up after a delete is cleared or pruned and recorded in the plan's `skipped` list, with a typed `LegacyImportSkipKind` and reason, instead of failing the import. A reference the rewrite would refuse to launch with (a prompt of the wrong purpose, a non-chat model as a chat default) is cleared as `IncompatibleReference`.
- A state legacy's own constraints made impossible (for example a starter without its character) still rejects.
- Anything absent, unsupported or lossy becomes an explicit notice. Nothing that legacy rows cannot supply (generation attempts, protected snapshots, memory-space revisions) is fabricated; it is reported absent.
- The complete decrypted source inventory stays attached to the plan, so later stages can recover fields that have no canonical owner yet without reopening the source.

Shared mappers keep the two paths identical: `legacy_model_parameters` maps a legacy model's `advanced_model_settings` (also the app layer and session layers) onto typed model settings, `legacy_prompt_purpose` is the legacy prompt type table, and `reconcile_legacy_persona_lorebooks` and `reconcile_legacy_lorebook_keywords` prune persona lorebook ids and uncompilable regex keywords.

### Admission and materialization

`LegacyImportRepository` (implemented in `lettuce-database`) runs an import as a durable run:

1. Admit. The run binds the source schema, an inventory fingerprint and the plan fingerprint, and assigns stable destination ids to everything it will create: accounts and their secret owners, profiles, prompts, personas, lorebooks and entries, characters, groups, ASR rows, conversations, logical media assets, and an opaque `SecretRef` per pending API key or header. Exact retries replay the assignments; any change in source ids or plan content conflicts before a destination write.
2. Media. Each planned media use is ingested through `lettuce-media` and recorded as an immutable completion (asset, ready blob, size, hash, time). Media from a live database is read by path; media from an archive is staged through the same restore workspace as current backups, with a legacy receipt binding the archive hash and seal to each original path, size and hash.
3. Secrets. `LegacyProviderSecretSource` yields temporary zeroizing values only for the sealed assignments. A completion receipt holds the destination reference, store generation and time, never plaintext or a digest. A retry verifies an already present value, so a crash between the store write and the receipt does not rotate it.
4. Materialize. Each domain has its own stage and receipt: authored graph, ASR, providers and models (only after every assigned secret is complete), characters, groups, audio, settings, direct and group conversations, usage records, images, creation helper, LLM metrics. Every stage accepts only the sealed plan, maps source ids to their assigned destinations and replays without duplicating rows.

For a version 1 backup, the plan itself serves as the source: `LegacyBackupConfigurationPlan` implements `LegacyProviderSecretSource`, `LegacyBackupCompatibilityPlan::media_bytes` returns retained media by archive path, and `database_inventory` derives the counts admission validates, so a backup runs the same import chain as a live database. Cleanup of the legacy source is never part of admission or execution.

`legacy_message_conflicts` turns unresolved legacy sync conflicts on messages into forks: every recorded side that differs from what the legacy message shows becomes a branch. A conflict on a chat's first message or on a message the import does not hold is returned as a skip.

## File formats

`files/` reads and writes single items:

- `character_card`: Character Card V1, V2 and V3 from JSON or from a PNG's `ccv3`, `chara` or `ccv2` text chunk (tEXt, zTXt, iTXt; raw or base64 JSON), into a `CharacterCardDraft`; V2 and V3 written from a `CharacterCardSource`. Detection order is UEC, V3, V2, V1, then the legacy package reader.
- `entity_package`: every character file reads into a `CharacterPackage`: UEC v1, UEC v2 (through its v1 downgrade, with v2 scene variants expanded into scenes), Character Cards (a new scene per greeting) or the pre-UEC package JSON. `build_character_uec`, `build_persona_uec` and `build_persona_package_uec` write UEC v2 cards (built as v1, then upgraded) through the `unified-entity-card` crate.
- `character_file_plan`: plans a package as a new character with fresh ids through the backup row mapper (`map_character_row`, `map_lorebooks`); bundled lorebooks get new ids, and references to models, prompts, lorebooks and user voices are kept only when they exist here (`CharacterFileReferences`). `CharacterFilePlan::import` produces one `CharacterFileImport` (lorebooks, character plan, bindings in active-lorebook order, companion scheduled notes, companion shared memory) once its images are stored; an image that was not stored is left off.
- `character_export`: `character_package` writes a stored character as a package (definition, active scenes and variants, starters as chat templates, enabled bound lorebooks in binding order, scheduled notes, companion config and shared memory, voices), and `export_character_file` writes UEC, Chara Card V3 or V2 from it.
- `lorebook_transfer`: SillyTavern World Info read and written, and USC 1.0 lorebook cards written.
- `chat_jsonl`: SillyTavern JSONL chat transcripts read and written for direct and group chats.
- `prompt_transfer`: prompt templates as USC 1.0 `system_prompt_template` cards or SillyTavern prompt-manager presets (the preset-level texts SillyTavern expects are in `resources/sillytavern-preset.json`, since they are SillyTavern's prompts rather than this app's).
- `chat_template_transfer`: starters as USC 1.0 `chat_template` cards or the legacy chat template JSON; references are resolved by the caller.
- `model_transfer`: model profiles as the legacy model JSON or a USC 1.0 `model_profile` card, with `legacy_advanced_model_settings` as the inverse of `legacy_model_parameters` for every value legacy could hold.

The ASR learning document (version 3) is the interchange format for vocabulary, corrections, ignored suggestions and voice examples. It has no row limit and carries each voice example's audio as an asset description (kind, origin, redacted provenance, hash, size, MIME type, duration) instead of a path; the application validates and remaps the whole graph before an atomic import. The legacy version 2 ASR JSON is a separate camelCase compatibility document.
