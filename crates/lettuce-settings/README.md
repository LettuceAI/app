# lettuce-settings

Typed settings, validation, defaults, scope resolution, secret references, and
secure-store ports.

## Boundary

Persists through injected ports and never reads SQLite directly. Secret bytes
are scoped store values and never ordinary settings or serializable DTOs.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Global settings retain lorebook-generator model/four prompt references and
authored target-count/output-token defaults. Resolution copies legacy priority:
explicit selection, generator setting, then default model or built-in prompt.
Target count defaults to 12 and clamps to 5..50; output tokens default to 4096
and clamp to 256..32768. These are existing legacy bounds. Missing fields in
older settings documents deserialize to defaults. Reference existence is
checked when the consumer resolves the selected model/prompt, not by JSON save.

Global settings also retain the normalized direct/group dynamic-memory mutation
and retrieval policy used to populate immutable conversation launch snapshots.
Defaults copy the legacy runtime fallbacks: 50 entries, a 2,000-token
hot budget, 0.35 minimum similarity, five smart results, 0.3 cold threshold,
0.5 delete confidence and per-cycle hard-delete ratio, 0.78 duplicate threshold,
and two-message context enrichment. Scores use integer basis points so persisted
policy values remain exact. Older settings documents receive these defaults.
The post-turn cycle settings follow the legacy schema defaults: disabled until
enabled, a 20-message summary interval, `auto` run mode (`ask_first` and
`manual` also exist), 0.08 per-cycle decay, recursive memory loops off with
a hard cap of 20 rounds, and the `xml` structured fallback format (legacy
`dynamicMemoryStructuredFallbackFormat`, `json` is the other value) for models
that answer the memory cycle without tool calls. `dynamic_memory_prompts` holds
the user's summarizer and manager prompt document overrides (legacy
`dynamicMemorySummarizerPromptTemplateId` / `dynamicMemoryManagerPromptTemplateId`,
shared by direct and group chats); unset means the built-in document.
`dynamic_memory_llama_sampler_overwrite_enabled` (legacy
`dynamicMemoryLlamaSamplerOverwriteEnabled`, default true) lets memory calls on
llama.cpp models drop the creative sampler. `help_me_reply` carries the legacy
`helpMeReply*` advanced settings with their defaults (enabled, streaming, 150
output tokens, ten history messages, `roleplay` style, no model or prompt
overrides); a zero history count reads as ten, as legacy ignored it. `embedding.dimensions` keeps the legacy
`embeddingDimensions` preference; unset means the embedding default.
`manual_mode_context_window` (legacy `manualModeContextWindow`, default 50) is
the history window for chats without dynamic memory; dynamic chats use the
summary interval instead. `image_generation` carries legacy `avatarGeneration*`,
`sceneGeneration*`, `sceneWriterModelId` and `creationHelperImageModelId`
(avatar on, scenes off, `auto` mode, no models). Every model the payload
selects is unselected when that model is deleted locally or through sync, and
a synced snapshot drops selections of models this device lacks. Deliberate
change: legacy's creation helper failed with "No image generation model
configured" when its saved image model was gone; the cleared selection now
means the first suitable model, like avatar and scene generation did.
`creation_helper` (legacy `creationHelperModelId`, `creationHelperStreaming`
on by default, `creationHelperEnabledTools` unset for every tool,
`creationHelperToolFallback` native/json/xml), `lorebook_entry_generator`
(`lorebookEntryGenerator*` model, entry prompt and JSON-default fallback
format plus `lorebookKeywordGeneratorPromptTemplateId`) and
`companion_soul_writer` (`companionSoulWriter*` model, fallback model, prompt
and JSON-default fallback format) keep the choices those jobs made before they
ran; the jobs themselves still take a resolved profile per request, picked by
`lettuce_app::{creation_helper_model, lorebook_entry_generator_model,
soul_writer_model, soul_writer_fallback_model}` with legacy's fallbacks.
Deliberate change: legacy never cleared these selections when a model was
deleted, so its creation helper failed with "Model not found" and its lorebook
entry writer with "Configured lorebook entry generator model could not be
resolved"; the cleared selection now falls back like an unset one (default
model, first text model). `creation_helper.enabled_tools` is kept although
only dead legacy code read it; a consumer honoring it would change behavior.
`ui_preferences` (`UiPreferences`, at most 256 KiB) is the app shell's own
JSON document under legacy's key names: `theme`, `settingsCardOpacity`,
`customColors`, `customColorPresets`, `chatsViewMode`, `groupChatsViewMode`,
`accessibility`, `navigationStyle`, `navigationSide`, `headerStyle`,
`navItems`, `navAlign`, `navEdge`, the global `chatAppearance` and
`llamaSamplerPresets`. Only the shell read them in legacy, so the backend stores,
bounds and syncs the document without interpreting it.
`auto_download_character_card_avatars` (legacy
`autoDownloadCharacterCardAvatars`, older `autoDownloadDiscoveryAvatars`,
default on, read like legacy: the first boolean of the two keys) is typed
because character-card import (not yet ported) reads it.
`DeviceUiStateStore` keeps (up to the same 256 KiB) the shell's state about this install (legacy
`onboarding`, `tooltips`, `lastSeenAppVersion` and the `appActiveUsage*`
counters) in the device-local `device_ui_state` row, which never syncs or
enters backups; a restore keeps an imported legacy install's state, otherwise
the previous database's.
`DeviceSettingsStore` keeps the typed `DeviceSettings` of this device in the
device-local `device_settings` row, which never syncs. A v2 backup carries the
trusted certificates and the embedding choice (user decision 2026-09-25) and a
restore writes them; the models folder is a path on this device, so the
restore keeps the previous file's unless the new file names one (an imported
legacy install's). Legacy
`trustedCertificates` (checked like legacy's importer: `BEGIN`/`END
CERTIFICATE` markers, no duplicate PEM, up to 1 MiB each so CA bundles fit;
over-long names are shortened, not dropped; `AppBackend::tls_policy` builds
the `TlsPolicy` the host passes to provider clients, and a root the TLS stack
cannot parse is tried alone and skipped with a warning, where legacy's rustls
build failed every client),
`dynamic_memory.min_similarity_basis_points` is optional: unset lets the
embedding model decide (Eidos's published thresholds, 0.35 on v4); the legacy
import treats the values legacy wrote on its own (0.32 and 0.35) as unset.
`embeddingModelVersion` (`v3`/`v4` kept as imported; `v5` is Eidos, and an
unset version prefers Eidos)/`embeddingMaxTokens`/`embeddingKeepModelLoaded` and
`customLlmModelsDir`. Legacy synced and backed these up with its settings row;
they describe one machine (paths, network trust, installed files),
so the rewrite keeps them per device. `image_generation.scene_default_size` is
legacy `sdDefaultSize` (portable, used by scene generation only).
An optional group-chat policy lives beside the direct policy in global settings,
matching the legacy settings editor and persistence owner. Group launches use
that complete override when present and otherwise inherit the direct policy.

The first slice is intentionally small: one opaque serializable `SecretRef`, a
bounded purpose vocabulary, redacted zeroizing `SecretValue`, metadata/status
types, and one object-safe asynchronous `SecretStore` port. The in-memory store
is a deterministic reference adapter for tests only; it does not encrypt,
authenticate, persist, or provide platform protection.

An OpenRouter adapter will use the boundary like this: the provider account owns
an opaque `SecretRef` with `ProviderApiKey { owner }`; a settings read returns
only the reference and `status(ref, purpose)`. The provider adapter calls
`load(ref, purpose)` for a bounded `SecretValue`, borrows it briefly while
constructing the authorization header, and then drops it. Initial creation uses
`put(record, value, None)`; rotation uses the same metadata plus
`Some(status.generation)` and the store computes the next generation. Stale
writes are rejected. A missing or unavailable secret is reported as a typed
state/error, never as an empty key or backend message.

Deletion is cleanup-idempotent: an absent reference returns `Missing` even
with an observed generation, while live entries validate purpose and reject a
stale observed generation.

`NativeSecretStore` is the production credential adapter for Linux Secret
Service, macOS/iOS Keychain, Windows Credential Manager and Android's Keystore-
backed encrypted SharedPreferences store. Each opaque reference names one native
credential containing a bounded versioned envelope with its exact purpose,
monotonic generation and value. Mutations are serialized inside the shared store
instance, rotation uses generation CAS, and corrupt, ambiguous, inaccessible or
wrong-purpose entries fail closed. Temporary encoded and decoded values are
zeroized, errors and debug output remain redacted, and no plaintext fallback
exists. Tests run against an injected deterministic backend and never access the
developer's credential store. The envelope bound accounts for worst-case JSON
escaping of every valid 16 KiB `SecretValue`.

Where the OS credential store is unavailable (Linux without a Secret Service),
`NativeSecretStore::with_passphrase_vault` keeps credentials in a
`PassphraseVault` the user explicitly creates: an Argon2id key derived from the
passphrase seals one ciphertext file with XChaCha20-Poly1305 under a fresh nonce
on every write, with the KDF header bound as associated data. Only the derived
key lives in memory for the unlocked session. The vault reaches storage through
the `VaultFile` port, whose writes must replace the file atomically; an existing
vault is never overwritten by `create`, and a wrong passphrase fails closed.
Credentials never fall back to a machine-derived key, a plaintext file or SQLite.

Android uses `android-native-keyring-store` 0.5 because it supports the workspace
Rust 1.85 baseline; newer releases require Rust 1.88. The production composition
root must call `NativeSecretStore::try_new` only after Tauri Mobile initializes
`ndk-context`. Initialization failure and all Keystore operations remain typed
store failures, and Android never installs or falls back to a mock keyring.

Native secret entry IPC, the broad legacy settings vocabulary and portable
backup/sync vaults remain later slices.

The database foundation adds a small closed `GlobalSettings` document and a
synchronous persistence port. It currently holds global safety/telemetry/update
preferences only; frontend appearance is deliberately excluded.
The application default and dynamic-memory model selections are separate typed
IDs with foreign keys and optimistic revision checks, not arbitrary setting
keys. The dedicated group speaker-selection model is a third normalized route.
The normalized app-wide direct prompt selection is a typed prompt-document ID;
legacy transfer sets it only when the selected source template was admitted.
Dynamic-memory and group-speaker selection have narrow CAS setters so a
successful retry can update only that feature route without rewriting unrelated
preferences.

App-wide secrets (2026-09-23): `SecretPurpose::app_secret_ref` gives the
Hugging Face token and the CivitAI token a fixed reference each (one per
install). A legacy import stores legacy's `meta.hugging_face_access_token` and
`meta.civitai_access_token` there (blank ones are unset, as legacy read
them), only where the store has none, so a token already set in the new app
is kept; a failed restore removes the ones it wrote. Nothing reads them yet:
the Hugging Face browser and CivitAI slices use these references. The host
API (legacy `hostApi` and its token) is not ported (user decision 2026-09-23);
an import records `advanced_settings.hostApi` as unsupported.
