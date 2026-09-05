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
workflows. These are successful checkpoint facts, not a complete attempt ledger:
responses rejected or cancelled before checkpointing and primary calls followed
by a failed fallback still need durable usage recording. Legacy recorded usage
before checking response success; full parity requires that remaining path.

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
Its growth prompt values also reuse the existing effective-Soul projection and
copy the legacy category order, labels, fact-line formatting, empty fallbacks,
and zero-based sixteen-memory list.
The domain-owned growth-run port freezes that prompt input, resolved profile,
successful memory attempt, Soul snapshot/revision, and a stable Soul operation
ID. Its only checkpoint immutably stores the reduced typed proposals before
any Soul mutation.

The exact legacy consolidation input is also available without a second policy
path: the twelve-active-changeable-fact readiness check, authored/core/growth
prompt values, required `consolidate_soul` schema, first matching native call,
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
preview state has no character-Soul mutation capability. The structured
fallback instructions are copied byte-for-byte from legacy alongside the
parser and reducer; application code only chooses when to issue them.

Character-owned scheduled notes now copy the legacy fields and recurrence
semantics for one-time, daily, weekly, monthly, and yearly activation. Disabled,
not-yet-available, and end-exclusive expired notes are filtered at the supplied
effective time; recurrence windows are also end-exclusive. Active notes keep
the legacy `available_at` then ID order, 1000-character per-note truncation,
4000-character block cap, bullet format, and exact background-context heading.
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

The pure prompt-state renderer copies legacy output byte-for-byte: relationship
bands, top expressed/blocked dimensions, interaction and continuity wording,
all twelve authored Soul categories, effective durable facts ordered by
clamped weight-times-confidence, style notes, active signals, and regulation
branches. It accepts typed state and an explicit effective clock. Context
assembler insertion, companion prompt-template selection, scheduled notes,
continuity episode hydration, and time-awareness clock resolution are wired.

The authored companion configuration also preserves the legacy nested
`prompting.promptTemplateId` and `prompting.styleNotes` fields. The template ID
remains a typed prompt-document reference, while style notes feed the exact
prompt-state renderer without reinterpretation.

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
Delete-after rewind marks affected effects as invalidated through an immutable
overlay. Their original processing/ready/failed evidence remains unchanged for
audit, while invalidated processing effects no longer appear in the worker
queue and cannot be settled later.
