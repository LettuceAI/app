# lettuce-memory

Memory provenance, extraction, retrieval, consolidation, and vector-index ports.

## Boundary

Source truth is independent from rebuildable embeddings.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first direct/group dynamic-memory slice now provides versioned required
tool declarations, strict typed arguments and outcomes, a deterministic ordered
reducer over revisioned memory-space snapshots, and an atomic compare-and-apply
repository port. It preserves duplicate suppression, low-confidence demotion,
cycle-start hard-delete limits, pin protection, hot-token budgeting, and
capacity trimming. The verified scenarios are pinned in
`fixtures/legacy-import/dynamic-memory-tool-scenarios-v1.json`.
Create calls also preserve the model-selected transcript message ID on the
authoritative memory item. This source identity is optional for the shared
direct/group tool contract and is the attribution boundary used by companion
post-turn effect settlement when several turns are coalesced into one cycle.

The new contract deliberately replaces ambiguous legacy text or six-digit
delete targets with stable memory UUIDs, reports missing targets explicitly,
and applies one validated change set instead of mutating session projections in
place. Companion-required source validation and supersession, provider-driven
category repair, and UI events remain later slices. ONNX inference runtime
ownership belongs in
`lettuce-embeddings`; this crate consumes typed duplicate evidence through the
application coordinator.
Semantic duplicate hints are qualified typed evidence: stable target ID,
embedding source revision, supported dimension count, cosine score, and applied
threshold. The reducer rejects unqualified evidence and still verifies that the
target exists in its current snapshot.

The SQLite `MemoryRepository` implementation and first admitted-round
application handler are now wired through `lettuce-database` and `lettuce-app`.
Every launched direct/group conversation with resolved manual or dynamic memory
receives one normalized memory space in the same creation transaction; the
repository resolves that authoritative space by `ConversationId`, removing the
previous test-only requirement for callers to invent a space ID.
The domain-owned cross-aggregate commit port carries one optional memory change
plus an optional planned-memory revision and a complete terminal
tool-transition batch. Infrastructure must check that revision even for a
no-op reduction and commit both sides or neither; orchestration and provider
continuation remain application-owned.
The domain also owns an immutable dynamic-memory preparation-plan port. A plan
captures its attempt/job, first execution ordinal and memory revision, policy
and duplicate threshold, ordered execution IDs, and every create seed, exact source text, embedding
source/dimensions, and qualified semantic evidence. Exact retries are
idempotent per provider round; changed identity conflicts so interrupted
recovery cannot silently recompute evidence against a different snapshot.
The repository also returns every plan in execution order so restart logic can
rebuild all already-settled provider context before it resumes the latest tail.
The same port exposes one atomic interrupted-parent to immediate-child recovery
operation. It returns newly identified running child executions plus their
remapped immutable plan, while preserving exact call arguments and prepared
semantic evidence from the terminal parent.

Background post-turn extraction now has a separate memory-owned durable run
boundary instead of fabricating a visible conversation generation turn. A run
freezes the authoritative starting memory snapshot, ordered source-message
roles, effective times, and active revision/candidate window, complete resolved model/profile snapshot,
frozen companion time-awareness mode, and its matching required dynamic-memory
tool contract. Time-aware runs require source attribution; other runs keep it optional.
Created time-aware memories retain the selected source role, observed effective
time, and legacy `turn` precision together; non-time-aware memory keeps those
temporal fields empty.
Companion runs also freeze whether create calls may declare `supersedes` IDs.
The reducer keeps only existing active targets, links each replaced item to the
new memory at creation time, omits superseded entries from later manager input,
and retains only the latest forty superseded records as legacy does. Other
direct/group runs do not advertise or apply supersession. The run also freezes
the selected legacy structured fallback format. This crate owns the copied
JSON/XML operation parsers and fallback prompts; an empty operation document is
a valid no-change response. Conversation-owned cumulative summaries retain the
legacy bounded text, token count, exact ordered source cursor, and root memory
CAS. Each background run freezes the resolved message interval and exact
half-open summary window so recovery cannot advance it after its own summary
checkpoint.
Each run may atomically checkpoint one validated cumulative summary before its
first memory-tool round. The checkpoint retains the exact provider-neutral
request, usage, provider request ID, resulting root revision, and summary text;
an exact replay returns it without a second summary CAS.
The memory boundary also owns the typed legacy `auto`/`askFirst`/`manual` run
mode and the conversation-owned pending-approval port. Its prompt threshold is
the copied interval rule: after one prompt, another is due only when another
full interval of unsummarized messages accumulates. Skip durably retains that
baseline and records the legacy skipped state; a consumed approval is removed
only after its forced recent-window job is admitted.
Retryable attempts own CAS lifecycle state and immutable ordered
provider rounds/calls. Each round also freezes the exact provider-neutral
request context used for its inference, so continuation and recovery do not
re-render mutable prompts or pre-round memory state. Interruption recovery
copies the exact admitted provider history and any settled typed-result
checkpoints into the processing child. This preserves the legacy cycle's frozen
window/model behavior while deliberately separating it from assistant-message
generation. The same port now owns atomic background round settlement: an
optional memory change and every ordered typed result commit together, and an
exact retry returns the stored settlement. Background provider continuation now
replays those calls/results into the frozen context and admits the next bounded
round. The starting snapshot remains available after restart for the existing
companion effect delta/summary calculation; host worker wiring remains a later
application slice.
Delete-after rewind now has a narrow idempotent repository contract. It restores
the earliest invalid run's immutable starting memory snapshot, restores the
latest earlier summary checkpoint (or clears the summary), and retires the
named companion effects in the same storage transaction. Selecting the invalid
suffix from tombstoned message IDs remains application-owned.
