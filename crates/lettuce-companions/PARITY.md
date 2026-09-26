# lettuce-companions: legacy parity notes

Facts about how `lettuce-companions` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- The verified companion growth, consolidation and Soul-writer behavior is pinned in `fixtures/legacy-import/companion-tool-scenarios-v1.json`.
- The Soul policy carries forward legacy's category mutability table, the exact 0.55/0.70/0.85/1.0 confidence thresholds, confidence and weight clamping, the validity rule, semantic-slot supersession, locked protection, the twelve-active-fact consolidation threshold and the forty-item superseded-history bound.
- Legacy's sixteen-item limit is kept as the growth coordinator's fresh-memory input limit and is not applied to the number of proposed facts.
- Growth exposes legacy's required `record_growth` contract, reduces the first matching native call or legacy's structured-text fallback, keeps legacy's fallback of source indices to all supplied memory ids, and copies legacy's category order, effective facts and first sixteen non-blank memories with zero-based numbering.
- Consolidation keeps legacy's input exactly: the readiness check, the authored/core/growth prompt facts, the required `consolidate_soul` schema, first matching native call, structured-text fallback, core adjustment filtering and retirement ids.
- The Soul writer copies legacy's six-tool contract and working-draft reducer: zero defaults, clamps, provider order, first `done` wins, the 0.7 authored-fact confidence gate, weight default and clamp, generated identity and timestamps, forced historical locking. The prompt values keep legacy's blank-input fallbacks, pretty current-Soul JSON and final instruction. The structured fallback accepts legacy's JSON aliases and `soul_ops`/`operations` XML roots.
- Scheduled notes copy legacy's fields and recurrence semantics, the `available_at` then id order, the 1000-character per-note truncation and 4000-character block cap.
- Durable Soul state is character-owned, matching legacy continuity: growth is shared across the character's sessions regardless of the shared-memory setting (when the new growth toggle is on, its default).
- The authored config keeps all twelve legacy identity categories and typed authored facts; initial durable state follows legacy's `normalize_for_storage` order; the serialized config keeps legacy's camel-case field names, including the nested `prompting.promptTemplateId` and `prompting.styleNotes`.
- The emotion and relationship math copies legacy's defaults and formulas: baseline affect, regulation style, expressed/blocked affect, 45-minute exponential decay, volatility scaling, momentum interpolation, passive tension and stability recovery, and the bipolar damage and recovery constants.
- `CompanionStateRepository` separates ownership as legacy did: emotional state per conversation, relationship per character plus persona, independent of memory sharing.
- The classifier reducer keeps legacy's GoEmotions behavior: first eight labels, the label-to-signal mapping of all 28 labels, grouped-name deduplication with accumulated effects, and the exact emotion, relationship, confidence, clamping and unavailable-model fallback values.
- The prompt state keeps legacy's relationship bands (release 2.2.5: seven steps for closeness, trust and affection with the -0.15 and -0.25 neutral floors, five tension steps), the 0.08 floor for top dimensions, and legacy wording through the catalog.
- `time_awareness` is legacy's `timeAwareness` or `context.timeAwareness`, the default for new conversations like legacy's seeded session preference.
- `share_memory_across_chats` is legacy's `memory.sharedAcrossSessions` (default on). A companion-mode character whose legacy config is missing or unreadable imports it off, as legacy read it; a null or mistyped `memory` or `sharedAcrossSessions` also imports off, as legacy's failed parse did. Legacy companion sessions keep their own memories beside the imported pool. Memory follows the memory toggle (see the `lettuce-database` README).
- Continuations keep legacy's zero-delta seed with no user-message owner; effect settlement copies legacy's summary formula.
- User edits are legacy's `companion_clear_soul_growth` (every entry, authored and locked included), `companion_remove_soul_growth` and `companion_set_soul_growth_lock`. An edit that changes nothing writes nothing, where legacy answered without a write. User edits never prune superseded history; legacy's edits did not bound it either.

## Deliberate differences from legacy

- A growth or consolidation change set is atomic at one expected revision.
- The emotion classifier thresholds come from the classifier (Thymos's calibrated per-class thresholds) instead of legacy's SamLowe thresholds (`neutral` 0.55; love, caring, gratitude, remorse, anger, sadness and fear 0.18; others 0.22). This was a user-approved change.
- `companion_remove_soul_growth` removes an entry by its stable id instead of its list position.
- `share_soul_growth_across_chats` is new (default on). Its on/off behavior was decided by the user on 2026-09-23.
- The other legacy `memory` config keys were never read and are recorded as dropped.

## Known gaps

- A growth run admitted before a sharing-toggle flip still applies to the owner it started with.
- Sync treats an authored-only, unsuperseded Soul (including an empty one, which is what a companion without authored facts starts from on every device) as an untouched seed that loses to a concurrent learned change. A clear, or an edit that leaves only authored facts, made while another device grows the Soul concurrently, is overridden by that growth. Telling them apart needs a synced "edited by the user" marker in the Soul payload.
- The usage ledger does not cover provider-internal retries or automatic pricing capture.

## History

- The Soul-writer fault injection tests verify no extra provider call or false checkpoint on admission and settlement faults; a successful later retry keeps the pending usage record separate from completed dispatches.
- SQLite app scenarios verify persisted usage details in all three workflows. Missing checkpoint usage in older saved runs defaults to none.
- Verified ONNX tokenization and model execution for the classifier moved to `lettuce-embeddings`; model discovery, prompt rendering and direct companion turn wiring are done. The previous README listed provider and job coordination and frontend events for the classifier, and worker execution for turn effects, as deferred application work.
- Soul growth user edits were added on 2026-09-23.
- The previous README said "Foundation scaffolding is active"; the crate has since filled in the domain described in the README.
