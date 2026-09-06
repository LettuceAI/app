# lettuce-usage

Immutable usage events, pricing snapshots, costs, budgets, and summaries.

## Boundary

Historical calculations retain their pricing provenance.
Provider-reported amounts are raw evidence carried by InferenceUsage, separate
from calculated costs. Known amounts must match the supplied authoritative total
in a cost basis. This does not fetch prices or generation details automatically.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first domain port records one immutable terminal text-inference event. An
exact retry for the same generation attempt returns the original event while
changed counters, outcome, provenance, or timestamp conflict. Provider-missing
counters remain an explicit unavailable reason; the ledger never invents zero
usage. SQLite persistence lives in `lettuce-database`.

`JobUsageLedger` retains non-conversation inference dispatches for companion
growth, consolidation and Soul writing. Each dispatch has a fresh event identity
and retains the logical attempt, owning job and resolved model/provider revisions.
Admission precedes inference; a single immutable result preserves optional raw
usage, cancellation or inference failure. Pending records explicitly mean the
result is unknown. Exact admission/settlement replay is accepted; changed evidence
conflicts. Retries never overwrite earlier dispatches. The cost ledger accepts
an explicit immutable price basis for each returned response with known usage through `record_job_cost` and `get_job_cost`. It reuses
the conversation cost validation and calculator; pending, failed, cancelled and
usage-missing dispatches cannot receive a cost. Separate dispatch IDs preserve
retry and fallback charges independently. Automatic pricing capture remains pending.
Admission verifies the job exists; evidence survives normal job retention cleanup.

The pure OpenRouter request-cost calculator is copied from legacy
`models/pricing/calc.rs`, with its pricing/result fields from `models/types.rs`.
It preserves per-token USD units, cache counter clamping and cache-write price
fallback, separate reasoning/request/search charges, and the authoritative-total
guard that prevents negative completion costs. It does not fetch current prices.
Malformed optional prices retain the legacy zero fallback; negative optional
prices also fall back to zero. Invalid/nonfinite/negative required prices,
counter overflow and nonfinite calculated totals now return None instead of
producing invalid costs or panicking. No new price or token limit is imposed.

Tests cover the breakdown, authoritative totals below component costs, invalid
totals, clamping, fallback and overflow.

`UsageCostLedger` associates one immutable versioned pricing/input basis with
an existing usage event. The basis retains model/provider identities, source
description, capture time, exact prices and explicitly supplied cost counters.
Input/output counters must match the event; unavailable usage cannot be costed.
Known cache-read/reasoning counters in the event must also match the cost basis.
Missing details stay unknown; known search/cache-write counters must also match
the supplied cost basis. OpenAI-compatible buffered/streaming normalization now captures
standard cached/reasoning token details, and the usage ledger retains them.
This is not yet automatic pricing capture. Nonfinite or negative authoritative totals are rejected at
this persistence boundary rather than serialized lossily.

SQLite stores the version-1 calculation basis separately from immutable raw
usage. Exact replay returns the same basis; changing it conflicts. Reads use
the version-1 legacy calculator, never live prices. Future formula changes
must preserve that version's interpretation. Database tests cover pricing
retention, replay, conflicts, mismatched ownership/counters, raw-event preservation,
SQL immutability and unavailable usage. Provider price fetching, automatic
inference-finalization wiring, adjustments, budgets, summaries and non-text
costing remain later work.

Job costs retain the same version-1 basis in a separate table referencing the
immutable dispatch evidence. Both survive job cleanup. File-backed tests cover
reopen, exact replay, conflicting prices, ownership and all known counter
mismatches, absent usage, SQL immutability and unchanged raw evidence. This
preserves legacy companion response accounting independently of workflow
validation; callers still supply verified OpenRouter pricing and cost inputs.
Native-provider billing normalization and price fetching remain later work.

OpenRouter endpoint-price and generation-detail result types retain routed
provider identity, endpoint tags and separate native/normalized counters for the
provider adapter's billing reads. They do not select a provider or overwrite
immutable event counters. Automatic capture and reconciliation remain pending.

Companion dispatch responses also retain an optional provider response-body ID,
separate from logical attempt and HTTP request identities. Older JSON defaults
to None. Primary/fallback calls preserve their own IDs; settlement rejects
changing a stored ID. This supplies generation lookup identity without altering
usage counters. Automatic generation enrichment remains pending.

`UsageCostBasis::from_openrouter_job` creates a basis from one matched routed
endpoint and fetched generation evidence. The optional evidence is stored in
the existing versioned basis JSON; older manual bases deserialize unchanged.
Native prompt/completion counts are required. Native cache/reasoning and total
cost take precedence over response details, retaining both sources separately.
All remaining auxiliary counts must be known; missing details do not become
zero. No/ambiguous provider match leaves cost unavailable. Matching is exact
case-insensitive provider/display name, without punctuation stripping or a
first-provider fallback. The existing calculator and its authoritative-total
guard remain unchanged. Generation-enriched bases are job-only until other
usage records retain response IDs.

Staged lorebook planner, writer/refinement and coherence execution now reuse
the existing job-dispatch usage ledger before provider invocation and before
response reduction/checkpointing. Every actual dispatch preserves its response
ID and optional usage, including invalid responses and project cancellation
during inference; returned transport failures remain explicit failed evidence.
Checkpoint replay creates no second dispatch record. Existing successful
checkpoint usage stays unchanged. The staged SQLite lifecycle scenario covers
all four stages, invalid planner usage, cancellation after response, independent
concurrent-writer failure evidence and replay without duplicated charges.

Legacy staged pipeline.rs called the provider without recording usage; recording
these dispatches corrects that accounting omission. Legacy single-entry primary
and fallback requests recorded usage before checking response success; their
new dispatch-ledger integration remains a separate follow-up. No new schema,
worker, pricing formula or host scheduling was introduced.

Lorebook entry and keyword native/fallback executions also use job dispatch
evidence. The shared helper distinguishes evidence persistence failure from
provider failure. Entry/keyword stop on evidence failure without writing a
false failed-native checkpoint or dispatching a fallback; provider failures
retain the existing fallback policy. Primary/fallback IDs and optional usage
remain separate, and successful replay adds no dispatch. Legacy entry generation
recorded both requests before response validation; legacy keyword generation
omitted that recording, which this corrects.

The same distinction fixes Soul-writer alternate-model fallback after evidence
or run-persistence/replay-cleanup failure. Other companion/staged callers retain
their existing public provider-error mapping. Fault-injection scenarios prove
admission failure sends zero requests, settlement failure sends one, no false
checkpoint is written, and later retry preserves the pending evidence. Existing
entry tests also prove provider-error fallback, cancellation and replay.
