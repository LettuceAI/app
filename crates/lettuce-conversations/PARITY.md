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

## Deliberate differences from legacy

- An explicit speaker missing from the cast is a typed error, never a panic.

## History

- The previous README said the legacy imports of the `timeAwarenessEnabled` and `timeOverride` preferences were still missing. `lettuce-app/src/legacy/legacy_direct_conversation_import.rs` reads both now.
- The previous README called runtime resolution of `model_settings` a later slice. Generation input (`lettuce-app/src/generation/conversation_generation_input.rs`) now passes it as the session layer of chat parameter resolution.
- The previous README said conversation checkpoints did not persist `provider_response_id`. The initial inference checkpoint stores the whole `InferenceOutcome`, including that field, and the database compares it with the usage evidence when settling.
- The previous README said the final schema "must" add a partial unique index on `generation_attempts(job_id)`. It exists: `generation_attempts_job_id_uq` in `lettuce-database/migrations/0008_conversations.sql`.
- The previous README described the tool contract as horizontal only, with the application coordinator and the legacy memory, creation, companion and lorebook handler migrations as separate slices.

## Not wired yet

- Adding a character to an existing group conversation (legacy `group_session_add_character`) is not implemented. It needs a member launch snapshot and a participant insert behind a new command, deferred to the command-surface phase.
- Until conversations follow their group live, the speaker-selection fallback is the launch-time group method, not the group's current one.
- The single in-flight turn rule is enforced by the adapters; its supporting index is deferred.
- Several async ports in `ports.rs` (`LaunchResolver`, `SpeakerPolicy`, `ModelResolver`, `MediaPort`, `MemoryPort`, `CompanionPort`, `JobPort`, `Clock`, `ConversationApplication`) have no implementation outside this crate; `lettuce-app` composes the generation flow directly.
