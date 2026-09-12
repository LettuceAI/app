# lettuce-memory

Memory provenance, extraction, retrieval, consolidation, and vector-index ports.

## Boundary

Source truth is independent from rebuildable embeddings. Each authoritative
space revision also has a stable UUIDv5 identity derived from the space ID and
numeric revision. Conversation preparation can therefore record the exact
revision it used without creating a parallel revision store or a random
reference.

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

Every memory item carries a legacy six-digit `short_id`, unique within its
space and fixed at creation (derived from the memory ID, probing upward past
ids in use). Models see `[short_id] text` and the legacy tool contract:
`delete_memory` takes `text` (a six-digit id, or the exact memory text),
`pin_memory`/`unpin_memory` take `id`, and `supersedes` lists ids. Arguments
keep the model's raw `MemoryReference`, which the reducer resolves against the
items current when the call applies (legacy cleaning of `# * " ' [ ] ( )`;
stable UUIDs still resolve). Missing targets are reported explicitly and the
batch applies one validated change set instead of mutating session projections
in place. Tool and parameter descriptions are catalog keys
(`DYNAMIC_MEMORY_TOOL_TEXT_KEYS`) resolved by the application, with direct and
group variants; runs freeze the resulting request and validation compares its
description-free shape. Arguments parse as leniently as legacy: unknown keys are
ignored, a non-UUID `source_message_id` or non-boolean `important` falls back to
its default, and `confidence` is clamped to 0..1. Group contracts never carry
source attribution or supersession. A call that cannot be applied (missing
or invalid text, missing or unknown category, missing target,
undeclared tool name) settles as `Skipped` with a typed reason instead of
failing the round, as legacy skipped such calls; a round may repeat `done`.
A created memory's text goes through legacy's checks first
(`normalize_memory_text`): a surrounding code fence and any thinking section
are stripped, whitespace collapses, and empty, over-280-byte, refusal-marker or
meta-marker text is skipped with its own reason, including legacy's false
positives (a memory containing `i cannot` or `user:` is dropped). The summary
validator shares the same normalization. Interleaved thinking tags of different
kinds are stripped pair by pair instead of by earliest opening tag, which
differs from legacy only for malformed output. A create skipped for a missing or
invalid category is re-tagged by legacy's single-tool repair contract
(`retag_memory`, required tool choice, the six categories as an enum), whose
prompt, parameter texts and structured fallback are catalog keys
(`MEMORY_REPAIR_TOOL_TEXT_KEYS`, `memory_repairs_fallback_prompt_key`); a repair
request that answers with nothing falls back to legacy's keyword buckets
(`guess_memory_category`, first matching bucket wins). Still missing from legacy:
the raw-arguments fallback for a missing `text` argument and legacy-shaped tool
results (six-digit ids, `updatedMemories`). Structured
fallback prompts are catalog keys (`memory_operations_fallback_prompt_key`).
Runs store up to 64 inference rounds and 4096 tool calls per attempt. Companion-required source validation and supersession
and UI events remain later slices. ONNX inference runtime
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
The retrieval-access port owns the narrow mutation applied after assistant
generation selects dynamic memories. It binds the exact ordered selection,
source revision and access time to one conversation turn attempt, promotes
selected cold items, updates only selected access metadata, and returns the
resulting root revision plus the ids it promoted from cold. Exact retries return the immutable receipt; stale
revisions or changed selections conflict before any item changes.

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

The authoritative space, item, summary and retrieval-access types also form the
versioned full-profile backup document. Backup validation reuses their existing
invariants and preserves the exact access metadata that affects later retrieval;
it does not recalculate scores. Separate versioned backup documents preserve
the exact embedding projection cache and the durable approval,
background run, retry, round, result and summary-checkpoint graph without
executing any workflow during export.
