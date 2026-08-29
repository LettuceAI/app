# lettuce-usage

Immutable usage events, pricing snapshots, costs, budgets, and summaries.

## Boundary

Historical calculations retain their pricing provenance.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first domain port records one immutable terminal text-inference event. An
exact retry for the same generation attempt returns the original event while
changed counters, outcome, provenance, or timestamp conflict. Provider-missing
counters remain an explicit unavailable reason; the ledger never invents zero
usage. SQLite persistence lives in `lettuce-database`. Pricing snapshots, cost
calculation, adjustments, summaries, and non-text modalities remain later
slices.
