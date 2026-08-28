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
Redirects/referers/proxies are disabled, credentials are per-request, and
diagnostics are redacted. Plain HTTP is allowed for
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
