# lettuce-contracts: status notes

Facts about `lettuce-contracts` that are not architecture. The crate README describes the current design.

## Not wired yet

- The crate description names versioned requests, responses, events and generated frontend bindings. The application API contracts (conversations, characters, errors and events) are exported to TypeScript; the provider catalog, model discovery and key verification contracts still use `ProviderAccountId` and `u64` fields, derive no `specta::Type` and are not exported. No contract carries a version field.
