# lettuce-media

Content-addressed user media, validation, derivatives, serving, retention, and repair.

## Boundary

Exposes asset IDs, never managed native paths.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Foundation contract

The first slice exposes a `MediaBlobRepository` for validated,
content-addressed blob metadata. Registering the same BLAKE3 hash returns the
existing blob. It does not expose deletion or native paths.

A blob is not a logical user-facing asset. The logical slice now exposes
validated `MediaAsset` records with distinct `AssetId` and `MediaBlobId`
identities, provenance format `AssetProvenanceV1`, origin, lifecycle state,
and persistent/library/temporary retention. Multiple assets may share one
blob without merging their provenance or retention policy.

`MediaAssetRepository` is a synchronous port with create/read, retention CAS,
missing-or-corrupt CAS, and paged library listing. It deliberately has no
physical-delete method. `AssetReferenceReader` and `AssetRetentionReader`
provide typed reachability ports for future character, context, conversation,
and message association adapters; no generic owner-kind strings are exposed.

The domain validates nonzero revisions, closed/versioned serialized shapes,
asset/blob kind compatibility, bounded provenance strings, and redacted
HTTP(S)-only source locators without credentials, queries, native paths,
bytes, prompts, or secrets. Ingestion, decoding, native paths, filesystem
access, derivatives, serving and garbage collection belong to later
adapter/use-case slices.
