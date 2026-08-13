# lettuce-context

Owns structured prompt programs, lorebooks, bindings, deterministic activation,
context assembly, validation, preview, and exact source attribution.

Prompt rendering and lorebook matching remain separate internal modules. The
crate consumes immutable inputs and never queries storage, invokes providers,
or mutates conversation history.
