# lettuce-characters

The authored library records: characters, personas, reusable groups, scenes and their variants, conversation starters, and the presentation (crops, gradients, cards, chat appearance, widgets) attached to them.

These are mutable library records guarded by optimistic `Revision` checks. "Versioned" here means closed, format-versioned payloads and compare-and-swap mutations, not an immutable history of authored edits. A conversation keeps its own launch snapshot of what it was started from (`lettuce-conversations`), so editing a library record does not rewrite existing history.

The crate is pure. Persistence and migrations (`lettuce-database`), media ingestion and cleanup (`lettuce-media`), transfer envelopes (`lettuce-transfer`), prompt rendering and lorebook matching (`lettuce-context`), conversations and Tauri commands are elsewhere. Companion Soul math and live state live in `lettuce-companions`.

## Aggregates

| Module | Records |
| --- | --- |
| `character.rs` | `Character`, `CharacterProfile`, `CharacterProvenance`, `CharacterDefaults`, `CharacterMedia`, voice and image recommendations |
| `persona.rs` | `Persona`, `PersonaMedia`, `PersonaDraftUpdate`, `PersonaDefaultState` |
| `group.rs` | `GroupProfile`, `GroupMember` |
| `scene.rs` | `Scene`, `SceneVariant`, `SceneDocumentV1`, `SceneAssetLink` |
| `starter.rs` | `ConversationStarter`, `StarterMessage` |
| `presentation.rs` | `CharacterPresentationV1`, `ChatAppearanceV1`, `Crop`, the widget tree |
| `selection.rs` | `LifecycleStatus`, `Selection<T>`, chat mode, speaker selection, memory policy |
| `ports.rs` | create plans, details aggregates, draft updates, the repository and reader traits |

A character carries its authored text and metadata: creator provenance, localized notes, sources, tags, the scenario and the ordered rules list (both inside `profile_json` with serde defaults, so older rows and backups decode unchanged), defaults (prompt, lorebooks, model, voice, interaction and memory settings), image recommendations, media, presentation, multiple ordered scenes with per-scene and per-variant directions and a selected variant, and ordered starters.

A group is a reusable profile of at least two members with at least one active: member order, mute and model overrides, persona, chat mode, speaker selection, memory policy, prompt defaults, presentation, background, whether character lorebooks are disabled, and an optional starting scene.

### Companion characters

A companion character may own an authored Soul configuration in its versioned defaults document; roleplay characters reject one. The character aggregate owns the twelve authored identity fields, the baseline affect, regulation style and relationship defaults the `lettuce-companions` state math starts from, the companion prompt-template reference and style notes. It rejects non-finite or out-of-range math inputs and oversized style notes. Fact normalization, live Soul invariants and the emotion and relationship transitions belong to `lettuce-companions`; runtime state, memory, prompting, classifiers and launch coordination are outside this crate.

## References

All media references are `AssetId`s. Widget image locators are either a logical asset reference or an explicit unresolved import token; native paths, data URLs, bytes and serving logic are never domain values. Prompt and lorebook ids are typed references only; the documents, bindings and matching belong to `lettuce-context`. Voice profiles and model artifacts are typed references too, with an unresolved legacy voice or recommendation form available only to import adapters.

Presentation exposes a deterministic, recursive list of the logical asset references it contains (unresolved widget tokens excluded). Character and persona media and scene associations reject reusing one `AssetId` across slots.

## Validation

Operational structs use `serde(deny_unknown_fields)` and explicit format versions. Constructors and `validate` enforce finite crop and appearance numbers, bounded authored text, contiguous ordering, unique child ids, that a selected variant belongs to its scene, that scene assets agree with the scene document, starter selection rules, the persona media and card invariants, and the group membership minimum. Every aggregate with a revision and timestamps rejects revision zero and `created_at > updated_at`.

The complete `CharacterDetails` and `GroupDetails` aggregates are serializable and validate by reusing their create-plan invariants, so the transfer boundary can carry their ordered graphs without a separate backup-only model.

## Repositories

`CharacterRepository`, `PersonaRepository`, `GroupRepository`, `SceneRepository` and `StarterRepository` are synchronous ports with one method per lifecycle operation: revise the profile, update defaults or presentation, attach, detach or reorder media, add, update, remove or reorder scenes, variants, starters and starter messages, change a starter's scene, prompt or lorebooks, set members, archive, restore and so on. There is deliberately no generic save, upsert or partial patch, no transaction or SQL in the interface, no row DTOs and no hard delete.

- Scene and starter mutations carry the character id and the expected root revision; the character root revision is the CAS token for its whole graph.
- Scene draft updates carry only scalar fields. Replacing assets is a complete base-and-variant association operation. Removing a scene fails while a starter depends on it, even when a replacement default scene is given.
- A group loads its optional starting scene atomically with the group, and that scene changes only through the group port, never through the character scene port.
- Persona edits use the closed `PersonaDraftUpdate`, so identity, lifecycle, media and revision metadata stay with the repository. `PersonaSearch` takes `include_archived` explicitly.
- The default persona is a separate revisioned `PersonaDefaultState`, read as one `PersonaDefaultSnapshot`; changing it does not bump the persona's revision. Archiving takes the persona's and the default's CAS tokens and returns a `PersonaArchiveResult`, so an adapter can clear a current default atomically and report the new default revision without another read.

`LifecycleStatus` controls library visibility. Archived scenes remain authored records and may stay selectable according to the owning aggregate's policy; persistence does not invent exclusion rules.

`CharacterDependencyReader`, `PersonaDependencyReader` and `GroupDependencyReader` report what references a record, for impact analysis before archive or delete. `ProfileDuplicateRepository` duplicates a profile transactionally: the request names source and destination roots, the source graph supplies all child data, and the result reports typed id remaps plus every retained media, scene, widget, prompt, lorebook, model, voice and artifact reference. Duplication and dependency reports exclude conversations, memories, companion runtime state and other live data.
