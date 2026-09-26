# lettuce-providers: legacy parity notes

Facts about how `lettuce-providers` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- Every remote chat provider the legacy app shipped is executable for buffered and streaming generation, one file per provider on a family wire trait with legacy-style delegation.
- The embedded llama.cpp provider maps legacy `LlamaCppAdapter` and `build_llama_extra_fields` onto the ported runtime. Attachments are inlined like legacy `build_multimodal_content`, and the mmproj requirement is legacy's rule. Streaming off running the request unstreamed is the legacy fallback.
- The legacy aliases `cerebras.ai`, `chutes.ai`, `nvidia-nim`, `moonshot-ai` and `z.ai` resolve.
- `list_models` is legacy `get_remote_models`, including custom dotted-path parsing; `verify_api_key` is legacy `verify_provider_api_key`'s probes.
- Media input follows legacy `build_multimodal_content`: attachments on user messages reach the provider only when the model's image or audio input is supported (legacy `inputScopes`). Gemini audio MIME types follow legacy `gemini_audio_mime`. Multimodal messages are never merged, as legacy only merged string contents.
- Custom accounts follow legacy `custom.rs`; an unknown `authMode` is bearer, as legacy's default arm.
- Request policy follows legacy `request_builder`: a stream sink with streaming off runs buffered (legacy `effective_streaming_enabled`).
- Gemini Express `-image` models run buffered with `responseModalities: ["TEXT","IMAGE"]`, as legacy `gemini_agent_platform_express.rs` did (`disables_streaming_for_model`, `body`).
- Ollama `num_ctx`/`num_predict` fall back as legacy `build_ollama_extra_fields` did; the legacy importer keeps `ollamaNumCtx` and `ollamaNumPredict` as Ollama settings instead of folding them into the generic fields.
- Returned chat images are read as legacy `extract_image_data_urls_from_value` (`chat_manager/sse.rs` 553-620) read them.
- Response parsing is as lenient as legacy (`tooling.rs`, `sse.rs`). Text, reasoning, SSE records and the wire byte count of generation streams are not capped; legacy had no cap. Stored reply text and reasoning are trimmed like legacy `normalize_thinking_content`.
- OpenAI usage keeps the legacy top-level cache-read and reasoning/thinking aliases and nested camelCase details with legacy's top-level precedence, and the legacy nested cache-write and server-tool web-search counters. Provider-reported cost is read from the legacy `cost`/`total_cost`/`totalCost` fields.
- Image, audio and total token counts are read the way legacy `usage_from_value` did; Anthropic, Ollama and llama.cpp never reported them.
- An OpenRouter endpoint without a provider name or usable prompt and completion prices is skipped, as legacy `parse_provider_pricings` did.
- Reasoning wire behaviour follows each legacy adapter (verified 2026-09-22), including every policy listed in the README.
- Remote image generation (2026-09-22) ports legacy `generate_image` for every kind but sdcpp: the eleven legacy adapters with their endpoints, fields, defaults and parsers, and ComfyUI with legacy `%TOKEN%` substitution. The `BulkHttpClient` behaviour (shrinking, the OpenRouter Image API with its chat fallback, the single transient retry, the 413 detail) is release 2.2.5 behaviour, and the error texts are legacy's. Usage is found the way legacy's `extract_usage` did.
- Both legacy importers now keep the ComfyUI `txt2imgWorkflow` and `img2imgWorkflow`; they were dropped before.

## Deliberate differences from legacy

- Anthropic tool follow-ups use native `tool_use`/`tool_result` blocks. The legacy follow-up path declared Anthropic tools but replayed OpenAI-shaped calls and results that the Anthropic adapter then discarded.
- Custom Anthropic accounts no longer advertise frequency and presence penalties; the Messages body never carried them.
- Gemini sends the key only as the `x-goog-api-key` header; legacy also copied it into the query string.
- Legacy's llama.cpp field allowlist dropped the `llamaDflash*` keys, so chat never enabled DFlash. The chat profile now passes them.
- Legacy lost cached and cache-write prompt tokens on its non-streamed llama.cpp path; they are always reported now.
- A buffered tool call without an id gets `tool_call_{n}`. Legacy used `tool_call` for every buffered call, which collided.
- Blank or non-object tool arguments become `{}`. Legacy passed the raw string to the tool; the domain only carries objects.
- A stop keeps every image already received with the partial reply, and a stop after only images keeps the images. Legacy discarded the whole reply on abort.
- A `custom` or `lettuce-host` image account without an endpoint fails instead of sending its key to api.openai.com.
- An Ollama pull has only an idle timeout (legacy had none; a 30-minute total limit would kill large pulls), and pull lines are split on bytes, so a character across chunks survives (legacy corrupted it).

## Known gaps

- Legacy's `create_memory` saved a llama.cpp call whose body was plain text by taking the raw text as the memory (`extract_text_argument`, `memory/flow.rs:4498-4507`). `ProposedToolCall` needs object arguments that its raw text parses to and cannot carry non-JSON raw text, so such a call has no `text` and saves nothing.
- Providers' remote image result URLs are not kept beside the stored bytes.
- Gemini and Ollama leave cache-write and web-search counters unknown, and non-OpenAI families leave `provider_response_id` unknown.
- Deferred horizontals: a custom-provider reasoning schema and structured output.

## History

- The previous README said streaming uses bounded byte framing. `StreamFramer` does not cap records (image-output models stream a whole picture as one record); only tool arguments, thinking signatures, error text and replay documents are bounded.
- The previous README listed constructing `JsonClient::with_tls` from the trusted certificates and exposing catalog, listing and verification as composition-root work. `lettuce-app/src/generation/provider_runtime.rs` now builds the client with the TLS policy and exposes `provider_descriptors`, `list_models` and `verify_api_key`.
- The previous README said companion dispatch evidence retains `provider_response_id` and other workflow checkpoints are not wired to it. The initial chat inference checkpoint now stores and checks it too.

- The previous README said custom OpenAI reasoning and embedded local runtimes were deferred until their wire contracts existed. Custom OpenAI-format accounts now use the `ReasoningObject` policy and llama.cpp sends its reasoning request; only a configurable custom reasoning schema is still open.

## Not wired yet

- Generic extra-body passthrough behind `extra_body_keys` belongs to the model-parameters slice (`lettuce-models` profile config). Ollama's twelve native options and the OpenRouter pinned endpoint are implemented; duplicate legacy context, output and repetition fields map to provider-neutral settings.
- Tauri commands for the catalog, model listing and key verification (phase (c)).
