# lettuce-usage

Records of what inference cost: immutable usage events for conversation generations, a ledger of every provider dispatch made by background jobs, cost records that tie an event to the exact prices it was calculated with, the request cost calculator, and the per-day app usage counter.

The crate defines the ledgers as ports and the calculation as pure functions; `lettuce-database` implements the ports in SQLite, and `lettuce-app` writes to them from generation and job coordinators. It never fetches prices or generation details itself: callers supply verified pricing. The raw counters (`InferenceUsage`) are defined in `lettuce-conversations` next to the generation lifecycle that produces them.

## Principles

- Evidence is immutable. An event, a dispatch result or a cost basis is written once; an exact retry returns the stored record and any changed evidence is a conflict.
- Missing is not zero. A provider that reported no counters gives an explicit unavailable reason, and the ledger never invents zero usage. Absent cache or reasoning details stay distinct from an explicit zero.
- Raw counters and calculated cost are kept apart. Provider-reported amounts are evidence on the usage record; a calculated cost lives in a separate record with the prices it used, so history keeps its pricing provenance and never reads live prices.

## Conversation usage

`UsageLedger` records one terminal `UsageEvent` per generation attempt (the `UsageRecord` from `lettuce-conversations`: counters, outcome, provenance and time). `get_for_attempt` reads the event an attempt already owns, so a settlement path can reuse it instead of recording a conflicting one.

Counters include input, output, image, audio and total tokens and optional cached-input and reasoning details. `total_tokens` is only a total the provider reported; readers use `InferenceUsage::effective_total_tokens` (the reported total, else input plus output). When several calls are combined, image and audio counts combine only if every call reported them, and a total if any call did.

## Job dispatch usage

`JobUsageLedger` keeps one `JobInferenceUsage` per actual provider dispatch made by a background job: companion growth, consolidation and Soul writer, lorebook entry, keyword and staged generation, the creation helper, and conversation memory continuation. Each dispatch has its own event id and records the logical attempt, owning job, and model and provider account with their revisions.

1. `admit_job_usage` is called before the provider is invoked, and checks that the job exists.
2. `settle_job_usage` records one immutable result: a response with optional usage and the provider's response id, an inference failure, or cancellation. It is called before the response is validated or reduced into a checkpoint, so invalid responses are recorded too.
3. A dispatch without a result is pending, which means the outcome is unknown, never free or successful.

Retries and fallbacks get new dispatch ids and never overwrite earlier ones, so each charge stays separate. A replayed checkpoint makes no new dispatch. Dispatch evidence survives normal job retention cleanup. A conversation's terminal event may aggregate the same dispatches that the job ledger holds, so reports must not add both as independent charges.

## Costs

`UsageCostBasis` (`costing.rs`) is a versioned, immutable record of how a cost was computed: model and provider identities, a description of the price source, capture time, the exact prices and the cost counters. `UsageCostLedger` stores one per usage event, and `record_job_cost` / `get_job_cost` one per job dispatch that returned known usage; pending, failed, cancelled and usage-missing dispatches cannot be costed.

The basis must agree with the evidence it prices: input and output counters must match the event, known cache-read and reasoning counts must match too, and known search and cache-write counts must match the basis. Unknown details stay unknown. Non-finite or negative authoritative totals are rejected at the persistence boundary. Stored bases are read with the calculator version they were written with (version 1), never with live prices, so a future formula change must keep version 1's interpretation.

### Calculator

`calculate_openrouter_request_cost` (`pricing.rs`) computes a request's cost from `ModelPricing` and counters: per-token USD prices, cache counters clamped to the prompt, cache-write price falling back to the prompt price, separate reasoning, request and web-search charges, and an authoritative-total guard that keeps the completion cost from going negative when the provider's total is lower than the components. Malformed or negative optional prices count as zero; invalid, non-finite or negative required prices, counter overflow and non-finite totals return `None` instead of an invalid cost.

### OpenRouter evidence

`OpenRouterEndpointPricing` and `OpenRouterGenerationDetails` (`openrouter.rs`) hold what the provider adapter reads for billing: the routed provider, endpoint tags, and native and normalized counters kept separately. `UsageCostBasis::from_openrouter_job` builds a basis from the matched endpoint and fetched generation details, stored in the same versioned basis JSON:

- Native prompt and completion counts and the native total cost win over the response's counts, and both sources are kept. A missing native count falls back to the response's.
- Cache and reasoning counts come from the response usage. Missing cache, cache-write, reasoning and web-search counts count as zero.
- The endpoint is matched by provider name on ASCII letters and digits, ignoring case; the first match wins, and a missing or unmatched name falls back to the first endpoint, which the basis records.

Generation-enriched bases exist for job dispatches only, since those are the records that keep a provider response id.

## App usage

`AppUsageRepository` adds up the time the app was in use per local calendar day (`YYYY-MM-DD`) in one atomic step and lists every day oldest first. It is per install and never syncs. A version 2 backup carries the days in its device state, and a restore writes them back.
