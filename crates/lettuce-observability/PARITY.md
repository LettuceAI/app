# lettuce-observability: legacy parity notes

Facts about how `lettuce-observability` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- The file layout is legacy's because its log viewer parses it: `app-YYYY-MM-DD.log` per local day in the host's log directory and the `[time] LEVEL component at=file:line:1 | message` line.
- Legacy never wrote TRACE, and its viewer only styles and filters the four levels from DEBUG up, so `LineLayer` skips TRACE.
- `sanitize_message` is legacy's sanitizer: newlines escaped, request and response bodies replaced by their length, bearer tokens and `key=`/`token=`-style parameters masked, messages over 1200 characters cut except for the `api_request`, `image_generator`, `llama_cpp` and `dynamic_memory` components, `full_url=` shortened for `api_request`.
- `LogSink` appends frontend records unsanitized, as legacy's `log_to_file` did.
- The viewer operations (`list`, `read`, `page`, `search`, `relevant_lines`, `delete`, `clear`) keep legacy's behavior and error texts, including `Invalid search pattern: ...`.
- Retention is legacy's: files stay until the user deletes them.
- Panic reports follow legacy's bootstrap: one file per panic in the log directory, and a log line saying where it went.

## Deliberate differences from legacy

- Legacy had no structured fields. Event fields other than the message are appended as ` name=value` (secret names masked) so they are not lost.
- Legacy lowercased the message before searching it, which shifted byte offsets on non-ASCII text. The sanitizer now searches case-insensitively without lowercasing.
- Legacy sanitized twice and always reported the placeholder's length in `len=`. `len=` is now the real body length.
- Frontend records are acknowledged synchronously, preserving the write failure returned by `old-code/src-tauri/src/infra/logger.rs:202`. Backend records keep the bounded lossy queue, so a full queue drops lines instead of blocking the code that logs.
- Legacy accepted file names containing `..` and could escape the log directory. Names must now be a single name inside it.

## Not wired yet

- The crate description also names health diagnostics, crash context and support bundles. None exist; the first slice deliberately left out support bundles, exporters, crash upload and any logging facade.
- Log exports use a host FileTarget in place of Downloads (`old-code/src-tauri/src/infra/logger.rs:815`). The persisted Android export folder and crash monitor remain deferred to the Android shell; no heartbeat timer is introduced (`old-code/src-tauri/src/platform/android_monitor.rs:285`).

## History

- Panic reports were added on 2026-09-23.
- The crate docs refer to a crate `PLAN.md`; there is none in the crate directory.

## Slice 12 logging

The shell installs local output and the panic hook before opening the API, retaining the guard until shutdown; legacy initialized its manager and chained a hook at `old-code/src-tauri/src/app/bootstrap.rs:99`. Daily rotation and reopening after explicit deletion retain `old-code/src-tauri/src/infra/logger.rs:196`.

The API exposes list, paged read, search, relevant lines, delete, clear, export and frontend append. Pages clamp the requested size instead of limiting the file; legacy accepted an arbitrary page limit (`old-code/src-tauri/src/infra/logger.rs:309`). The existing search and relevant-line algorithms remain unchanged (`old-code/src-tauri/src/infra/logger.rs:351`, `old-code/src-tauri/src/infra/logger.rs:402`).

Symlink reads are refused in addition to single-component names. List and clear surface directory errors; legacy flattened them (`old-code/src-tauri/src/infra/logger.rs:547`). Frontend timestamps must be RFC3339, labels cannot contain whitespace, and message newlines are escaped so one append remains one record; legacy accepted raw fields (`old-code/src-tauri/src/infra/logger.rs:640`).

Written lines are mirrored through a typed application event only while developer mode is enabled. Legacy emitted `chat://debug` from its tracing bridge (`old-code/src-tauri/src/infra/utils.rs:373`) and listened in `old-code/src/App.tsx:394`; the new mirror follows written lines and the committed settings feed. Failed mirror delivery writes to stderr to avoid recursively mirroring its own failure. The mirror receives lines on its own thread through a bounded queue and skips lines while that queue is full; legacy emitted from the tracing bridge on the logging thread (`old-code/src-tauri/src/infra/utils.rs:373`).

Export refuses the source file as its own target, including hard links and platform URI aliases, by comparing open file identities before truncating the target. Legacy read the whole content before its Downloads write (`old-code/src-tauri/src/infra/logger.rs:828`); streamed exports need this guard to preserve the source.
