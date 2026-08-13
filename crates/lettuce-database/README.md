# lettuce-database

Epoch-2 SQLite schema, migrations, maintenance, and repository adapters.

## Boundary

The only crate allowed to depend on SQLite libraries.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Foundation schema

Migration 1 creates only migration bookkeeping plus four usable roots:
global settings, provider accounts, model profiles, and content-addressed media
blobs. Migration 2 adds the logical media-asset catalog, retaining the blob
catalog as the physical metadata boundary. Later feature tables arrive with
their owning vertical slices.

Migration 3 adds authored character graphs and starter-owned content. Migration
4 adds personas and their media/default associations. Migration 5 adds
reusable group profiles, ordered members, typed image associations, and the
optional complete group-owned starting scene graph. Migration 6 adds the
provider-neutral prompt/lorebook documents, ordered entries, and typed
character/persona/group lorebook bindings. Prompt/lorebook JSON is strictly
versioned and aggregate revisions use CAS; bindings follow the latest
lorebook revision while archived references remain readable.

Sessions, conversation assembly/resolution, starter-link normalization,
import/export, hard purge, sync/backup, FTS, and legacy text-column retrofits
remain outside this slice. Existing prompt/lorebook text columns are not
silently migrated or overwritten.

`Database` owns a serialized `rusqlite` connection, enables foreign keys and a
bounded busy timeout, and uses WAL for persistent files. Repository traits stay
in their domain crates; this crate contains their SQLite implementations. The
application is responsible for running these synchronous operations on its
database worker rather than a UI or async-runtime thread.

Secrets are never stored here. Provider rows contain opaque `SecretRef` values
only. Blob registration validates SQLite-representable metadata and preserves
the first immutable metadata record for a content hash; physical `BlobState`
remains separate operational state and is not changed by deduplication.
Logical assets store only versioned, redacted provenance and retention; asset
mutations use revision CAS. Library pagination uses an opaque
`(updated_at, id)` keyset cursor. It is deliberately non-snapshot pagination:
rows added or updated between page requests may move relative to a prior page.
