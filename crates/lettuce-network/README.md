# lettuce-network

The workspace's HTTP layer. It is the only crate allowed to depend on `reqwest` (`architecture.toml`), so every request the app makes goes through one of its three clients, each with fixed limits, timeouts, redirect rules and credential handling:

- `JsonClient` for provider API calls: buffered JSON GET, POST and DELETE, and streamed POST.
- `BulkHttpClient` for image requests and results, which carry encoded images.
- `ArtifactDownloadClient` for large model and runtime downloads with byte ranges.

The crate knows nothing about providers. Callers pass an endpoint, a path, headers and a `JsonAuth`, and get back status, body and a few bounded headers; classifying the response is the provider adapter's job. `lettuce-providers`, `lettuce-speech`, `lettuce-image-generation` and `lettuce-app` use it. Everything is in `src/lib.rs`.

## Requests

A request is built from pieces the crate validates itself:

- URL. `build_url(endpoint, path)` joins the path beneath the endpoint's base path, so `https://host/v1` plus `/chat` is `/v1/chat`. The endpoint must be http or https with a host and no userinfo, query or fragment. The path must start with `/`, have no `//`, `.` or `..` segments, backslashes or encoded separators (`%2f`, `%5c`, `%2e`). The path may carry one fixed query for custom endpoints (`/chat/completions?api-version=...`). Plain http is allowed because users point the app at LAN servers.
- Query values. Non-secret provider parameters go in a typed `JsonQueryParameter` list (at most 16, names up to 128 bytes, values up to 256, no duplicates) and are percent-encoded by the client, so a value containing `&` or `=` cannot create another parameter. They are never embedded in the path.
- Credentials. `JsonAuth` is `Bearer`, a named `Header`, a `Query` parameter, or `None`, always holding a `SecretValue`. Extra account headers come as `JsonSecretHeader`s. Credentials are applied to one request, marked sensitive, and never installed as client defaults. `JsonAuth` prints as redacted.
- Headers. Static provider headers are `JsonStaticHeader`s. Precedence: the account's own headers override the auth header, which overrides the provider's static headers; a later header of the same name replaces an earlier one. Transport headers the client owns (`host`, `content-length`, `content-type`, `transfer-encoding`, `connection` and the like) are never taken from an account.

Provider requests (`JsonClient` and `BulkHttpClient` except `fetch_url`) carry the `HTTP-Referer: https://github.com/LettuceAI/` and `X-Title: LettuceAI` attribution headers, and the automatic `Referer` is disabled. System proxy settings (`HTTP(S)_PROXY`, `ALL_PROXY`, `NO_PROXY` and the platform settings `reqwest` reads) apply to all clients. Cookies are not handled.

## JsonClient

`RequestPolicy` picks the timeout and TLS mode per request:

| `RequestTimeout` | Total | Retries |
| --- | --- | --- |
| `Generation` | 30 min | 2 |
| `Probe` (key verification) | 10 s | 0 |
| `Transfer` (e.g. a model pull) | 7 days, idle timeout 30 min | 0 |

The connect timeout is always 10 s. `delete_json` never retries. Otherwise a retry happens on a 5xx, on a 429 (after `Retry-After` when it is at most 30 s; a longer `Retry-After` returns the response instead), on a timeout, and on a connection failure, with 200 ms then 400 ms backoff. A streamed POST retries only before any body bytes have reached the caller.

Buffered responses (`get_json`, `post_json`, `delete_json` and their `_with_query` forms) return a `JsonResponse` with the status, the body, and the bounded `x-request-id`/`request-id`, `Retry-After` and `Content-Type` values. Error statuses are returned as responses, so adapters can classify them with the body in hand. Request bodies are capped at `MAX_REQUEST_BYTES` (64 MiB, since chat bodies inline base64 attachments). Response bodies are capped at 8 MiB unless the client is built `with_max_response_bytes`. Remote TTS and chat inference raise it to 256 MiB, since buffered replies can carry audio or images (the chat cap is `MAX_PROVIDER_RESPONSE_BYTES` in `lettuce-app`, asserted to stay at or below the media blob limit so any reply image can be stored). `Content-Type` lets binary-producing adapters pass the declared media type into their own validation.

`post_json_stream` returns a `JsonResponseStream` that the caller pulls one chunk at a time with `next_chunk`. There is no reader task or channel in between, so backpressure reaches the socket, and dropping the stream cancels the request. Each chunk waits at most the idle timeout. The total streamed size is not capped; `read_error_body` buffers a non-success body under the buffered cap.

Redirects are followed up to ten times, but only to the host the request went to and never from https down to http, so a credential never reaches another host. A cross-host redirect comes back as the response.

TLS uses the OS trust store plus the webpki roots and the user's extra roots from `TlsPolicy` (the trusted certificates in device settings). The rustls backend parses roots only while building a client, so each root is first tried alone and skipped with a warning if it fails, instead of one bad certificate breaking every client. `JsonClient` holds a strict and an insecure `reqwest` client; `allow_invalid_tls` in the policy selects the insecure one and must come from an explicit per-account setting, never a provider default.

`Debug` output of clients, responses and streams redacts bodies, request ids and retry metadata.

## BulkHttpClient

Image generation requests and results: `get`, `post_json` and `post_multipart` (`MultipartField::Text` and `File`), with 64 MiB requests and 256 MiB responses and a 30-minute timeout. It never retries, because a retry could run a paid or long generation twice; callers decide when to send again. Its redirect rule is the same as `JsonClient`'s: a 307 or 308 replays the POST with its body, while 301, 302 and 303 turn it into a GET. `fetch_url` downloads an image a provider linked to, http or https only, with its own limit of five redirects. `status_text` formats a status with its reason phrase for error messages.

## ArtifactDownloadClient

Downloads of large pinned files. It follows at most five redirects and only to https, and a stream never reads past the expected size.

- `open_hugging_face(url, offset, expected_size)` and `open_https(...)` send an optional `Range` from `offset`, accept `200` (from zero) or a `206` whose `Content-Range` matches the expected total, and return an `ArtifactDownloadStream` whose `start()` says where the bytes begin. Chunks wait at most 30 s. Backpressure and cancellation stay with the caller, who also verifies size and digest.
- `with_hugging_face_token(token, endpoint)` signs Hugging Face requests in for gated repositories, but only for URLs with the endpoint's origin. `with_civitai_token` signs requests to `civitai.com` hosts only; neither token follows a redirect to another host.
- `probe_https_size` learns a file's size from a one-byte ranged request. `read_hugging_face_prefix` reads the first bytes of a file, for example a GGUF header.

The file URLs come from `lettuce-model-hub`'s shared Hugging Face client (`resolve_url`, `pinned_resolve_url`).
