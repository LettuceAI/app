# lettuce-contracts: status notes

Facts about `lettuce-contracts` that are not architecture. The crate README describes the current design.

## Not wired yet

- The crate description names versioned requests, responses, events and generated frontend bindings. The application API contracts (conversations, characters, errors and events) are exported to TypeScript; the provider catalog, model discovery and key verification contracts still use `ProviderAccountId` and `u64` fields, derive no `specta::Type` and are not exported. No contract carries a version field.

## Live speaker signals

The typed generation speaker events replace legacy group status payloads (`old-code/src-tauri/src/group_chat_manager/mod.rs:314-335,6584-6592`). Character names and avatars remain in participant views rather than being repeated in the event.

Typed runtime notice codes replace llama warning toast strings (`old-code/src-tauri/src/llama_cpp/mod.rs:2598-2607,3544-3556`). Job throughput replaces the global heartbeat counters the memory UI consumed (`old-code/src/ui/pages/chats/CompanionMemoryPage.tsx:645-655`); text remains a separate delta. Runtime report changes name matching model profiles rather than passing a filesystem path (`old-code/src-tauri/src/llama_cpp/mod.rs:480-490`).
