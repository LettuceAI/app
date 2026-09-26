# lettuce-observability

The one place in the workspace that configures logging: it installs the process-wide `tracing` subscriber, writes the daily log files the in-app log viewer reads, sanitizes what goes into them, serves the viewer's reads, and writes panic reports.

Other crates only emit `tracing` spans and events. They never install a subscriber, create an exporter, log through Tauri or add a logging facade of their own. Progress the user sees travels through typed job and application events, never through log parsing. User content and secrets are kept out of logs by default.

## Structure

- `config.rs`: `ObservabilityConfig` holds the `EnvFilter` directives (invalid directives fall back to `info`), the stderr format (`Compact` or `Pretty`) and an optional `LocalOutputConfig` with the log directory and queue capacity (default 1024, at most 1 000 000). Validation only checks what it was given; the crate never discovers or creates a log directory on its own.
- `init.rs`: `install` builds the subscriber (filter, stderr layer, and the file layer when local output is configured) and sets it as the global default once. It returns a `LocalOutput` with the `WorkerGuard` that flushes queued lines on drop and a `LogSink`.
- `line_layer.rs`: `LineLayer` turns each event into one log line.
- `log_files.rs`: the line format (`LogEntry`), `sanitize_message`, `DailyLogWriter` and `LogDirectory`, the viewer's reads.
- `correlation.rs`: `CorrelationContext` carries an `OperationId` and optional request, job, conversation and generation-turn ids, and opens a `lettuce.operation` span with them. Correlation is ids only, never prompts or payloads. The field names are exported as constants so every crate records them the same way.
- `redaction.rs`: `Sensitive<T>` and `UserContent<T>` print `[REDACTED]` for both `Debug` and `Display`, also when nested inside another formatted error. The value is reachable only through `as_inner` or `into_inner`. Wrapping content does not make it loggable; it makes accidental formatting safe.
- `panic.rs`: `install_panic_reports`.

## Log files

The composition root passes the host's log directory. Records go to `app-YYYY-MM-DD.log`, one file per local day, one line per record:

```
[<rfc3339 local time>] LEVEL component at=file:line:1 | message name=value [span=outer > inner]
```

The viewer's parser depends on this layout. `LineLayer` builds each line like this:

1. TRACE events are skipped; the viewer only knows the four levels from DEBUG up.
2. The component is the event's `component` field, or its target. The message is the `message` or `msg` field.
3. `at=` is written only when the source path is relative and has no whitespace, since the viewer matches `at=\S+`.
4. The message goes through `sanitize_message`.
5. Every other field is appended as ` name=value`, each value sanitized on its own. Values of `key`, `api_key`, `apikey`, `access_token`, `token`, `authorization` and `x-api-key` are written as `***`.
6. The span path from the root is appended as `[span=a > b]`.

`sanitize_message` escapes newlines, replaces everything after a `request body:` or `response body:` marker (and two similar markers) with `<redacted body len=N>` giving the real length, masks the token after `bearer ` and the value of every `name=` parameter in the secret list, and cuts messages over 1200 characters, except for the `api_request`, `image_generator`, `llama_cpp` and `dynamic_memory` components. For `api_request` it shortens `full_url=` to `url=`. Matching is case-insensitive on ASCII without lowercasing the message, so byte offsets stay correct for non-ASCII text.

`DailyLogWriter` sits behind `tracing-appender`'s non-blocking writer, bounded to the configured capacity and lossy: when the queue is full, lines are dropped rather than blocking the caller. It reopens the file when the local date changes or when the current file was deleted.

`LogSink` appends records that do not come from `tracing`, such as the frontend's, to the same files through the same queue. They are written as given, without sanitizing, and a full queue drops them silently.

Files are kept until the user deletes them; there is no automatic retention.

## Log viewer

`LogDirectory` serves the viewer:

- `list` returns the `.log` files, newest name first.
- `read` returns a whole file; `page` returns an offset and limit of lines plus the total line count.
- `search` matches plain text, case-insensitively, as a whole word, or as a regex; a bad pattern fails with `Invalid search pattern: ...`.
- `relevant_lines` finds lines related to a reference line: any line containing one of its UUIDs, or a line from the same component that is within 30 seconds of it or shares at least two of its words of five or more letters (or more than 40% of them).
- `delete` removes one file and `clear` removes every `.log` file.

File names must be a single name inside the directory; anything else is not found.

## Panic reports

`install_panic_reports(directory)` chains a panic hook in front of the existing one. For each panic it writes a report (local time, thread, location, payload, a forced backtrace) to its own `panic-<local time>-p<pid>-<thread>-<counter>.log`, prints it to stderr, and logs where the file went. The host installs it after `install`, so the log line reaches the daily file.
