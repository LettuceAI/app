# lettuce-app: legacy parity notes

Facts about how `lettuce-app` relates to the legacy app (2.2.x): what follows legacy and where, where it deliberately differs, the decisions behind it, what the old README said that has changed, and the host wiring still missing. The crate README describes the current design.

## Legacy parity

### Launch and prompts

- Prompt launch follows legacy `build_system_prompt_entries`. Legacy accepted an app-wide template in companion mode only when its type was companion chat. A starter's prompt is ignored for companions as in release 2.2.5 `221fe1aa` (the companion template wins over any session template).
- An archived inherited direct prompt falls through to the app default like legacy's not-found fallback.
- A direct chat resolves its chat model live like legacy `select_model_with_credential`.
- The non-companion prompt order is legacy's. The launch pin reproduces how legacy pinned `session.promptTemplateId` at creation (`repo.ts` 1586-1609); a chat whose launch fell back to the app default chain is legacy's null session template. Branches keep the pin as legacy's branch copy did; the rewrite has no branch-to-another-character flow.
- The legacy direct importer pins only what legacy pinned: an imported session template becomes the current override, and a null or unimported one gets a disabled launch prompt, which means "no selection", distinct from a disabled current-settings prompt, which means no prompt.
- Legacy set the session template from a chat template whose picker (`ChatTemplateEditorPage.tsx` 276-281) offered direct and group templates and rendered it whatever its type, so a group prompt selected by a one-to-one chat renders there; render values its purpose does not admit (scene LoRA keywords) are left out like legacy's placeholder replacement. Feature prompts (memory, reply helper, lorebook, image) could never be a session template.
- Lorebooks are read live on every turn the way legacy re-read them (`prompt_engine.rs` 2309-2366, `group_chat_manager/mod.rs` 4897-4945); an empty or disabled group selection is legacy's empty `lorebookIds`; entries order by legacy display order.
- Persona resolution: legacy `choose_persona` (`storage.rs` 509-521), legacy `persona_disabled`, group persona order (`group_sessions.rs` 560-563), a missing persona as in legacy `load_persona`. Group chat mode and character-lorebook switch follow `group_sessions.rs` 509-600; the legacy importer fills the conversation's values from a session's `config_overrides` `chatType` and `disableCharacterLorebooks`.
- A retry reads whatever is saved when it runs, as legacy did.
- Runtime text sections carry legacy wording. Depth counting follows legacy `insert_in_chat_prompt_entries`; condensing follows legacy `condense_entries_into_single_system_message` (direct and group). Identity tokens resolve like legacy.
- Legacy sent manual memories twice, in the Key Memories section and in the "Relevant memories" block (`prompt_engine.rs` 3813-3852, `completion.rs` 352-410); the rewrite keeps the duplication.
- The companion state and scheduled-note lines are byte-identical to legacy by default.
- The catalog keys and entry keys are the stable legacy identities. Legacy removed empty sections by string replacement in Rust; the catalog uses entry conditions instead.
- Scene image protocol: the `sceneImageProtocol` fact is legacy's `append_image_directive_instructions` id filter as data; LoRA keyword values follow the resolved model like legacy (`scene_loras`).
- Pure mode: every provider answer goes through the guard as legacy's API layer did.

### Generation

- Time handling resolves legacy `is_companion_mode` and the session clock; time placeholders are legacy `time_placeholder_values`; history time stamps follow legacy `push_user_or_assistant_message_with_context`; the memory host freezes time awareness into the run as legacy `flow.rs` did. Legacy applied the clock whenever time awareness was on without checking companion mode, but only companion sessions could turn it on, so the companion requirement changes nothing reachable. Echoed stamps are stripped like legacy `strip_echoed_time_stamps`.
- Temporal queries are legacy `detect_temporal_query_range`. A zero-norm vector scores 0 like legacy. Missing memories are embedded like legacy `migrate_session_memory_embeddings_if_needed`. The x0.7 cold penalty applies to the calibrated score as legacy applied it to raw cosine. Duplicate evidence compares strictly greater, as legacy.
- A direct chat launched with dynamic memory runs as manual memory while the global switch is off (legacy `is_dynamic_memory_active`); groups follow only their memory mode, as legacy groups did. A group regeneration leaves the replaced reply out of the retrieval query, as legacy did.
- Memory reaches the prompt as in release 2.2.5; 2.2.0 filled `{{key_memories}}` with every hot or pinned memory and added a `Relevant memories:` block on send. A group's `{{context_summary}}` is the stored summary in manual mode too, as legacy group chats always used it. Retrieval embedding unavailability continues without keys like legacy. Access recording is legacy's count, time and importance update.
- Legacy manual memory has no selected-revision concept, and the current snapshot field has no writer or content authority, so a non-empty `selected_revision_ids` fails closed instead of inventing a second store.
- Speaker selection: mentions bypass mute as in legacy; LLM selection without a launch snapshot resolves the model per call as legacy did; the narrator prompt and the `[Name]: ` transcript prefix are legacy's.
- Chat replies declare no tools, as legacy sends them with no tool config.
- Reply images are attached as legacy `generated_image_attachments` (`group_chat_manager/persistence.rs` 32-53) and the chat flows did.
- Chat parameters resolve in legacy's order (conversation, then model, then app). `feature_parameter_input` is legacy `feature_model_overrides` plus `prepare_feature_request`, with the legacy `FeatureSamplingDefaults` constants and legacy's per-feature field lists.
- Growth and consolidation resolve their model as legacy's `feature_model_overrides(CompanionMemory, COMPANION_MEMORY_DEFAULTS)` did.
- The llama.cpp worker starts lazily like the legacy worker. The smart-offload layer count is cleared on every open as legacy did on every start.
- Exit cancels running inference as legacy `AbortRegistry::abort_all` did on `ExitRequested`.
- Reply helper is legacy `chat_generate_user_reply` and `group_chat_generate_user_reply`, swapping as legacy `swapped_prompt_entities`, with legacy's temperature 0.8 and cleanup; its streaming downgrade is legacy `effective_streaming_enabled_with_override`.

### Companions

- Companion sends read the current authored companion config like the legacy completion flow and use the legacy neutral update when classification is blank, missing or unavailable.
- The turn-effect seed, the zero-delta continuation seed, the largest-delta summary selection, percent rounding, wording, pluralization and three-part cap are legacy's. Automatic admission uses the resolved legacy message interval.
- The sixteen-fresh-memory read is the legacy growth trigger and evidence boundary; growth, consolidation and Soul-writer prompts, instructions and fallbacks are legacy's values. Scheduled notes render the legacy block.
- A failed companion follow-up is not retried, as in legacy.

### Memory

- Post-turn memory is legacy `enqueue_post_turn_dynamic_memory`: send and continue earn a cycle, never regenerate, with legacy's gates. A missing model reschedules the job like legacy's "Summarisation model not configured" retry; a missing prompt falls back like legacy's built-in entries; the llama.cpp sampler strip is legacy's; creates are seeded with the tokenizer, zero on failure, as legacy.
- `trigger`, retry, `skip` and `pending_approval_count` are legacy `trigger_dynamic_memory`, `retry_dynamic_memory`, `skip_dynamic_memory_cycle` and `dynamic_memory_pending_approval`. An explicit trigger clears the pending approval as legacy did. Approval matches the legacy forced-cycle cursor move; the group trigger keeps the legacy oldest cursor prefix; group retry has no model override, matching the legacy API. An existing run keeps its frozen settings as legacy's in-progress cycle kept the settings it had read.
- The round-zero prompt uses the copied legacy budget variables and runtime-input wording; the tool-disabled fallback request, the tool result payloads, the tool contract (with legacy group wording and `123456` fallback examples) and the time-aware transcript format are legacy's. Recursive loops follow legacy's settings, and a recursive round without tool calls ends the cycle normally as legacy did.
- Repair: the last answer for a text wins as with legacy's map insert, a response without a usable call still runs the structured fallback as legacy re-checked, and a failed repair round is logged and the cycle kept as legacy did. `start_cycle` is legacy's pre-summary decay.
- The post-turn driver coalesces per conversation like legacy's scheduler; manually triggered cycles are not resumed after a crash, as legacy lost them. A failed window is admitted again on the next turn, as legacy retried a failed cycle.
- Create preparation embeds at legacy `embeddingDimensions` (default 768).
- Re-embedding counts tokens with no special tokens and no cut, as legacy counted.

### Lorebooks and creation

- Single-entry and keyword preparation copy legacy formatting (chronological messages, durable memory, relative time, existing entries, `(none)`, title, content, keywords, direction). Selected memory lines are numbered contiguously over non-blank memories, as legacy did. Fillers and instructions carry legacy wording.
- Staged lorebooks: legacy per-source and total byte limits and excerpt truncation; unset temperature and top_p are legacy's 0.3 and 1.0; reasoning is disabled as in legacy feature requests; target-count defaults and bounds, the brief, excerpt formatting, final instructions and drafted-entry rendering are legacy's.
- Legacy single-entry primary and fallback requests recorded usage before checking response success, as the rewrite does.
- Legacy memory `flow.rs` recorded summary and memory responses before validation; the job usage ledger restores that boundary. Legacy `creation_helper/service.rs` also accounted for responses before accepting their content.
- The creation helper follows the legacy agent path (`creation_helper/agent/run.rs`): rounds continue until a response has no tool calls, and after eight rounds legacy stopped iterating and kept the draft. The single leading system message on llama.cpp is what release 2.2.5 does. Undeclared tools are answered as legacy did. Tool results follow the legacy `{success, message, error?, ...}` shape and argument errors.

### Images

- Avatar image prompts (2026-09-22) read the documents the way legacy's avatar sheet did, with legacy's nine placeholders; with no entry selected the bundled seed stands in for legacy's template text.
- Scene images (2026-09-23) are legacy `chat_generate_scene_image`; unreadable references are skipped as legacy skipped files it could not load.
- Scene prompts (2026-09-23) are legacy `chat_generate_scene_prompt`, with legacy's loaded window, placeholder values, condition facts and in-chat offsets.
- Design references (2026-09-23) are legacy `chat_generate_design_reference_description` (legacy placed no entry by depth or message count, and took data URLs from the editor).
- Scene tags (2026-09-23) replace legacy `sceneImageProtocol.ts`, which the old frontend ran; the stored text is trimmed as legacy persisted it. Group chats never had the tag.
- Local stable-diffusion.cpp sees discrete GPUs and accelerators as legacy's device list did; reading the installed list removes stale `sdcpp:` rows as legacy purged them on that read.
- The avatar gradient is the old `generate_avatar_gradient` extraction; the round image falls back to the square avatar as the old command did when `avatar_round.webp` was missing.
- Image bundle browsing and registration, and CivitAI token handling, follow legacy.

### Speech and models

- Whisper recommendation classifications and the first-filename default are legacy's.
- The saved-voice flow passes the provider voice id to the user-voice save; the legacy flow did not refresh the provider cache there, so neither does this.
- Kokoro keeps the legacy starter pack, model override, blend document and optional `lexicon.json`.
- Downloads delete their partial on cancel or permanent failure, as legacy deleted its `.tmp` on cancel, stall and failure. Whisper and Kokoro check at use time only what legacy checked, that the files are present.
- `register_downloaded_gguf` adds the model the old browser created; the first model becomes the default as legacy's `addOrUpdateModel` did. Sprout runnability uses the remote hardware as legacy did.
- ONNX Runtime follows legacy `ensure_ort_init`, reusing legacy's download folder; Kokoro calls it first on every platform as legacy's TTS commands did. The download is checked by size, as legacy checked nothing.

### Files

- Character files follow legacy `character_import_preview_from_bytes`, legacy's character form rules for embedded books, and how the old character form saved tags, sources and creator notes. A failed image leaves only that image off, a remote background is skipped and an unreadable image is left out of an export, like legacy.
- Persona files write persona, bindings and default in one transaction like legacy; a missing description imports as empty, like legacy.
- The old app's frontend named a nameless lorebook file after its filename.
- Group chat import uses legacy's group defaults. Every imported model file is a chat model, like legacy.

### Legacy import

- Provider/model, prompt, persona and lorebook plans match the SQLite planners for the same database (parity test in `lettuce-database`).
- A session's selected scene carries its background, direction and prompt as in legacy. Legacy kept reasoning only on the message; without a selected variant legacy rendered the message content.
- An all-muted legacy cast unmutes one member because legacy refused to generate in that state.
- Legacy branches copied memories with the same ids, so memory ids derive per session. Legacy kept memory texts apart from embeddings; manual memory, branched chats and character file imports held texts without embeddings that its prompt and memory list used.
- `companionState.preferences` is read like legacy `temporal.rs`; a companion session without saved state takes the character's `time_awareness`, which legacy seeded on its next save.
- Legacy created continuity episodes lazily and kept sessions without a persona under a separate `__default__` key, hence the renumbered single chain.
- A session prompt override that is unavailable leaves the launch prompt like legacy's fallback; a session imported as a companion chat imports no override because release 2.2.5 never read it.
- Speed stats come from legacy `storage_manager/sessions.rs` `json_usage_summary` (the column held MTP and DFlash stats alike); a non-JSON value is dropped as legacy's reader dropped it.
- Legacy entry display order becomes the contiguous stored order.
- App usage import writes `appActiveUsageByDayMs` into `app_usage_days` (max per day, so replays and earlier usage are kept), records an unreadable day or a total above the days' sum as lost, and drops the started-at and last-updated timestamps, which the table's days and `updated_at` replace. Legacy kept a total, a day map and both timestamps inside the settings JSON with a read-modify-write.

### Sync, recovery and shutdown

- The sharing device's user approves each authenticated peer, like legacy. The application does not persist the legacy one-time PIN or invent a trusted-peer registry.
- Startup recovery reruns nothing, as legacy lost in-flight generation on a crash.
- The host flushes app usage every 30 s and on exit, like legacy.

## Deliberate differences from legacy

- The legacy inventory walk follows no symbolic links and leaves out names that cannot form a safe archive path, unlike the legacy exporter.
- Legacy used an app-wide template of any type in a direct chat; only a direct-chat document is used.
- `Clear` stores a disabled prompt, a state legacy never had; the frontend's reset must send `UseLaunchDefault`.
- The bundled continuity entry drops its "Scheduled Background Context" sub-section through a condition; legacy stripped the rendered heading afterwards (`prompt_engine.rs` 4349-4350) and left four newlines behind.
- `{{group_characters}}` renders empty in a direct chat; the old direct engine left the literal placeholder.
- Group prompts get time values; legacy left the literal placeholders.
- Echoed timestamps are stripped before the empty-reply check, so a reply that was only a timestamp is an empty-reply failure; legacy stripped afterwards and saved an empty message.
- Legacy's 90-second per-memory embedding timeout and progress toast are not ported; the embedding call follows the job's cancellation.
- Calendar arithmetic is checked; legacy panicked on inputs like "200000000000 days ago".
- The history window is bounded by the 512-message context policy; legacy loaded only the latest 120 messages.
- Legacy stripped a leading `- ` from the whole numbered memory line, so a memory whose text is only dashes lost the space before its note; the rewrite strips the memory text only.
- The author and swap notes name `{{persona.name}}`; legacy wrote "user" or "the user persona".
- Legacy's turn-effect diff was taken before decay; the run's starting snapshot is post-decay, which matters only for an existing memory from the current window whose importance changed.
- A memory `finish_cycle` revision conflict retries twice and then keeps the cycle; legacy saved last-write-wins.
- A forced memory window never reaches back into summarized messages, which legacy's `next_window(force)` could.
- A trigger while a cycle runs returns `CycleInProgress`; legacy started a second concurrent run.
- The group reply helper honours `history_count`; legacy hardcoded ten messages.
- The legacy final fallback of LLM speaker selection to the first configured model is omitted because it had no explicit selection provenance.
- Staged lorebook runs freeze the prompt document at admission instead of legacy's execution-time live template lookup. Older runs without a snapshot still require the caller's exact admitted prompt revision.
- Legacy staged `pipeline.rs` called the provider without recording usage; legacy keyword generation also omitted it. Both are recorded now.
- The Soul-writer alternate-model fallback is not taken after an evidence, run-persistence or replay-cleanup failure.
- Creation helper: lorebook drafts list entry ids (legacy never showed them, since entries went straight to the database); a lorebook description is never shown (legacy showed `<unset>`); legacy had no history cap, and the oldest turns are now dropped under the 512-message limit; legacy flattened tool arguments and turned every `"` into `'`, which broke the dialogue block, and arguments are kept verbatim; legacy always sent an empty preview or confirmation message (it read `note`) and reported "scene updated" on a failed scene edit.
- Imported summaries: without a usable cycle legacy restarted its summary cursor at message 0, which a summary window cannot express, so an imported summary covers the first message.
- Scene images: legacy's top-level attachments survived switching variants, here the image belongs to the revised content; the image step uses the effective persona, including the inherited default (legacy's image step read only an explicit session persona while its prompt writer used the default); the prompt is bounded at 64 KiB (legacy had no bound); more than 16 references drop persona references first (legacy had no limit); the scene prompt is not kept as the attachment filename (legacy's seed for "regenerate"); the message is re-read before attaching, where legacy overwrote concurrent edits.
- Scene prompts: image tokens left in a system or assistant entry are removed (legacy's hints there were always empty and assistant entries leaked the raw token); blank messages are not sent; `{{char}}`-style names in descriptions are substituted, where legacy sent them raw; `{{image[...]}}` tokens in a scene writer template render empty instead of leaking; usage is recorded also when the answer is empty.
- Design references record usage (legacy recorded none) and remove image tokens left in a text entry.
- Soul growth: legacy addressed edits through a session whose character owned the Soul; the old relationship page removed by list position, now by id. Legacy's clear emptied the shared Soul but other sessions re-persisted their copies on the next save; the rewrite's clear really clears it.
- The post-turn memory driver does not sleep 1200 ms before each pass as legacy did; the running pass already coalesces later turns. Group memory runs through the same driver; legacy awaited group memory inside send and continue.
- App usage counts only focused time; legacy never paused on blur and counted the time the app was open.
- The app version marks only `-cuda`; the old app also marked `-rocm` and `-vulkan`, and the CPU and Vulkan builds are now one normal build with no ROCm build.
- Avatar gradients are cached per process by content hash instead of `gradient-*.json` files; a single-color image produced `#hex NaN%` and now spans 0% to 100%; GIF avatars, which failed to decode as `.webp`, now decode.
- Character file images are stored as given instead of re-encoded to WebP.
- Persona files do not store lorebook ids that do not exist.
- Chat import: a blank header name falls back to the file stem (legacy stored a blank title); a companion character gets the chat as a continuity episode (legacy saved a roleplay session, which the rewrite cannot represent for a companion; the legacy backup import makes the same choice); a transcript whose speakers map to one character becomes a direct chat (legacy made a one-member group).
- Chat template files leave off missing or mismatched references; legacy copied the ids as they were.
- Model files need a provider account; legacy stored the model without credentials.
- `gguf_library` also rewires stable-diffusion.cpp paths when moving the folder; legacy left them pointing at the old folder.
- Kokoro assets come from a host-resolved managed root, replacing legacy's caller-supplied asset-root commands; the legacy automatic `system-kokoro` database row is not reproduced, because local providers use the ordinary typed configuration and no longer store an editable asset root.

## Decisions

- 2026-09-11: speakers whose character is gone become disabled, muted "Unknown" members of an imported group conversation.
- 2026-09-14: a group launch accepts companion characters as ordinary members. Imported companion sessions bind the character's shared pool, taking the legacy shared companion memory or else the most recently updated session's memories. Backup restore and legacy load are replacements, never merges.
- 2026-09-23: the legacy OpenAI-compatible host API is not ported. The legacy `<<image:{json}>>` directive is dropped; only the scene tag remains. App usage tracking was redesigned with the user. The AMD integrated GPU of a unified-memory machine is included in local diffusion placement.
- Approved removals: the legacy SamLowe `roberta-base-go_emotions-onnx` classifier is not ported, and its files under `<legacy app folder>/models/embedding/companion-emotion` are neither imported nor touched (legacy kept no other companion-model state); Thymos must be downloaded. The legacy companion NER and router (NLI) models are not ported.
- Legacy round avatar files (`avatar_round.webp`) are not imported; the round image falls back to the square avatar until a positioning system exists.
- The legacy database and storage tree are never modified or removed; source cleanup waits for full migration verification and explicit user-approved deletion.

## Known gaps

- Memory import does not carry the per-cycle tool log and its revert (no rewrite equivalent), nor the cursors of non-carrier pool conversations (backups do not carry per-conversation memory cursors).
- Group sessions whose group profile is gone are recorded as `MissingGroup` skips and left out.
- The group conversation importer passes no session model settings, and voice autoplay has no destination.
- A pruned terminal memory job would restart its key chain onto a stale run id (no memory job is pruned today). An input error that never clears (settings, missing model) keeps its memory job retrying each turn.
- A crash between growth success and consolidation admission loses the consolidation (legacy lost it too). With a shared pool, overlapping memory passes of two chats can count one chat's new memories as fresh for the other's growth.
- An `artifact_id` image recommendation (made by the new app) has no resolvable LoRA file yet, so it binds as having no LoRA.
- A Thymos job that succeeded without a readable hint cannot be finished, since the job store does not record the pinned revision; the next admission installs again.
- Creation helper drafts do not show the avatar, background, model, prompt, gradient and image-gallery lines (the fields do not exist yet) or the non-native fallback-protocol entry (the legacy UI never enabled it).
- The creation helper's avatar prompt path (template text, untrimmed, no conditions) is not ported.
- The per-model DynamicMemory sampler profile and order, min_p, typical_p and DRY resets of legacy have no destination before the llama.cpp runtime slice.

## History

- Dates on sections of the old README: avatar prompts and remote image generation 2026-09-22; scene image protocol, scene images, scene prompts, design references, scene tags, Soul growth edits, startup recovery, the post-turn memory driver, app usage, companion follow-ups and requested-work recovery 2026-09-23.
- Before the scene image protocol condition, the remote entry was active in every direct chat.
- The memory host used to hardcode time awareness off. The old README also said "companion time awareness has no destination yet, so the host passes it disabled"; `resolve_runtime_inputs` now reads it from the companion clock.
- Imported memory texts without embeddings used to be dropped. An imported summary used to claim the whole dialogue.
- Direct chats did not get the swap-places note and companion-state fallback, and groups got a generic continue line instead of the begin and continue-same-speaker notes.
- A window whose memory job failed or was cancelled used to keep its key, so automatic memory stopped for good; input errors of `run_claimed` used to keep the claim.
- Nothing admitted growth and consolidation after a memory cycle before `CompanionFollowUpHost`; legacy ran both inline. Growth used to reuse the memory cycle's DynamicMemory sampling.
- Pending effects used to be listed with a status-agnostic page that stopped admitting after 512 settled effects and could be filled by other conversations. The running-cycle check used to look only at the first 200 memory jobs.
- Pure mode's Low level used to be imported as Standard.
- Pairing protocol version 1 (backlog #22) derived the key from the PIN and a cleartext salt, so a captured handshake allowed an offline PIN search and decryption of the whole session; version 2 uses SPAKE2.
- Restore used to give no way to back up a database holding unfinished work (see lettuce-database History).
- The old README said conversation launch now creates the memory space in the launch transaction, removing an external `MemorySpaceId` input from the post-turn worker.

Contradictions in the old README, resolved by the code:

- It said the memory loop's "eight-round and 64-call admission limits remain authoritative" and that recursive loops are "bounded by the 64-round storage limit". `CompanionMemoryLoopPolicy::round_cap` is `min(recursive_memory_loop_hard_cap, MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS)`, which is 100, and an attempt holds up to 4096 calls.
- It said companion temporal-range filtering "remains pending"; `temporal_query` filters retrieval for time-aware companion chats.
- It said manual memories "render once"; the "Relevant memories" block still repeats them on a one-to-one send outside dynamic memory (`context_assembler.rs`).
- It said "debounce/startup wiring remains a later slice", "durable request-body recovery remains a later slice" and "full automated send/regenerate/retry orchestration ... remain later". `PostTurnMemoryScheduler`, `resume_after_restart`, the initial-dispatch request replay and `execute_next` exist.
- It said the bridge leaves "host-specific runtime inputs and terminal job progress" outside; `CompanionMemoryHostCoordinator::resolve_runtime_inputs` and `run_claimed` supply them.
- It said "Legacy per-session sampler values, background, voice autoplay and group chat mode or starting scene ... have no destination yet" and that session settings, memory fields, companion state and attachments "are not mapped yet". The direct importer maps author note, prompt and lorebook overrides, model settings, background, memory, companion state and the clock; the group importer maps chat mode, speaker selection, background and its starting scene. Only the group session model settings and voice autoplay stay unmapped; attachments were not checked.
- It said legacy's per-model HelpMeReply and DynamicMemory feature overrides have no destination; the model feature slots (`feature_parameter_input`) now carry them. The llama.cpp sampler profile, order, min_p, typical_p and DRY resets are still listed as gaps.
- It said staged lorebook "project-owned source associations remain pending"; migration 11 stores them (see lettuce-database).
- It said the Whisper removal path refuses retained legacy files; a Whisper model imported from a retained legacy folder can now be removed (its file only when inside the managed folder).
- It said Kokoro "remains unavailable until its native runtime is routed"; the TTS runtime routes Kokoro to native synthesis.
- It said sockets, secure pairing and status events "remain later", and described a persona-only media sync coordinator. The transport, pairing and a general `SyncMediaCoordinator` exist; secrets have their own phase.
- It said "Restore remains a separate application slice" and "Conversation UsageLedger and cost bodies remain a later full-backup section"; `BackupRestoreCoordinator` exists and the export carries usage events and cost bases.
- It said the version-2 restore carries the sync journal as device-local state. `carry_device_local_state_from` explicitly leaves the sync identity, journal, frontiers and conflicts behind.
- It said the CivitAI browser takes Pure mode "until the content filter is ported"; the filter is ported (`ProviderRuntime::content_filter`) and the level is still passed in.

## Test coverage

- File-backed scenarios cover: direct, automatic group, explicit mention and director continuation preparation with attribution and recovery; OpenRouter cost capture (native and normalized disagreement, raw amounts, routed endpoint selection, generation and price provenance, old basis JSON, changed-account gating, missing, error and ambiguous lookup retry, reopen after account deletion without network replay); staged lorebook dispatch evidence for all four stages, invalid planner usage, cancellation after response, concurrent writer failures and replay without duplicate charges; fault injection for entry, keyword and Soul-writer evidence (admission failure sends zero requests, settlement failure sends one, no false checkpoint, later retry keeps pending evidence); memory summary fallback totals versus separate raw responses, failed primary and fallback calls, continuation and terminal replay, missing usage, storage failure without fallback and cancellation with kept evidence; creation two-round evidence; backup export of the authored graph, ASR library, conversation history (empty starter message, Director-mode group, rejection of a broken parent link, decrypted starter snapshot), generation runtime (three stages, a settled tool, a failed attempt), jobs (claimed, progressed, retry-scheduled), usage ledger with pricing basis and unavailable counters, operations and outbox (contiguous order, operations without events, duplicated sequence rejected) and companion state (real transition, corrupt episode index rejected).
- `tests/fixtures/legacy_avatar_gradients.tsv` pins the gradient extraction against the old functions on 400 sample sets.

## Not wired yet

Everything the Tauri phase (phase (c)) still has to connect:

- Tauri commands and frontend surfaces for every coordinator: chat send, regenerate, retry and cancel commands; streaming progress events; creation helper commands; Soul-writer preview IPC; microphone and transcription scheduling; lorebook, chat, character, persona, prompt, chat template and model file commands (including naming a nameless lorebook file after its filename); the purge notice list (`Database::purge_notices`); sync listener discovery and status events; the "memory paused" explanation when a memory window is held after `LeaseLost`.
- Startup order: `recover_after_restart`, then the memory host's and follow-up host's `resume_after_restart`, then workers. Claim and run, or recover, queued `ArtifactInstall` jobs at startup, since a queued Thymos job keeps every later Thymos admission, completion and removal busy. Run `sweep_orphan_media_files` at startup.
- After each sync session, run `collect_media_garbage`.
- Embeddings: nothing calls `EmbeddingModelCoordinator::adopt_legacy_install`, `load_active` or `complete_install`, `EmbeddingModelCatalog::pin` or the embedding `ArtifactInstall` job. The host must adopt the legacy v4 files at startup, load the active service with the device's dimension and token budget, and run install jobs from the model hub UI, finishing them with `complete_install`.
- Local stable-diffusion.cpp: nothing calls `finish_runtime_install` when a catalog or engine download completes; the host must run it (extract the engine, then `register_catalog_model`) from the install job's completion, as legacy `sdcpp.rs` did after each download.
- Remote avatar URLs in character and persona files: legacy downloaded character avatars when `autoDownloadCharacterCardAvatars` was on and persona avatars always; today the URL is dropped. The download policy (the setting, the fetch through the shared network client, storing the bytes) belongs in the character and persona file use cases, not the Tauri shell.
- The macOS bundle needs legacy's `entitlements.plist` with `com.apple.security.cs.disable-library-validation`, or the hardened app refuses the re-signed ONNX Runtime download.
- Android eSpeak hosting for Kokoro.
- Automatic staged lorebook host wiring.
- Automatic OpenRouter cost capture after inference (no host polling, scheduling or cache exists; `capture_job` is caller-triggered).
- The analytics exit event.
- Passing the Pure mode level to `CivitaiBrowser`.
- TTS preview caching.
- Scene images: optimistic placeholders and the askFirst approval flow.
- Group companions and group growth scheduling.
- Adding a character to an existing group conversation (see lettuce-conversations).
