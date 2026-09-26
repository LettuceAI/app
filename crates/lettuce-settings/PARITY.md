# lettuce-settings: legacy parity notes

Facts about how `lettuce-settings` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- Lorebook generator resolution copies legacy's priority: explicit selection, generator setting, then default model or built-in prompt. The target count (default 12, 5..50) and output tokens (default 4096, 256..32768) are legacy's bounds.
- The dynamic-memory defaults copy legacy's runtime fallbacks: 50 entries, a 2000-token hot budget, 0.35 minimum similarity, five smart results, 0.3 cold threshold, 0.5 delete confidence and per-cycle hard-delete ratio, 0.78 duplicate threshold, and two-message context enrichment.
- The post-turn cycle settings follow legacy's schema defaults: disabled until enabled, a 20-message summary interval, `auto` run mode (`ask_first` and `manual` also exist), 0.08 per-cycle decay, recursive memory loops off with a hard cap of 20 rounds, and the `xml` structured fallback format (legacy `dynamicMemoryStructuredFallbackFormat`, `json` the other value).
- `dynamic_memory_prompts` is legacy `dynamicMemorySummarizerPromptTemplateId` / `dynamicMemoryManagerPromptTemplateId`, shared by direct and group chats.
- `dynamic_memory_llama_sampler_overwrite_enabled` is legacy `dynamicMemoryLlamaSamplerOverwriteEnabled` (default true).
- `help_me_reply` carries legacy's `helpMeReply*` advanced settings with their defaults (enabled, streaming, 150 output tokens, ten history messages, `roleplay` style, no model or prompt overrides). A zero history count reads as ten because legacy ignored it.
- `embedding.dimensions` is legacy's `embeddingDimensions` preference.
- `manual_mode_context_window` is legacy `manualModeContextWindow` (default 50).
- `image_generation` carries legacy `avatarGeneration*`, `sceneGeneration*`, `sceneWriterModelId` and `creationHelperImageModelId` (avatar on, scenes off, `auto` mode, no models). `scene_default_size` is legacy `sdDefaultSize` (portable, used by scene generation only).
- `creation_helper` is legacy `creationHelperModelId`, `creationHelperStreaming` (on by default), `creationHelperEnabledTools` (unset for every tool) and `creationHelperToolFallback` (native/json/xml).
- `lorebook_entry_generator` is legacy `lorebookEntryGenerator*` (model, entry prompt, JSON-default fallback format) plus `lorebookKeywordGeneratorPromptTemplateId`.
- `companion_soul_writer` is legacy `companionSoulWriter*` (model, fallback model, prompt, JSON-default fallback format). The resolvers in `lettuce-app` use legacy's fallbacks.
- `creation_helper.enabled_tools` is kept although only dead legacy code read it; a consumer honoring it would change behavior.
- `ui_preferences` uses legacy's key names: `theme`, `settingsCardOpacity`, `customColors`, `customColorPresets`, `chatsViewMode`, `groupChatsViewMode`, `accessibility`, `navigationStyle`, `navigationSide`, `headerStyle`, `navItems`, `navAlign`, `navEdge`, the global `chatAppearance` and `llamaSamplerPresets`. Only the shell read them in legacy, so the backend does not interpret them.
- `auto_download_character_card_avatars` is legacy `autoDownloadCharacterCardAvatars`, or the older `autoDownloadDiscoveryAvatars`, default on, read like legacy (the first boolean of the two keys). It is typed because character-card import reads it.
- `DeviceUiStateStore` holds legacy `onboarding`, `tooltips`, `lastSeenAppVersion` and the `appActiveUsage*` counters. A restore keeps an imported legacy install's state, otherwise the previous database's.
- Trusted certificates are checked like legacy's importer: `BEGIN`/`END CERTIFICATE` markers, no duplicate PEM, up to 1 MiB each.
- The optional group-chat memory policy beside the direct one matches legacy's settings editor and persistence owner.
- The legacy import stores `meta.hugging_face_access_token` and `meta.civitai_access_token` at the app-wide secret references (blank ones stay unset, as legacy read them), only where the store has none, so a token already set in the new app is kept. A failed restore removes the ones it wrote.
- The normalized app-wide direct prompt selection is set by legacy transfer only when the selected source template was admitted.

## Deliberate differences from legacy

- Legacy's creation helper failed with "No image generation model configured" when its saved image model was gone. A cleared selection now means the first suitable model, as avatar and scene generation already did.
- Legacy never cleared the creation helper and lorebook entry writer selections when a model was deleted, so they failed with "Model not found" and "Configured lorebook entry generator model could not be resolved". The cleared selection now falls back like an unset one (default model, first text model).
- Legacy synced and backed up `trustedCertificates`, `embeddingModelVersion`, `embeddingMaxTokens`, `embeddingKeepModelLoaded` and `customLlmModelsDir` with its settings row. They describe one machine, so the rewrite keeps them per device. A v2 backup still carries the certificates and the embedding choice (user decision 2026-09-25).
- Over-long certificate names are shortened, not dropped.
- A root the TLS stack cannot parse is tried alone and skipped with a warning; legacy's rustls build failed every client.
- `embeddingModelVersion` `v3`/`v4` are kept as imported; `v5` is Eidos, and an unset version prefers Eidos. Embedding max tokens are stored clamped to 512..4096 on import.
- `dynamic_memory.min_similarity_basis_points` is optional: unset lets the embedding model decide (Eidos's published thresholds, 0.35 on v4). The legacy import treats the values legacy wrote on its own (0.32 and 0.35) as unset.

## Approved removals

- The host API (legacy `hostApi` and its token) is not ported (user decision 2026-09-23). An import records `advanced_settings.hostApi` as unsupported.

## Not wired yet

- Character-card import, which reads `auto_download_character_card_avatars`, is not ported yet.
- Native secret entry IPC, the broad legacy settings vocabulary and portable backup/sync vaults were listed as later slices.
- `NativeSecretStore` and `PassphraseVault` have no callers outside this crate yet; the composition root does not construct them.

## Build notes

- Android uses `android-native-keyring-store` 0.5 because it supports the workspace's Rust 1.85 baseline; newer releases need Rust 1.88.

## History

- App-wide secret references for the Hugging Face and CivitAI tokens were added on 2026-09-23.
- The previous README said nothing reads the app-wide tokens yet. The Hugging Face and CivitAI browsers in `lettuce-app` now read them.
- The previous README said `GlobalSettings` held only safety, telemetry and update preferences and deliberately excluded frontend appearance. The document has since grown the feature sections and `ui_preferences`, which carries the shell's appearance settings.
- The crate docs refer to a crate `PLAN.md`; there is none in the crate directory.
