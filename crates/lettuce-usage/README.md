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
usage. SQLite persistence lives in `lettuce-database`.

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
totals, clamping, fallback and overflow. This calculator is not yet connected
to persisted pricing snapshots or ledger adjustments. Provider price fetching,
snapshot persistence, budgets, summaries and non-text costing remain later work.
