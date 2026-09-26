# lettuce-models

Provider accounts and model profiles: how the app connects to a provider, what it knows about each model, every per-model setting, and the resolver that turns a model selection plus the conversation and app layers into the exact parameters one request runs with.

The crate does not run inference, talk to providers or install files. It holds typed data, validation and pure resolution functions; persistence is behind synchronous repository traits that `lettuce-database` implements (the SQLite adapter is synchronous, so the application calls them on its database worker). Secrets are never model data: an account holds `SecretRef`s from `lettuce-settings`, and only the provider adapter loads the values.

## Structure

- `lib.rs`: `ProviderAccount`, `ProviderConfig`, `ModelProfile`, `ModelProfileConfig`, connection validation, the dependency vocabulary and the repository traits.
- `capabilities.rs`: `ModelCapabilities`, the evidence of what a model supports.
- `parameters.rs`: `ChatParameterProfile` (set values) and `ChatParameterOverrides` (`Inherit`, `Set`, `Clear` per field), prompt caching, reasoning, Ollama and OpenRouter options.
- `local_runtime.rs`: `LlamaCppSettings` with `LlamaSamplerSettings`, `StableDiffusionSettings`, `FeatureParameters` and `ModelSettingsLayer`.
- `resolution.rs`: `resolve_chat_profile`.
- `llama_resolution.rs`: `resolve_llama_settings`.
- `model_paths.rs`: rewriting stored model file paths when a folder moves.

## Provider accounts

A `ProviderAccount` is one connection: a stable id, a `secret_owner_id` that binds its secrets, a free `provider_kind` (`openrouter`, `ollama`, `comfyui` and so on), a `ProviderProtocol` (`OpenAiCompatible`, `Anthropic`, `Gemini`, `Ollama`, `LlamaCpp`, `StableDiffusion`), a label, an optional endpoint, enabled, streaming and `allow_invalid_tls` flags, an optional `api_key_ref`, custom headers as `SecretHeader`s (name plus `SecretRef`, so header values are never plain strings) and a `ProviderConfig`:

- `Standard` for most accounts.
- `Custom(CustomProviderConfig)` for user-defined OpenAI- or Anthropic-format providers: chat path, optional models path (unset disables model fetching) with JSON paths into the model list response, streaming, `CustomAuth` (`None`, `Bearer`, a named header or a query parameter), role name remapping, whether to merge same-role messages, whether to send chat template kwargs, and the tool choice mode.
- `ComfyUi(ComfyUiConfig)` with the API-format text-to-image and image-to-image workflows (at most 4 MiB each).
- `Ollama(OllamaConfig)` with an optional Sprout hardware probe next to a remote Ollama server (`enabled`, `url`, `api_key_ref` to the account's `SproutApiKey` secret). `active_sprout` returns it only when it is switched on and has a URL. Anything that accepts a standard Ollama account accepts this one.

`validate_provider_connection` checks the non-secret metadata shared by storage and resolution: endpoint shape, custom paths (a `/route` with an optional fixed query, or a whole http(s) URL), that `ComfyUi` and `Ollama` configs sit on the matching provider kind, that custom accounts have an endpoint, that no header is one the HTTP client owns (`host`, `content-type` and the like), and that header names and secret refs are unique and do not reuse the API key's reference.

## Model profiles

A `ModelProfile` always points at one explicit provider account. There is no fallback to a provider label, a sole account or the first account. It carries the provider's `external_model_id`, a display name, a `ModelKind` (`Chat`, `Image`, `Embedding`, `Speech`) and a `ModelProfileConfig`. For local runtimes (llama.cpp and stable-diffusion.cpp) the external id is the absolute path of the model file. A profile received through sync before a file was picked on this device uses `UNPICKED_LOCAL_MODEL_FILE`, since model files and their paths stay on the device that has them.

`ModelProfileConfig` owns every per-model setting as typed data, because the model editor is where per-model configuration lives:

- `capabilities`: `ModelCapabilities`.
- `chat_parameters`: `ChatParameterProfile`, including `send_thinking_state`.
- `feature_parameters`: one `FeatureGenerationParameters` (chat parameter overrides plus llama.cpp sampler overrides) for each app feature that runs inference on the model: dynamic memory, companion Soul writer, companion memory, lorebook entry generator, lorebook generator, scene writer, help-me-reply, group speaker selection, creation helper.
- `llama_cpp`: `LlamaCppSettings`: GPU and CPU layer placement, multi-GPU distribution (automatic or manual layer assignments), KV cache type (or separate K and V types) and placement, threads, batching, RoPE, flash attention, SWA, chat template override or preset, the multimodal projector, raw completion fallback, MTP and DFlash speculative decoding (DFlash: enabled, 1 to 15 draft tokens, minimum draft probability 0 to 1, drafter path), streaming, forced Gemma 4 reasoning, and the `LlamaSamplerSettings` (profile, stage order, min-p, typical-p, penalties, DRY, XTC, seed, adaptive-p).
- `stable_diffusion`: `StableDiffusionSettings`: generation defaults, hires fix, skip-layer guidance, cache and offload modes, VAE tiling, reference image handling, base LoRAs, prompt writer instructions, and the `StableDiffusionCppBinding` the model catalog installed (component paths, runtime release and backend, feature flags).

Empty groups are not serialized. `validate_parameters` checks all groups; enum values and ranges follow the model editor. The runtimes consume these settings unchanged; sizing formulas live in the runtime crates.

`ModelSettingsLayer` has the same shape without capabilities or feature slots. It is used for the two other layers: a conversation's own model settings and the app-wide defaults behind `GlobalModelSettingsRepository`, which are stored with the global settings row and share its revision. Feature slots exist only at the model level.

### Capabilities

`ModelCapabilities` records what is known about a model, with the `CapabilityEvidence` (source: provider-reported, catalog, probed, user override, or unspecified; source version; observation time). Every capability is three-valued, `Supported`, `Unsupported` or `Unknown`, because a missing probe or catalog entry is not evidence that a feature is missing. It covers input and output modalities (text, image, audio), streaming, tools, structured output, reasoning, prompt caching, context length, visible and total output limits (`None` means no limit was established, not zero), and `ParameterSupport` for the common sampling parameters.

### Dependencies

Deleting a profile that authored data still uses fails with `ModelRepositoryError::InUse`, listing typed `ModelDependencyReference`s: `CharacterDefault { character_id }` or `GroupMemberOverride { group_id, character_id }`. The vocabulary lives here so the storage adapter does not need the characters crate. Deleting an account with profiles fails with `AccountInUse`; `delete_with_profiles` clears the global default, removes the profiles and the account in one transaction, and leaves secret cleanup to a separate workflow.

`ModelLookup` finds an account by kind and label and a profile by external id, for installers that register the same account or model again. `ModelCatalog` lists everything oldest first.

## Resolving a chat request

`resolve_chat_profile(expected, profile, account, input, requirements)` produces a `ResolvedChatProfile`, everything a provider adapter needs for one request:

1. Identity. `ExpectedModelIdentity` is what the caller selected (ids, revisions, external id, display name, protocol, kind); any difference from the live records fails with `IdentityMismatch`, so a request never runs against a model that changed underneath it.
2. Account. The connection must validate, the account must be enabled, the profile must be a chat model, the protocol cannot be stable-diffusion, and an API key reference must be present where the protocol or custom auth needs one.
3. Requirements. `ChatRequirements` lists the modalities and capabilities the caller needs; each must be `Supported` (`Unknown` fails too, with its own error).
4. Parameters. Each field resolves operation, then session, then model, then app. `Set` wins, `Clear` stops the lookup with no value, `Inherit` defers. Temperature, top-p, output tokens, context length and the Ollama options use all four layers; top-k, the penalties, reasoning and prompt caching have no app layer, so ordinary sessions inherit the model while an operation can still disable something explicitly. `send_thinking_state` takes the model's value, then the session's.
5. Reasoning. The mode is on only when it resolves to `Enabled`; an effort or budget never turns it on. The budget still resolves (explicit, else 2048, 8192 or 16384 for low, medium or high effort), and the total completion allowance is the visible output cap plus the budget.
6. Checks. An OpenRouter endpoint pin (the model's, else the app's) is rejected on other providers; only the endpoint tag is stored, never display names, logos or prices. A configured sampling parameter is rejected only when `ParameterSupport` says `Unsupported`; `Unknown` passes the value through. Reasoning settings need reasoning support and enabled prompt caching needs prompt cache support. Context and output values above a known limit fail; unknown limits produce warnings.
7. llama.cpp. For llama.cpp accounts the result also carries `resolve_llama_settings`.

Ollama keeps its native options in the nested `ollama` group: `num_ctx` and `num_predict` (sent in place of the neutral context length and output cap when set), `num_keep`, batching, GPU and thread counts, tail-free, typical and min-p sampling, mirostat, seed and stop strings. Ordinary chat controls stay on the provider-neutral fields.

Prompt caching is a closed policy (`Disabled` or `Enabled` with a `PromptCacheRetention` of in-memory, five minutes, one hour or 24 hours) rather than seconds or wire strings; adapters translate it and reject combinations the provider cannot do.

### llama.cpp settings

`resolve_llama_settings(model, input)` resolves each field from the session layer, then the model, then the app layer. For app features the session layer is the feature slot's sampler. Some fields have their own rules:

- A single-GPU pin is dropped when multi-GPU is enabled at the same or a more specific layer.
- The K and V cache types resolve as one unit from the first layer that sets any of them.
- `force_gemma4_reasoning` takes the model first, then the session, and has no app layer.
- An adaptive-p target of 0 turns adaptive-p off and hides lower layers' values.
- With `LlamaMemorySampler` set, a dynamic-memory request replaces the sampler with the fixed memory sampler (penalties, grammar, top-k, top-p, temperature, DRY, min-p, typical; direct memory uses DRY 0.8/1.75/2/-1, group memory DRY multiplier 0), keeps seed and XTC, and turns off the sampler profile defaults. Whether a feature slot overrides the memory sampler is `overrides_memory_sampler`; the adaptive-p values do not count.

## Model paths

Local model files are stored by absolute path. `rewrite_path_prefix` maps a path under an old folder to a new one, `relocate_profile_paths` applies a mapping to every path a profile stores (the model file itself for local runtimes, the llama.cpp projector, MTP and DFlash drafts, the stable-diffusion.cpp components and base LoRAs), and `ModelPathRelocation` applies it to every profile in one transaction when the user moves the models folder.
