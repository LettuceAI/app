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

Deferred horizontals: media input, custom-provider reasoning schema, and
structured output.

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
