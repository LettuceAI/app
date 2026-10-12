# lettuce-tauri: legacy parity notes

Local runtime warning toasts become typed request-scoped notices (`old-code/src-tauri/src/llama_cpp/mod.rs:2598-2607,3544-3556`), and the job channel carries throughput and text separately instead of the global heartbeat (`old-code/src/ui/pages/chats/CompanionMemoryPage.tsx:645-655`). Runtime report signals carry model ids instead of paths (`old-code/src-tauri/src/llama_cpp/mod.rs:480-490`). The shell forwards the application events without business rules.

Generation speaker signals replace group status events (`old-code/src-tauri/src/group_chat_manager/mod.rs:314-335,6584-6592`); the application supplies the resolved character and replays it to late attachments.

Model-load progress moves from the global legacy event (`old-code/src/App.tsx:586-588`) to the requesting turn or job channel; typed stage/status, model name and per-GPU progress preserve the displayed load information (`App.tsx:534-580`). Overall percentages are integer and updates are coalesced by visible payload changes.

Lorebook generators use durable jobs and typed results rather than direct long-running commands and the process-only staged registry (`old-code/src-tauri/src/chat_manager/lorebook_generator/state.rs:168-177`). The shell forwards library changes and affected-owner events after hard deletes; the application owns atomic reference cleanup.

Provider verification uses the draft before saving, preserving the legacy editor and onboarding gates (`old-code/src/ui/pages/settings/hooks/useProvidersPageController.ts:298-331`, `old-code/src/ui/pages/onboarding/hooks/useOnboardingController.ts:349-398`). Public OpenRouter discovery still needs no account (`old-code/src-tauri/src/providers/openrouter.rs:93-112`). The shell forwards typed provider messages instead of raw provider response JSON.

Provider mutation and certificate commands delegate to the composition root with revision and replay contracts. The legacy frontend read certificate bytes and rewrote settings (old-code/src/ui/pages/settings/SecurityPage.tsx:181-218); the shell now supplies only a FileSource.

Model profile commands expose atomic defaults and receipts in place of frontend storage sequencing (`old-code/src/core/storage/repo.ts:949-987`). Duplicate takes the UI's localized display name (`old-code/src/ui/pages/settings/ModelsPage.tsx:241-254`). NanoGPT usage failures and warnings are typed; the application owns the event-driven quota checks formerly started by `old-code/src-tauri/src/providers/nanogpt_usage.rs:105-149`.

Settings commands replace legacy frontend storage-column writes with typed section CAS operations (`old-code/src-tauri/src/storage_manager/settings.rs:800-837`). The filter log is developer-mode gated and refreshed through a coalesced event instead of the development-build panel's five-second poll (`old-code/src/ui/pages/settings/SecurityPage.tsx:68,101-110`). The shell forwards application errors and events unchanged.

File pickers are backend commands (`files_pick_open`, `files_pick_save`) instead of frontend calls: legacy opened `@tauri-apps/plugin-dialog` from the UI and passed the path or `content://` URI to Rust (old-code/src/core/storage/files.ts:1090-1150) and read images through webview `<input type="file" accept="image/*">` elements that handed bytes to JavaScript. The filters follow legacy's dialog extension lists (images png/jpg/jpeg/webp, audio wav/mp3/flac/m4a/ogg, chat logs jsonl/json, certificates pem/crt/cer, models gguf/safetensors/sft/ckpt/pt, documents txt/md/markdown/pdf/text); backups were unfiltered in legacy and now offer `.lettuce` and `.zip` on desktop.

Slice 12 installs daily file logging and the panic hook, following `old-code/src-tauri/src/app/bootstrap.rs:99`. Log exports take a FileTarget instead of writing directly to Downloads (`old-code/src-tauri/src/infra/logger.rs:815`). Android crash monitoring and its two-second heartbeat remain Not wired yet (`old-code/src-tauri/src/platform/android_monitor.rs:285`); the persisted Android export folder remains deferred (`old-code/src-tauri/src/infra/logger.rs:722`). Window chrome flags, accessibility sounds, chat themes and analytics availability remain UI or shell work (`old-code/src-tauri/src/app/bootstrap.rs:335`, `old-code/src-tauri/src/infra/utils.rs:609`, `old-code/src-tauri/src/chat_appearance/mod.rs:100`, `old-code/src-tauri/src/storage_manager/settings.rs:772`).

Metric wrappers preserve the Performance page's read-after-clear flow (`old-code/src/ui/pages/settings/PerformancePage.tsx:133`) through an idempotent clear request. Automatic retention is removed under slice 12's no-count-cap rule; legacy pruned at 500 (`old-code/src-tauri/src/storage_manager/llm_metrics.rs:31`).

Usage clear-before preserves the strict cutoff from `old-code/src-tauri/src/usage/repository.rs:503-518`. It deliberately skips unsettled dispatches and nonterminal owners, commits costs and exact ownership tombstones with the deletion receipt, and consumes synchronized re-sends by id. Legacy counted with a zero fallback and deleted in a separate statement (`old-code/src-tauri/src/usage/repository.rs:506-518`); the rewrite fails typed and commits atomically. Backups retain proofs for terminal references instead of losing their audit integrity.

Slice 12 exposes kept-database inventory and explicit deletion as new storage commands under decision 15. No native database paths cross their contracts.

Slice 12 media export uses a picked FileTarget instead of a fixed Downloads copy returning a native path (`old-code/src-tauri/src/storage_manager/media.rs:898-958`). Export targets hold an operating-system lease until close; concurrent writers fail Busy, and process exit permits retry. Android document locking remains subject to the document provider and has no device validation in this slice.

The media library wrappers replace the separate filesystem inventories (`old-code/src-tauri/src/storage_manager/media.rs:559-576,875-895`) with image or audio pages. Referenced removal returns InUse instead of deleting a referenced image file (`old-code/src-tauri/src/storage_manager/media.rs:988-1023`).

The registered diagnostics report replaces frontend assembly in `old-code/src/ui/pages/settings/LogsPage.tsx:1114-1267`. Clipboard and copy feedback remain UI responsibilities (`LogsPage.tsx:1270-1281`); required backend failures are typed.

Usage reporting delegates the unified query, local-day statistics and picked-target CSV export to the application. Export preserves the fixed header from `old-code/src-tauri/src/usage/repository.rs:700-709` and replaces the frontend's export completion flow (`old-code/src/ui/pages/settings/UsagePage.tsx:651-668`) with a typed command result.

The storage optimization wrapper returns JobAccepted in place of `db_optimize`; its completion is watched through the jobs API, replacing the immediate developer-page success message (`old-code/src/ui/pages/settings/DeveloperPage.tsx:1967-1974`).

Usage recalculation returns JobAccepted and typed job completion in place of the immediate summary string (`old-code/src/ui/pages/settings/DeveloperPage.tsx:2002-2010`). It resolves credentials in the application and fills missing costs instead of the legacy overwrite (`old-code/src-tauri/src/usage/commands.rs:200-215`).

The storage summary is an application API command with byte categories and no native path fields. Legacy offered a file-count/session-count summary (`old-code/src-tauri/src/storage_manager/usage.rs:140-190`), exposed only through the unused reset-summary wrapper (`old-code/src/core/storage/reset.ts:40-53`).

Desktop reset acknowledges clearing the owned webview localStorage and sessionStorage and checks executable spawning before exit. Legacy awaited backend reset and application-state clearing (`old-code/src/core/storage/reset.ts:25-32`) and then exited without restarting (`old-code/src/ui/pages/settings/ResetPage.tsx:26-39`). Native webview and process restart behavior has no device validation in this slice. Android and iOS reset complete the same cutover and then exit the process without relaunching it, as legacy did on every platform (`old-code/src/ui/pages/settings/ResetPage.tsx:26-29`); the user reopens the app. Once the workers were stopped, the process always ends, also when the reset failed or the webview storage or relaunch step failed, so it never keeps running on a closed or fenced database; the failure stays on the job. Android crash monitoring remains the separately deferred shell work.

The legacy_state wrapper exposes the upgrade requirement for old bin storage instead of invoking legacy's importer (`old-code/src-tauri/src/storage_manager/importer.rs:12-34,407-431`). Detection and schema validation remain application and database responsibilities.

Slice 11a routes backup file deletion through the host file adapter and preserves removal failures, correcting legacy Android pruning on failure (`old-code/src-tauri/src/storage_manager/backup.rs:4564-4584`). Every command snapshots the current application context for in-place database rebind.

Slice 11a keeps the shell as the filesystem and lifecycle adapter for backup jobs. Android public metadata failures are classified with the existing plugin permission and primary-volume APIs, so revoked permission or unavailable storage does not prune an entry as legacy did (`old-code/src-tauri/src/storage_manager/backup.rs:4571-4584`). Native desktop publication uses atomic replacement and durable completion; Android preserves safe collision naming and clears pending publication through the plugin.

The restore adapter rebuilds through the same constructor and startup used at launch, then the application emits DatabaseReloaded. This retains the legacy completion notification (`old-code/src-tauri/src/storage_manager/backup.rs:4427`) and settings refresh (`old-code/src/App.tsx:695-708`) while making failure after pointer switch a typed reopen requirement. No backup or media bytes cross command IPC, unlike legacy byte-based import (`old-code/src-tauri/src/storage_manager/backup.rs:4780-4785`).

Slice 11a review fixes delay context publication until restart recovery and worker startup finish. Desktop atomic backup replacement preserves protected-file checks and permits existing readers without an additional target lease.

Desktop backup temporary locations and publication resolve parent-directory aliases while preserving the final filename. Atomic replacement replaces a selected final symlink and leaves its referent intact; the returned index path matches the admission target.

Desktop publication uses std::fs::rename on Windows as on other desktop platforms. The synchronized complete file and approved full-header publication recovery protect the index boundary. Reload teardown shares context retirement with ApiContextSlot, and the shell integration test exercises the real ResetHost and stop_for_restore path.

The shared typed event channel forwards SyncConflictsChanged from the application commit feed. Legacy emitted sync-conflicts-changed directly in the shell resolve command (old-code/src-tauri/src/sync/commands.rs:67); the shell now owns no conflict notification policy.

Sync wrappers retain legacy's separate approve_connection and start_sync_session gates (old-code/src-tauri/src/sync/manager.rs:492-541; old-code/src/ui/pages/sync/SyncPage.tsx:690-709) while typed session state and conflict policy stay in lettuce-app. Local stop is Idle and peer disconnection is typed Failed, matching legacy (old-code/src-tauri/src/sync/manager.rs:559-570,1333,1446). Addresses and PIN are returned for the deferred frontend pairing screen and QR rendering.

Slice 11c inspection exposes the template and model shapes legacy read in the frontend (`old-code/src/core/storage/chatTemplateTransfer.ts:94`, `old-code/src/core/storage/modelTransfer.ts:118-145`) and USC lorebooks legacy could export (`old-code/src-tauri/src/storage_manager/lorebook.rs:1039-1059`). CharX and ordinary ZIP archives no longer appear as backups.

Entity import and export adapters carry paths and typed preview data instead of legacy character File bytes and browser downloads (old-code/src/core/storage/characterTransfer.ts:164-204). Prompt exports remain USC only (old-code/src-tauri/src/chat_manager/commands/mod.rs:800-811); template and model requests retain JSON and USC (old-code/src/core/storage/chatTemplateTransfer.ts:4; modelTransfer.ts:89-112). Chat JSONL import returns a durable job instead of completing inside the IPC call.

Android entity exports omit FileTarget and return the published URI from the existing pending Download/LettuceAI flow, including collision suffixes. Legacy wrote to Downloads and returned its path (old-code/src-tauri/src/storage_manager/entity_transfer/mod.rs:4048-4094). Desktop exports require a selected FileTarget.

Entity exports keep no durable publication state. A process crash can leave a desktop `lettuce-backup-<uuid>.partial` beside the selected target or an Android pending entry in Download/LettuceAI. Startup does not sweep these interrupted entity exports; cleanup and recovery remain deferred. The shared temporary prefix is retained for the Android publication adapter.
