# lettuce-observability

The workspace's single Rust observability layer: `tracing` subscriber configuration, redaction, rotating local output, health diagnostics, crash context, and support bundles.

## Boundary

User content and secrets are excluded by default.

Other crates emit structured `tracing` spans and events. They do not install subscribers, create telemetry exporters, depend on Tauri for logging, or introduce a parallel logging facade. User-visible progress travels through typed job/application events rather than log parsing.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first foundation slice provides typed correlation, unconditional redaction
wrappers, safe `tracing_subscriber` installation, and optional daily local
output.

Log files keep the old app's layout because its log viewer parses them: one
`app-YYYY-MM-DD.log` per local day in the host's log directory, one
`[rfc3339 local time] LEVEL component at=file:line:1 | message [span=a > b]`
line per record. `LineLayer` writes every DEBUG-or-higher event that way (the
old app never wrote TRACE, and its viewer only styles and filters the four
levels): the component is the event's `component` field or its target, the
message is the `message` or `msg` field, `at=` is written only for relative
source paths without whitespace (the viewer's `at=\S+` pattern), and fields
other than the message are appended after the sanitized message as
` name=value`, each value sanitized on its own and the values of
`key`/`api_key`/`apikey`/`access_token`/`token`/`authorization`/`x-api-key`
fields replaced by `***` (the old app had no structured fields, so they would
otherwise be lost). The message passes the old sanitizer (newlines
escaped, request/response bodies replaced by their length, bearer tokens and
`key=`/`token=`-style parameters masked, messages over 1200 characters cut
except for the old `api_request`/`image_generator`/`llama_cpp`/
`dynamic_memory` components, `full_url=` shortened for `api_request`). The
sanitizer searches case-insensitively without lowercasing the message, which
corrects the old byte-offset mismatch on non-ASCII text; `len=` reports the
real body length, where the old app sanitized twice and always reported the
placeholder's length. `DailyLogWriter` sits
behind the bounded lossy non-blocking queue and reopens the day's file when
the date changes or the file was deleted; `LogSink` appends records that do
not come from `tracing` (the frontend's) unsanitized to the same files, as
the old `log_to_file` did; they share the bounded lossy queue, so a full queue
drops them silently where the old synchronous write returned an error the
frontend only warned about. `install` returns both as `LocalOutput`.

`LogDirectory` is the log viewer: `list` (`.log` files, newest name first),
`read`, `page` (offset/limit plus the line count), `search` (plain,
case-insensitive, whole word or regex; "Invalid search pattern: ..."),
`relevant_lines` (lines sharing a UUID with the reference line, or from its
component within 30 seconds or sharing at least two of its words of five or
more letters, or more than 40% of them), `delete` and `clear` (every `.log`
file), with the old error texts. File names must be a single name inside the
directory, correcting the old `..` escape. Retention stays as the old app had
it: files are kept until the user deletes them. Saving a log to Downloads and
the Android export folder are host work for (c).

Panic reports (2026-09-23): `install_panic_reports(directory)` chains a panic
hook that writes each panic's report (time, thread, location, payload, forced
backtrace) to its own `panic-{local time}-p{pid}-{thread}-{counter}.log` in the
host's log directory and logs where it went, as legacy's bootstrap did; the
host installs it after `install`.
