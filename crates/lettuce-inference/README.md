# lettuce-inference

Provider-neutral generation admission, attempts, streaming, tools, approvals,
safety evaluation, cancellation, and terminal outcomes.

## Boundary

Tools and safety remain independent internal modules. The crate does not read
conversations, construct prompts, persist usage, or implement provider wire
protocols.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Foundation scaffolding is active. Implement behavior with tests before exposing new public APIs, and keep compatibility code at explicit application or migration boundaries.
