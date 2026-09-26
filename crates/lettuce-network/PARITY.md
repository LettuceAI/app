# lettuce-network: legacy parity notes

Facts about how `lettuce-network` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- Timeouts and retries follow legacy's transport: 10 s connect, 30 minutes for a generation, up to two retries with 200/400 ms backoff on 5xx, 429 (honoring `Retry-After` up to 30 s), timeouts and connection failures. Verification probes use a 10 s timeout and no retries.
- Streamed bodies are not size-capped, as legacy's were not.
- System proxies are honored and redirects followed up to ten times, as legacy's default `reqwest` client did.
- Plain http is allowed for user-configured hosts, as legacy allowed LAN endpoints.
- Extra PEM roots come from legacy's trusted certificates (`appState.trustedCertificates`).
- Every provider request carries legacy's `HTTP-Referer` and `X-Title` attribution headers.
- Remote TTS and chat inference raise the buffered response cap to 256 MiB because legacy read audio and non-streamed image replies without a bound.
- `BulkHttpClient` never retries, as legacy never retried an image request.
- `status_text` prints a status with its reason phrase as legacy's error texts did.
- The artifact client honors system proxies as legacy's client did.

## Deliberate differences from legacy

- Redirects are followed only on the host the request went to and never from https down to http, so no credential reaches another host; a cross-host redirect is returned as the response. Legacy's default client followed any redirect.
- Streams are cancelled by dropping their owner instead of a detached reader task, and keep socket backpressure.

## Not wired yet

- The crate description names SSRF policy. There is no SSRF filtering beyond scheme, userinfo and path validation; plain http to any host is allowed on purpose for LAN endpoints.
- Cookies are not handled.

## History

- `get_json_with_query` and buffered `post_json_with_query` were added using the same validated parameter list as the streaming POST; existing GET callers delegate with an empty list. OpenRouter's HTTP fixture checks that a generation id containing `+`, `&` and `=` cannot create another query parameter.
- The previous README described `ArtifactDownloadClient` as an unauthenticated Hugging Face transport. It now signs Hugging Face requests (same origin only) and CivitAI requests (civitai.com hosts only) with the app-wide tokens, and also probes sizes and reads file prefixes.
