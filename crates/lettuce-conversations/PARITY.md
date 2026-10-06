# lettuce-conversations: legacy parity notes

Facts about how `lettuce-conversations` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- A turn stopped after its reply streamed visible text or a whole image finalizes that partial reply, as the legacy chat page persisted the streamed placeholder on stop (`useChatAbortController.ts` 29-148).
- Editing an assistant reply rewrites its selected variant in place, as the legacy chat page did (`useChatMessageActionsController.ts` 227-241).
- `mentioned_participant` keeps legacy's `@"Full Name"` then `@Word` parsing: quoted names match exactly, unquoted names exactly and then by prefix, case-insensitively, trailing punctuation trimmed.
- The heuristic's recency distance excludes the user message a send is answering because legacy numbered every stored group message as a turn, scored `current_turn - last_spoke_turn` and built that context before it saved the new user message (`group_chat_manager/mod.rs` 6430-6443).
- Round robin continues after the last speaker when still selectable and otherwise restarts at the first selectable member (`selection.rs` 449-467).
- A group conversation can change its speaker-selection method after launch, as legacy did per session.
- `append_user_message` is legacy `group_chat_add_user_message`.
- `CurrentConversationSettings.model_settings` is the legacy session `advanced_model_settings`. Legacy resolved every field as session, then model, then app, and an unset field deferred to the next layer.
- A conversation background of `None` follows the selected scene, then the character or group, like legacy.
- The verified direct and group dynamic-memory tool scenarios are pinned in `fixtures/legacy-import/dynamic-memory-tool-scenarios-v1.json`.
- A group conversation keeps one prompt choice per chat mode, like legacy's two session columns picked by chat type (`old-code/src-tauri/src/storage_manager/group_sessions.rs:2453-2499`).
- At least one member must stay active, as legacy refused muting every member (`group_sessions.rs:2316-2325`); the rule covers removing a member too.
- Adding a character that was removed before enables its kept row, as legacy kept participation rows and reused them on re-add (`group_sessions.rs:1984-2028`).
- The member list, muted flags and member models are owned separately, like legacy's `characterIds`, `mutedCharacterIds` and `characterModelOverrides` override keys (`group_sessions.rs:510-615`).

## Deliberate differences from legacy

- An explicit speaker missing from the cast is a typed error, never a panic.
- A group scene override is refused only when the conversation's own chat mode is conversation; before, the launch chat mode decided, which a chat that follows its group into roleplay could no longer use.
- Every mutation accepts an archived conversation, like legacy where archiving was a list flag only (`old-code/src-tauri/src/storage_manager/sessions.rs:3819-3829`); a user write (send, added user message, continue, regenerate, retry) also restores it to Active in the same transaction, which legacy never did.

## History

- The previous README said the legacy imports of the `timeAwarenessEnabled` and `timeOverride` preferences were still missing. `lettuce-app/src/legacy/legacy_direct_conversation_import.rs` reads both now.
- The previous README called runtime resolution of `model_settings` a later slice. Generation input (`lettuce-app/src/generation/conversation_generation_input.rs`) now passes it as the session layer of chat parameter resolution.
- The previous README said conversation checkpoints did not persist `provider_response_id`. The initial inference checkpoint stores the whole `InferenceOutcome`, including that field, and the database compares it with the usage evidence when settling.
- The previous README said the final schema "must" add a partial unique index on `generation_attempts(job_id)`. It exists: `generation_attempts_job_id_uq` in `lettuce-database/migrations/0008_conversations.sql`.
- The previous README described the tool contract as horizontal only, with the application coordinator and the legacy memory, creation, companion and lorebook handler migrations as separate slices.

## Not wired yet

- The single in-flight turn rule is enforced by the adapters; its supporting index is deferred.
- Several async ports in `ports.rs` (`LaunchResolver`, `SpeakerPolicy`, `ModelResolver`, `MediaPort`, `MemoryPort`, `CompanionPort`, `JobPort`, `Clock`, `ConversationApplication`) have no implementation outside this crate; `lettuce-app` composes the generation flow directly.

A fork stores `"{conversation title} (branch)"` without numbering, matching `old-code/src/core/storage/repo.ts:1683`. Its own label can evolve independently while the root name remains the conversation title; legacy represented each branch as a separately titled session (`old-code/src/ui/pages/chats/ChatTreePage.tsx:213-220`).

Branch list, fork, rename and select are exposed through the application API. Legacy tree nodes were independent sessions ordered by creation (`old-code/src-tauri/src/storage_manager/sessions.rs:2429-2493`, `old-code/src/ui/pages/chats/ChatTreePage.tsx:45-64,105-116`); the rewrite lists surviving branches in creation order and counts visible messages across all roles. Root rename changes the conversation title and fork rename changes only its stored label.

Conversation roots can retain lineage for direct-to-character copies, matching legacy parent-session and branched-message metadata (`old-code/src/core/storage/repo.ts:1744-1748`). The source ids remain readable after deletion of the source; they do not retain that conversation or its messages.
