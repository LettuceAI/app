# lettuce-companions

The domain of companion mode: the companion's Soul (durable identity facts that grow over time), its emotional and relationship state, the reducers for the three background model jobs (Soul growth, Soul consolidation, Soul writer), the emotion-classifier signal mapping, the prompt state, scheduled notes, and per-turn effects.

The crate is pure. It never reads chat storage, renders prompt text or calls a model. It decides what the state is, how a model's tool calls change it, and which facts the prompt should mention; `lettuce-app` coordinates jobs, calls providers, runs the classifier (`lettuce-embeddings`) and renders every model-facing line from the built-in prompt catalog; `lettuce-database` implements the repository ports. Companion features are additive: they sit beside the ordinary chat systems and do not change them.

## Soul

`soul.rs` holds the Soul model.

- `CompanionSoulConfig` is what the author writes on a companion character (stored in `lettuce-characters`' defaults document): twelve identity text categories, typed authored facts, baseline affect, regulation style, relationship defaults, prompting (a companion prompt-template reference and style notes), whether time awareness is on for new conversations, and the two sharing toggles `share_memory_across_chats` and `share_soul_growth_across_chats` (both on by default). Serialized field names are camel case.
- `SoulState` is the durable state: the facts and a revision. A `SoulFact` has a `SoulCategory`, a semantic slot, text, confidence, weight, source ids, evidence count, validity times, a kind (authored, core, growth) and a locked flag. `SoulMutability` per category and `SoulFactPolicy` decide what growth and consolidation may touch.
- `initial_soul_state` turns authored facts into durable ones (`normalize_authored_fact`): a UUID for a blank id, confidence and weight clamped, a blank slot defaulted to its category, evidence count from the source ids, zero creation and valid-from times filled, historical facts locked.
- `SoulOwner` says whose Soul a conversation uses. `SoulOwner::for_conversation` returns the character's shared Soul while `share_soul_growth_across_chats` is on, else `SoulOwner::Conversation`, the conversation's own copy.

### Changing a Soul

Every change is a `SoulChangeSet` applied atomically at one expected revision through `SoulRepository::apply`, which records an immutable idempotency receipt. The pure functions prepare change sets:

- `prepare_growth_change_set` applies proposed facts. A fact needs confidence above its category's threshold (0.55, 0.70, 0.85 or 1.0 by mutability), confidence and weight are clamped, and a new fact in an occupied semantic slot supersedes the old one unless it is locked.
- `prepare_consolidation_change_set` applies a consolidation: core adjustments (filtered to allowed categories) and retirements, with the same thresholds and locked protection.
- `prepare_user_edit` handles the user's own edits: clear all growth (every entry, authored and locked included), remove one entry by stable id, or set an entry's lock. An edit that changes nothing returns `None` and writes nothing. User edits never prune superseded history. The change hash includes user edits only when there are any, so older receipts keep their hashes.

`apply_change_set` keeps at most 40 superseded records; `validate_state` checks the invariants. The SQLite adapter stores facts and their source and supersession lists in normalized tables and applies the pure policy inside one immediate transaction.

Soul state belongs to the character by default, so growth is shared across that character's conversations independently of whether memory is shared. When `share_soul_growth_across_chats` is turned off, every companion conversation without a Soul of its own (and any created while it is off) gets a copy of the shared Soul, while a conversation that kept its Soul from an earlier off period resumes it. Turning it back on makes the most recently updated Soul of a still-existing conversation the shared one when it is newer and differs, and keeps the conversation Souls. Leaving or entering companion mode changes nothing, and a synced toggle change only seeds. Growth and consolidation runs record the owner they started with (`soul_conversation_id`), the prompt reads the owner the toggle picks now, and user edits take the owner from their caller.

## Background jobs

Three model-driven jobs change a Soul. Each has a pure tool contract and reducer here, a durable run port whose checkpoint stores typed results before anything is applied, and a coordinator in `lettuce-app`. Tool descriptions and prompt lines are catalog keys the application resolves; the crate only decides the shape.

### Growth

After a successful memory run, growth reads fresh memories and records new Soul facts.

1. `growth_prompt_facts` builds the prompt input from the effective Soul: the categories in `GROWTH_PROMPT_CATEGORIES` order, the effective facts and the first `MAX_GROWTH_MEMORIES` (16) non-blank memories, numbered from zero. Labels, line formats and empty fallbacks are catalog text in `prompt_app_companion_runtime`.
2. `growth_tool_request` is the required `record_growth` tool. `parse_growth_proposals` takes the first matching native call, or the structured-text fallback, and reduces it into typed proposals. Source indices fall back to all supplied memory ids.
3. `CompanionGrowthRun` freezes the prompt input, resolved model profile, the successful memory attempt, the Soul snapshot and revision and a stable Soul operation id. Its only checkpoint stores the reduced proposals before any Soul mutation, and the proposals then go through `prepare_growth_change_set`.

The 16-memory limit is an input limit of the coordinator; it does not limit how many facts a run may propose. A growth or consolidation change set is always atomic at one expected revision.

### Consolidation

When a Soul has twelve active changeable facts (`consolidation_ready`, `CONSOLIDATION_THRESHOLD`), consolidation asks the model to merge them. `consolidation_prompt_facts` supplies the authored, core and growth facts, `consolidation_tool_request` is the required `consolidate_soul` tool, and `parse_consolidation_proposal` reads the first matching native call or the structured-text fallback into core adjustments and retirement ids, consumed by `prepare_consolidation_change_set`. `CompanionConsolidationRun` freezes one authoritative post-growth Soul snapshot and accepts one immutable proposal checkpoint.

### Soul writer

The Soul writer drafts a companion's authored configuration from a description. `soul_writer_tool_request` is a six-tool required contract (`set_identity`, `set_authored_facts`, `set_baseline_affect`, `set_regulation_style`, `set_relationship_defaults`, `done`). `normalize_soul_writer_draft` completes a partial current JSON value with zero defaults and clamps known fields. `reduce_soul_writer_calls` applies calls in provider order until the first `done`: identity updates are non-blank partial overwrites, affect and regulation values clamp to [0, 1], closeness, trust and affection to [-1, 1], tension to [0, 1], and authored facts need confidence 0.7 and get a generated id, timestamps, a default or clamped weight and forced historical locking.

`soul_writer_prompt_values` builds the input values with blank-input fillers (`SoulWriterPromptText`, e.g. "Not provided."), the pretty current-Soul JSON and the final authoring instruction. `parse_soul_writer_fallback_calls` reads the structured fallback: JSON objects or arrays under several aliases, or `soul_ops`/`operations` XML roots, filtered to the six operations in call order, with numeric XML fields coerced and JSON-encoded authored facts passed to the same reducer.

`CompanionSoulWriterRun` is a preview run: one job-bound request freezes the primary and optional fallback profile, the prompt identity and revision, the exact input values, the normalized starting draft and the fallback format, and accepts at most eight ordered round checkpoints. Each checkpoint records which profile authored it; fallback rounds cannot go back to primary, an exact round replay is idempotent, and completion forbids later rounds. A preview has no way to change a character's Soul. The coordinator stops the model fallback on run persistence, dispatch evidence and replay cleanup failures; only provider and content failures continue to the alternate model.

### Usage

Growth and consolidation checkpoints keep optional normalized inference usage, and Soul-writer rounds keep primary and fallback usage separately; replaying a checkpoint does not count as new inference, and cached and reasoning details keep the difference between absent and zero. Separately, the coordinators admit every actual provider dispatch to a job-owned usage ledger before calling the provider and record returned usage before validating the response, so primary and fallback calls, retries, errors and cancellation each leave their own evidence. A dispatch interrupted before returning stays pending, never zero-cost or successful.

## Emotional and relationship state

`state.rs` holds the live state and its math.

- `EmotionalState` (felt, expressed and blocked `EmotionVector`s plus active signals) belongs to one conversation, so branches inside a conversation share it.
- `RelationshipState` (closeness, trust, affection, tension, stability) belongs to the companion character plus the selected persona, or an explicit default-persona scope. It is independent of the memory-sharing toggle.
- `apply_turn` applies a turn's signals: baseline affect, regulation style (`regulate_expressed`), 45-minute exponential decay (`apply_passive_decay`), volatility scaling, momentum interpolation, passive tension and stability recovery, and separate bipolar damage and recovery constants for closeness, trust and affection (`apply_bipolar_delta`).

`CompanionStateRepository` replaces both scopes in one atomic write that advances both revisions, so concurrent conversations of the same character and persona cannot lose relationship updates; an exact operation retry returns the stored receipt. It also stores the continuity episode (`CompanionContinuityEpisode`) that the prompt reads, rather than deriving it from message history.

Three prepared writes let storage commit companion state together with the conversation change it belongs to:

- `PreparedCompanionLaunch` / `CompanionConversationCreator`: the initial state and continuity sequence commit atomically with a new direct conversation. `with_time_awareness` writes an enabled `companion_clock` into the new conversation's settings when the character's config asks for it; later changes to the character leave existing conversations alone.
- `PreparedCompanionSend` / `CompanionConversationSender`: a state replacement is validated against the same conversation as the user message, and the message, generation turn and attempt, both state scopes, the operation and the outbox commit together. An exact replay does not apply the transition again.
- `PreparedCompanionContinue` / `CompanionConversationContinuer`: the same for continuations.

### Signals

`signals_from_classification` (`signals.rs`) turns emotion classifier output into a `CompanionSignalBundle`. Each `EmotionLabelScore` carries the classifier's calibrated threshold for its label (Thymos's per-class thresholds from its `labels.json`), and a label counts when `score >= threshold`. Only the first eight scored labels are considered; all 28 labels map to signals, grouped signal names are deduplicated while their numeric effects still add up, and the emotion, relationship, confidence and clamping values are fixed. `unavailable_signal_bundle` is the fallback when no model is installed.

## Prompt state

`prompt_state` (`prompt.rs`) decides what the companion prompt says, as typed facts with no model-facing text. From typed state and an explicit clock it produces:

- relationship bands: seven steps each for closeness, trust and affection (neutral floor -0.15 for closeness and trust, -0.25 for affection) and five for tension, each with its raw value for the stance line;
- the top expressed and blocked emotion dimensions above 0.08;
- the continuity episode;
- all twelve authored Soul categories in `SOUL_PROMPT_ORDER`, with effective durable facts ordered by clamped weight times confidence;
- style notes, active signals, and the regulation and reassurance cues.

The application renders each fact from the `prompt_app_companion_runtime` catalog document and inserts it through the context assembler.

## Scheduled notes

A `CompanionScheduledNote` (`scheduled_note.rs`) belongs to a character and activates once, daily, weekly, monthly or yearly. `active_scheduled_notes` filters out disabled, not-yet-available and expired notes at the supplied time; end times and recurrence windows are exclusive. Active notes are ordered by `available_at` then id, each truncated to 1000 characters, and the block is capped at 4000 characters measured on the rendered line (`scheduled_note_lines`); the line format, truncation marker and heading are catalog text passed in. `CompanionScheduledNoteRepository` offers list, upsert and delete; scheduling and frontend commands are elsewhere.

## Turn effects

A `CompanionTurnEffect` (`effect.rs`) records what one turn did to the companion, for the UI and for rewinds: the relationship delta, felt, expressed and blocked changes and ordered signal changes, copied from the one `apply_turn` transition (there is no second calculation), plus memory changes and the source message window. Its lifecycle is processing, ready or failed.

- With dynamic memory, a send keeps a hidden seed with the admitted user turn, and the visible processing effect is created only with the durable assistant message. Continuations keep a zero-delta seed with no user message. Failure or cancellation before finalization discards the seed, and exact send or finalization replays never duplicate an effect.
- The application settles effects of coalesced turns from before and after memory snapshots and exact source message ids.
- The repository lists processing effects in stable conversation, time and effect order. Those rows are the durable pending queue: worker jobs can be rebuilt after a crash without a second copy of the seed. `list_processing_for_conversation` applies status and invalidation filters before the page limit, so settled history or other conversations never hide pending work.
- A rewind after deleting messages marks affected effects invalidated through an immutable overlay. The original evidence stays for audit, and invalidated processing effects leave the queue and can no longer be settled.

Effects, seeds, deltas, memory changes and source windows are strictly serializable, and a full backup exports the same validated values the repository uses.
