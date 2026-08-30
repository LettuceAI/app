# lettuce-companions

Relationship events, projections, growth policies, milestones, and scheduled effects.

## Boundary

Does not read chat storage or invoke models directly.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.

The verified companion growth, consolidation, and Soul-writer behavior is
pinned in `fixtures/legacy-import/companion-tool-scenarios-v1.json`. The pure
typed Soul/growth policy is active: it carries forward the legacy category
mutability table, exact `0.55`/`0.70`/`0.85`/`1.0` confidence thresholds,
confidence/weight clamping, validity rule, semantic-slot supersession, locked
protection, twelve-active-fact consolidation threshold, and forty-item
superseded-history bound. The typed boundary deliberately makes a proposed
growth or consolidation change set atomic at one expected revision. Legacy's
sixteen-item limit remains a fresh-memory coordinator input limit and is not
misapplied to the number of proposed Soul facts. Provider calls, prompt
rendering, job coordination, database adapters, and frontend events remain
outside this slice.

Durable Soul state is character-owned, matching legacy continuity: companion
Soul growth is shared across that character's sessions regardless of the
separate shared-memory setting. `SoulRepository` owns create/load and atomic
expected-revision change-set application. The SQLite adapter stores facts and
their source/supersession lists in normalized tables, applies the pure policy
inside one immediate transaction, bounds superseded history to forty entries,
and records immutable idempotency receipts. Session, persona, relationship,
prompt, provider, and frontend coordination remain outside this persistence
slice.
