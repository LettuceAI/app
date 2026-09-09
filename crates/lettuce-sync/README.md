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

The local journal port admits a validated change request under a stable
operation ID and returns the allocated immutable change. Exact retries replay;
changed operation reuse conflicts. The SQLite adapter owns device identity,
monotonic origin sequence and hybrid-clock allocation in the same transaction
as the journal row.

The first entity codec is the complete validated persona snapshot. It includes
authored fields, lifecycle, revision/timestamps and ordered media references,
while media bytes remain content-addressed elsewhere. Stable create and revise
operation identities let the SQLite persona repository append the insert or
update in the same transaction as its aggregate mutation. The update base is
the canonical pre-mutation snapshot hash.

Persona media-only mutations, default selection, lifecycle changes and every
other aggregate still need explicit journal wiring. Peer negotiation, staged
apply, conflict resolution, blob exchange and legacy sync-state migration also
remain later slices. No legacy database, source or user asset is read, rewritten
or deleted by this crate.
