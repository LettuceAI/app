# lettuce-sync

Versioned domain changes, peer sessions, conflict policy, and blob negotiation.

## Boundary

Replication never exposes or copies raw SQLite state.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The canonical change boundary is active. It defines immutable change and device
identities, hybrid timestamps, bounded causal frontiers, entity identities and
distinct insert, update and delete contracts. Versioned canonical payload bytes
carry their own content hash, and a deterministic change fingerprint binds all
causal, entity, operation, revision and payload facts.

This deliberately replaces legacy raw SQLite changesets and schema-position
coupling at the domain boundary. Insert requires a payload without a base
revision, update requires both and delete requires a base revision without a
payload. Payloads retain the legacy eight-megabyte single-revision ceiling while
identifiers and causal frontiers now have explicit bounds.

Journal persistence, entity-specific codecs, peer negotiation, staged apply,
conflict resolution, blob exchange and legacy sync-state migration remain later
slices. No current database, legacy source or user asset is read, rewritten or
deleted by this crate.
