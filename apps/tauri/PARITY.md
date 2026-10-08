# lettuce-tauri: legacy parity notes

Local runtime warning toasts become typed request-scoped notices (`old-code/src-tauri/src/llama_cpp/mod.rs:2598-2607,3544-3556`), and the job channel carries throughput and text separately instead of the global heartbeat (`old-code/src/ui/pages/chats/CompanionMemoryPage.tsx:645-655`). Runtime report signals carry model ids instead of paths (`old-code/src-tauri/src/llama_cpp/mod.rs:480-490`). The shell forwards the application events without business rules.

Generation speaker signals replace group status events (`old-code/src-tauri/src/group_chat_manager/mod.rs:314-335,6584-6592`); the application supplies the resolved character and replays it to late attachments.

Model-load progress moves from the global legacy event (`old-code/src/App.tsx:586-588`) to the requesting turn or job channel; typed stage/status, model name and per-GPU progress preserve the displayed load information (`App.tsx:534-580`). Overall percentages are integer and updates are coalesced by visible payload changes.

Lorebook generators use durable jobs and typed results rather than direct long-running commands and the process-only staged registry (`old-code/src-tauri/src/chat_manager/lorebook_generator/state.rs:168-177`). The shell forwards library changes and affected-owner events after hard deletes; the application owns atomic reference cleanup.

Provider verification uses the draft before saving, preserving the legacy editor and onboarding gates (`old-code/src/ui/pages/settings/hooks/useProvidersPageController.ts:298-331`, `old-code/src/ui/pages/onboarding/hooks/useOnboardingController.ts:349-398`). Public OpenRouter discovery still needs no account (`old-code/src-tauri/src/providers/openrouter.rs:93-112`). The shell forwards typed provider messages instead of raw provider response JSON.

Provider mutation and certificate commands delegate to the composition root with revision and replay contracts. The legacy frontend read certificate bytes and rewrote settings (old-code/src/ui/pages/settings/SecurityPage.tsx:181-218); the shell now supplies only a FileSource.

Model profile commands expose atomic defaults and receipts in place of frontend storage sequencing (`old-code/src/core/storage/repo.ts:949-987`). Duplicate takes the UI's localized display name (`old-code/src/ui/pages/settings/ModelsPage.tsx:241-254`). NanoGPT usage failures and warnings are typed; the application owns the event-driven quota checks formerly started by `old-code/src-tauri/src/providers/nanogpt_usage.rs:105-149`.
