# lettuce-media: status notes

Facts about `lettuce-media` that are not architecture: legacy comparisons and what is not wired yet. The crate README describes the current design.

## Legacy parity

- Invalid UTF-8 is not accepted as a text source document, matching legacy text intake.
- Feature-specific legacy source limits stay at intake in the calling feature; this crate applies only its own size bound and retention.
- `lettuce-app` reads source documents for the legacy PDF and text extraction path.

## Not wired yet

- Creation-project-owned source document associations are not wired.
- `AssetReferenceReader` and `AssetRetentionReader` have no implementations yet; they are the ports for character, context, conversation and message association adapters.
- The crate description also names derivatives and serving. There are no derivative or serving paths in the crate.
- Blob duration is never filled on ingest (audio sniffing records no duration), and no `Video` format is sniffed.

Slice 12 exposes ready assets through a streamed backend FileTarget export rather than native source paths and a fixed Downloads directory (`old-code/src-tauri/src/storage_manager/media.rs:898-958`). Missing objects fail typed; media never crosses IPC as bytes. Managed identity checks protect stored media against export overwrite, and orphan collection no longer caps files per hash bucket or substitutes zero for failed metadata reads.

Slice 12 adds a library read across all retention classes and guarded removal with references, replacing the filesystem inventories (`old-code/src-tauri/src/storage_manager/media.rs:559-576,875-895`) and preventing the dangling references left by image deletion (`old-code/src-tauri/src/storage_manager/media.rs:988-1023`). The separate association ports `AssetReferenceReader` and `AssetRetentionReader` remain unwired; the library uses the broader reference evidence needed by GC, including pending imports, jobs and sync.

Audio removal now refuses referenced assets with typed InUse instead of deleting the file and attempting attachment cleanup with warning-only failures (`old-code/src-tauri/src/storage_manager/media.rs:834-871`). Legacy audio confirmation awaited deletion before filtering the displayed item and logged failures (`old-code/src/ui/pages/library/AudioLibraryPanel.tsx:125-143`); API completion and typed failure now let the caller preserve that completion gate.

Reset preserves kept-file media rather than removing the storage and generated image directories (`old-code/src-tauri/src/storage_manager/usage.rs:105-120`). Every application-owned database handle closes before the old file moves; the shared GC retention rule determines its protected hashes.
