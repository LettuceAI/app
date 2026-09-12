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
summary interval instead.
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
