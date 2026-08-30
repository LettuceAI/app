# lettuce-companions

Relationship events, projections, growth policies, milestones, and scheduled effects.

## Boundary

Does not read chat storage or invoke models directly.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

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
rendering, job coordination, database adapters, and frontend events remain
outside this slice.

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
a direct conversation.

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
assembler insertion, companion prompt-template selection, scheduled notes, and
time-awareness clock resolution remain later application slices.

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
copying the legacy summary formula. Durable post-turn scheduling and recovery
discovery remain application-owned follow-up work.
