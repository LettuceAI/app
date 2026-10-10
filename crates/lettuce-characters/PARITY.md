# lettuce-characters: legacy parity notes

Facts about how `lettuce-characters` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- The crate preserves legacy's selected authored surface: creator provenance, localized notes, sources, tags, defaults, voice and image recommendation values, crop, gradient and card presentation, the sparse chat appearance vocabulary and recursive widget tree, group speaker, memory and persona choices, multiple ordered scenes, per-scene and per-variant directions, selected variants, inline assets and starter message ordering.
- `CharacterProfile` keeps legacy's authored `scenario` and ordered `rules`. Legacy rendered `{{rules}}` as empty text and never rendered the scenario; the rewrite preserves the data without changing prompt output.
- The companion baseline affect, regulation style and relationship defaults are legacy's.

Library records follow live values for new turns while recorded history remains fixed, matching the group profile resolution in `old-code/src-tauri/src/storage_manager/group_sessions.rs:510-615`.

The Banner slot preserves the independent `avatar_banner.webp` written by `old-code/src-tauri/src/storage_manager/media.rs:1032-1038` and edited in `old-code/src/ui/pages/characters/components/IdentityStep.tsx:170-196`. Profile duplication copies bindings, including archived lorebooks, and scheduled notes with fresh ids, while initializing companion Soul from authored configuration rather than copying runtime growth. This is the specified authored-only library feature; the developer clone copied conversations and live Soul in `old-code/src-tauri/src/storage_manager/characters.rs:1186-1480`.
