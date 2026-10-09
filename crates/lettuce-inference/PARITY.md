# lettuce-inference: legacy parity notes

Facts about how `lettuce-inference` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- `content_filter` is legacy's content filter engine: normalization, leet and homoglyph folding, dictionary scoring with the allowlist context, the 500-byte stream window and the redacted 200-entry hit log. The dictionary is legacy's.
- The blocked answer fails with legacy's `CONTENT_BLOCKED` code and text.
- Checking both a stream window and the whole text mirrors legacy, whose whole-text and windowed scores could also differ.

## Deliberate differences from legacy

- Legacy's filter only knew English. The per-language lexicons are new.
- The LDNOOBW lexicons are revision `5faf2ba`, CC BY 4.0, (c) 2012-2020 Shutterstock, Inc.

## Not wired yet

- The crate description names generation admission, attempts, tools, approvals and terminal outcomes. Admission and attempts live in `lettuce-conversations`, outcome assembly in `lettuce-providers`; tools and safety beyond Pure mode were listed as later horizontal slices.

The in-memory debug log retains the newest 200 records and drops the oldest, preserving the legacy ring (`old-code/src-tauri/src/content_filter/mod.rs:113,146-167`). The 200-character redacted snippet remains. Legacy's development-build SecurityPage polled the log every five seconds (`old-code/src/ui/pages/settings/SecurityPage.tsx:68,101-110`); the host now gates it with developer mode and follows notifications. Filtering still works while hit recording is off. Turning logging off clears its volatile records atomically with disabling recording.

Commit generation ordering prevents a delayed request from restoring an older logging choice after a settings update; recording and disabling serialize on the same mutex.
