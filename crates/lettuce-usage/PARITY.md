# lettuce-usage: legacy parity notes

Facts about how `lettuce-usage` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- Counters carry image, audio and total tokens like legacy usage records (2026-09-22). `effective_total_tokens` falls back to input plus output, as legacy did.
- The OpenRouter request-cost calculator is copied from legacy `models/pricing/calc.rs`, with its pricing and result fields from `models/types.rs`: per-token USD units, cache counter clamping, cache-write price fallback, separate reasoning, request and search charges, and the authoritative-total guard. Malformed optional prices keep legacy's zero fallback.
- `from_openrouter_job` prices cache and reasoning from the response usage as legacy did, and counts missing cache, cache-write, reasoning and web-search values as zero, as legacy's `apply_openrouter_cost_to_usage` did.
- Job cost records preserve legacy's companion response accounting independently of workflow validation.

## Deliberate differences from legacy

- Legacy counted cached prompt tokens as image tokens when a provider sent no image count; that is not reproduced.
- Negative optional prices also fall back to zero. Invalid, non-finite or negative required prices, counter overflow and non-finite totals return `None` instead of producing invalid costs or panicking. No new price or token limit is imposed.
- Legacy's staged lorebook `pipeline.rs` called the provider without recording usage, and legacy keyword generation omitted it too; both are recorded now. Legacy single-entry generation recorded primary and fallback requests before checking response success.

## Not wired yet

- Automatic pricing capture, provider price fetching, automatic inference-finalization wiring and generation enrichment, reconciliation, adjustments, budgets, summaries and non-text costing. Callers still supply verified OpenRouter pricing and cost inputs. Native-provider billing normalization is also open.
- The crate description names budgets and summaries; neither exists.

## History

- OpenAI-compatible buffered and streaming normalization captures standard cached and reasoning token details, and the ledger keeps them. This was not automatic pricing capture.
- Companion dispatch responses gained an optional provider response-body id, separate from the logical attempt and HTTP request identities; older JSON reads it as absent, primary and fallback calls keep their own ids, and settlement rejects changing a stored id.
- The staged lorebook stages, lorebook entry and keyword generation, and the Soul writer moved onto the job dispatch ledger. Evidence persistence failures are told apart from provider failures: entry and keyword generation stop on evidence failure without a false failed checkpoint or a fallback dispatch, and the Soul writer's alternate-model fallback stops on evidence, run persistence and replay cleanup failures. Fault injection shows admission failure sends zero requests, settlement failure sends one, no false checkpoint is written, and a later retry keeps the pending evidence. The staged SQLite lifecycle scenario covers all four stages, invalid planner usage, cancellation after a response, concurrent writer failure evidence and replay without duplicated charges. No new schema, worker, pricing formula or host scheduling was introduced for this.
- Database tests cover pricing retention, replay, conflicts, mismatched ownership and counters, raw event preservation, SQL immutability and unavailable usage, for both conversation and job costs, including file-backed reopen. Calculator tests cover the breakdown, authoritative totals below component costs, invalid totals, clamping, fallback and overflow.
- The previous README did not mention `AppUsageRepository`, the per-day app usage counter.
- The doc comment on `AppUsageRepository` says app usage never enters backups, but the version 2 backup graph carries `device.app_usage_days` and the restore writer inserts them. The README follows the code.
