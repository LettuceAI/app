# lettuce-media

Content-addressed user media, validation, derivatives, serving, retention, and repair.

## Boundary

Source documents use the existing content-addressed store and descriptor-backed
`open_ready` path with `AssetKind::SourceDocument` / `MediaKind::Document`.
PDF signatures (1.0–1.7 and 2.0) and UTF-8 text are accepted; Markdown shares
canonical `text/plain` metadata so identical bytes deduplicate consistently.
Invalid UTF-8 is not accepted as text, matching legacy text intake. PDF header
recognition is not structural validation, extraction or sanitization; a later
extractor must reject malformed/encrypted documents as appropriate. Nothing is
rendered or executed during ingestion. Existing size bounds and retention
policies apply; feature-specific legacy source limits remain at intake.
Lettuce-app can read source documents for legacy PDF/text extraction; creation
project-owned source associations are not yet wired.

Exposes asset IDs, never managed native paths.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Foundation contract

The first slice exposes a `MediaBlobRepository` for validated,
content-addressed blob metadata. Registering the same BLAKE3 hash returns the
existing blob. It does not expose deletion or native paths.

A blob is not a logical user-facing asset. The logical slice now exposes
validated `MediaAsset` records with distinct `AssetId` and `MediaBlobId`
identities, provenance format `AssetProvenanceV1`, origin, and
persistent/library/temporary retention. Multiple assets may share one
blob without merging their provenance or retention policy.

`MediaAssetRepository` is a synchronous port with create/read, retention CAS,
and paged library listing. It deliberately has no
physical-delete method. `AssetReferenceReader` and `AssetRetentionReader`
provide typed reachability ports for future character, context, conversation,
and message association adapters; no generic owner-kind strings are exposed.

The domain validates nonzero revisions, closed/versioned serialized shapes,
asset/blob kind compatibility, bounded provenance strings, and redacted
HTTP(S)-only source locators without credentials, queries, native paths,
bytes, prompts, or secrets. Ingestion, decoding, native paths, filesystem
access, derivatives, serving and garbage collection belong to later
adapter/use-case slices.

Missing, quarantined, or corrupt bytes are physical blob/location state, not
logical asset state. Keeping that distinction is required because several
assets can share one blob and one synced asset can have different availability
on different devices.
