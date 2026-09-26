# lettuce-characters: legacy parity notes

Facts about how `lettuce-characters` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- The crate preserves legacy's selected authored surface: creator provenance, localized notes, sources, tags, defaults, voice and image recommendation values, crop, gradient and card presentation, the sparse chat appearance vocabulary and recursive widget tree, group speaker, memory and persona choices, multiple ordered scenes, per-scene and per-variant directions, selected variants, inline assets and starter message ordering.
- `CharacterProfile` keeps legacy's authored `scenario` and ordered `rules`. Legacy rendered `{{rules}}` as empty text and never rendered the scenario; the rewrite preserves the data without changing prompt output.
- The companion baseline affect, regulation style and relationship defaults are legacy's.
