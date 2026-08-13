# lettuce-media

Content-addressed user media, validation, derivatives, serving, retention, and repair.

## Boundary

Exposes asset IDs, never managed native paths.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Foundation contract

The first slice exposes a `MediaBlobRepository` for validated,
content-addressed blob metadata. Registering the same BLAKE3 hash returns the
existing blob. It does not expose deletion or native paths.

A blob is not a logical user-facing asset. Logical `AssetId` records,
provenance, retention, and typed character/message associations arrive with
the feature slice that can enforce those lifecycles.
