# lettuce-media

User media as a content-addressed store: the catalog records for physical blobs and logical assets, header sniffing and validation on ingest, the on-disk object layout, reading ready assets, resumable transfer for sync, installing backup objects, and releasing bytes nothing needs any more.

Callers only ever see `AssetId`s, `MediaBlobId`s and descriptor-backed readers, never a native path. Files are reached through `lettuce-platform` (`MediaBlobs` capabilities for the local store, a `ConfinedInstallStore` for sync and restore). The catalog is behind two synchronous repository traits that `lettuce-database` implements. Nothing is decoded, rendered or executed: formats are recognized from bounded headers only.

## Blobs and assets

The crate separates physical content from what the user sees.

A `MediaBlob` (`blob.rs`) is one set of bytes, identified by its BLAKE3 `ContentHash`: `MediaKind` (`Image`, `Audio`, `Video`, `Document`), MIME type, size, optional width and height (always both or neither) and duration, a validation version and a `BlobState` (`Staged`, `Ready`, `Quarantined`, `Missing`). `MediaBlobRepository::register` inserts a staged row or returns the existing row for the same hash, so the same bytes are stored once. `finalize_staged_to_ready` runs after the object file is committed; `restore_missing_to_ready` brings back a blob whose bytes were released and then ingested again.

A `MediaAsset` (`asset.rs`) is a logical, user-facing record with its own `AssetId` pointing at a blob: an `AssetKind` (avatar original, background, illustration, lorebook icon, message image or audio, generated image, synthesized speech, other image or audio, source document), an `AssetOrigin` (upload, import, remote fetch, generated, synthesized, legacy), a `RetentionClass` and `AssetProvenanceV1`. Each asset kind requires one blob kind (`AssetKind::blob_kind`), checked when the asset is created and opened.

Several assets can share one blob without merging their provenance or retention. That is why missing, quarantined or corrupt bytes are blob state and not asset state: one blob backs many assets, and one synced asset can have its bytes on one device and not on another.

`RetentionClass` is `Persistent`, `Library` (listed in the media library) or `Temporary { expires_at }`, so a temporary asset without an expiry cannot be represented. `MediaAssetRepository` offers create, get, a retention change with a revision CAS, and a paged listing of library assets. It has no delete; physical removal goes through the release paths below.

`AssetProvenanceV1` is versioned and bounded: an optional source label, a redacted source URI (http or https only, no credentials, query, `..`, whitespace or native path), the producing job, the model profile and an imported format token. It cannot carry bytes, prompts, paths or provider bodies.

`AssetRetainer` lists the typed owners that can keep an asset reachable (character, persona, group, scene and its variants and links, conversation, message, message revision and candidate, prompt document, lorebook and entry). `AssetReferenceReader` and `AssetRetentionReader` are the ports a reachability or repair workflow would use; there are no free-form owner-kind strings.

## Object layout

Objects live under the media root at `objects/<first two hex>/<next two hex>/<hash>`. Resumable sync downloads write `sync/<hash>.partial`, backup installs write `restore/<hash>.partial`. Because the name is the content hash, an existing object is never replaced: a create-new that finds the file already there checks its size and hash and accepts it only if they match.

## Ingest

`LocalMediaBlobStore::ingest(reader, IngestRequest)` handles one local upload, import or generated file:

1. Validate the provenance and read the input, up to `MAX_MEDIA_BLOB_BYTES` (2 GiB, equal to the largest file a backup can carry and to the platform's managed read limit, so every stored object stays restorable). Empty input is rejected.
2. Sniff the format from its header. Images: PNG, JPEG, GIF, WebP, with dimensions read from the header and capped at 100 million pixels so downstream decoders are never asked for a pathological allocation. Audio: WAV, Ogg, FLAC, M4A, MP3. For `SourceDocument` assets: a PDF header with version 1.0 to 1.7 or 2.0, otherwise valid UTF-8 text as `text/plain` (Markdown included, so identical bytes deduplicate the same way). Recognizing a PDF header is not structural validation; an extractor must still reject malformed or encrypted documents.
3. Check that the sniffed kind matches the asset kind and, when the caller declared a MIME type, that it agrees (with the usual aliases such as `image/jpg` or `audio/x-wav`).
4. Take the process-wide lifecycle lock, hash the bytes and commit the object create-new under its content key.
5. Register the blob (or get the existing one), move it to `Ready` (from `Staged`, or from `Missing` when released bytes come back), and create the asset.

`ingest` allocates a new `AssetId`. `ingest_with_id` takes one from a durable import workflow: an exact retry returns the existing ready asset and blob, while different content or metadata for the same id fails instead of creating a second asset.

`open_ready(asset_id)` opens a ready asset for reading. It checks asset and blob kinds, the blob state (`NotReady` otherwise), and that the object file exists with the recorded size, then returns the records and a `ReadHandle`.

## Releasing bytes

Bytes leave the store in three ways, all under the same lifecycle lock as ingest, so no asset can pick up a blob while its bytes are being deleted:

- `release_blob(blob_id, release)` deletes one ready blob's object after the callback has marked its catalog row `Missing` (the callback returns `false` if the blob must stay). The asset rows stay, so records that point at them remain valid and `open_ready` reports `NotReady`. Ingesting or syncing the same bytes later restores the row to `Ready`. Persona files and the TTS audio cache use this.
- `remove_released_objects(release)` runs a callback that commits the catalog removal of unreferenced blobs and returns their objects, then deletes those files. A file that cannot be deleted is counted and left for the sweep. Hard delete uses this.
- `sweep_orphan_objects(retained)` walks `objects/` and deletes files named by a content hash that the callback says the catalog no longer keeps: bytes a crash left behind between the catalog commit and the file deletion. Partial sync and restore files are never touched.

`pin_media_objects(read)` runs a catalog read and pins the objects it names before any deletion can run. While the `MediaObjectPin` is alive, released objects are deferred and the sweep skips them. A backup export pins the objects its catalog snapshot names and a restore pins what it installs, until they have read or written them all.

## Sync transfer

`LocalSyncMediaStore` moves media between devices over a `ConfinedInstallStore` on the media root:

- `snapshot(asset_id)` returns the asset and blob records of a ready asset after re-hashing the object.
- `read_sync_chunk(hash, offset, max)` serves chunks of at most `MAX_SYNC_MEDIA_CHUNK_BYTES` (1 MiB).
- On the receiving side, `receive_offset` says how many bytes of the hash-named partial already survived an interruption or restart, `append_sync_chunk` appends at exactly that offset, and `finish_sync_asset` checks the declared size and BLAKE3, publishes the object create-new, and only then registers the blob and creates the asset. A hash mismatch restarts the partial. When several remote assets share a hash, they reuse the one local blob.

## Backup install

`install_backup_media_object(root, hash, size, reader)` streams backup bytes into `restore/<hash>.partial`, verifies the hash and publishes the object. It uses its own partial so it never truncates a sync download of the same blob. An object that is already there must match size and hash. Catalog rows are not touched, because the restored database carries its own blob and asset rows.

`sniff_media_kind` exposes the sniffer for callers that need to classify bytes without ingesting them, such as the legacy backup importer in `lettuce-transfer`.
