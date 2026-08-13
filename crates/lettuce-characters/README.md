# lettuce-characters

Typed authored aggregates for characters, personas, reusable group profiles,
scenes, scene variants, presentation, and conversation starters.

## Contract

The aggregates are mutable library records with optimistic `Revision` checks;
“versioned” describes closed, format-versioned payloads and CAS mutations, not
an immutable authored-history table. A conversation will copy a complete launch
snapshot later, so editing a library profile cannot rewrite existing history.

The crate preserves the selected authored surface, including creator provenance,
localized notes, sources, tags, defaults, voice and image
recommendation values, crop/gradient/card presentation, the sparse chat
appearance vocabulary and recursive widget tree, group speaker/memory/persona
choices, multiple ordered scenes, per-scene and per-variant directions, selected
variants, inline assets, and starter message ordering.

All media references use `AssetId`. Widget image locators are either logical
asset references or an explicit unresolved import token; native paths, data
URLs, bytes, and serving logic are not domain values. Prompt and lorebook IDs
are typed references only; their documents, matching, and bindings belong to
`lettuce-context`. Voice profiles and model artifacts are typed references, with
an unresolved legacy voice/recommendation form available only for import
adapters.

## Repository boundary

`CharacterRepository`, `PersonaRepository`, `GroupRepository`,
`SceneRepository`, and `StarterRepository` are synchronous, lifecycle-specific
ports. Character scene and starter mutations carry the character ID and
expected root revision. Groups load their optional complete starting scene with
the group atomically; that zero-or-one scene can be changed only through the
group port, not the multi-scene character port. Ports intentionally do not
expose generic save/upsert/partial patch operations, transactions, SQL, row
DTOs, or hard-delete methods. Dependency readers and the named
profile-duplication port make impact analysis and transactional duplication
explicit while excluding conversations, memories, companion runtime state, and
other live data.

Operational structs use `serde(deny_unknown_fields)` and explicit format
versions. Constructors and `validate` methods enforce finite crops and
appearance numbers, bounded authored text, contiguous ordering, unique child
IDs, selected-variant ownership, scene asset/document agreement, starter
selection semantics, the persona media/card invariants, and the reusable-group
minimum of two members with at least one active member.

Persistence, migrations, media ingestion/GC, transfer compatibility envelopes,
prompt rendering, lorebook matching, conversations, and Tauri commands remain
outside this crate.
