# lettuce-app

The sole composition root, application workflows, desktop IPC boundary, and
optional local OpenAI-compatible host API.

## Boundary

May wire all crates; no crate may depend on it. Host API handlers reuse the
model and inference services without constructing product conversations.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The composition root initializes storage and built-in prompts, plans direct and
group conversation launches, assembles provider-neutral context, and can build
a reusable remote-provider runtime. Provider composition requires the host to
supply its real `SecretStore` and current TLS policy; the app never substitutes
an in-memory credential store in production. The runtime loads provider
accounts through `lettuce-database` repositories and exposes catalog, model
discovery, and key verification through `lettuce-contracts` DTOs. It also owns
and exposes the matching `InferenceRuntime`, so later generation workflows can
register bounded stream receivers and cancellation tokens against the same
runtime used by the provider adapters.

The first direct/group dynamic-memory handler path accepts an already admitted
and running ordered tool round, validates the exact v1 feature contract, joins
precomputed create metadata, reduces it against one stored memory-space
snapshot, and compare-and-applies at most one change. It returns ordered typed
provider-neutral outputs for the conversation coordinator to settle durably;
continuation and job recovery remain later coordinator work.

The composition layer can also verify an installed embedding manifest through
`lettuce-model-hub`, load one serialized ONNX embedding runtime, and execute
request-scoped inference with a `lettuce-jobs` cancellation token. Hosts remain
responsible for scheduling this synchronous CPU/model-load work on the job
worker rather than a UI or async-runtime thread.

## Bundled prompts

The app owns the closed, versioned built-in prompt catalog in
`resources/built-in-prompts/v1.json`. It is embedded with `include_str!`, so
bootstrap and reset work from the application binary on desktop and Android
without depending on installation paths or writable files.

`BuiltInPromptService` reconciles unedited seeds during startup and supports
explicit reset of one or every protected prompt. Consumers keep the returned
`BuiltInPromptIds` instead of hard-coding database UUIDs. The stable legacy
catalog keys and entry keys are compatibility identities; user-facing names
and content remain editable, while purpose and lifecycle protection do not.

`AppBackend::open` is the supported production database-open path. It opens the
caller-provided SQLite path, applies database migrations, bootstraps the
catalog, and returns the database together with the complete typed prompt ID
set. This prevents a host from accidentally serving an initialized database
without the required app prompts. `AppBackend::open_in_memory` provides the
same invariant for tests.

The adjacent `v1.snap` pins a canonical digest for every individual entry,
including its stable key, role, content, scheduling, conditions, payload, and
system-prompt flag. A change to one legacy field therefore identifies the
specific prompt entry instead of appearing only as a catalog-wide checksum.
