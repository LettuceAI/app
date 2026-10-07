# lettuce-tauri: legacy parity notes

Local runtime warning toasts become typed request-scoped notices (`old-code/src-tauri/src/llama_cpp/mod.rs:2598-2607,3544-3556`), and the job channel carries throughput and text separately instead of the global heartbeat (`old-code/src/ui/pages/chats/CompanionMemoryPage.tsx:645-655`). Runtime report signals carry model ids instead of paths (`old-code/src-tauri/src/llama_cpp/mod.rs:480-490`). The shell forwards the application events without business rules.

Generation speaker signals replace group status events (`old-code/src-tauri/src/group_chat_manager/mod.rs:314-335,6584-6592`); the application supplies the resolved character and replays it to late attachments.
