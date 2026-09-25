# lettuce-providers

OpenAI-compatible buffered and SSE responses preserve optional
`prompt_tokens_details.cached_tokens` and
`completion_tokens_details.reasoning_tokens` in normalized usage. Missing
details remain None, distinct from a reported zero. Both paths also preserve
legacy top-level cache-read and reasoning/thinking aliases and nested camelCase
details, with the legacy top-level precedence. Invalid optional counters fall
through to valid aliases without inventing zero. Anthropic buffered/SSE
usage also preserves cache_read_input_tokens; it does not infer a reasoning
count from thinking text. Gemini buffered/streaming usage preserves
cachedContentTokenCount and thoughtsTokenCount. Native input/output totals
remain unchanged: these details are evidence, not a conversion to OpenRouter
billing semantics. In particular, Anthropic cache reads may exceed its native
input_tokens, and Gemini thoughts may exceed candidatesTokenCount. Do not
apply OpenRouter's inclusive-token cost formula to these native totals.
OpenAI-compatible buffered/SSE usage also preserves legacy nested cache-write
and server-tool web-search counters, including their camelCase aliases. Anthropic
buffered/SSE responses preserve cache_creation_input_tokens and
server_tool_use.web_search_requests. Both message_start and message_delta update
the cumulative native counters; omitted fields keep earlier evidence, while an
explicit zero replaces it. Gemini/Ollama leave these new counters unknown;
OpenAI-compatible responses retain provider-reported monetary cost from the
legacy cost/total_cost/totalCost fields, accepting numeric values and numeric
strings. Nonfinite or negative amounts are ignored. Missing cost stays unknown;
stream frames replace supplied cumulative cost and retain it when omitted.
OpenAI streaming usage frames update only counters they actually supply; partial
or null usage frames retain earlier facts. Explicit zero replaces a prior count,
and cumulative frames are not summed as separate requests.

Remote provider catalogs, request translation, stream parsing, and error mapping.

## Boundary

Provider-native DTOs remain private adapters.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Every remote chat provider the legacy app shipped is executable for buffered
and streaming generation, one file per provider on a family wire trait with
legacy-style delegation:

- OpenAI envelope (`OpenAiWireProvider`): `openai`, `openrouter`, `custom`,
  `cerebras`, `deepseek`, `groq`, `xai`, `mistral`, `qwen`, `featherless`,
  `chutes`, `anannas`, `nanogpt`, `nvidia`, `moonshot`, `literouter`,
  `intenserp`, `pollinations`, `zai`, `lmstudio`
- Anthropic Messages (`AnthropicWireProvider`): `anthropic`, `custom-anthropic`
- Gemini generateContent (`GeminiWireProvider`): `gemini` (+ `google`,
  `google-gemini`), `gemini-agent-platform-express`
- Ollama native `/api/chat`: `ollama`
- Embedded llama.cpp (desktop, `llama_cpp.rs`, enabled with
  `RemoteProviders::with_local_llama`): legacy `LlamaCppAdapter` and
  `build_llama_extra_fields` mapped onto the ported runtime. OpenAI-shaped
  messages (assistant tool-call turns carry null content when they have no
  text, tool results are `tool` messages), `parallel_tool_calls` whenever
  tools are offered, output cap plus reasoning budget (4096 fallback), the
  reasoning request turning on the template's reasoning format, the thinking
  switch only with `send_thinking_state`, the per-field llama settings from the
  chat profile (DFlash included: legacy's field allowlist dropped the
  `llamaDflash*` keys, so chat never enabled it), the conversation as prompt
  cache key for chat turns. Local
  failures are non-retryable `LOCAL_INFERENCE_FAILED`; metrics, the runtime
  report and UI events go to the app's `LlamaHost`. User attachments are
  inlined like legacy `build_multimodal_content` (text first, then images as
  data URLs with `detail: auto` and audio as `input_audio`) when the model's
  image/audio input is supported, read through the `ProviderMediaSource` the
  host attaches; attachments on other roles or unsupported modalities are
  dropped. The runtime needs the model's mmproj for them (legacy rule). Streaming turned off for
  the account or model runs the request unstreamed (legacy fallback). Cached
  and cache-write prompt tokens are always reported; legacy lost them on its
  non-streamed path.

Legacy aliases (`cerebras.ai`, `chutes.ai`, `nvidia-nim`, `moonshot-ai`,
`z.ai`) resolve; unknown kinds, `lettuce-host` and `lettuce-engine` are
rejected. `RemoteProviders` also exposes `list_models` (legacy
`get_remote_models`, incl. custom dotted-path parsing) and `verify_api_key`
(legacy `verify_provider_api_key` probes), and every provider file carries a
`DESCRIPTOR` (catalog metadata, key requirement, parameter/reasoning/caching
support, extra-body allowlist) served through `provider_descriptors()`.
Descriptors report native tool translation for all four remote wire families
and keep structured output false. Anthropic and Gemini now advertise signed
tool replay and reasoning-with-tools alongside reasoning-capable
OpenAI-envelope adapters; Ollama remains false for the explicit replay
limitations below.

Streaming uses bounded byte framing and provider-specific normalization for
OpenAI SSE, Anthropic Messages SSE, Gemini SSE, and Ollama NDJSON. It preserves
native reasoning plus legacy thinking tags, usage, finish reasons, safety
outcomes, request IDs, socket/channel backpressure, and cooperative
cancellation. Malformed, oversized, incomplete, or contradictory protocol
records fail closed. Provider request bodies opt into streaming only when a
stream sink is present; cancellation alone does not change the wire protocol.

Reasoning requests use explicit provider policies instead of treating
"OpenAI-compatible" as one wire contract: OpenAI-style completion allowance,
OpenRouter's nested reasoning object, Qwen/Moonshot thinking fields, zAI's
thinking state, Anthropic's budgeted thinking block, Gemini's model-family
thinking config, and Ollama HTTP's native `think` field. Buffered responses
preserve native and tagged reasoning with the same normalized outcome shape as
streaming. Custom OpenAI reasoning and embedded local runtimes remain deferred
until their wire/runtime contracts are implemented explicitly.

OpenAI-envelope providers support typed function definitions, provider-specific
tool-choice policies, assistant-call/result transcript replay, buffered calls,
and bounded fragmented SSE calls. Mistral maps required choice to `any`; custom
providers retain their configured choice mode and optional
`chat_template_kwargs`; OpenRouter cache-controls the final tool definition.
Provider adapters only translate and validate calls: durable execution and
handler dispatch remain owned by `lettuce-conversations`.

Anthropic and custom Anthropic use their native tool definitions, choices,
`tool_use`/`tool_result` transcript blocks, buffered responses, and bounded SSE
input fragments. This intentionally corrects the legacy follow-up path, which
declared Anthropic tools but replayed OpenAI-shaped calls and results that the
Anthropic adapter then discarded. Prompt caching covers the system, final tool
definition, and final user text. Extended-thinking tool rounds store one
conversation-retained native assistant-block document behind an opaque replay
reference. Buffered responses retain the exact content-array bytes; SSE
reconstructs one bounded canonical native array including thinking signatures.
Continuation validates the artifact against every call and embeds the stored
JSON unchanged. Malformed signatures, changed calls, tampered artifacts, and
unavailable replay storage fail closed.
Gemini and Gemini Express use native function declarations, AUTO/ANY/named
choices, grouped `functionCall`/`functionResponse` transcript parts, buffered
calls, and bounded SSE calls. Missing provider call IDs remain absent instead
of being fabricated. Standard Gemini explicit-cache resources own both tool
definitions and tool choice; a missing resource retry restores the clean
uncached request. Signed Gemini function-call rounds retain the exact buffered
native assistant `parts` array or the bounded canonical SSE parts sequence,
including each `thoughtSignature`. Continuation verifies ordered call
ID/name/arguments against the materialized artifact and embeds its JSON
unchanged. Standard Gemini and Agent Platform Express share this boundary;
unsigned Gemini behavior is unchanged. Ollama HTTP uses native function definitions and ordered
assistant-call/tool-result replay, parses buffered calls, and accumulates atomic
calls across NDJSON chunks without fabricating missing provider IDs. Its native
API has no tool-choice field, so only Auto is supported; Required/named choice
and reasoning-plus-tools remain rejected rather than approximated or replayed
lossily.

Media input follows legacy `build_multimodal_content`: attachments on user
messages reach the provider only when the model's image or audio input is
supported (legacy `inputScopes`), read through the host's
`ProviderMediaSource`; everything else (assistant reply images, other roles,
text-only models) is dropped and the text is sent alone, never rejected.
OpenAI-envelope providers get `image_url` data URLs with `detail: auto` and
`input_audio`; Anthropic gets base64 `image` blocks (no audio); Gemini gets
`inline_data` for images and audio (legacy `gemini_audio_mime`); Ollama gets
the native `images` array (no audio). Multimodal messages are never merged
with their neighbours, as legacy only merged string contents. Request bodies
share the 64 MiB bulk bound so phone photos fit.

Deferred horizontals: custom-provider reasoning schema and structured output.

Response parsing is as lenient as legacy (`tooling.rs`, `sse.rs`):
`tool_calls`/`reasoning`/`error` may be `null`; usage counters that are not
integers are unknown; a tool call without an id gets `tool_call_{n}` (legacy
used `tool_call` for every buffered call, which collided; corrected); a streamed
fragment without an index opens a new call; `<parameter=x>` argument strings
are parsed, a double-encoded object is unwrapped, and blank or non-object
arguments become `{}` (legacy passed the raw string to the tool, the domain
only carries objects); Anthropic `tool_use` blocks count under any
`stop_reason`; a `tool_calls` finish with no calls keeps the text. OpenAI SSE
streams may end without `[DONE]` or a final blank line, and a plain JSON body
answering a stream request is read as one record. Text, reasoning, SSE records
and the wire byte count of generation streams are not capped (legacy had no
cap). The stored reply text and reasoning are trimmed like legacy
`normalize_thinking_content`. Error bodies that are not JSON, or JSON without
`error`/`message` text (FastAPI `detail`), keep their text as the message.

Explicit prompt caching is executable for Anthropic, custom Anthropic, and
OpenRouter through typed cache-control annotations, and for OpenAI through its
typed request-retention field. Catalog descriptors expose the exact supported
retention choices. Groq and Gemini Express remain automatic and emit no
app-side cache controls. Standard Gemini creates explicit `cachedContents`
resources for the stable prefix, reuses them in process until their typed
five-minute or one-hour expiry, and sends the final content as the live turn.
Cache creation is best-effort: failures send the original clean request, and a
missing cached resource is evicted before one uncached retry. Cache names are
never persisted because the provider owns and expires those resources.

Normalized outcomes retain the provider's raw finish reason and bounded
header request ID. Non-success responses carry a typed status/category plus
bounded provider code and message; debug formatting redacts the message.

Legacy data still without a destination, owned by named later slices:

- model-parameters slice (`lettuce-models` profile config): generic extra-body
  passthrough behind `extra_body_keys`. Ollama's twelve native options and the
  OpenRouter pinned endpoint are implemented; duplicate legacy
  context/output/repetition fields map to provider-neutral settings.
- composition root (`lettuce-app`): constructing `JsonClient::with_tls` from
  the settings' trusted certificates and exposing catalog, listing and
  verification commands.

Approved corrections of legacy tables: custom Anthropic accounts no longer
advertise frequency/presence penalties (the Messages body never carried them);
Gemini uses header-only `x-goog-api-key` (legacy also copied the key into the
query string).

OpenRouter billing reads use `openrouter_endpoint_pricing` and
`openrouter_generation_details` on the existing RemoteProviders adapter. They
reuse account-owned credentials, the bounded JSON client and its 10-second probe
policy. Endpoint/model and generation response identities must match the request;
404 generation lookup returns None, while authentication and transport failures
remain typed errors. Native and normalized token counts remain separate; absent
counts never become zero. Endpoint names/tags and exact price strings are retained
without selecting a fallback provider. Required prices and monetary evidence must
be finite and nonnegative. Wire response structs remain private; usage owns the
billing result types. These reads do not add a cache or automatic cost writes.

Contracts checked against the official [endpoint pricing documentation](https://openrouter.ai/docs/api/api-reference/endpoints/list-all-endpoints-for-a-model)
and [generation metadata documentation](https://openrouter.ai/docs/api/api-reference/generations/get-request-&-usage-metadata-for-a-generation).
Local HTTP fixtures cover bearer auth, base-path/version handling, encoded query
IDs, 404 versus 401, free prices and separate native/normalized counters; parser
tests reject wrong identities and malformed required evidence.

OpenAI buffered/SSE outcomes retain the response-body `id` separately from the
HTTP `provider_request_id`. Omitted/null SSE IDs preserve the earlier identity;
changed or non-string IDs reject the stream. Other native adapters currently
leave response identity unknown. Companion dispatch evidence retains this ID
before workflow validation; other workflow checkpoints are not yet wired to it.

Reasoning wire behaviour follows each legacy adapter (2026-09-22): with
reasoning off, OpenAI-envelope providers still send `max_tokens` = cap +
budget, except Mistral (reasoning ignored) and custom OpenAI-format (budget
never added; `reasoning: {effort, max_tokens}` when on); LM Studio sends
`max_completion_tokens`, the effort and a `reasoning` object; DeepSeek,
Featherless use `max_tokens` + effort; LiteRouter, IntenseRP
`max_completion_tokens` + effort; OpenRouter sends `reasoning: {}` when on
with neither effort nor budget. Anthropic adds the budget and forces
temperature 1.0 only when thinking is actually sent (reasoning on with a
budget). Nothing rejects effort or budget with reasoning off.

Remote image generation (`RemoteImageProviders`, 2026-09-22) ports legacy
`generate_image` for every kind but sdcpp: the eleven legacy adapters
(OpenAI and its `custom`/`lettuce-host` aliases, OpenRouter, Pollinations,
Gemini with the key as `?key=`, Gemini Express with `x-goog-api-key`,
Stability, xAI, NanoGPT, LiteRouter binary responses, Automatic1111,
Diffusers) with their endpoints, fields, defaults and parsers, and ComfyUI
(upload, legacy `%TOKEN%` substitution, `/prompt`, `/history` polled every
1.5 s up to 400 times, `/view`). Requests go over `BulkHttpClient`
(release 2.2.5 behavior): reference images and mask are shrunk first
(`shrink_for_upload`); OpenRouter posts to its Image API (`{base}/v1/images`,
nearest legacy aspect ratio, `input_references`) and falls back to chat
completions on 404, or 400/422 naming the model/endpoint; an HTTP 5xx or a
transient `error` in a 2xx body (500/502/503/504/529) is retried once after
1.5 s, other body errors fail as `Provider error {code}: {message}`, and 413
gets a readable detail; error texts are legacy's. Linked results are
downloaded, data URLs and raw base64 decoded, usage found the way legacy's
`extract_usage` did. The job's cancellation token ends the request. ComfyUI
workflows are the account's `ProviderConfig::ComfyUi` (both legacy importers
now keep `txt2imgWorkflow`/`img2imgWorkflow`; they were dropped before).
Deliberate corrections: a `custom` or `lettuce-host` account without an
endpoint fails instead of sending its key to api.openai.com. Open gap:
providers' remote result URLs are not kept beside the stored bytes.

Image, audio and total token counts are read the way legacy `usage_from_value`
did (OpenAI `usage`, Gemini `usageMetadata` incl. AUDIO modality details) for
chat and image responses; Anthropic, Ollama and llama.cpp never reported them.

Ollama model store: `RemoteProviders::ollama_inventory`, `ollama_delete` and
`ollama_pull` (NDJSON progress per line, cancelled by dropping the future)
talk to an Ollama account's server with its credentials. A pull is sent once
with only an idle timeout (legacy had none; a 30-minute total limit would kill
large pulls), a delete is never retried, pull lines are split on bytes so a
character across chunks survives (legacy corrupted it), and a single progress
line is capped at 1 MiB.
