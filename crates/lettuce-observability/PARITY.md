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
- Legacy wrote frontend records synchronously and returned an error the frontend only warned about. They now share the bounded lossy queue, so a full queue drops them silently.
- Legacy accepted file names containing `..` and could escape the log directory. Names must now be a single name inside it.

## Not wired yet

- The crate description also names health diagnostics, crash context and support bundles. None exist; the first slice deliberately left out support bundles, exporters, crash upload and any logging facade.
- Saving a log to Downloads and the Android export folder is host work, deferred with the frontend APIs and Tauri commands.

## History

- Panic reports were added on 2026-09-23.
- The crate docs refer to a crate `PLAN.md`; there is none in the crate directory.
