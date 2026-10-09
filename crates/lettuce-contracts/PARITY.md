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

Declared scope edits are authoritative without remote metadata: removed Supported modalities become Unknown, explicit Unsupported is retained, and reported metadata overrides echoed modality statuses. Views derive scopes directly from Supported capability statuses. Deletion emits SettingsChanged for models when the shared core changes a stored selection (`old-code/src/core/storage/repo.ts:969-980`).

NanoGPT usage refresh=false reuses only a successful recent account result, preventing warning toasts from making a second HTTP request. Refresh=true preserves explicit refresh, and both join a running check. Legacy fetched when the panel selection or refresh changed (`old-code/src/ui/pages/settings/NanoGptUsagePanel.tsx:411-474`); callers now distinguish cached event reads from refresh actions.


Scope views derive only Supported modalities from capability statuses. Separate stored declaration fields were removed by re-review decision. A declaration that remains Unsupported fails typed before any write; reported metadata replaces its side, including clearing echoed Supported modalities not listed. Legacy stored input/output scopes without this typed capability check (`old-code/src-tauri/src/storage_manager/models.rs:126-159`).

Explicit refresh=false quota reads retry an earlier error instead of retaining it after a credential repair. Successful reads still reuse the 300-second result, and background completion checks retain their per-account failure coalescing. Legacy manual reads fetched directly (`old-code/src-tauri/src/providers/nanogpt_usage.rs:163-174`).

Settings section updates replace blind frontend read-modify-write storage with one shared revision CAS (`old-code/src-tauri/src/storage_manager/settings.rs:800-837`). The typed UI choices retain the legacy key map, appearance and widget shapes (`old-code/src/core/storage/schemas.ts:2988-3050,3070-3223`; `old-code/src/core/storage/chatWidgetSchemas.ts:1-153`); no preset or widget count cap is copied. Developer-only filter reads and clearing replace the development-build panel and its five-second poll (`old-code/src/ui/pages/settings/SecurityPage.tsx:68,85-110,533`). Creation-helper enable remains absent by the slice 7 amendment; other creation-helper fields have no update variant.

Optional UI choices reject explicit null instead of silently omitting it; legacy Zod optional string choices also rejected null (`old-code/src/core/storage/schemas.ts:2992-3005`). Clearing a UI preference uses null on the typed key change itself.

DeviceEmbedding updates use the device view revision rather than the global settings revision, keeping install-local changes outside the sync document. The model-folder path is read-only and changes through the existing relocation job.

The second settings review narrows SettingsChanged to actual settings-table commits and gives every device record write the device label. Catalog-only writes use ModelsChanged, replacing the broad legacy broadcast after every save or removal (`old-code/src/core/storage/repo.ts:965,979`). Default changes and deletion that clears or promotes a selection retain the settings notification.

The log API preserves viewer operations from `old-code/src-tauri/src/infra/logger.rs:660` with typed errors and FileTarget exports. Its live event carries a written line instead of legacy chat://debug JSON (`old-code/src-tauri/src/infra/utils.rs:373`). App usage reads include the current stretch, as legacy flushed before reading (`old-code/src-tauri/src/usage/commands.rs:62`), with an event replacing the frontend timer (`old-code/src/ui/pages/settings/UsagePage.tsx:448`).

Metrics keep the Performance page's summaries and samples (`old-code/src/ui/pages/settings/PerformancePage.tsx:100`) with an owned envelope rather than flattening arbitrary JSON. Cursor pages replace the legacy whole-list fetch (`old-code/src/core/storage/metrics.ts:9`); explicit clear carries a durable operation identity.

Usage clear-before preserves the strict cutoff from `old-code/src-tauri/src/usage/repository.rs:503-518`. It deliberately skips unsettled dispatches and nonterminal owners, commits costs and exact ownership tombstones with the deletion receipt, and consumes synchronized re-sends by id. Legacy counted with a zero fallback and deleted in a separate statement (`old-code/src-tauri/src/usage/repository.rs:506-518`); the rewrite fails typed and commits atomically. Backups retain proofs for terminal references instead of losing their audit integrity.
