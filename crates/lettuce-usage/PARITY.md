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

- Adjustments, budgets, summaries and non-text costing remain open. Native-provider billing normalization is also open.
- The crate description names budgets and summaries; neither exists.

## History

The shared text inference writer now snapshots model and provider display metadata before dispatch and captures response identity and reported finish reason at settlement. Chat attribution also retains its character and direct or group operation, as legacy copied them into usage (`old-code/src-tauri/src/chat_manager/service.rs:515-538`, `old-code/src-tauri/src/usage/tracking.rs:45-57`). Unreported successful finish reasons remain absent; legacy normalized reported aliases in `tracking.rs:82-95`. Other workflow attribution, image dispatch snapshots and failed terminal snapshots are still to be wired.

Terminal usage can persist frozen display and result snapshots alongside the counters copied by legacy (`old-code/src-tauri/src/chat_manager/service.rs:515-538,617-646`). Optional fields retain unknown values, including memory and summary counts; legacy left those counts absent unless positive (`service.rs:550-566`). This storage contract does not itself populate snapshots in inference coordinators.

- OpenAI-compatible buffered and streaming normalization captures standard cached and reasoning token details, and the ledger keeps them. This was not automatic pricing capture.
- Companion dispatch responses gained an optional provider response-body id, separate from the logical attempt and HTTP request identities; older JSON reads it as absent, primary and fallback calls keep their own ids, and settlement rejects changing a stored id.
- The staged lorebook stages, lorebook entry and keyword generation, and the Soul writer moved onto the job dispatch ledger. Evidence persistence failures are told apart from provider failures: entry and keyword generation stop on evidence failure without a false failed checkpoint or a fallback dispatch, and the Soul writer's alternate-model fallback stops on evidence, run persistence and replay cleanup failures. Fault injection shows admission failure sends zero requests, settlement failure sends one, no false checkpoint is written, and a later retry keeps the pending evidence. The staged SQLite lifecycle scenario covers all four stages, invalid planner usage, cancellation after a response, concurrent writer failure evidence and replay without duplicated charges. No new schema, worker, pricing formula or host scheduling was introduced for this.
- Database tests cover pricing retention, replay, conflicts, mismatched ownership and counters, raw event preservation, SQL immutability and unavailable usage, for both conversation and job costs, including file-backed reopen. Calculator tests cover the breakdown, authoritative totals below component costs, invalid totals, clamping, fallback and overflow.
- The previous README did not mention `AppUsageRepository`, the per-day app usage counter.
- The doc comment on `AppUsageRepository` says app usage never enters backups, but the version 2 backup graph carries `device.app_usage_days` and the restore writer inserts them. The README follows the code.

Usage clear-before preserves the strict cutoff from `old-code/src-tauri/src/usage/repository.rs:503-518`. It deliberately skips unsettled dispatches and nonterminal owners, commits costs and exact ownership tombstones with the deletion receipt, and consumes synchronized re-sends by id. Legacy counted with a zero fallback and deleted in a separate statement (`old-code/src-tauri/src/usage/repository.rs:506-518`); the rewrite fails typed and commits atomically. Backups retain proofs for terminal references instead of losing their audit integrity.

Conversation terminal failures, cancellation and restart interruption now retain dispatch display snapshots and available response identity rather than losing them at terminal settlement. Failed rows keep the coordinator error corresponding to legacy's error field (`old-code/src-tauri/src/chat_manager/service.rs:617-646`). An attempt without dispatch evidence keeps only known launch attribution.

Legacy reports read only `usage_records`, including their statistics (`old-code/src-tauri/src/usage/repository.rs:270-306,483-501`). Messages without a real usage row were never included in those totals. Reconstructed conversation events remain immutable evidence with `legacy_import` origin; the imported usage records are the historical reporting source. The required origin is carried through backup and synchronization rather than inferred from ids, even after conversation deletion. Older rewrite backups containing events without origin are deliberately rejected with a typed error; legacy 2.2.5 conversion remains supported.

Unified reporting counts actual dispatches rather than adding their conversation aggregate a second time. Cleared dispatch proofs prevent an older aggregate from reappearing as a charge. Legacy timestamp bounds were inclusive and records were read ascending (`old-code/src-tauri/src/usage/repository.rs:275-306`); the dashboard and activity views then ordered newest first (`old-code/src/ui/pages/settings/UsagePage.tsx:457-462`; `UsageActivityPage.tsx:38-46`). The API supports both directions with stable identity ties and cursor paging instead of fetching every row into the frontend.

Day grouping preserves the device-local calendar used by the dashboard (`old-code/src/ui/pages/settings/UsagePage.tsx:490-496`) with an explicit IANA timezone and DST-correct conversion. The API accepts an unbounded range; legacy's all preset actually stopped ten years back (`UsagePage.tsx:75-97`). Nullable totals and explicit unknown-request counts replace the dashboard's zero substitutions (`UsagePage.tsx:475-498`). CSV retains the exact legacy header and text escaping (`old-code/src-tauri/src/usage/repository.rs:700-709`), but unknown values are empty instead of its zero defaults (`repository.rs:711-765`). Provider-reported total fallback and signed historical completion adjustments remain available (`repository.rs:120-125,159-162`).

The application now automatically captures OpenRouter costs for settled chat dispatches through durable event-driven jobs using the stored response identity. Recalculation fills only missing costs instead of overwriting legacy records (`old-code/src-tauri/src/usage/commands.rs:128-215`). Required missing evidence fails typed instead of the legacy warning-only fallback (`old-code/src-tauri/src/chat_manager/service.rs:338-395,462-491`). An exact cleared-dispatch tombstone proves that a racing capture must leave the row deleted.
