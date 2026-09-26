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
