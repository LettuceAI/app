# lettuce-contracts: status notes

Facts about `lettuce-contracts` that are not architecture. The crate README describes the current design.

## Not wired yet

- The crate description names versioned requests, responses, events and generated frontend bindings. The application API contracts (conversations, characters, errors and events) are exported to TypeScript; the provider contracts still use internal `ProviderAccountId` and `u64` fields, with account ids exported as strings through Specta field mappings. No contract carries a version field.

## Live speaker signals

The typed generation speaker events replace legacy group status payloads (`old-code/src-tauri/src/group_chat_manager/mod.rs:314-335,6584-6592`). Character names and avatars remain in participant views rather than being repeated in the event.

Typed runtime notice codes replace llama warning toast strings (`old-code/src-tauri/src/llama_cpp/mod.rs:2598-2607,3544-3556`). Job throughput replaces the global heartbeat counters the memory UI consumed (`old-code/src/ui/pages/chats/CompanionMemoryPage.tsx:645-655`); text remains a separate delta. Runtime report changes name matching model profiles rather than passing a filesystem path (`old-code/src-tauri/src/llama_cpp/mod.rs:480-490`).

Model-loading DTOs preserve the fields displayed by the legacy model-load toast (`old-code/src/App.tsx:534-580`): stage, status, model name and GPU labels/percentages. Overall progress crosses the API as an integer percentage rather than the legacy fractional float, with change-based coalescing.

Provider catalog and control DTOs now derive Specta types. Account views omit secret owners/references and expose api_key_set rather than the legacy editor's plaintext key (`old-code/src/ui/pages/settings/hooks/useProvidersPageController.ts:142`). Verification failures carry a redacted provider message for the legacy inline error flow (`useProvidersPageController.ts:314-319`); public OpenRouter endpoint fields retain the picker data (`old-code/src-tauri/src/providers/openrouter.rs:6-16`).

Provider account keys are write-only, blank preserves and explicit clear removes. Revision/idempotency failures and malformed model lists are typed instead of hidden. Legacy populated the editor key and displayed verify failures (old-code/src/ui/pages/settings/hooks/useProvidersPageController.ts:142,298-331). Certificate inputs move from frontend-read PEM to FileSource (old-code/src/ui/pages/settings/SecurityPage.tsx:196-218).

Stored certificate views include validity and a typed InvalidPem reason; runtime client construction skips invalid stored roots so users can list and remove them. New imports validate strictly before writing.

Duplicate certificate imports return Conflict with CertificateAlreadyImported and the existing certificate id, separate from operation-id conflicts, preserving the distinct duplicate message at old-code/src/ui/pages/settings/SecurityPage.tsx:208-210.

Provider verification preserves string messages, string error types and JSON-stringified error values from old-code/src-tauri/src/providers/util.rs:162-182, with credential redaction. Missing provider text uses typed MissingApiKey or InvalidApiKey details and an absent provider_message.

Model profile commands replace frontend storage calls with revisioned receipt-backed requests (`old-code/src/core/storage/repo.ts:949-987`). Duplicate names remain caller-authored and localized (`old-code/src/ui/pages/settings/ModelsPage.tsx:241-254`). NanoGPT usage retains the data consumed by `old-code/src/core/usage/nanogpt.ts:6-34`; quota warnings carry account id and typed level instead of the legacy toast's usage and English fields (`old-code/src/ui/components/NanoGptQuotaMonitor.tsx:27-62`). The UI can read current usage through the usage command.
