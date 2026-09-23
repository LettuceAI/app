# lettuce-companions

Relationship events, projections, growth policies, milestones, and scheduled effects.

## Boundary

Does not read chat storage or invoke models directly.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Growth and consolidation proposal checkpoints now retain optional normalized
inference usage. Soul-writer rounds retain primary and structured-fallback usage
separately, without changing draft reduction or counting replay as new inference.
The existing InferenceUsage value is serializable; missing checkpoint usage
defaults to None for older saved runs. Cached/reasoning details preserve absence
versus explicit zero. SQLite app scenarios verify persisted details in all three
workflows. A separate job-owned usage ledger admits each actual inference dispatch
before calling the provider and records returned usage before response validation
or checkpoint reduction. Primary and fallback calls, retries, inference errors,
and cancellation therefore retain independent evidence. An interrupted dispatch
without a returned result stays pending, not zero-cost or successful. This covers
growth, consolidation and Soul-writer coordinators; provider-internal retries and
automatic pricing capture are not implemented by this ledger.

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.

The verified companion growth, consolidation, and Soul-writer behavior is
pinned in `fixtures/legacy-import/companion-tool-scenarios-v1.json`. The pure
typed Soul/growth policy is active: it carries forward the legacy category
mutability table, exact `0.55`/`0.70`/`0.85`/`1.0` confidence thresholds,
confidence/weight clamping, validity rule, semantic-slot supersession, locked
protection, twelve-active-fact consolidation threshold, and forty-item
superseded-history bound. The typed boundary deliberately makes a proposed
growth or consolidation change set atomic at one expected revision. Legacy's
sixteen-item limit remains a fresh-memory coordinator input limit and is not
misapplied to the number of proposed Soul facts. Provider calls, prompt
document rendering, job coordination, database adapters, and frontend events
remain outside this domain.

The domain now exposes the same required `record_growth` tool contract and
reduces the first matching native call or legacy structured-text fallback into
typed proposals. Source indices preserve the legacy fallback to all supplied
fresh-memory IDs; the existing Soul reducer remains the only policy path.
Its growth prompt facts (`growth_prompt_facts`) reuse the existing
effective-Soul projection and copy the legacy category order, effective facts
and first sixteen non-blank memories; labels, line formats (zero-based memory
numbering), empty fallbacks and the tool description are catalog text the
application renders (`prompt_app_companion_runtime`).
The domain-owned growth-run port freezes that prompt input, resolved profile,
successful memory attempt, Soul snapshot/revision, and a stable Soul operation
ID. Its only checkpoint immutably stores the reduced typed proposals before
any Soul mutation.

The exact legacy consolidation input is also available without a second policy
path: the twelve-active-changeable-fact readiness check, authored/core/growth
prompt facts (rendered by the application from the catalog), required
`consolidate_soul` schema, first matching native call,
structured-text fallback, core adjustment filtering, and retirement IDs. Its
typed output is consumed by the existing atomic
`prepare_consolidation_change_set`; confidence thresholds, locked retirement
protection, core-category filtering, and revision behavior stay there.
The companion-owned consolidation-run port freezes one authoritative
post-growth Soul snapshot and accepts one immutable typed proposal checkpoint;
storage and application crates implement admission and execution around that
domain boundary.

The pure Soul-writer boundary copies the legacy six-tool required contract and
working-draft reducer. A partial current JSON value is completed with the same
zero defaults, known fields are preserved and clamped, calls apply in provider
order, and the first `done` suppresses later calls. Identity updates remain
nonblank partial overwrites; affect and regulation values clamp to `[0,1]`;
relationship closeness, trust, and affection clamp to `[-1,1]`; and tension
clamps to `[0,1]`. Authored facts retain the legacy `0.7` confidence gate,
weight default/clamp, generated identity and timestamps, and forced historical
locking. Prompt rendering, recursion, structured fallback, model selection,
durable proposal application, and frontend events remain application work.
The writer prompt-value helper also preserves the exact legacy blank-input
fallbacks, pretty current-Soul JSON, and final authoring instruction. Its
structured fallback accepts the legacy JSON object/array aliases and
`soul_ops`/`operations` XML roots, filters to the six known operations,
preserves call order, coerces numeric XML fields, and keeps JSON-encoded
authored facts for the same reducer. Provider fallback dispatch and recursion
remain outside this pure boundary.
The domain also owns the explicit preview run port: one job-bound request
freezes the primary and optional fallback profile, prompt identity/revision,
exact input values, normalized starting draft, fallback format, and at most
eight ordered round checkpoints. Each checkpoint records whether the primary
or fallback profile authored it, fallback rounds cannot return to primary,
exact round replay is idempotent, completion forbids later rounds, and this
preview state has no character-Soul mutation capability. The final
instruction (`SOUL_WRITER_FINAL_INSTRUCTION_KEY`), structured fallback
instructions (`soul_writer_fallback_prompt_key`,
`soul_writer_fact_fallback_prompt_key`) and six tool descriptions
(`SOUL_WRITER_TOOL_TEXT_KEYS`) are catalog keys the application resolves, which
also supplies the `Not provided.` / `No special direction.` fillers through
`SoulWriterPromptText`; the parser and reducer stay here.

Character-owned scheduled notes now copy the legacy fields and recurrence
semantics for one-time, daily, weekly, monthly, and yearly activation. Disabled,
not-yet-available, and end-exclusive expired notes are filtered at the supplied
effective time; recurrence windows are also end-exclusive. Active notes keep
the legacy `available_at` then ID order, 1000-character per-note truncation
and 4000-character block cap measured on the rendered line; the line, the
truncation marker and the background-context heading are catalog text the
application passes in (`scheduled_note_lines`).
The repository port owns list/upsert/delete only; host scheduling and frontend
commands remain outside this domain.

Durable Soul state is character-owned, matching legacy continuity: companion
Soul growth is shared across that character's sessions regardless of the
separate shared-memory setting. `SoulRepository` owns create/load and atomic
expected-revision change-set application. The SQLite adapter stores facts and
their source/supersession lists in normalized tables, applies the pure policy
inside one immediate transaction, bounds superseded history to forty entries,
and records immutable idempotency receipts. Session, persona, relationship,
prompt, provider, and frontend coordination remain outside this persistence
slice.

Authored companion Soul configuration keeps all twelve legacy identity text
categories and typed authored facts. Initial durable state copies the legacy
`normalize_for_storage` order directly: generate a UUID for a blank ID, clamp
confidence and weight, default a blank slot to its category, derive a zero
evidence count from source IDs, fill zero creation/valid-from timestamps, and
lock historical facts. The serialized config retains legacy camel-case field
names.

Pure typed emotion and relationship state copies the legacy defaults and math
directly: baseline affect, regulation style, expressed/blocked affect,
45-minute exponential decay, volatility scaling, momentum interpolation,
passive tension/stability recovery, and the distinct closeness/trust/affection
bipolar damage and recovery constants. Authored configuration owns the exact
baseline affect, regulation style, and relationship defaults consumed by these
helpers.

`CompanionStateRepository` separates durable ownership the same way as legacy:
immediate emotional state and signals belong to one conversation, while the
relationship belongs to the companion character plus the selected persona (or
the explicit default-persona scope). Relationship continuity is independent of
the dynamic-memory sharing flag. Both revisions advance in one atomic replace,
so concurrent sessions for one character/persona cannot lose relationship
updates; exact operation retries return an immutable receipt. Branches inside
one conversation naturally retain that conversation's immediate emotional
state. `PreparedCompanionLaunch` and `CompanionConversationCreator` now let the
application freeze that initial state and let storage commit it atomically with
a direct conversation. The same launch boundary persists the character/persona
continuity sequence, and the state repository exposes the stored episode to
prompt assembly without deriving it from message history.

The pure emotion-classifier reducer copies the legacy GoEmotions behavior
directly: only the first eight scored labels are considered, per-label
thresholds remain `0.18`/`0.22`/`0.55`, grouped signal names are deduplicated
while their numeric effects still accumulate, and the exact emotion,
relationship, confidence, clamping, and unavailable-model fallback values are
preserved. Verified ONNX tokenization/model execution now lives behind the
auxiliary-analysis boundary in `lettuce-embeddings`; model discovery, prompt
rendering and direct companion turn wiring are complete. Provider/job
coordination and frontend events remain deferred.

`PreparedCompanionSend` and `CompanionConversationSender` provide the atomic
write boundary needed by turn coordination: a prepared state replacement is
validated against the same conversation as the user send, and storage commits
the message, generation turn/attempt, dual-scope companion state, operation,
and outbox together. Exact conversation-operation replay does not reapply the
state transition. Classification and transition preparation are application
work rather than storage behavior and are now wired for direct user sends.

The pure prompt-state function (`prompt_state`) holds no model-facing text: it
decides the legacy relationship bands, top expressed/blocked dimensions (0.08
floor), continuity episode, all twelve authored Soul categories with effective
durable facts ordered by clamped weight-times-confidence, style notes, active
signals and the regulation/reassurance branches, and returns them as typed
facts. The application renders each fact from the built-in
`prompt_app_companion_runtime` catalog document with legacy wording. It
accepts typed state and an explicit effective clock. Context
assembler insertion, companion prompt-template selection, scheduled notes,
continuity episode hydration, and time-awareness clock resolution are wired.

The authored companion configuration also preserves the legacy nested
`prompting.promptTemplateId` and `prompting.styleNotes` fields. The template ID
remains a typed prompt-document reference, while style notes feed the exact
prompt-state renderer without reinterpretation.
`CompanionSoulConfig.time_awareness` (legacy `timeAwareness` or
`context.timeAwareness`) is the default for new conversations, like legacy's
seeded session preference: a companion launch writes an enabled
`companion_clock` into the new conversation's settings
(`PreparedCompanionLaunch::with_time_awareness`), and later changes to the
character leave existing conversations alone.
`share_memory_across_chats` is legacy `memory.sharedAcrossSessions` (default
on; a companion-mode character whose legacy config is missing or unreadable
imports it off, as legacy read it; a null or mistyped `memory` or
`sharedAcrossSessions` also imports off, as legacy's failed parse did).
Legacy companion sessions keep their own memories beside the imported pool.
`share_soul_growth_across_chats` is new
(default on). The other legacy `memory` keys were never read and are recorded
as dropped. Memory follows the memory toggle (`lettuce-database` README). Soul growth
follows `SoulOwner::for_conversation`: the character's Soul while it shares,
else `SoulOwner::Conversation`, the conversation's own. Growth and
consolidation runs record the owner they started with
(`soul_conversation_id`), the prompt reads the owner the toggle picks now, and
the user edits take the owner from their caller. Turning the toggle off gives
every companion conversation without a Soul of its own (and one created while
it is off) a copy of the shared Soul, while a conversation that kept its Soul
from an earlier off period resumes it; turning it back on makes the most
recently updated Soul of a conversation that still exists the shared one when
it is newer and differs, keeping the conversation Souls (user decisions
2026-09-23). Leaving or entering companion mode changes nothing, and a synced
toggle change only seeds. Known gap: a growth run admitted before a flip
still applies to the owner it started with.

Typed companion turn effects copy their relationship, felt/expressed/blocked,
and ordered signal changes directly from the existing legacy-math transition;
there is no second calculation path. The domain port exposes the durable
processing/ready/failed lifecycle, typed memory changes, and source window.
Dynamic-memory sends retain a hidden seed with the admitted user turn, while a
visible processing effect is created only with the durable assistant message.
Dynamic-memory companion continuations retain the legacy zero-delta seed and
no user-message owner.
Failure or cancellation before finalization discards the seed, and exact send
or finalization replay does not duplicate an effect.
The application terminal coordinator now settles coalesced effects from
authoritative before/after memory snapshots and exact source-message IDs while
copying the legacy summary formula. Worker execution remains application-owned
follow-up work.

The repository can list bounded processing effects in stable
conversation/time/effect order. Those normalized rows are the durable pending
queue authority; runtime worker jobs can be reconstructed after process loss
without storing a second copy of the turn seed or source identities.
`list_processing_for_conversation` lists one conversation's pending effects with
the status and invalidation filter applied before the page limit, so settled
history or other conversations can never hide pending work.
Delete-after rewind marks affected effects as invalidated through an immutable
overlay. Their original processing/ready/failed evidence remains unchanged for
audit, while invalidated processing effects no longer appear in the worker
queue and cannot be settled later.

The effect, seed, relationship delta, memory-change and source-window values are
strictly serializable for the versioned full-profile backup boundary. Serialization
does not add a second effect model or change transition calculations; the same
validated values used by the repository are exported and checked on backup open.

Soul-writer model fallback stops on run persistence, dispatch evidence and replay
cleanup failures. Only provider/content failures continue through the existing
alternate-model policy. Admission/settlement fault injection verifies no extra
provider call or false checkpoint; a successful later retry retains the pending
usage record independently from completed dispatches.

Soul growth user edits (2026-09-23): `SoulChangeSet.user_edits` carries legacy
`companion_clear_soul_growth` (every entry, authored ones and locked ones
included), `companion_remove_soul_growth` (one entry, now by its stable id
instead of list position) and `companion_set_soul_growth_lock`.
`prepare_user_edit` returns `None` for an edit that changes nothing, where
legacy answered without a write. The edits go through the CAS-guarded,
receipt-recorded `SoulRepository::apply`. Change hashes include the edits only
when there are any, so existing receipts keep their hashes. A locked fact still
cannot be superseded by growth or consolidation. User edits never prune superseded
history (legacy's edits did not bound it). Known limit: sync treats an
authored-only, unsuperseded Soul (including an empty one, which is what a
companion without authored facts starts from on every device) as an
untouched seed that loses to a concurrent learned change. So a clear, or an
edit that leaves only authored facts, made while another device grows the
Soul concurrently, is overridden by that growth. Telling them apart needs a
synced "edited by the user" marker in the Soul payload.
