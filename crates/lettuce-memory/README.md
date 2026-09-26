# lettuce-memory

The domain model for long-term conversation memory: the memory space and its items, the cumulative summary, the dynamic-memory tool contract a model uses to edit memory, the reducer that applies those tool calls, and the durable records of a background memory run.

The crate is pure. It validates state and computes change sets; it never touches SQLite, calls a model or computes an embedding. Those jobs belong to other crates and reach this one through the repository traits in `port.rs`:

- `lettuce-database` implements the repositories.
- `lettuce-app` drives memory runs, prepares tool calls, selects memories for a prompt and renders tool results and prompt text.
- `lettuce-embeddings` owns the embedding runtime. Embeddings are a rebuildable projection of item text, so they are not part of the memory model; semantic similarity reaches the reducer only as typed evidence (see below).
- `lettuce-transfer` uses these types as its backup documents.

## Memory spaces

A `MemorySpaceSnapshot` is the authoritative state: a space id, a revision and the ordered `MemoryItem`s. Every direct or group conversation launched with manual or dynamic memory gets one space in the same transaction that creates the conversation, and repositories resolve it by `ConversationId`. A companion character's memory pool is one space shared by several conversations.

Each revision has a stable identity, `memory_revision_id(space_id, revision)`, a UUIDv5 of the two. Conversation preparation records which revision a prompt was built from without a separate revision table.

A `MemoryItem` carries:

- `id` and `short_id`. The short id is the six-digit number a model sees and quotes back. It is derived from the memory id, probed upward past ids in use, unique within the space and fixed at creation (`MemoryShortId::allocate`).
- `text` and one of six `MemoryCategory` values.
- Hot/cold and pinned flags. Cold memories stay in the space and can still be retrieved; retrieval promotes a selected cold memory back to hot. A pinned memory is never cold, and snapshot validation rejects that state.
- Scores (`importance` and friends) as `Score`, basis points from 0 to 10 000, so they compare and serialize exactly.
- Retrieval metadata: `access_count` and `last_accessed_at`.
- Optional source attribution: the transcript message the model named as the memory's source, its role (user or assistant), the observed time and `"turn"` precision. Time-aware memories carry all four; others carry at most the message id. When several turns are coalesced into one cycle, the message id is what companion post-turn effect settlement uses to attribute a memory to its turn.
- Supersession links (`supersedes`, `superseded_by`, `superseded_at`).

A space holds any number of items. `MemoryPolicy` decides what a memory cycle keeps: `max_entries` (at most `MAX_MEMORY_ITEMS`, 4096), the hot token budget, the cold threshold, the default delete confidence, the per-cycle hard-delete ratio and the decay rate.

`MemorySummary` is the cumulative summary of the conversation so far. Besides the text and token count it stores its window, a half-open range of message positions, and the exact ids of the messages it covers; the window length must equal the id count. The window end is the summary cursor: where the next cycle starts reading. In a companion pool the cursor is per conversation (`MemorySummaryRepository::summary_cursor`).

## Writing memory

All item changes go through one compare-and-swap: `MemoryRepository::compare_and_apply(MemoryChangeSet)` checks `expected_revision`, replaces the complete item set and bumps the revision once. The summary has its own CAS on the same root revision (`compare_and_apply_summary`).

Replacing the whole set keeps the reducer simple. It is a pure function from a snapshot and a batch of tool calls to one validated change set, and the caller commits that change set atomically or not at all. Nothing edits a session projection in place.

## Retrieval bookkeeping

When a turn's generation selects memories, the application records it with `MemoryRetrievalRepository::apply_retrieval_access`. The access is bound to the turn and attempt, and for the selected ids it increments the access count, sets the last access time and promotes cold items to hot. The receipt returns the revision the prompt was built from and the ids that were promoted.

These are per-row updates that neither check nor advance the space revision. A chat turn and a background memory cycle can therefore run at the same time without either failing the other's CAS. A selected memory that a cycle removed in the meantime is skipped. Retrying the same attempt returns the stored receipt; a different selection for the same attempt is a conflict.

## The memory cycle

Dynamic memory runs as a background job after chat turns. The model gets the unsummarized messages and the current memories, writes a summary, then edits memory through tool calls over several rounds. One cycle looks like this.

1. Trigger. `DynamicMemoryRunMode` is `Auto`, `AskFirst` or `Manual`. In ask-first mode, `DynamicMemoryApprovalRepository::prompt_dynamic_memory_if_due` raises a pending approval once a full interval of unsummarized messages has accumulated; after one prompt, the next is due only after another full interval. Skipping keeps that baseline and records the skip. A consumed approval is removed only after the forced job for the recent window has been admitted.

2. Admission. `DynamicMemoryRunRepository::admit_dynamic_memory_run_attempt` stores a `DynamicMemoryRun` that freezes everything the cycle depends on: the starting memory snapshot, the source messages with roles and effective times, the summary window, the resolved model profile, whether time awareness and supersession are on, the structured fallback format and the tool request with its texts. `MemoryToolReducer::start_cycle` produces the cycle-start change: every hot, unpinned item loses `decay_rate / (1 + sqrt(access_count))` importance (floored at zero, rounded to basis points) and goes cold below `cold_threshold`. That change is committed with the run (`cycle_start_change`), and the run's `starting_memory` is the post-decay state, so a replay never decays twice.

3. Summary. Before the first tool round the run may commit one cumulative summary (`commit_dynamic_memory_summary`). The checkpoint keeps the exact request, usage, provider request id, resulting revision and text; an exact replay returns it instead of applying a second summary CAS.

4. Manager rounds. Each round is admitted with the exact provider-neutral request context it was sent with, its output parts and its tool calls (`admit_dynamic_memory_inference_round`). The application parses each call (`MemoryToolArguments::parse_or_skip`), prepares creates with a new id, token count and semantic duplicate evidence, and runs `MemoryToolReducer::reduce_round`. The resulting change set and every ordered result commit together through `commit_dynamic_memory_background_round`; an exact retry returns the stored settlement. The next round replays the calls and results into the frozen context. The loop ends on `done` or at the round cap.

5. Repair. Creates skipped for a missing or invalid category go to a single repair request with one required tool, `retag_memory`, whose category parameter is an enum of the six categories (`memory_repair_tool_request`). If the repair answers with nothing, `guess_memory_category` assigns a category from keyword buckets, first matching bucket wins. Rounds record their `DynamicMemoryRoundKind` (`Manager` or `Repair`), so a resumed attempt never continues the manager loop from the repair request.

6. Finish. `MemoryToolReducer::finish_cycle` applies the policy once for the whole cycle: trim to `max_entries` (lowest score first, score being 70% importance and 30% recency, pinned items exempt), then demote the least recently accessed hot, unpinned items until the hot tokens fit the budget. The result is one more change set through the memory CAS.

7. Settle. The summary becomes the space's summary when the attempt succeeds or when only its tools phase fails. The summary cursor advances only past a succeeded run, so a failed or cancelled tools phase leaves the window for the next cycle or a retry.

### Attempts and recovery

A run has one or more `DynamicMemoryAttempt`s. An attempt moves `Created → Processing → Succeeded | Failed | Cancelled | Interrupted` (or straight from `Created` to `Cancelled`), each transition a CAS on the attempt revision. After a crash the repository recovers an interrupted attempt into a child attempt and copies the admitted provider history and any settled results into it (`recover_dynamic_memory_attempt`). Because every round froze its request context, recovery never re-renders a prompt from state that has since changed. An attempt holds at most `MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS` (100, the hard cap below which the user's own cap applies) and `MAX_DYNAMIC_MEMORY_ATTEMPT_TOOL_CALLS` (4096) calls.

The run's starting snapshot stays available after a restart, so companion post-turn effects can still compute their memory delta and summary.

## The tool contract

`dynamic_memory_tool_request_for_run` builds the tool request: `create_memory`, `delete_memory`, `pin_memory`, `unpin_memory` and `done`, all required. Group chats use a variant without source attribution or supersession. Time-aware runs make `source_message_id` required; other direct runs keep it optional. Companion runs may also allow `supersedes`.

Every description in the contract is a catalog key (`DYNAMIC_MEMORY_TOOL_TEXT_KEYS`, with direct and group variants) that the application resolves through the built-in prompt catalog. The same holds for the repair tool (`MEMORY_REPAIR_TOOL_TEXT_KEYS`) and the structured fallback instructions. Prompt text lives in the catalog and is editable there; this crate only decides the shape. A run stores the resolved request, and validation compares it to the options with descriptions stripped (`dynamic_memory_tool_shape`), so a catalog edit never invalidates a stored run.

### Arguments

Arguments are read leniently. Unknown keys are ignored, a non-UUID `source_message_id` or non-boolean `important` falls back to its default, `confidence` is clamped to 0..1, and a padded `category` is trimmed (a blank one counts as missing). A call that cannot be applied at all (missing or bad text, missing target, undeclared tool name, non-object arguments) becomes `MemoryToolArguments::Unusable` and settles as `Skipped` with a typed `MemoryToolSkipReason` instead of failing the round. Every call gets a result, so tool results always line up with their calls.

A create's category stays a `CategoryArgument` (tagged, missing or invalid) through parsing and is checked after the duplicate check. An untagged create whose text already exists is reported as a duplicate, not as a bad category.

Created text is normalized by `normalize_memory_text`: a surrounding code fence and thinking sections are removed, whitespace collapses, and text that is empty, longer than 280 bytes, or contains a refusal or meta marker is skipped with its own reason. Markers are plain substrings, so a memory that contains `i cannot` or `user:` is dropped too. Thinking tags are stripped pair by pair, per tag kind. The summary validator uses the same normalization.

### Reducer rules

Targets are `MemoryReference`s, kept raw from the model and resolved against the items as they are when the call applies. Surrounding `# * " ' [ ] ( )` are cleaned off. A six-digit reference resolves only as a short id; anything else may be a memory UUID, and `delete_memory` also accepts the exact memory text. A reference that resolves to nothing settles as `TargetNotFound`.

- `create_memory` needs a `CreateMemoryPreparation` from the application. It is skipped as a duplicate when an existing memory matches, checked per memory in item order: equal normalized text, then qualified semantic evidence, then at least 90% keyword overlap for candidates of three or more words. `important: true` creates it pinned.
- `delete_memory` hard-deletes when confidence is at least 0.7 and the cycle's hard-delete budget allows it. Otherwise it soft-deletes: the item goes cold at the cold threshold, with the reason (low confidence or budget reached).
- `pin_memory` pins and makes the item hot at full importance; `unpin_memory` clears the pin.
- `done` ends the round; later calls in the same batch settle as `StoppedAfterDone`. A round may repeat `done`.

The hard-delete budget, `MemoryCycleBudget`, is `floor(item_count_at_cycle_start * ratio)`, at least one, counting cold items; an empty space allows none. It is spent across all rounds of the cycle, so each round passes the `Deleted` count of earlier rounds. `reduce` is the first round of a fresh cycle.

Semantic duplicates arrive as `SemanticDuplicateEvidence`: target id, embedding source revision, dimension count (64 to 768), cosine score and the threshold applied. The reducer rejects unqualified evidence and still checks that the target exists in its snapshot.

Each `MemoryToolOutcome` carries what the model is told afterwards: the six-digit id of a created, deleted, pinned or unpinned memory, a deleted memory's text, the `[short_id] text` listing right after the call (`ListedMemory`, superseded items excluded), and which duplicate check matched (`DuplicateKind`, with cosine and threshold for a semantic match). The application renders these into tool-result payloads.

### Supersession

When a companion run allows it, a create may list `supersedes` ids. Only existing, not yet superseded targets are kept. Each is linked to the new memory at creation time, superseded items drop out of later manager input and listings, and only the latest forty superseded records are retained.

### Structured fallback

For models that do not return tool calls, the run freezes a JSON or XML fallback format. The model is asked for an operations document (`memory_operations_fallback_prompt_key`), and `parse_memory_operations_from_text` turns it into the same tool calls. An empty operations document is a valid "no change". Fallback booleans are trimmed and lowercased. The repair pass has its own fallback (`memory_repairs_fallback_prompt_key`, `parse_memory_repairs_from_text`).

## Rewinding after a delete

When messages are deleted, memory built from them must go too. `DynamicMemorySuffixRewindRepository::rewind_dynamic_memory_suffix` takes the earliest invalid run and, in one transaction:

- reverts the stored tool outcomes of that run and every later run of the same conversation, latest first, with `undo_memory_tool_outcomes`: a created memory is removed (releasing what it superseded), a hard delete is restored from the run's starting snapshot, a soft delete is made hot again, and pins flip back. Decay, retrieval access, user edits and, in a companion pool, other conversations' changes stay;
- restores the summary of the latest earlier successful run's checkpoint, or clears the summary;
- retires the named companion effects.

It is idempotent by operation id. Working out which run is the first invalid one from the deleted message ids is up to the application.

## Backups

The space, item, summary and retrieval-access types are also the versioned backup document. Backup validation reuses their invariants and keeps the exact access metadata that later retrieval depends on; scores are never recalculated. Separate versioned documents carry the embedding projection cache and the full run graph (approvals, runs, attempts, rounds, results and summary checkpoints). Export runs no workflow.
