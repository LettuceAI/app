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

- `GenerationOnce` retains the generation timeout but sends a non-repeatable external operation once, including server errors, dropped responses and redirects. ElevenLabs billable voice creation selects this policy under the Slice 10 user decision; ordinary generation retry policy is unchanged.

- Redirects are followed only on the host the request went to and never from https down to http, so no credential reaches another host; a cross-host redirect is returned as the response. Legacy's default client followed any redirect.
- Streams are cancelled by dropping their owner instead of a detached reader task, and keep socket backpressure.
- Hugging Face browsing and the Sprout `/specs` read use the 30 s, no-retry `Browse` timeout, as legacy's Hugging Face client (`image_bundle.rs` 45-59) and Sprout client (`sprout.rs` 36-94 over `hf_browser/mod.rs` 1988-1994) did; they had used the 10 s probe and the 30-minute generation budget with retries.
- An artifact download fails as `TimedOut` after 120 s without a chunk, legacy's stall timeout (`hf_browser/mod.rs` 2840-2848); it had been 30 s. The wait for the response head is bounded the same way, which legacy did not bound. The artifact client connects within 30 s, legacy's download client's connect timeout (`hf_browser/mod.rs` 2742); it had been 10 s.
- Hugging Face browsing keeps the 10 s connect timeout inside its 30 s budget: the JSON client is built once with the provider transport's connect limit (legacy's provider transport used 10 s), and legacy's browse client had no connect limit of its own, only the 30 s total.

## Not wired yet

- The crate description names SSRF policy. There is no SSRF filtering beyond scheme, userinfo and path validation; plain http to any host is allowed on purpose for LAN endpoints.
- Cookies are not handled.

## History

- `get_json_with_query` and buffered `post_json_with_query` were added using the same validated parameter list as the streaming POST; existing GET callers delegate with an empty list. OpenRouter's HTTP fixture checks that a generation id containing `+`, `&` and `=` cannot create another query parameter.
- The previous README described `ArtifactDownloadClient` as an unauthenticated Hugging Face transport. It now signs Hugging Face requests (same origin only) and CivitAI requests (civitai.com hosts only) with the app-wide tokens, and also probes sizes and reads file prefixes.

Invalid stored trusted roots are skipped at runtime, preserving old-code/src-tauri/src/tls.rs:26-39; new imports remain strict. Shared client state supports immediate trust changes without restart, preserving legacy per-request certificate reads (tls.rs:4-24). Bundles trust every certificate rather than only one.

Stored certificate views include validity and a typed InvalidPem reason; runtime client construction skips invalid stored roots so users can list and remove them. New imports validate strictly before writing.
