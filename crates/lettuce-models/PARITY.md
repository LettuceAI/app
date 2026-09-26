# lettuce-models: legacy parity notes

Facts about how `lettuce-models` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- `ModelSettingsLayer` is the shape of legacy's two other layers: a conversation's own model settings (`CurrentConversationSettings.model_settings`) and the app-wide defaults (stored in `app_settings.model_settings_json`). Legacy resolved every field as conversation, then model, then app; an unset field defers. `resolve_chat_profile` takes the app layer as `global` and the conversation layer as `session`.
- `LlamaCppSettings` enum values and ranges mirror the legacy model editor.
- llama.cpp settings resolve per field like legacy's `resolve_llama_*`. App features use the feature slot's sampler as the session layer, as legacy's synthetic session did. The fixed dynamic-memory sampler (direct DRY 0.8/1.75/2/-1, group DRY multiplier 0) is legacy's and also turns off the profile defaults.
- An adaptive-p target of 0 turns adaptive-p off and masks lower layers, as legacy's post-lookup filter did. The adaptive-p values do not count as a slot override that keeps the fixed memory sampler from applying, as legacy's check ignored them.
- `force_gemma4_reasoning` resolves model first, then session, with no app layer, as legacy did.
- Reasoning resolves like legacy (user decision 2026-09-22): the mode is on only when set to enabled (session, then model); an effort or budget never turns it on. The budget resolves even when reasoning is off, and the total output allowance is the visible cap plus the budget. Legacy imports keep effort and budget with reasoning off.
- A configured sampling parameter is rejected only when capability evidence says `Unsupported`; `Unknown` passes through, as legacy sent every configured value. Nothing fills `parameter_support` yet, so imports and new models carry `Unknown`.
- Legacy unset override fields map to `Inherit`; an explicit `Clear` stays supported.
- The model catalog lists accounts and profiles oldest first, like legacy's settings list (`ORDER BY created_at`).
- The Sprout probe fields are legacy's `sproutEnabled` / `sproutUrl` / `sproutApiKey` from the Ollama credential config; the ComfyUI workflows are legacy's `txt2imgWorkflow` / `img2imgWorkflow`.
- Feature slots are legacy's `featureGenerationSettings`.

## Deliberate differences from legacy

- Legacy showed the adaptive-p values in each feature slot but never copied them into the request. Feature slots now apply them.

## History

- The lorebook generator's sampling used to live in `ModelProfileConfig.lorebook_generator_parameters`. It moved to `feature_parameters.lorebook_generator.parameters`; configs that still carry the old field are migrated on read, and a stored `legacy_advanced_settings` field is ignored because its keys are migrated from their source. The staged generator admission supplied these parameters with legacy feature defaults and reasoning disabled; ordinary chat resolution does not read feature slots.
- The previous README said legacy Ollama `num_ctx`, `num_predict` and `repeat_penalty` import into the neutral `context_length`, `max_output_tokens` and `repetition_penalty`, and that the nested `ollama` group held only twelve native controls. `OllamaOptions` now also has `num_ctx` and `num_predict` of its own, sent in place of the neutral values when set.
- The previous README described chat parameter resolution as `operation -> session -> model -> global` for every field. In the code, top-k, the penalties, reasoning and prompt caching have no app layer.
- The previous README said the character and group persistence slice did not yet report `GroupMemberOverride` references on profile and account deletion. `lettuce-database` now reports them.
- The previous README said embedded-runtime sampler controls were outside the feature-parameter slice. Feature slots now carry llama.cpp sampler overrides next to the chat parameter overrides.
