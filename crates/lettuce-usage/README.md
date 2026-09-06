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
