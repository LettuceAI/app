# lettuce-database: legacy parity notes

Facts about how `lettuce-database` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison, the recorded decisions and the history of the old README.

## Legacy parity

- Memory items keep legacy `turn` precision beside their source-message attribution.
- Retrieval access applies the legacy cold promotion and access-count updates.
- Scheduled notes keep the legacy recurrence, availability, expiry, enabled and recurrence-window fields and list in legacy `available_at`, id order.
- Companion turn-effect seeds are the legacy seed, normalized; continuations use the legacy zero-delta seed with a nullable user-message reference.
- Companion clock stamping covers both sides of legacy's `is_companion_mode` (session mode or character mode), as legacy stamped `effective_at`; a regenerated reply keeps its original time like legacy.
- The companion continuity sequence is legacy's character and persona sequence.
- Companion pools and own spaces are legacy's two stores; the active space is read on every use like legacy `resolve_effective_memory_owner`.
- A summary saved by a run whose tools phase failed does not advance the owner's cursor, reproducing the legacy tool-error branch in `flow.rs` (it saved the summary while `event_advances_cursor` ignored the error event).
- Suffix rewinds undo tool results like legacy `replay_memory_state_after_rewind` over the effective owner's memories.
- ASR queries reproduce the legacy scope and language filters and runtime order; legacy ASR row transfer goes through the same validated records, keeps every counter and resolves managed audio assets, so native paths are never copied into live records.
- `set_default_prompt_document` stores legacy `settings.prompt_template_id`.
- Local generation metrics readers are the old `llm_metrics_*` commands; the list keeps legacy's newest 500. The old frontend's `llm_metrics_attach_message` has nothing left to do, because a local generation records its metrics under its attempt id.
- Legacy stored each message's (and variant's) time to first token, tokens per second and MTP stats on the message, where they outlived the 500-row list and its clear; `message_stats_only` rows keep that behaviour.
- `llama_runtime_reports` is legacy `llamaLastRuntimeReport`, now on the newest llama.cpp model profile for the file instead of inside the synced model config, and deleted with the model.
- Hard delete follows legacy `session_delete` and `group_session_delete` (old-code `storage_manager/sessions.rs:3794`, `group_sessions.rs:1962`) and legacy `character_delete` (`characters.rs:1066`, pool at `characters.rs:1081`). Legacy left a deleted character's id in the group's list and its reads skipped it; purge removes the member instead.
- A LoRA path in both files keeps the more recently updated row, the rule the legacy images stage uses.

## Legacy import details

- Accepted source versions are `LEGACY_DATABASE_SCHEMA_VERSIONS`, 92 to 96: the released 2.2.0 stored 92 or 94, 2.2.1 stored 95 and 2.2.2 to 2.2.5 stored 96. 93 and 94 only repaired group columns, 95 rewrote group session override encodings (the importer reads both forms) and 96 reordered columns and renamed leftover tables. Older databases are refused (the old app migrates them when opened once), and pre-SQLite `.bin` stores are not read (legacy imported them itself on startup).
- Memory embedding copies in the legacy documents use one canonicalization for session, group, companion and owner documents: normalized rows in the legacy `MemoryEmbedding` field order and f32 text, falling back to the legacy column only when no rows exist.
- Legacy model rows always carry `model_type = 'chat'` once saved by the modern editor, so a model whose output scopes are image-only plans as an image model; text-and-image outputs stay chat models with image output in their capabilities, and the old `imagegeneration` type still maps to image.
- Lorebook planning validates exact legacy enum and boolean encodings and rejects malformed keywords, timestamps and orphan entries. External world-info fields the legacy importer already discarded are not invented as database fields.
- Provider and model planning applies the legacy credential resolution order, validates defaults and parent bindings, maps custom protocol configuration and model scopes and settings into typed candidates, and enforces separate account and profile bounds. Unmapped field names identify later embedded-runtime, image and feature-policy work without copying their values. File-backed tests compare the database bytes before and after planning.
- The version-92 plaintext `api_key` column is authoritative because migration 7 of the old app backfilled it from the former secrets table. The obsolete `api_key_ref` is not treated as secret evidence or copied. Legacy llama.cpp models keep their runtime behaviour through a deterministic secret-free built-in account instead of becoming orphans.
- A dangling legacy app prompt selection stays sealed evidence and falls back instead of inventing a target; a stale prompt reference resolves to the app default, as legacy did. Model-level prompt references, the deprecated character system prompt and unsupported field names stay sealed in the plan fingerprint and the retained source.
- ASR admission assigns destination UUIDs to all four legacy row kinds; a later transaction writes the live ASR rows only after each voice asset has an immutable media completion, and its result receipt replays exactly.
- The characters stage writes profile, scenario, rules, provenance, defaults, presentation, image recommendation and media links; scenes with deterministic background links, variants and starters; companion Soul state; character lorebook bindings. An already selected default persona, model or prompt is kept instead of aborting. The persona stage checks only persona and lorebook media uses, because the shared media plan also carries character and group media.
- The groups stage keeps legacy group, scene and variant ids, remaps explicit persona and member model overrides (the planner already cleared missing or non-chat ones), remaps group prompts (a missing one becomes the app default) and backgrounds, and writes group lorebook bindings.
- The audio stage needs no earlier stage (characters reference voice profiles without a foreign key); a provider keeps its deterministic API key reference.
- The settings stage rewrites the `app_settings` payload with the legacy global settings, remaps the lorebook-generator, dynamic-memory and help-me-reply selections and the dynamic-memory and group-speaker model columns, keeps the default model and prompt from the provider stage, and keeps settings an earlier run already imported. The revision advances once.
- Legacy usage records are written unchanged into the immutable `legacy_usage_records` table, keyed by run and source id and linked to the imported model when it exists. They are historical rows, not `usage_events`.
- The creation helper stage leaves out a legacy draft that exceeds the rewrite's creation limits instead of aborting.
- The `llm_metrics` stage runs after both conversation stages; rows keep their legacy id, timestamp, summary and samples (`model_name` becomes `model_path`), except that the newest row attached to each imported message is stored under the attempt id of that message's selected candidate. A message imported without candidates, a link to a message not imported, or an attempt id another row holds leaves the legacy id and the row loses its message link; a speed-stats row does not count as held, and the full legacy row replaces it. The per-message stats columns (`first_token_ms`, `tokens_per_second`, `mtp_stats` on `messages`, `message_variants`, `group_messages`, `group_message_variants`, added by legacy migrations v69-v70 and v75-v76) become `message_stats_only` rows with the runtime's summary keys.
- `stage_receipt` lets a caller return a committed receipt before rebuilding stage input; it checks the run's stored plan and source fingerprints.
- A run admitted from the shared legacy backup planner seals that plan's fingerprint as its `source_fingerprint`, and seals the skips of the domains later stages write together with the admitted sub-plan skips.
- Legacy message conflicts: `legacy_message_conflicts` decodes the recorded bincode row snapshots of an unresolved conflict on a `messages` or `group_messages` row and takes every side whose content differs from the imported message; `fork_legacy_message_conflicts` copies the message with each such content into a branch from its parent and flags it for the user. A conflict on a chat's first message, or on a message the import does not hold, is recorded at planning as a `message_conflict` skip. A fork that fails to write is rolled back alone, counted and logged, and the row stays in provenance. Conflicts on other tables stay only in provenance. A legacy companion effect whose assistant message was written with a generation turn becomes a companion effect record (newest turn of that message, memory changes limited to imported memories).

### Skips

- A settings default provider or model that no longer exists (legacy `provider_delete` and `model_delete` never cleared them and the old app ignored a stale id), and a settings default prompt that no longer exists (recorded as a prompt reference; legacy fell back to the built-in default, while the execution step used to drop it silently).
- A model whose provider credential no longer resolves, recorded as a `model_profile` skip; references to it are cleared like any stale model reference.
- Media references whose file is gone: a persona avatar with its crop, each persona design reference and a lorebook avatar. The old app showed no image for them. The count is not limited; unsafe or ambiguous references still abort.
- `reconcile_legacy_persona_lorebooks` prunes persona bindings to lorebooks absent from the plan and drops repeated ids (legacy kept a deleted lorebook's id in `activeLorebookIds`, its editor listed only existing lorebooks and chats resolved it to no entries; repeats counted once). Admission rejects an unreconciled or repeated binding.
- `reconcile_legacy_lorebook_keywords` prunes a regex keyword that does not compile in the form the matcher uses, because legacy never matched it. Admission rejects a plan that still holds one.
- `legacy_value` fallbacks: an unknown prompt type (Undefined, then direct chat; types are read through the legacy prompt store's own table, so its snake_case lorebook aliases keep their purpose and runtime text is unknown); prompt entries that are not a JSON array (empty, then the content entry; a valid array the new types cannot read still aborts); malformed persona design reference or lorebook id lists (empty; a non-UUID lorebook id counts as a missing binding); an unknown lorebook detection mode (recent message window); malformed entry keyword JSON (empty); an unknown keyword match mode (literal).
- Provider `config` and `headers` and model `input_scopes`, `output_scopes` and `advanced_model_settings` that are not valid JSON fall back like the legacy settings reader, which omitted them (empty config, no headers, text-only scopes, empty advanced settings); JSON `null` counts as absent. The same fallback applies to the wrong shape (non-object config, headers or advanced settings, non-text header values). A header repeated with the same name keeps its last value like legacy's JSON parsing. Unknown scope names, invalid advanced setting values and header names or values the new types reject still abort.

## Deliberate differences from legacy

- Wrong-shaped provider or model JSON is recorded as a skip and the record kept. Legacy's frontend salvage dropped the whole provider (with every model on it) or model, and wrong headers or advanced settings also failed its typed Rust settings read.
- A deleted character is removed from its groups instead of leaving a dangling id.
- Legacy `companion_turn_effects` and `sync_v2_conflicts` rows are preserved verbatim in the run's provenance instead of being lost.

## Decisions

- 2026-09-13: models of a deleted provider are not imported.
- 2026-09-14: work in progress when a backup was taken is restored as interrupted. A companion character has one shared memory pool.
- 2026-09-23: accepted legacy schema versions are 92 to 96. Each companion conversation also keeps its own space (legacy's two stores). The share-Soul-growth transitions.
- 2026-09-25: after any restore, v2 or legacy, the database joins sync as a new device; the sync identity, journal, frontiers and conflicts are not carried. Legacy `companion_turn_effects` and `sync_v2_conflicts` rows are kept in `legacy_import_preserved_rows`.
- Any future step that deletes legacy data must require a `completed` run together with explicit user approval; the legacy source database and storage tree are retained until separate verification and user approval.

## History

- Restore used to give an unfinished attempt a usage id without an event, so a restored database with such work could not be backed up again.
- Job pruning used to delete terminal jobs a speech transcription or synthesis still bound; one such job made every later prune fail.
- The previous README said the database exposed only an internal create/read slice while the conversation repository mutation port was still being implemented. `Database` implements `ConversationRepository` in `conversation/conversation_mutations.rs`, and the helpers in `conversation_vertical_slice.rs` are shared by the adapters.
- The previous README said "the future full repository must validate projection equality on every write and hydrate". The mutation kernel does: `read_operation_any` checks the stored projection columns against the decoded reference.
- The previous README listed sessions, conversation assembly, starter-link normalization, import and export, hard purge, sync and backup, FTS and legacy text-column retrofits as outside the conversation slice. Import, export, purge, sync and backup exist now; there is still no FTS, and existing prompt and lorebook text columns are not silently migrated.
- The previous README said a legacy run whose planned stages all finish ends as `partial` because characters, sessions, groups, memories, companion tables, usage, audio and settings were not imported by any stage. All ten stages exist (`LegacyImportStage::ALL`), and `complete_legacy_import_run` moves a run to `completed` once every stage has a result row.
- The previous README said the history writer rejected failed or cancelled turns without a candidate, attributed turns and provider replay artifacts. Restore writes all of them through the writer's exact mode.
- The previous README said persona mutations are the journal call sites and that network sessions, incoming apply and conflict handling were later work. The state scan now journals every synced entity; see lettuce-sync.
- The previous README described what `carry_device_local_state_from` copies as Whisper manifests and discovered voices only. It also carries the models folder, LoRA library, generation metrics, install state, device settings, usage days and llama.cpp reports.
- The previous README's per-migration notes described the file-backed scenario tests (lifecycle preparation, usage cost replay, ASR CRUD, OpenRouter evidence, job-result replay of old bytes without a response id). Those tests remain in the crate; job-result replay compares decoded values, so an older response without the optional id is an exact replay of an absent-id response.

## Not wired yet

- Outbox consumer leases and acknowledgements: migration 8 stores no lease table, so backups do not invent delivery state.
- The single in-flight turn rule is enforced in the adapter; its supporting index is deferred.
