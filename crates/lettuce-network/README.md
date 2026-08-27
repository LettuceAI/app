# lettuce-network

Central HTTP clients, retries, limits, proxy/TLS, redaction, and SSRF policy.

## Boundary

Other crates cannot construct standalone HTTP clients.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

`JsonClient` is the one buffered JSON transport (POST and GET) with bounded
request/response bodies, disabled redirects/referers/proxies, per-request
credentials, and redacted diagnostics. Plain HTTP is allowed for
user-configured hosts (legacy LAN endpoints). Timeouts and retries follow the
legacy transport: 10 s connect, 30 min generation total, up to two retries
with 200/400 ms backoff on 5xx, 429 (honoring `Retry-After` up to 30 s),
timeouts and connection failures; verification probes use a 10 s timeout and
no retries. TLS uses the OS trust store plus webpki roots, extra PEM roots from
`TlsPolicy` (legacy trusted certificates), and a per-request
`allow_invalid_tls` opt-in that callers must derive from an explicit account
setting. Every request carries the legacy `HTTP-Referer` and `X-Title`
attribution headers. Streaming and cookies remain outside this slice. A
validated chat path is joined beneath the endpoint base path
(`https://host/v1` plus `/chat` becomes `/v1/chat`); query, fragment, userinfo,
and encoded separators are rejected.
