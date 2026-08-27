# lettuce-providers

Remote provider catalogs, request translation, stream parsing, and error mapping.

## Boundary

Provider-native DTOs remain private adapters.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Every remote chat provider the legacy app shipped is executable for text-only,
non-streaming generation, one file per provider on a family wire trait with
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

Deferred horizontals: streaming, tools, media input, reasoning fields
(`enable_thinking`, `reasoning_effort`, Gemini thinking config, Ollama `think`,
`chat_template_kwargs`), prompt-cache controls, and structured output.

Normalized outcomes retain the provider's raw finish reason and bounded
header request ID. Non-success responses carry a typed status/category plus
bounded provider code and message; debug formatting redacts the message.

Legacy data still without a destination, owned by named later slices:

- model-parameters slice (`lettuce-models` profile config): the OpenRouter
  pinned endpoint (`provider: {order, allow_fallbacks}`) plus the generic
  extra-body passthrough behind `extra_body_keys`. Ollama's twelve native
  options are implemented; its duplicate legacy context/output/repetition
  fields map to the provider-neutral settings.
- composition root (`lettuce-app`): constructing `JsonClient::with_tls` from
  the settings' trusted certificates and exposing catalog, listing and
  verification commands.

Approved corrections of legacy tables: custom Anthropic accounts no longer
advertise frequency/presence penalties (the Messages body never carried them);
Gemini uses header-only `x-goog-api-key` (legacy also copied the key into the
query string).
