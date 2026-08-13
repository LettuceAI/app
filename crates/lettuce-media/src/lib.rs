//! Logical media identities and content-addressed blob metadata.
//!
//! This crate exposes IDs and validated metadata only. Ingestion, decoding,
//! native paths, filesystem access, derivatives, serving and garbage
//! collection belong to later adapter/use-case slices.

#![deny(unsafe_op_in_unsafe_fn)]

mod asset;
mod blob;
mod repository;

pub use asset::{
    ASSET_PROVENANCE_FORMAT_VERSION, AssetKind, AssetOrigin, AssetProvenanceError,
    AssetProvenanceV1, AssetReference, AssetReferenceReader, AssetReferenceReaderError,
    AssetRetainer, AssetRetentionReader, MediaAsset, MediaAssetMutationError,
    MediaAssetValidationError, RetentionClass,
};
pub use blob::{
    BlobState, MediaBlob, MediaBlobRepository, MediaBlobRepositoryError, MediaBlobValidationError,
    MediaKind,
};
pub use repository::{MediaAssetRepository, MediaAssetRepositoryError};
