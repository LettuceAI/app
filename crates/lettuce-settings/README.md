# lettuce-settings

The app's typed settings and its secret store: the global settings document with its defaults and effective values, the settings that belong to one device, the shell's per-install UI state, and the secret references, values and stores that keep API keys out of ordinary data.

The crate persists nothing itself. Settings go through the synchronous store traits (`GlobalSettingsStore`, `DeviceSettingsStore`, `DeviceUiStateStore`), which `lettuce-database` implements; the crate never reads SQLite. Secrets go through the async `SecretStore` trait, and secret bytes are never a setting or a serializable DTO. It is the only crate allowed to depend on `keyring` (`architecture.toml`). `lettuce-app` reads the settings, resolves the effective choices and builds the stores at composition time.

## Global settings

`GlobalSettings` (`global.rs`) is one closed serde document (`deny_unknown_fields`, format version `GLOBAL_SETTINGS_FORMAT_VERSION`). Every section except the three top-level flags has a serde default, so an older document that lacks a field reads as the default. The sections:

- `pure_mode` (`Off`, `Standard`, `Strict`, `Low`), `analytics_enabled`, `update_checks_enabled`.
- `dynamic_memory`: the direct-chat memory policy, `DynamicMemorySettings`. `group_dynamic_memory` is an optional complete override for group chats; `effective_group_dynamic_memory` returns it or falls back to the direct policy. Launching a conversation copies the effective policy into the conversation's snapshot.
- `dynamic_memory_prompts`: the summarizer and manager prompt overrides, shared by direct and group chats; unset means the built-in document.
- `dynamic_memory_llama_sampler_overwrite_enabled` (default on): memory calls on llama.cpp models drop the creative sampler.
- `help_me_reply`, `lorebook_generator`, `lorebook_entry_generator`, `creation_helper`, `companion_soul_writer`, `image_generation`: the model, prompt and format choices of the helper jobs.
- `embedding.dimensions`: the embedding dimension preference; unset means the embedding default.
- `manual_mode_context_window` (default 50): the history window for chats without dynamic memory. Dynamic chats use the summary interval instead.
- `auto_download_character_card_avatars` (default on): fetch http(s) avatars a character card links to while importing it.
- `ui_preferences`: the app shell's own JSON document (theme, colors, view modes, navigation, the global chat appearance, saved sampler presets and so on). Only the shell reads it; the backend stores, bounds (256 KiB) and syncs it without interpreting it.

### Memory policy

`DynamicMemorySettings` defaults:

| Field | Default |
| --- | --- |
| `enabled` | off |
| `summary_message_interval` | 20 messages |
| `run_mode` | `Auto` (`AskFirst`, `Manual`) |
| `max_entries` | 50 |
| `hot_memory_token_budget` | 2000 |
| `min_similarity_basis_points` | unset |
| `retrieval_limit`, `retrieval_strategy` | 5, `Smart` (`Cosine`) |
| `cold_threshold_basis_points` | 3000 |
| `delete_confidence_basis_points` | 5000 |
| `max_hard_delete_ratio_basis_points` | 5000 |
| `duplicate_threshold_basis_points` | 7800 |
| `decay_rate_basis_points` | 800 |
| `context_enrichment_enabled` | on |
| `recursive_memory_loops`, `recursive_memory_loop_hard_cap` | off, 20 rounds |
| `structured_fallback_format` | `Xml` (`Json`) |

Scores are integer basis points (0 to 10 000) so a stored policy compares and round-trips exactly. An unset minimum similarity lets the embedding model decide: a calibrated model's published thresholds (Eidos), or `DEFAULT_MIN_SIMILARITY_BASIS_POINTS` (3500, that is 0.35) on raw cosine. The structured fallback format is the document format the memory cycle asks for when a model answers without tool calls.

### Helper job settings

- `HelpMeReplySettings`: enabled, streaming, 150 output tokens, ten history messages (`history_count()` reads zero as ten), `Roleplay` style (or `Conversational`), and optional model and per-style prompt overrides.
- `LorebookGeneratorSettings`: a model and four prompts (planner, writer, refine, coherence) plus the target entry count (default 12, clamped to 5..50) and output tokens (default 4096, clamped to 256..32768). `select` resolves each choice as explicit override, then the generator setting, then the default model or built-in prompt. Whether the chosen model or prompt still exists is checked when the consumer resolves it, not when the document is saved.
- `LorebookEntryGeneratorSettings`: model (unset means the first text model), entry and keyword prompts, JSON fallback format by default.
- `CreationHelperSettings`: model (unset means the default model), streaming on, `enabled_tools` (unset means all), tool fallback `Native`, `Json` or `Xml`.
- `CompanionSoulWriterSettings`: model (unset means the default model, then the first text model), a fallback model, prompt, JSON fallback format by default.
- `ImageGenerationSettings`: avatar generation on, scene generation off, scene mode `Auto` (`AskFirst`, `Manual`), and the avatar, scene, scene writer and creation helper image models (unset means the first suitable model). `scene_default_size` is used by scene generation when the scene model sets none (unset means 1024x1024).

These store the choices only. The jobs take a resolved model profile per request, picked by the resolvers in `lettuce-app` (`creation_helper_model`, `lorebook_entry_generator_model`, `soul_writer_model`, `soul_writer_fallback_model` and others).

### Model selections

`selected_model_profiles` lists the ten model profiles the document selects, and `clear_model_profiles(removed)` unselects every one the predicate matches. The app calls it when a model is deleted locally or through sync, and a synced settings snapshot drops selections of models this device does not have. A cleared selection behaves like one never made: the job falls back to its default (default model, first text model or first suitable image model).

### Storage and routes

`StoredGlobalSettings` wraps the document with a revision, timestamps, and four selections kept outside the JSON as typed ids with foreign keys: the default model profile, the default prompt document for direct chats, the dynamic-memory model and the group speaker-selection model. `GlobalSettingsStore::save` writes the document and the default model with a revision CAS. `set_dynamic_memory_model_profile` and `set_group_speaker_model_profile` are narrow CAS setters, so a retried feature route updates only its own selection without rewriting the rest. A selection naming a missing profile fails with `ModelProfileMissing`.

## Device settings

`DeviceSettings` (`device.rs`) describes this machine rather than the user, lives in the device-local `device_settings` row and never syncs:

- `trusted_certificates`: up to 64 root certificates every provider request trusts. Each needs a non-blank name (at most 1 KiB), PEM text with `BEGIN CERTIFICATE` and `END CERTIFICATE` markers, at most 1 MiB so CA bundles fit, and no duplicate id or PEM. `trusted_roots_pem` hands them to `lettuce-app`, which builds the `TlsPolicy` for provider clients and skips (with a warning) a root the TLS stack cannot parse.
- `embedding`: which embedding model loads (`V3`, `V4` or `V5`, Eidos; unset prefers Eidos), its token window (unset means 4096) and whether the model stays loaded.
- `llm_models_dir`: where GGUF downloads go, image models in its `image` folder; unset means the app's own models folder.

These are per device because they describe one machine: a path, network trust, installed files. A backup carries the certificates and the embedding choice and a restore writes them; the models folder is a local path, so a restore keeps the previous file's unless the restored file names one.

`DeviceUiStateStore` keeps the shell's state about this install (onboarding progress, dismissed hints, the last version seen, active-usage counters) as a JSON object of at most 256 KiB in the device-local `device_ui_state` row. It never syncs and never enters a backup; a restore keeps the previous database's state.

## Secrets

`secrets.rs` defines the boundary:

- `SecretRef` is an opaque UUID that a provider account, audio provider or the app stores in place of the key.
- `SecretPurpose` binds a reference to what it is for: `ProviderApiKey { owner }`, `ProviderSecretHeader { owner, name }`, `AudioApiKey { owner }`, `SproutApiKey { owner }` (the Sprout hardware probe an Ollama account points at), `HuggingFaceAccessToken` and `CivitaiAccessToken`. Every operation names the purpose and fails with `PurposeMismatch` if it does not match.
- `SecretValue` is at most 16 KiB, zeroized on drop, prints `[REDACTED]`, is neither `Clone` nor `Serialize`, and is only readable inside `with(|value| ...)`.
- `SecretStatus` is metadata only: reference, purpose, generation, `SecretState` (`Present`, `Missing`, or `Unavailable` with a reason such as `UserAuthRequired` or `KeyInvalidated`) and when this device last wrote it. Presence and backend availability are separate on purpose.

`SecretStore` has four operations. `put(record, value, expected_generation)` creates with `None` and rotates with `Some(status.generation)`; the store computes the next generation and rejects a stale one. `load` returns a `SecretValue`, `status` returns the metadata. `delete` is idempotent: an absent reference returns `Missing` even with an expected generation, while a live entry still checks purpose and generation. A missing or unavailable secret is always a typed state or error, never an empty key or a backend message.

A provider adapter uses it like this: the account holds a `SecretRef` with `ProviderApiKey { owner }`, settings reads show only `status`, and when a request is built the adapter calls `load`, borrows the value to build the authorization header, and drops it.

### Native store

`NativeSecretStore` (`native_secrets.rs`) is the production adapter: Secret Service on Linux, Keychain on macOS and iOS, Credential Manager on Windows, and Android's Keystore-backed encrypted shared preferences (`android-native-keyring-store`). Each reference is one credential under the service `com.lettuceai.app.secrets.v1`, holding a versioned JSON envelope with the exact purpose, the generation, the value and the write time. The envelope limit (128 KiB) leaves room for the worst-case JSON escaping of a 16 KiB value.

Mutations are serialized inside the store instance and rotation is a generation CAS. A corrupt, wrong-version, wrong-purpose or unreadable entry fails closed. Encoded and decoded buffers are zeroized, and there is no plaintext fallback. On Android, `try_new` must be called only after Tauri has initialized `ndk-context`; initialization and Keystore failures are typed store errors, and Android never falls back to a mock keyring. Tests run against an injected in-memory backend and never touch the developer's credential store.

### Passphrase vault

Where no OS credential store exists (Linux without a Secret Service), `NativeSecretStore::with_passphrase_vault` stores credentials in a `PassphraseVault` the user creates explicitly (`passphrase_vault.rs`). An Argon2id key (64 MiB, 3 iterations, 4 lanes, random salt) derived from the passphrase seals one file with XChaCha20-Poly1305 under a fresh nonce on every write, with the KDF header as associated data. Only the derived key is kept in memory, for the unlocked session; the passphrase is never stored. A wrong passphrase fails closed.

The file is reached through the `VaultFile` port. Its `write` must replace the file atomically, so a crash leaves the old or the new vault, and `create_new` must fail when a vault already exists, so `create` never overwrites one, even under a concurrent create. Replaced entries are zeroized. Credentials never fall back to a machine-derived key, a plaintext file or SQLite.

### App-wide secrets

The Hugging Face and CivitAI tokens are one per install and have no owner. `SecretPurpose::app_secret_ref` gives each a fixed reference (a UUIDv5 in the crate's namespace), so every part of the app finds the same entry without storing the reference anywhere. The Hugging Face and CivitAI browsers in `lettuce-app` read them, and backup and legacy import write them.

`InMemorySecretStore` is a deterministic test adapter with no encryption, persistence or platform protection.
