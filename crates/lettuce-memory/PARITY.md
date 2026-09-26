# lettuce-memory: legacy parity notes

Facts about how `lettuce-memory` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- The verified dynamic-memory tool scenarios are pinned in `fixtures/legacy-import/dynamic-memory-tool-scenarios-v1.json`.
- The cycle hard-delete budget is legacy's `floor(count_at_cycle_start * ratio).max(1)` over every item, cold included, counted across every round of the cycle.
- `finish_cycle` trims first and then demotes, as legacy did after its loop and repair pass. Trimming and hot-budget demotion run once per cycle, not per round.
- `start_cycle` is legacy's pass before the summary phase (decay by `decay_rate / (1 + sqrt(access_count))`, cold below `cold_threshold`).
- A memory space holds any number of items, like legacy; only the cycle's policy pass trims to `max_entries`.
- Six-digit `short_id`s and the tool contract are legacy's: models see `[short_id] text`; `delete_memory` takes `text` (a six-digit id or the exact memory text), `pin_memory`/`unpin_memory` take `id`, `supersedes` lists ids.
- Reference cleaning of `# * " ' [ ] ( )` is legacy's; stable UUIDs also resolve.
- Arguments parse as leniently as legacy (unknown keys ignored, bad optional fields defaulted, `confidence` clamped, padded `category` trimmed).
- A call that cannot be applied settles as `Skipped`, as legacy skipped such calls; a round may repeat `done`.
- Created text goes through legacy's checks (`normalize_memory_text`), including legacy's false positives: a memory containing `i cannot` or `user:` is dropped.
- The duplicate check runs before the category check, exactly as legacy checked text, embedding, duplicate and then category.
- The category repair is legacy's single-tool repair contract (`retag_memory`, required tool choice, six categories as an enum); an empty repair answer falls back to legacy's keyword buckets (`guess_memory_category`).
- Every outcome carries what legacy echoed back to the model (six-digit id, deleted text, the listing after the call, the duplicate kind); the application renders them into the legacy tool-result payloads.
- Structured fallback booleans are trimmed and ASCII-lowercased like legacy. The JSON/XML operation parsers and fallback prompts were copied from legacy.
- Supersession keeps only the latest forty superseded records, as legacy does.
- Time-aware memories keep legacy `turn` precision.
- Cumulative summaries keep the legacy bounded text, token count and exact ordered source cursor.
- Run modes are the legacy `auto`/`askFirst`/`manual`; the ask-first prompt threshold is the copied interval rule, and skip records the legacy skipped state.
- Background runs preserve the legacy cycle's frozen window and model while staying separate from assistant-message generation.
- Retrieval access is recorded as narrow per-row updates, like legacy.

## Deliberate differences from legacy

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

The previous README listed these as later slices. Supersession and a host worker (`lettuce-app/src/companion/companion_memory_host.rs`) exist now, so check the code before treating any of them as open:

- companion-required source validation;
- supersession;
- UI events for memory runs;
- host worker wiring for background runs.
