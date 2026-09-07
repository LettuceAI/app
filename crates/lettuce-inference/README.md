# lettuce-inference

Provider-neutral generation admission, attempts, streaming, tools, approvals,
safety evaluation, cancellation, and terminal outcomes.

## Boundary

Tools and safety remain independent internal modules. The crate does not read
conversations, construct prompts, persist usage, or implement provider wire
protocols.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The runtime foundation provides a bounded in-process stream registry and
notification-driven cooperative cancellation. Stream delivery validates the conversation
operation/turn/attempt identity and monotonic sequence before applying channel
backpressure; no detached fan-out task or durable payload store is involved.
The runtime can signal an exact registered job token by durable job ID and
reports whether a live execution was present; registration and removal remain
owned by the application execution lifetime.
Provider adapters race cancellation against connection setup, socket reads,
buffered responses, and backpressured delivery. Job and conversation domains
remain authoritative for persisted lifecycle state. Provider normalization and
outcome assembly are implemented by `lettuce-providers`; tools and safety
remain later horizontal slices.
