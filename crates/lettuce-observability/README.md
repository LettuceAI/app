# lettuce-observability

The workspace's single Rust observability layer: `tracing` subscriber configuration, redaction, rotating local output, health diagnostics, crash context, and support bundles.

## Boundary

User content and secrets are excluded by default.

Other crates emit structured `tracing` spans and events. They do not install subscribers, create telemetry exporters, depend on Tauri for logging, or introduce a parallel logging facade. User-visible progress travels through typed job/application events rather than log parsing.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first foundation slice provides typed correlation, unconditional redaction
wrappers, safe `tracing_subscriber` installation, and optional daily rolling
local output. The local writer has a bounded non-blocking queue and writes
without ANSI escape sequences.

Daily rotation is enabled, but retention and cleanup are intentionally not
implemented yet. They belong to a later filesystem-coordinated slice; callers
must not treat the current writer as a bounded retention policy.
