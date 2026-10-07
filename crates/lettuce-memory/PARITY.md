# lettuce-memory: legacy parity notes

Facts about how `lettuce-memory` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- The verified dynamic-memory tool scenarios are pinned in `fixtures/legacy-import/dynamic-memory-tool-scenarios-v1.json`.
- The cycle hard-delete budget is legacy's `floor(count_at_cycle_start * ratio).max(1)` over every item, cold included, counted across every round of the cycle.
- `finish_cycle` trims first and then demotes, as legacy did after its loop and repair pass. Trimming and hot-budget demotion run once per cycle, not per round.
- `start_cycle` is legacy's pass before the summary phase (decay by `decay_rate / (1 + sqrt(access_count))`, cold below `cold_threshold`).
- A memory space holds any number of items, like legacy; only the cycle's policy pass trims to `max_entries`. Capacity trimming exempts pinned items only, so an unpinned user-written memory can be evicted. The stable ascending ranking uses 0.7 importance and 0.3 recency with time bounds over unpinned items, matching `old-code/src-tauri/src/chat_manager/memory/dynamic.rs:671-726`.
- Six-digit `short_id`s and the tool contract are legacy's: models see `[short_id] text`; `delete_memory` takes `text` (a six-digit id or the exact memory text), `pin_memory`/`unpin_memory` take `id`, `supersedes` lists ids.
- Reference cleaning of `# * " ' [ ] ( )` is legacy's; stable UUIDs also resolve.
- Arguments parse as leniently as legacy (unknown keys ignored, bad optional fields defaulted, `confidence` clamped, padded `category` trimmed).
- A call that cannot be applied settles as `Skipped`, as legacy skipped such calls; a round may repeat `done`.
- Created text goes through legacy's checks (`normalize_memory_text`), including legacy's false positives: a memory containing `i cannot` or `user:` is dropped.
- The duplicate check runs before the category check, exactly as legacy checked text, embedding, duplicate and then category.
- Stored memories preserve legacy companion categories and null categories without changing them to `other`. The model tool parser retains the six roleplay categories.
- The category repair is legacy's single-tool repair contract (`retag_memory`, required tool choice, six categories as an enum); an empty repair answer falls back to legacy's keyword buckets (`guess_memory_category`).
- Every outcome carries what legacy echoed back to the model (six-digit id, deleted text, the listing after the call, the duplicate kind); the application renders them into the legacy tool-result payloads.
- Structured fallback booleans are trimmed and ASCII-lowercased like legacy. The JSON/XML operation parsers and fallback prompts were copied from legacy.
- Supersession keeps only the latest forty superseded records, as legacy does.
- Time-aware memories keep legacy `turn` precision; user-observed dates keep `user` precision without a required source message. Changing or clearing the date preserves prior source attribution, like `old-code/src-tauri/src/storage_manager/sessions.rs:4561-4568`.
- Memory origins preserve the user/model/import badges. User-authored summaries have no model source coverage; model and imported summaries keep bounded text, token count and exact ordered source cursor.
- Run modes are the legacy `auto`/`askFirst`/`manual`; the ask-first prompt threshold is the copied interval rule, and skip records the legacy skipped state.
- Background runs preserve the legacy cycle's frozen window and model while staying separate from assistant-message generation.
- Retrieval access is recorded as narrow per-row updates, like legacy.

## Deliberate differences from legacy

- Memory reads identify the conversation branch. Legacy branches were separate sessions with copied memory (`old-code/src/core/storage/repo.ts:1655-1709`); in-conversation branches keep separate own spaces while companion pools remain shared.

- Legacy's cycle-start pass also restored pinned-but-cold items to hot. The snapshot invariant already rejects that state, so `restored_pinned` stays zero for stored spaces.
- Interleaved thinking tags of different kinds are stripped pair by pair instead of by earliest opening tag. This differs from legacy only for malformed output.
- Legacy used the raw argument string as the memory text when `text` was missing. That is not reproduced: the raw arguments of such a create are the JSON of the other fields, not a memory, so the call is skipped for missing text.
- Legacy silently dropped some calls from its results (a create without a string `text`, a delete without a string `text`, a pin without a string `id`, non-object arguments, and in group chats a pin or unpin whose target was not found). The dropped result shifted every later tool result onto the wrong call. These now settle as `Skipped` or `TargetNotFound` with a typed reason.
- The memory listing excludes superseded items in group runs too; legacy's group copy did not filter. Group runs never supersede, so nothing is hidden.
- Background post-turn extraction has its own durable run boundary instead of fabricating a visible conversation generation turn.

## History

- Callers used to have to invent a memory space id in tests; the repository now resolves the authoritative space by `ConversationId`.
- The dynamic-memory slice was delivered in stages: tool contract and reducer, SQLite repository and first admitted-round handler (in `lettuce-database` and `lettuce-app`), background runs, summary checkpoints, approvals, round settlement and continuation, delete-after rewind.
- The previous README said runs store up to 64 inference rounds. The code caps an attempt at `MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS` (100).
- The previous README said a delete-after rewind restores the earliest invalid run's starting snapshot. The adapter now undoes the stored tool outcomes of that run and every later one instead, which keeps decay, retrieval access, user edits and other pool conversations' changes.

## Not wired yet

The memory host validates admitted companion effects and source-message identities and applies supersession through the shared reducer. The API job handlers, the generation-worker driver, the streamed job output and the status read model are wired in the composition root; what remains is the frontend surface for them.

Manual item and user-summary counts can remain unknown without a tokenizer. Legacy substituted zero when counting failed (`old-code/src-tauri/src/chat_manager/memory/flow.rs:3791-3792`); null preserves the distinction. Hot-budget reduction refuses unknown counts instead of treating them as zero.

Manual setters preserve legacy pin and temperature rules (`old-code/src-tauri/src/storage_manager/sessions.rs:4359-4404,4460-4504`). The branch edit history keeps legacy message anchors (`sessions.rs:3936-3993`) without its fifty-entry cap. The API transaction commits the edit, history and replay receipt together; legacy appended its edit history separately.

Manual undo follows the recorded previous values used by legacy rewind (`old-code/src-tauri/src/conversation_manager/memory.rs:196-284`). Pin undo changes the pin flag; temperature undo sets importance to zero or one. User edit history is uncapped instead of the legacy fifty-event cap (`memory.rs:6`), so an older cut does not lose its undo evidence.

Reverting a cycle undoes that run's recorded tool outcomes and the summary it published, and only when no later cycle that changed memory, published a summary or still runs exists for the space. Legacy marked the event reverted and rebuilt the whole session from the remaining events, with no dependency check and a whole-session save that overwrote anything a cycle had written since the page loaded (`old-code/src/ui/pages/chats/ChatMemories.tsx:1472-1519`, `old-code/src/core/storage/memoryToolEvents.ts:184-265,299-310`). User edits made after the cycle and a user-authored summary survive the revert; a revert is refused with the later cycle named otherwise. Reverted runs stay in the activity log with their outcomes and no longer count for summary windows or the cursor.
