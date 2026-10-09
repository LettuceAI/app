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
