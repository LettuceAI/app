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

Media library kind selects image or audio and each item returns its role, replacing the separate legacy inventories (`old-code/src-tauri/src/storage_manager/media.rs:559-576,875-895`). Referenced removal is a typed InUse result with reference details instead of a dangling file path (`old-code/src-tauri/src/storage_manager/media.rs:988-1023`).

Usage filters preserve the inclusive legacy date bounds (`old-code/src-tauri/src/usage/repository.rs:275-283`) and add stable cursor pages over one deduplicated charge read. Statistics group days in an explicit device-local timezone rather than relying on webview Date conversion (`old-code/src/ui/pages/settings/UsagePage.tsx:490-496`). Nullable fields and unknown-request counts replace zero substitutions. CSV completion is returned after writing a FileTarget, preserving the UI's export-before-completion flow (`UsagePage.tsx:651-666`).

Storage optimization exposes a job and typed completion or failure instead of the legacy developer button assuming completion from a void command (`old-code/src/ui/pages/settings/DeveloperPage.tsx:1967-1974`).

Full persona draft validation distinguishes an explicit null from an omitted nullable field. Legacy persona save omitted LoRA fields in its repository payload and then wrote NULL (old-code/src/core/storage/repo.ts:2257-2270; old-code/src-tauri/src/storage_manager/personas.rs:342-346); an incomplete typed draft now fails before any write.

Character editor drafts make the complete authored graph explicit, including provider voice choices and image recommendations. Legacy used a partial save payload and replaced source and scenario in the repository (old-code/src/core/storage/repo.ts:1106-1130); the full typed draft keeps those fields under the caller's control on edits.

Group editor contracts preserve the legacy authored name, chat type, member order, persona, speaker selection, background and starting scene (old-code/src-tauri/src/storage_manager/group_characters.rs:499-575,638-709). Persona inheritance and explicit disabling use the rewrite's existing tri-state domain selection instead of overloading a nullable id.

The typed legacy startup state replaces direct bin import from `old-code/src-tauri/src/storage_manager/importer.rs:12-34,407-431` with the slice 11a requirement to open 2.2.5 once. Native legacy paths and payloads stay outside the contract.

Slice 11a represents explicit backup deletion with an operation key and typed failures; the legacy UI refreshed the list only after a successful delete (`old-code/src/ui/pages/settings/hooks/useBackupRestore.ts:282-293`). Public metadata follows the approved restricted v2 header.

Slice 11a review fixes preserve actionable file failure distinctions in durable backup job results and watch events.

LegacyState carries failed detection as a typed ApiError state, so startup and AppStatus remain available when a legacy source cannot be opened. BackupIndexView reports availability per entry instead of failing the list. Backup job reasons distinguish format limits and existing restore targets without introducing a new disk-full error code.

SyncConflictsChanged is the typed application event for committed conflict evidence changes. Legacy used a string-named event only after explicit resolution (old-code/src-tauri/src/sync/commands.rs:67); the inbox subscribed to it (old-code/src/ui/components/SyncConflictInbox.tsx:143-154).

Sync statuses replace legacy string Error messages with ApiErrorCode and SyncFailure details (old-code/src-tauri/src/sync/manager.rs:137-177). Host approval and start remain separate, and completion preserves the model-refresh flow (old-code/src/ui/pages/sync/SyncPage.tsx:572-597,690-709). Read-only legacy differences retain both stored snapshots instead of choosing a winner from an unpreserved entity-state table (old-code/src-tauri/src/sync/v2/conflicts.rs:145-184). Canonical candidates expose catalog AssetRef values rather than media bytes.

Legacy conflict fields use NotMapped for every current value when the entity is missing; actual null fields on existing entities remain JSON null.

Slice 11c exposes template and model file kinds for the formats legacy parsed in the frontend (`old-code/src/core/storage/chatTemplateTransfer.ts:94`, `old-code/src/core/storage/modelTransfer.ts:118-145`). Lorebook includes legacy USC exports (`old-code/src-tauri/src/storage_manager/lorebook.rs:1039-1059`); unsupported ZIPs are Other, following the legacy manifest inspection boundary (`old-code/src-tauri/src/storage_manager/backup.rs:3572-3641`).

Entity import previews replace character editor prefill and IPC byte uploads (old-code/src/core/storage/characterTransfer.ts:154-173; old-code/src/ui/pages/characters/hooks/useCharacterForm.ts:654-695). Named image warnings replace log-only character and persona image failures (old-code/src-tauri/src/storage_manager/entity_transfer/mod.rs:2316-2350,3841-3870). Prompt export accepts only USC, matching old-code/src-tauri/src/chat_manager/commands/mod.rs:800-811; template and model export retain JSON and USC (old-code/src/core/storage/chatTemplateTransfer.ts:4; modelTransfer.ts:89-112). Export requests deliberately have no replay key because publication writes only a file.

Android entity exports omit FileTarget and return the published URI from the existing pending Download/LettuceAI flow, including collision suffixes. Legacy wrote to Downloads and returned its path (old-code/src-tauri/src/storage_manager/entity_transfer/mod.rs:4048-4094). Desktop exports require a selected FileTarget.

LorebookExportFormat replaces Usc with Uec for UEC 3 output. Legacy's export menu selected USC or World Info (`old-code/src/ui/components/LorebookExportMenu.tsx:5-33`), and its editor closed the menu after download and displayed export failures (`old-code/src/ui/pages/library/StandaloneLorebookEditor.tsx:757-776`). World Info output and USC input remain supported.
