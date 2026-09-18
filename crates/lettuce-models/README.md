# lettuce-models

Provider connections, model profiles, capability evidence, and selection.

## Boundary

Does not execute inference or install model files.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Foundation contracts

`ProviderAccount` owns a stable connection identity, protocol, endpoint,
enabled state, typed non-secret configuration, and opaque secret references.
Custom header values are never serializable strings; each is a `SecretRef`.

`ModelProfile` always points to an explicit provider account. There is no
provider-label, sole-account, or first-account fallback. Repository ports are
synchronous because the SQLite adapter is synchronous; application composition
places calls on its database worker.

Model deletion dependencies use closed, typed `ModelDependencyReference`
values (for example, `CharacterDefault` and `GroupMemberOverride`) and surface
through `ModelRepositoryError::InUse`. A group-member override identifies both
the owning group and the character whose override retains the model profile.
Future adapters must report `GroupMemberOverride` references in both direct
model-profile deletion and provider-account-with-profiles deletion before
removing the referenced profile or account. The current character/group
persistence slice is not yet responsible for the group SQL; it must add those
typed references when that slice is implemented.

Chat parameters resolve per field in `operation -> session -> model -> global`
order. Provider-neutral controls have one canonical field: legacy Ollama
`num_ctx`, `num_predict`, and `repeat_penalty` import into `context_length`,
`max_output_tokens`, and `repetition_penalty`. The nested `ollama` profile is
reserved for its twelve native controls (`num_keep`, batching/device/thread
controls, tail-free/typical/min-p sampling, mirostat, seed, and stop strings).
This keeps provider-specific wire details typed without duplicating ordinary
chat settings.

`ModelProfileConfig.lorebook_generator_parameters` stores feature-specific
sampling using the existing `ChatParameterOverrides` type. Older documents
default to inherited values. SQLite validates and persists these with the model;
ordinary chat resolution does not consume them. Staged generator admission
supplies them to the operation layer with legacy feature defaults and reasoning
disabled. Canonical explicit `Clear` remains supported; legacy unset fields map
to `Inherit`. Embedded-runtime sampler controls remain outside this slice.

Prompt caching uses a closed retention policy rather than arbitrary seconds or
provider wire strings. It resolves `operation -> session -> model` with no
global fallback, so ordinary sessions inherit the model while specialized
operations can explicitly disable caching.

OpenRouter routing stores only an optional endpoint tag. A model-level pin
wins over the global default, is rejected for other provider kinds, and never
stores endpoint display names, logos, or pricing cache data.

Provider accounts and model profiles expose serialization only for the
validated canonical backup snapshot. Secret fields remain opaque references;
plaintext values are never model-domain data.

`ModelProfileConfig` owns every per-model setting as typed data (model editor
rule): `chat_parameters` (now with `send_thinking_state`), `feature_parameters`
(one `FeatureGenerationParameters` per app feature: dynamic memory, companion
Soul writer, companion memory, lorebook entry generator, lorebook generator,
scene writer, help-me-reply, group speaker selection, creation helper, each with
chat parameter overrides and llama.cpp sampler overrides), `llama_cpp`
(`LlamaCppSettings`: GPU/CPU placement and distribution, KV type and placement,
threads, batching, RoPE, flash attention, chat template, mmproj, MTP, streaming
and the `LlamaSamplerSettings`) and `stable_diffusion` (`StableDiffusionSettings`:
generation defaults, hires, SLG, cache, VAE tiling, LoRAs and the
stable-diffusion.cpp binding). Enum values and ranges mirror the legacy editor.
Empty groups are not serialized. `validate_parameters` checks all groups. The
former `lorebook_generator_parameters` moved to
`feature_parameters.lorebook_generator.parameters`. The embedded runtimes read
these settings when they land; runtime formulas stay unchanged.

`ModelSettingsLayer` (chat parameters, llama.cpp and stable-diffusion settings)
is the shape of the two other legacy layers: a conversation's own model settings
(`CurrentConversationSettings.model_settings`) and the app-wide defaults behind
`GlobalModelSettingsRepository` (stored in `app_settings.model_settings_json`,
revisioned with the global settings row). Legacy resolved every field as
conversation, then model, then app; an unset field defers. Feature generation
slots exist only at the model level. `resolve_chat_profile` takes the app layer
as `global` and the conversation layer as `session`.

A configured sampling parameter is rejected only when the model's capability
evidence says `Unsupported`; `Unknown` support passes the value through, as
legacy sent every configured value and nothing in the rewrite fills
`parameter_support` yet (imports and new models carry `Unknown`).
