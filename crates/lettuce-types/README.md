# lettuce-types

The small vocabulary every other LettuceAI crate shares: typed ids, millisecond timestamps, revisions, content hashes, page requests and a stable error shape.

The crate has no internal LettuceAI dependencies and does no I/O. It depends only on `serde`, `thiserror` and `uuid`. Business rules belong in the domain crates, and infrastructure is reached through ports that the calling crate owns, so nothing here knows about SQLite, the filesystem or the network. Compatibility code for old data lives at the application or migration boundary, not in these types.

## Ids

`id.rs` defines one newtype per identity with the `define_id!` macro: `CharacterId`, `ConversationId`, `MessageId`, `MemorySpaceId`, `ProviderAccountId`, `JobId`, `OperationId` and about fifty more. Each wraps a `Uuid`, serializes transparently as the UUID string, parses from text, converts to and from `Uuid`, and `new()` (also `Default`) generates a random v4.

Separate types keep a message id from being passed where a conversation id is expected, at no runtime cost. Every stored record is addressed by a UUID-backed id rather than a SQLite row id, so ids survive export, import and sync unchanged. The ASR learning records (vocabulary terms, corrections, ignored suggestions, voice examples) follow the same rule so that learning data never exposes row identities.

Deterministic ids (UUIDv5 of some parent identity) are built by the crates that need them and wrapped with `from_uuid`.

## Time and revisions

`TimestampMillis` is a signed count of milliseconds since the Unix epoch. `now()` and the `SystemTime` conversion work on both sides of the epoch and fail with `TimeError::OutOfRange` instead of wrapping.

`Revision` is a `u64` starting at `Revision::INITIAL` (1). `next()` is checked and returns `RevisionOverflow` rather than wrapping. Repositories across the workspace use it as the compare-and-swap token: a mutation names the revision it expects and the adapter bumps it once.

## Hashes

`ContentHash` is a BLAKE3 digest as 64 hexadecimal characters. `parse` checks the length and characters and lowercases the value, so two hashes of the same bytes always compare equal. The crate does not compute hashes; callers hash with `blake3` and parse the hex.

## Paging

`PageRequest` is an opaque cursor plus a `PageLimit`. The limit defaults to 50, and `PageLimit::new` maps 0 to the default and clamps anything above 200, so no query can ask for an unbounded page. `Page<T>` returns the items and the next cursor; `Page::last` is a page with no cursor. What the cursor encodes is up to each adapter.

## Errors

`AppError` is a serializable error with an `ErrorCode` (`Cancelled`, `Conflict`, `CorruptData`, `Forbidden`, `Internal`, `InvalidInput`, `NotFound`, `NotReady`, `ResourceBusy`, `StorageUnavailable`, `Unsupported`), a message, a `retryable` flag and a string map of safe details. It is meant for errors that cross into the UI. The domain crates currently define their own typed error enums and do not construct `AppError`.

## Adding to the crate

Keep the public surface small. A new type belongs here only when several crates need the same identity or value; anything with business meaning belongs in its domain crate. New behavior comes with tests.
