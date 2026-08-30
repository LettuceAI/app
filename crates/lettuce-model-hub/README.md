# lettuce-model-hub

Remote model discovery, durable verified downloads, compatibility, installed
artifact manifests, leases, installation, and removal planning.

## Boundary

Does not load or execute models. Download completion produces verified artifact
facts and never calls a runtime module directly.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The first installed-artifact contract verifies the Lettuce embedding v4 model
and tokenizer as regular bounded files against an immutable source identity,
byte size, and BLAKE3 digest before exposing runtime paths. The legacy download
used an unpinned Hugging Face `main` URL; new installs must persist an immutable
revision and verified hashes instead of treating filenames as identity.
The audited v4 upstream revision is
`8fe12dc548f75865bfb120593fd5a514e9186ca0`; its model config declares 2,048
trained positions and 768 native dimensions.

The companion-emotion installed contract separately verifies the exact model,
tokenizer, and config triplet used by the GoEmotions auxiliary classifier. It
also requires an immutable source revision before exposing paths to the runtime;
model loading and config interpretation remain owned by `lettuce-embeddings`.
