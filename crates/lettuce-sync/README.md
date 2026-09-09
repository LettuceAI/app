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
while media bytes remain content-addressed elsewhere. Stable create, revise and
media-mutation operation identities let the SQLite persona repository append
the insert or update in the same transaction as its aggregate mutation. Media
operation identities bind the normalized mutation kind and intent. The update
base is the canonical pre-mutation snapshot hash.

Persona media replacement, attachment, detachment and reordering now journal
the complete resulting snapshot atomically and replay exact retries without a
second sequence. The revisioned application-default persona singleton has its
own versioned canonical state, entity identity and intent-bound set/clear
operations. Default selection changes share their SQLite transaction with one
canonical update and retain the prior singleton snapshot hash as the base.
Persona archive and restore use distinct lifecycle operation identities and the
same complete snapshot update contract. Archiving the selected default also
emits a distinct default-clear update in the same transaction, so both
revisions and both journal rows either commit together or remain unchanged.
Every other aggregate still needs explicit journal wiring. Peer negotiation,
staged apply, conflict resolution, blob exchange and legacy sync-state migration
also remain later slices. No legacy database, source or user asset is read,
rewritten or deleted by this crate.
