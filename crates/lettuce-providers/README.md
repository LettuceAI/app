# lettuce-providers

The provider adapters: they turn a provider-neutral `InferenceRequest` into one provider's HTTP request (or an embedded llama.cpp call), read the buffered or streamed answer back into an `InferenceOutcome`, and do the same for remote image generation. The crate also serves the provider catalog, model listing, key verification, the Ollama model store and OpenRouter billing reads.

Provider-native DTOs are private. Callers see only the neutral types from `lettuce-conversations` and `lettuce-image-generation`, the descriptors, and a few result types. The crate does not decide what to send: the context, the resolved chat profile (model, parameters, reasoning, caching, streaming flags) and the tool request arrive already built. It does not execute tools, store anything or hold secrets; keys and secret headers are read from `lettuce_settings::SecretStore` per request.

- `lettuce-app` builds `RemoteProviders` and `RemoteImageProviders` with a TLS-configured client, attaches the media source, replay artifact store and embedded llama runtime, and calls them from chat, background jobs and settings screens.
- `lettuce-network` supplies the HTTP clients: `JsonClient` for chat (bounded requests, the generation timeout and retry policy, streaming responses) and `BulkHttpClient` for image traffic.
- `lettuce-inference` supplies `InferenceRuntimePort`, through which adapters emit stream deltas and check job cancellation.
- `lettuce-usage` owns the billing result types the OpenRouter reads fill.

## Structure

| Path | What it holds |
| --- | --- |
| `lib.rs` | `RemoteProviders`, the dispatch by protocol and kind, the `InferencePort` implementation |
| `providers/openai_compatible.rs` | The OpenAI-envelope family: `OpenAiWireProvider`, request encoding, buffered parsing, model listing |
| `providers/anthropic_messages.rs` | The Anthropic Messages family and `AnthropicWireProvider` |
| `providers/gemini_generate.rs`, `gemini_cache.rs` | The Gemini generateContent family, `GeminiWireProvider`, explicit cache resources |
| `providers/ollama.rs`, `ollama_hub.rs` | Ollama `/api/chat`, and the model store (inventory, pull, delete) |
| `providers/llama_cpp.rs` | The embedded llama.cpp runtime as a provider (desktop only) |
| `providers/<kind>.rs` | One small file per provider: its wire-trait overrides and its `DESCRIPTOR` |
| `providers/openrouter_pricing.rs` | OpenRouter endpoint pricing and generation details |
| `streaming/` | Byte framing (SSE, NDJSON), per-protocol normalization, the stream loop |
| `common.rs` | Auth plans, credentials, endpoint joining, usage and cost readers, lenient decoding, errors |
| `media.rs` | `ProviderMediaSource`, attachment loading, generated image extraction |
| `descriptor.rs`, `catalog.rs`, `verify.rs` | Provider descriptors, the catalog, key probes |
| `images/` | `RemoteImageProviders`: per-provider image adapters, ComfyUI, image usage |

## Provider families

There is no single "OpenAI-compatible" contract. Each wire protocol is a family with its own module and trait, and each provider is one file that implements the trait for its family:

| Family | Trait | Providers |
| --- | --- | --- |
| OpenAI envelope | `OpenAiWireProvider` | `openai`, `openrouter`, `custom`, `cerebras`, `deepseek`, `groq`, `xai`, `mistral`, `qwen`, `featherless`, `chutes`, `anannas`, `nanogpt`, `nvidia`, `moonshot`, `literouter`, `intenserp`, `pollinations`, `zai`, `lmstudio` |
| Anthropic Messages | `AnthropicWireProvider` | `anthropic`, `custom-anthropic` |
| Gemini generateContent | `GeminiWireProvider` | `gemini` (also `google`, `google-gemini`), `gemini-agent-platform-express` |
| Ollama `/api/chat` | none, one provider | `ollama` |
| Embedded llama.cpp | none | the `LlamaCpp` protocol, enabled with `RemoteProviders::with_local_llama` |

Every trait method has the standard behaviour as its default, and a provider overrides only what differs: endpoint and path, roles, same-role merging, auth, static headers, parameter validation, the reasoning policy, extra body fields, tool choice, streaming support. A provider may delegate to another provider's implementation (`OpenAi.chat_path(...)`). The result is that DeepSeek is a 34-line file and every provider difference is visible in one place.

`RemoteProviders::run` dispatches on the profile's `ProviderProtocol` and then on the provider kind with explicit match arms. Aliases (`cerebras.ai`, `chutes.ai`, `nvidia-nim`, `moonshot-ai`, `z.ai`) resolve; an unknown kind is rejected, never defaulted to OpenAI. `lettuce-host` and `lettuce-engine` are rejected. Stable diffusion and llama.cpp on mobile are rejected at the same point.

Each provider file carries a `DESCRIPTOR`: kind, display name, protocol, aliases, default endpoint and whether it is editable, API key requirement and header, streaming, model listing and key verification support, reasoning and prompt caching support, parameter flags and the extra-body allowlist. `provider_descriptors()` serves them as the catalog. Descriptors report native tool translation for all four remote families and keep structured output false. Anthropic, Gemini and the reasoning-capable OpenAI-envelope providers advertise signed tool replay and reasoning with tools; Ollama does not (see Tools).

## A request, step by step

Taking the OpenAI-envelope family as the example; the other families have the same shape.

1. Validate the request: common checks, tools against the model's capability, the reasoning allowance (the output allowance must be the visible cap plus the reasoning budget, as resolution computes it), and the provider's own parameter rules.
2. Fit prompt caching to the provider (see Prompt caching).
3. Resolve the target. The profile endpoint or the provider default is normalized, the provider picks the chat path, and a path that is a whole URL replaces the endpoint. The standard path is `/v1/chat/completions`, or `/chat/completions` when the endpoint already ends in `/v1`.
4. Load attachments through the `ProviderMediaSource` and build wire messages: roles mapped (`Scene` becomes `system`), media inlined when allowed, tool calls and results in the family's transcript shape. Providers that ask for it get consecutive plain messages of the same role merged with a blank line between them.
5. Encode the body: parameters, the reasoning fields from the provider's `ReasoningWirePolicy`, tools and tool choice, cache controls, and `stream: true` only when a stream sink is present and both the profile and the provider allow streaming.
6. Load auth and secret headers from the secret store.
7. Send through `JsonClient` with the generation policy (long timeout, bounded retries), wrapped in `await_cancelable` so the job's cancellation ends the wait.
8. Parse the buffered response, or run the stream loop (below).
9. Check the tool calls in the outcome against the request, and trim the reply text and reasoning.

A stream sink on a provider or account with streaming off runs the request buffered instead of failing it. Cancellation alone never changes the wire protocol. Gemini Express `-image` models always run buffered, with `responseModalities: ["TEXT","IMAGE"]`.

## Endpoints, auth and headers

Custom accounts (`custom`, `custom-anthropic`) take their shape from `CustomProviderConfig`. `chatEndpoint` and `modelsEndpoint` may be a `/route` with a fixed query (Azure's `?api-version=`), a bare segment joined with `/`, or a whole `http(s)://` URL that replaces the account endpoint. Header, bearer and query auth are sent only when the account has a key, so keyless local servers work in every mode; an unknown `authMode` means bearer. Custom accounts also choose whether same-role messages merge, their tool-choice mode and optional `chat_template_kwargs`.

Header precedence is account headers over the auth header over static headers (`User-Agent`, `Accept`, `Authorization`, `HTTP-Referer`, `X-Title` included). Transport headers owned by the client are ignored with a warning. `allowInvalidTls` is honoured only for providers with an editable endpoint (local and custom); hosted providers always validate certificates. Gemini sends its key only as the `x-goog-api-key` header.

## Reasoning

Reasoning is a per-provider wire policy, not a flag. The OpenAI-envelope policies (`ReasoningWirePolicy`):

- `MaxTokens` (DeepSeek, Featherless and others) and `MaxCompletionTokens` (LiteRouter, IntenseRP): the output cap plus the budget in that field, plus the effort. With reasoning off the cap plus budget is still sent.
- `OpenRouter`: the nested `reasoning` object; `reasoning: {}` when on with neither effort nor budget.
- `EnableThinking` (Qwen, Moonshot) and `Zai` (zAI's thinking state).
- `ReasoningObject` (custom OpenAI-format): `reasoning: {effort, max_tokens}` when on, the output cap never raised by the budget.
- `MaxCompletionTokensAndReasoningObject` (LM Studio): `max_completion_tokens`, the effort and a `reasoning` object.
- `Ignored` (Mistral): reasoning settings are dropped and the budget is not added.

Anthropic sends a budgeted thinking block and adds the budget and forces temperature 1.0 only when thinking is actually sent (reasoning on with a budget). Gemini uses a thinking config chosen by model family, and Ollama its native `think` field. Nothing rejects an effort or budget while reasoning is off. Buffered and streamed responses keep native reasoning and tagged thinking in the same normalized shape.

## Tools

Adapters translate and validate tool calls; admission, execution and handler dispatch belong to `lettuce-conversations` and `lettuce-app`.

- OpenAI envelope: typed function definitions, provider-specific tool choice (Mistral maps required to `any`, zAI always sends `tool_choice: "auto"`, custom accounts keep their configured mode), assistant-call and `tool` result replay, buffered calls and fragmented SSE calls. OpenRouter cache-controls the final tool definition.
- Anthropic: native definitions and choices, `tool_use` and `tool_result` blocks, buffered responses and SSE input fragments. `tool_use` blocks count under any `stop_reason`.
- Gemini: native function declarations, AUTO, ANY and named choice, grouped `functionCall` and `functionResponse` parts, buffered and SSE calls. When a Gemini explicit cache resource is used it owns both the tool definitions and the tool choice.
- Ollama: native definitions and ordered call and result replay; calls are accumulated atomically across NDJSON chunks. The API has no tool-choice field, so only `Auto` is supported; required or named choice and reasoning with tools are rejected rather than approximated.

Missing provider call ids stay absent in the Gemini and Ollama paths instead of being invented. Where an id is needed in the OpenAI-envelope family, a call without one gets `tool_call_{n}`, 1-based within the response.

### Signed replay

Extended thinking with tools only works if the provider gets its own signed assistant content back byte for byte. For Anthropic, a thinking tool round stores one conversation-retained native assistant-block document behind an opaque replay reference, through `ProviderReplayArtifactPort`. Buffered responses keep the exact content-array bytes; streaming rebuilds one bounded canonical native array including the thinking signatures. On continuation the artifact is validated against every call and the stored JSON is embedded unchanged.

Gemini does the same for signed function-call rounds: the exact buffered `parts` array or the canonical streamed parts sequence, including each `thoughtSignature`, verified against the ordered call ids, names and arguments on continuation. Standard Gemini and Agent Platform Express share this path, and unsigned Gemini behaves as before. A Gemini image part is neither counted toward nor kept in the signed replay.

Malformed signatures, changed calls, tampered artifacts and unavailable replay storage fail closed.

## Streaming

The stream loop in `streaming/streaming.rs` has three layers:

1. `StreamFramer` splits bytes into records, SSE or NDJSON. Records are not size-capped, because image-output models stream a whole base64 picture as one record.
2. `StreamNormalizer`, one per protocol (`OpenAi`, `Anthropic`, `Gemini`, `Ollama`), turns records into text and reasoning deltas and accumulates tool calls, usage, finish reasons, safety outcomes, response ids and replay bytes. Tool arguments, thinking signatures, error text and replay documents have their own bounds.
3. The loop emits each delta through the runtime with an increasing sequence, checks cancellation between chunks, and at the end emits the normalizer's tail and returns the outcome.

Malformed, incomplete or contradictory protocol records fail closed. An OpenAI stream may end without `[DONE]` or a final blank line, and a plain JSON body answering a stream request is read as one record. The text, reasoning and total wire size of a generation stream are not capped. Socket backpressure comes from reading the response one chunk at a time.

When a job is cancelled mid-stream, the normalizer builds a cancelled outcome from the text and reasoning that already reached the sink, plus every image already received, so the partial reply can be kept. A stop after only images keeps the images.

## Parsing responses

Buffered and streamed parsing are deliberately lenient about what providers actually send:

- `tool_calls`, `reasoning` and `error` may be `null`; `null` and missing mean the default.
- A usage counter that is not an integer or integer string is unknown, never an error.
- A streamed tool fragment without an index opens a new call.
- Tool arguments: `<parameter=x>` bodies are parsed, a double-encoded object is unwrapped, and blank or non-object arguments become `{}`.
- A `tool_calls` finish with no calls keeps the text.
- A reply with only images is not empty.
- Error bodies that are not JSON, or JSON without `error` or `message` text (FastAPI's `detail`), keep their text as the message.

Outcomes keep the provider's raw finish reason and the bounded request id from the response headers. OpenAI buffered and SSE outcomes also keep the response body `id` as `provider_response_id`; an omitted or null id in a later SSE frame keeps the earlier one, and a changed or non-string id rejects the stream. Other families leave the response id unknown.

## Usage and cost

Usage is evidence, reported in each provider's own terms. Native input and output totals are never converted to another provider's billing semantics: Anthropic cache reads may exceed its native `input_tokens`, and Gemini thoughts may exceed `candidatesTokenCount`, so OpenRouter's inclusive-token cost formula must not be applied to them.

- OpenAI envelope: `prompt_tokens_details.cached_tokens` and `completion_tokens_details.reasoning_tokens`, the older top-level cache-read and reasoning or thinking aliases and nested camelCase details (top-level wins), cache-write and server-tool web-search counters with their camelCase aliases. An invalid optional counter falls through to a valid alias without inventing zero.
- Anthropic: `cache_read_input_tokens`, `cache_creation_input_tokens` and `server_tool_use.web_search_requests`. Both `message_start` and `message_delta` update the cumulative counters; an omitted field keeps earlier evidence, an explicit zero replaces it. No reasoning count is inferred from thinking text.
- Gemini: `cachedContentTokenCount` and `thoughtsTokenCount`.
- Image, audio and total token counts come from OpenAI `usage` and Gemini `usageMetadata` (including AUDIO modality details) for chat and image responses; image tokens are also taken from `completion_tokens_details`, and cached prompt tokens are not image tokens.
- Gemini and Ollama leave the cache-write and web-search counters unknown.

A missing detail stays `None`, distinct from a reported zero. OpenAI streaming usage frames update only the counters they supply; partial or null frames keep earlier facts, an explicit zero replaces a count, and cumulative frames are not summed as separate requests.

OpenAI-envelope responses keep a provider-reported monetary cost from `cost`, `total_cost` or `totalCost`, as a number or numeric string. Non-finite or negative amounts are ignored, a missing cost stays unknown, and stream frames replace a supplied cumulative cost and keep it when omitted.

## Prompt caching

`normalize_prompt_caching` fits the profile's caching request to the provider. Providers without explicit caching drop it. The stored TTL is read per provider: cache-control providers use one hour if chosen and otherwise five minutes, Gemini uses five minutes if chosen and otherwise one hour, and OpenAI uses 24 hours if chosen and otherwise in-memory retention. Descriptors expose the exact retention choices.

- Anthropic, custom Anthropic and OpenRouter use typed cache-control annotations on the system prompt, the final tool definition and the final user text.
- OpenAI uses its typed request-retention field.
- Groq and Gemini Express cache automatically and get no app-side controls.
- Standard Gemini creates explicit `cachedContents` resources for the stable prefix, reuses them in process until their five-minute or one-hour expiry, and sends the final content as the live turn. Creation is best effort: a failure sends the original clean request, and a missing resource is evicted before one uncached retry that restores the clean request. Cache names are never persisted, because the provider owns and expires them.

## Media

Input media reaches a provider only from user messages, only when the model supports that input (image or audio), and only through the `ProviderMediaSource` the application attaches. Anything else (assistant reply images, other roles, text-only models) is dropped and the text is sent alone; the request is never rejected for it.

| Family | Images | Audio |
| --- | --- | --- |
| OpenAI envelope and llama.cpp | `image_url` data URLs with `detail: auto`, after the text | `input_audio` |
| Anthropic | base64 `image` blocks | not sent |
| Gemini | `inline_data` | `inline_data` |
| Ollama | the native `images` array | not sent |

Multimodal messages are never merged with their neighbours. Request bodies share the 64 MiB bulk bound so phone photos fit.

Output media: images a chat model returns ride on the candidate as `GeneratedMedia` (MIME type and the provider's base64). OpenAI-style `delta.images` and `message.images` entries count when their `image_url.url` is a `data:image/...;base64,` URL; Gemini `inlineData` or `inline_data` parts count when their MIME type is `image/*` (`image/png` when missing), skipping thought parts and non-image data. Streamed images are kept whole per record with no cap of their own. Finalization in the application stores them as assets.

## Errors

Adapters fail with `AdapterError`, which maps to the conversation `PortError`: malformed and empty responses become `Empty`, rejected requests and credentials `Rejected`, missing secrets and transport failures `Unavailable`, cancellation `Cancelled`, and provider failures `Provider(ProviderFailure)`. A provider failure carries a typed status and category plus the bounded provider code and message; `Debug` redacts the message.

## Embedded llama.cpp

`providers/llama_cpp.rs` runs llama.cpp models on the in-process runtime from `lettuce-local-llm`. `LocalLlama` holds the runtime and the app's `LlamaHost`, which receives metrics, the runtime report and UI events. `LocalRuntimeExclusion` lets the host stop the stable-diffusion.cpp server before every llama.cpp request.

The request is OpenAI-shaped: an assistant tool-call turn has null content when it has no text, and tool results are `tool` messages with their call id. `parallel_tool_calls` is on whenever tools are offered. The output cap plus the reasoning budget (4096 when unset) is the allowance, the reasoning request turns on the template's reasoning format, and the thinking switch is sent only with `send_thinking_state`. Per-field llama settings come from the chat profile, DFlash included. For chat turns the conversation is the prompt cache key. Streaming off for the account or model runs the request unstreamed. Cached and cache-write prompt tokens are always reported.

Tool calls keep every argument shape the chat loop executes: `<parameter=k>v</parameter>` bodies become `{"k": v}`, and empty, `null` or non-JSON arguments become `{}`. The raw text is kept only when it is the JSON form of the arguments.

Attachments are inlined as for the OpenAI envelope (text first, images as data URLs with `detail: auto`, audio as `input_audio`) when the model supports that input; the runtime needs the model's mmproj for them. Local failures are non-retryable `LOCAL_INFERENCE_FAILED`.

## Catalog, listing and verification

- `list_models` fetches one account's model list per family. Custom accounts can point at any JSON shape with dotted paths (`data`, `result.models[0]`, `id`), and fall back to the OpenAI `data[]` parser when the paths match nothing. zAI and Gemini Express return an empty list.
- `verify_api_key` probes an account's credential with the short probe policy, as the settings page does on save.
- Ollama: `num_ctx` and `num_predict` come from the account's Ollama settings first and fall back to the generic context length and output cap. Ollama's twelve native options and OpenRouter's pinned endpoint are sent as their own fields; the descriptors list them as the extra-body keys `options` and `provider`.

### Ollama model store

`ollama_inventory`, `ollama_delete` and `ollama_pull` talk to an Ollama account's server with its credentials. A pull reports each NDJSON progress line and is cancelled by dropping the future. It is sent once with only an idle timeout, since a total limit would kill large pulls. Lines are split on bytes, so a character split across chunks survives, and one progress line is capped at 1 MiB. A delete is never retried, because a lost response may hide a completed delete.

### OpenRouter billing

`openrouter_endpoint_pricing` and `openrouter_generation_details` reuse the account's credentials, the bounded JSON client and its 10-second probe policy. Endpoint and generation response identities must match the request. A 404 generation lookup returns `None`; authentication and transport failures stay typed errors. Native and normalized token counts stay separate and absent counts never become zero. Endpoint names, tags and exact price strings are kept without picking a fallback provider. Required prices and monetary evidence must be finite and non-negative; an endpoint without a provider name or usable prompt and completion prices is skipped. The wire structs are private and the result types belong to `lettuce-usage`. These reads add no cache and write no costs.

The contracts follow OpenRouter's [endpoint pricing](https://openrouter.ai/docs/api/api-reference/endpoints/list-all-endpoints-for-a-model) and [generation metadata](https://openrouter.ai/docs/api/api-reference/generations/get-request-&-usage-metadata-for-a-generation) documentation. Local HTTP fixtures cover bearer auth, base path and version handling, encoded query ids, 404 versus 401, free prices and separate counters; parser tests reject wrong identities and malformed required evidence.

## Remote image generation

`RemoteImageProviders` implements `lettuce_image_generation::ImageProviderPort` for every image provider except sd.cpp, which runs locally. It uses `BulkHttpClient`, and the job's cancellation token ends a request.

Adapters: OpenAI (with its `custom` and `lettuce-host` aliases), OpenRouter, Pollinations, Gemini (key as `?key=`), Gemini Express (`x-goog-api-key`), Stability, xAI, NanoGPT, LiteRouter (binary responses), Automatic1111, Diffusers and ComfyUI, each with its endpoints, fields, defaults and parser. A `custom` or `lettuce-host` account without an endpoint fails instead of sending its key to api.openai.com. Local image servers (Automatic1111, Diffusers, ComfyUI) run without auth when their optional key cannot be read.

The request flow:

1. Reference images and the mask are shrunk for upload (`shrink_for_upload`).
2. The adapter's request is sent. OpenRouter posts to its Image API (`{base}/v1/images`, the nearest supported aspect ratio, `input_references`) and falls back to chat completions on a 404, or a 400 or 422 that names the model or endpoint.
3. An HTTP 5xx, or a transient `error` in a 2xx body (500, 502, 503, 504, 529), is retried once after 1.5 s. Other body errors fail as `Provider error {code}: {message}`, and a 413 gets a readable hint about reference image size.
4. Linked results are downloaded, data URLs and raw base64 decoded, and usage read from the response.

ComfyUI uses the account's `ProviderConfig::ComfyUi` workflows: upload the inputs, substitute `%TOKEN%` placeholders, post to `/prompt`, poll `/history` every 1.5 s up to 400 times, and fetch the result from `/view`.
