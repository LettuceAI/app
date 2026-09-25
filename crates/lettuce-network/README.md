# lettuce-network

Central HTTP clients, retries, limits, proxy/TLS, redaction, and SSRF policy.

## Boundary

Other crates cannot construct standalone HTTP clients.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

`JsonClient` is the one JSON transport for buffered POST/GET and pull-based
streaming POST requests. Request bodies and cumulative response bytes are
bounded; streamed bodies retain socket backpressure, apply an idle timeout,
and are cancelled by dropping their owner instead of a detached reader task.
Referers are disabled, credentials are per-request, and diagnostics are
redacted. System proxies (`HTTP(S)_PROXY`, `ALL_PROXY`, `NO_PROXY` and the
platform settings reqwest reads) are honored and redirects are followed, as
legacy's default reqwest client did (up to ten), but only on the host the
request went to and never from HTTPS down to HTTP, so no credential reaches
another host; a cross-host redirect is returned as the response. The buffered
and streamed response cap is 8 MiB unless the caller raises it with
`with_max_response_bytes` (remote TTS uses 256 MiB, since legacy read audio
responses without a bound). Plain HTTP is allowed for
user-configured hosts (legacy LAN endpoints). Timeouts and retries follow the
legacy transport: 10 s connect, 30 min generation total, up to two retries
with 200/400 ms backoff on 5xx, 429 (honoring `Retry-After` up to 30 s),
timeouts and connection failures; verification probes use a 10 s timeout and
no retries. TLS uses the OS trust store plus webpki roots, extra PEM roots from
`TlsPolicy` (legacy trusted certificates), and a per-request
`allow_invalid_tls` opt-in that callers must derive from an explicit account
setting. Every request carries the legacy `HTTP-Referer` and `X-Title`
attribution headers. Cookies remain outside this slice. A
validated chat path is joined beneath the endpoint base path
(`https://host/v1` plus `/chat` becomes `/v1/chat`); query, fragment, userinfo,
and encoded separators are rejected. Provider-owned non-secret query values
use a bounded typed parameter list instead of being embedded in paths; secret
query authentication remains a separate redacted credential type.

`get_json_with_query` and buffered `post_json_with_query` use the same validated
parameter list as streaming POST.
Values can borrow request-local strings; they are percent encoded by the central
client. Existing GET callers delegate with an empty list. OpenRouter's HTTP
fixture verifies a generation ID containing plus, ampersand and equals signs
cannot create another query parameter. Existing query and response bounds apply.

Buffered responses also retain a bounded `Content-Type` value. Binary-producing
adapters can pass the provider's declared media type into their domain-owned
validation boundary without constructing a separate HTTP client; response
bodies, request identifiers and retry metadata remain redacted from debug output.

`ArtifactDownloadClient` is the narrow unauthenticated Hugging Face transport
for large pinned artifacts. It takes the file URL `lettuce-model-hub` builds
(`resolve_url` / `pinned_resolve_url`), sends an optional byte range, accepts only coherent complete or
partial responses, limits redirects to five HTTPS locations, honors system
proxies as legacy's client did, applies a
per-chunk idle timeout, and leaves backpressure and cancellation with the
caller. The model-hub layer owns expected-size and digest verification.

`open_https` serves pinned HTTPS artifacts such as GitHub release assets with
the same range handling.

`BulkHttpClient` carries image requests and results: 64 MiB requests,
256 MiB responses and no retries (legacy never retried an image request, and
a retry could run a paid or long generation twice). Its request clients
follow redirects as `JsonClient` does: up to ten, on the request's host only
and never from HTTPS down to HTTP; a 307 or 308 replays the POST with its
body (reqwest resends a buffered body), while 301/302/303 turn it into a
GET. `fetch_url` keeps its own five-redirect limit. `status_text` prints a
status with its reason phrase as legacy error texts did.
