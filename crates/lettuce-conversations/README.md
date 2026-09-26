# lettuce-conversations

The domain model for direct and group conversations: the conversation and its launch snapshots, branches, messages with their revisions and candidates, the generation turn and its attempts, tool executions, and the command and repository contracts that change them.

The crate is pure. It validates state, defines the commands and describes persistence as synchronous, transaction-shaped repository traits; it never opens SQLite, calls a provider or runs a job. The pieces around it:

- `lettuce-database` implements the repositories, including the artifact store and the checkpoint repositories.
- `lettuce-app` plans launches, assembles context, resolves models, runs the provider call, drives tools and finalizes turns.
- `lettuce-providers` and `lettuce-inference` consume `InferenceRequest` and produce `InferenceOutcome`; their family codecs translate the provider-neutral tool and context values.
- `lettuce-memory` owns memory semantics. This crate stops at admitting tool calls and tracking their lifecycle.
- `lettuce-transfer` uses `ConversationAggregate` as the typed root of the encrypted conversation-history document and adds global limits, timeline ordering and cross-record checks.

The public surface is kept small. Invariants live in the domain types and their `validate` methods; infrastructure is reached only through narrow ports. There is no schema, migration, provider adapter, frontend or engine client here.

## Structure

| Module | What it holds |
| --- | --- |
| `model.rs` | `Conversation`, `ConversationKind`, participants, branches, current settings, `ConversationAggregate`, `ConversationHistory` |
| `content.rs` | `Message`, `MessageRevision`, `MessageCandidate`, `MessagePart`, annotations, replay references |
| `snapshot.rs`, `document.rs` | Launch snapshots and the versioned documents stored inside their protected artifacts |
| `artifact.rs` | Protected bytes, the artifact store, replay and transfer ports, `PreparedConversationLaunch` |
| `generation.rs` | `GenerationTurn`, `GenerationAttempt`, their state machines, checkpoint and stream envelopes |
| `commands.rs` | Command structs, `ConversationMutation`, the settings patch |
| `ports.rs` | Repository traits, outbox events, operation records, context and inference contracts, usage |
| `service.rs` | `ConversationManager`, the thin façade over a repository |
| `tool.rs` | Tool definitions, requests, proposed calls, transcript values, `ToolExecution` |
| `initial_inference.rs`, `speaker_inference.rs` | Durable checkpoints for the first provider call and for LLM speaker selection |
| `speaker_policy.rs` | Deterministic group speaker selection and `@mention` parsing |
| `effective_settings.rs` | Resolves launch values against conversation overrides |
| `clock.rs` | `CompanionClockSettings` and effective time |
| `scene_tag.rs` | Extracts the `<img>…</img>` scene prompt from a reply |

## Conversations

A `Conversation` has a lifecycle (`Active`, `Archived`, `Tombstoned`), a title, a kind, participants, the active branch, a revision and an optional `CurrentConversationSettings`. `ConversationKind` is a tagged union of `Direct(DirectConversationDetails)` and `Group(GroupConversationDetails)`: one aggregate type covers both, so storage, transfer and history code do not fork on chat type.

The kind holds the launch snapshots: what the character, persona, scene, prompt, lorebooks, model, memory and voice settings were when the conversation was created. Each is a `SnapshotSelection` (`Inherited(value)`, `Explicit(value)` or `Disabled`); inherited selections carry the fully resolved value, not a pointer back to the source. Snapshots never change for the life of the conversation.

What the user changes afterwards lives in `CurrentConversationSettings`, a separate revisioned record. Each overridable field has a `SettingProvenance` (`LaunchInherited`, `CurrentOverride`, `Disabled`). `resolve_effective_settings` combines the two for one turn: a current override wins, `Disabled` yields nothing, and otherwise the launch value applies. The current settings hold no live values derived from other conversations or sources; the only inputs are the launch snapshot and the explicit overrides.

For a group, effective settings are resolved per speaker. The member's prompt falls back to the group's, the participant's model to the group model, the group's scene applies only in roleplay chat mode, and lorebooks are the group's books (or the conversation's own selection, which replaces only those) followed by the speaker's books unless the group disables character lorebooks. `effective_memory`, `effective_persona` and `effective_speaker_selection` answer the same question without a speaker.

Current settings also carry:

- `model_settings`, a `lettuce_models::ModelSettingsLayer` of chat parameters, llama.cpp and stable-diffusion settings. Each unset field defers to the model and then the app settings. It is stored as `conversation_settings.model_settings_json` (NULL when empty) and travels with the conversation in backups; generation input passes it as the session layer of chat parameter resolution.
- `background`: `Image { asset_id }` or `Hidden`. `None` follows the selected scene, then the character (direct) or group. It is stored as `background_asset_id` and `background_hidden`, the asset must be an image, and deleting a referenced asset is restricted. It travels in backups and sync with its media.
- `companion_clock` for direct conversations (see Time below).
- For groups, `speaker_selection`, `chat_mode` and `disable_character_lorebooks` as overrides of the group's launch values.

The speaker-selection patch can set a method or return to the one the group was launched with; it cannot be cleared, and setting it on a direct conversation is rejected. `effective_speaker_selection` falls back to the launch-time group method.

### Participants

A participant has a role (`User`, `Character`, `System`), an ordinal, enabled and muted flags, a typed `ParticipantSource`, and the display name, description and model selection it had at launch. A group has exactly one user participant, at least one character and no system participant, and its initial participant policy lists one entry per character. `UpdateParticipantPolicy` changes the enabled, muted and model-override values later.

## Branches and messages

A conversation is a tree of branches. The root branch has no parent; every other branch names its parent and the `fork_message_id` it was forked at, and the two must be present together. `ConversationAggregate::validate` checks there is exactly one root, the active branch exists and is active, and a bounded walk proves the parent links contain no cycle. A freshly forked branch has no head; its timeline is the inherited ancestry up to the fork message. `ConversationHistory` carries the full root-to-branch ancestry so validation can tell a legal fork prefix from an unrelated sibling.

A `Message` belongs to one branch and links to its parent message. It has a role, a logical time, an effective time, visibility (`Visible`, `Hidden`, `Tombstoned`), pinned and scene-edited flags, and an `active_render_source` that points at either a revision or a candidate. User and assistant messages need an author participant; system and scene messages must not have one.

The content itself is never edited in place:

- A `MessageRevision` is an immutable version of the message's parts, numbered by sequence, optionally tied to the turn that produced it and to a provider replay artifact.
- A `MessageCandidate` is one assistant variant produced by a turn attempt. It records the speaker, the model snapshot and the parts. Choosing a candidate sets the render source and restores its author onto the message.

Editing an assistant reply rewrites the selected variant: the new revision records `supersedes_candidate_id`, selecting that candidate renders the latest revision that supersedes it, and the candidate's own pre-edit parts stay only as history. A variant list shows the superseding revision's text at that candidate's position, and regenerating an edited reply appends one new variant.

`MessagePart` is a closed set: text, media asset (with a role such as inline, attachment, avatar, scene or reference), reasoning summary, tool call and tool result (both by `ToolExecutionId` only) and annotations (scene edited, safety, system notice, companion effect). All documents and parts are bounded and deserialize with `deny_unknown_fields`.

`Scene` is its own authorless role. Request assembly must not turn it into a provider `system`, `user` or `assistant` message; scene text reaches the model through the scene-context path of prompt rendering.

## Launching a conversation

1. The application's launch planner resolves every selected source, builds the snapshot documents (`build_snapshot_draft` over the versioned `*SnapshotBodyV1` types) and the `CreateConversationPlan`, including the initial timeline.
2. The initial timeline is a bounded, versioned list of `InitialMessageDraft`s: the selected scene and, for direct chats, the starter messages. Each becomes an ordinary immutable message revision with a stable id and an `InitialMessageOrigin` naming the protected scene or starter snapshot it came from.
3. The planner packs the plan and exactly one `SnapshotArtifactDraft` per distinct snapshot reference into a `PreparedConversationLaunch`. It walks both the launch settings and the initial-message origins (`conversation_launch_snapshot_references`) and rejects the bundle if plan and drafts do not describe the same set.
4. `ConversationCreator::create` consumes the bundle in one adapter transaction: it stages the artifacts, revalidates the plan and inserts the aggregate. There is no separate pre-verification step that could race with staging.

`PreparedConversationLaunch` is neither `Clone` nor serializable. Only one owner holds the protected drafts, so staging and cleanup are unambiguous and the bytes cannot leak through serialization. An origin row proves which snapshot was selected, not that the text came from it, which is why only the planner builds the timeline from the same validated scene and starter graph used to create the artifacts. IPC callers never construct initial timelines, and the creator only persists and revalidates.

Snapshot documents are structural copies of authored values: media is an `AssetId` link, and secrets, provider auth, endpoints and native paths never enter them (`SnapshotProviderDescriptorV1` is the non-secret half of a provider account). Launch documents carry a format version. Dynamic memory snapshots may carry the exact retrieval and mutation policy used by generation; the field is optional so older documents stay readable, new dynamic launches fill it and manual or disabled modes reject it. An LLM-selected group may carry a dedicated speaker-selection model snapshot, valid only with the LLM method, which freezes that model and its provider account independently of the members' models.

## Artifacts

Launch snapshots and provider replay payloads are stored as immutable artifacts. Conversation rows hold only references: `ProtectedSnapshotRef` (source, source revision, artifact id, digest, schema version, size) and `ReplayArtifactRef` (id, digest, schema, codec, retention, size). No provider payload bytes appear in rows or public DTOs.

The bytes travel as `ProtectedArtifactBytes`: bounded (128 MiB), redacted in `Debug`, not serializable and zeroized on drop. Three separate capabilities reach them:

- `ConversationArtifactStore`, reachable from the full repository, puts snapshot and replay payloads immutably by identity and digest (the same payload deduplicates, a different one conflicts) and offers verification and orphan cleanup. It is used for launch, replay finalization and retention work.
- `ProviderReplayArtifactPort` lets a trusted provider adapter stage, verify and materialize exact provider-native replay bytes, for example signed reasoning that must be sent back verbatim.
- `ConversationArtifactTransferPort` streams chunks into a `TrustedArtifactSink` for encrypted backup and sync. It is deliberately not reachable through `ConversationRepository::artifact_store`, and backup exports reuse conversation-retained references instead of creating a second retention identity.

Ordinary repositories have no artifact read-back, and IPC never carries raw artifact bytes. `ConversationSnapshotMaterializer` decodes one launch snapshot through the conversation's own reference and never resolves live source rows.

## Mutations

Every mutation is a command carrying the expected revision and an `OperationToken` (idempotency key plus request digest). `ConversationMutation` enumerates them: send, continue, regenerate, retry, cancel, choose candidate, edit, flags, fork, select branch, tombstone, archive, restore, rename, participant policy and settings.

A repository returns `MutationCommit<T>`: the value, the `OperationRecord` and the outbox records, all written in one transaction. Replaying the same operation returns the original operation and outbox records with the value rehydrated from current state; the same key with a different request conflicts. Reads return plain values and create no operation or outbox record. Mutations other than restore require an active conversation, and the begin methods require that no non-terminal turn exists, so a conversation has at most one turn in flight.

Some command details:

- `ForkBranch` forks at a message or, without one, at the source head; forking a headless branch is a conflict.
- `TombstoneMessage` takes a `DescendantPolicy`: `Preserve`, `Tombstone` or `Fork`. Tombstoning descendants leaves the branch head where it is, because a tombstone is a flag and the timeline still renders the entries. The policy is branch-local; cross-branch descendants belong to `Fork` or branch archival.
- `ArchiveConversation` is metadata only; an in-flight generation keeps running.
- `append_user_message` adds a user message without starting a reply. A group director uses it and then continues with a forced speaker.

Outbox events (`ConversationCreated`, `MessageCommitted`, `MessageRevised`, `MessageTombstoned` and the turn events) carry typed ids, stages and counters, plus `AssetReferenceDelta`s that tell the media crate which assets became active, historical or released. Terminal turn events carry the attempt, usage, message and candidate references and bounded memory revision references, so consumers can be idempotent by turn id without reading conversation internals.

## Generation turns

A `GenerationTurn` is the durable record of one reply. It stores the operation (`Send`, `Continue`, `Regenerate`), the input (the new user message, the existing head or an existing candidate), the target (a new assistant message with its parent, or a new candidate on an existing message), guidance, the requested model override, a forced speaker, and after preparation the resolved model, the selected speaker and the prompt, lorebook and memory attributions. The target belongs to the turn so retry and recovery can reconstruct the exact message and candidate identity without the original command.

The turn state machine:

```
Created → Preparing → [SelectingSpeaker] → ContextPrepared → Running → Finalizing → Succeeded
                                                                              ↘ Failed | Cancelled | Interrupted → Recovering
any live state → CancellationRequested → Finalizing | Cancelled | Failed | Interrupted
```

A retry is a new turn linked by `retry_of_turn_id` to the terminal turn it replaces.

Each turn has one or more `GenerationAttempt`s, with their own states, candidates, usage event and failure code. An attempt owns its scheduler key, `attempt_job_idempotency_key(turn, attempt)`, so retries and recovery children never coalesce with a parent's job. Attaching a job is its own CAS (`AttachAttemptJob`). Attachment is repository-wide: an attempt is attached once, and a job id belongs to at most one attempt across all conversations, reported as `JobAlreadyAttached` or `JobInUse`. The database enforces the second rule with a partial unique index on `generation_attempts(job_id)`.

### One turn, step by step

1. Begin. `begin_send`, `begin_continue`, `begin_regenerate` or `begin_retry` creates the turn and its first attempt (and for a send, the user message) and returns `BeginGeneration`.
2. Job. The application starts a job with the attempt's key and attaches it.
3. Speaker (groups only). A forced speaker, an `@mention`, a director target or the original author of a regenerated reply is used directly. Heuristic and round-robin selection are computed by `select_group_speaker`. LLM selection goes through the speaker inference checkpoint. `resolve_group_speaker` stores the decision.
4. Prepare. `PrepareGeneration` binds the resolved model and the context attributions to the turn, checks the attached attempt and job and their revisions, and advances to `ContextPrepared` atomically. Replaying the same preparation is idempotent; a different one conflicts. The database checks the provenance of the attached model artifact.
5. Dispatch. The first provider call goes through the initial inference checkpoint (below). Stage checkpoints advance the attempt from created to preparing to running in the same transaction as the turn stage, so the stored attempt state matches what tool workflows read.
6. Tools. If the reply proposes tool calls, they are admitted as `ToolExecution`s and continued by the application.
7. Finalize. `finalize_generation` takes the candidate drafts and usage and writes the assistant message or new candidate, the revision, the outbox events and the terminal state in one transaction. `fail_generation` and `interrupt_generation` record the other endings; `recover_generation` starts a child attempt after an interruption.

Preparation stores provenance; the initial dispatch checkpoint stores the full resolved provider-neutral request. Between them a turn can be replayed or recovered after the model or the context sources have changed.

### Checkpoints and streaming

Durable progress uses `GenerationCheckpointEnvelope`: turn, attempt, job, correlation id, a sequence that starts at one and increases by one, and an event (stage, progress count, candidate ready, usage recorded, completed, failed, cancelled). Checkpoints are operation-bound, and `latest_checkpoint_sequence` lets a producer continue after the events already stored, including after a restart.

Streaming text is a different channel. `GenerationStreamEventEnvelope` carries text and reasoning deltas, is not serializable and must never reach a repository or the job port. Durable records carry stages, counters and ids, never prompt or reply text.

### Cancellation

Cancellation has two steps. `request_cancellation` commits `CancellationRequested`; the application asks the job runtime to stop; `settle_cancellation` commits the terminal `Cancelled` state together with its usage. A turn stopped after its reply had streamed visible text or a whole image instead finalizes that partial reply from `CancellationRequested`, as a new assistant message or a new candidate for a regeneration, while its job still ends `Cancelled`.

### Initial inference checkpoint

`InitialInferenceBinding` fingerprints the prepared request with BLAKE3 over the conversation, turn, attempt, operation, resolved profile, context, job, media grants and tools. The stream sink is left out, so a reattached caller produces the same binding and replays the same dispatch.

`InitialInferenceRepository` admits one pending dispatch per attempt and settles it once, with either the exact `InferenceOutcome` or a typed `PortError`. It returns the stored request by exact binding or by attempt ownership for recovery; the stored request omits the stream sink, and reads check its fingerprint again before use. A changed request conflicts, a pending record is reported as pending instead of being dispatched again, and a settled response keeps its conversation-retained replay references consistent. Interpreting the response and admitting tools are separate steps.

### Speaker inference checkpoint

LLM speaker selection has its own checkpoint. The binding covers the conversation, turn, attempt, job, resolved profile, prompt context and the required tool request. Admission returns the usage event id for the actual provider call; settlement stores only the validated speaker decision. A pending record blocks a second dispatch, and a settled record replays the decision without keeping a second copy of the provider response.

## Group speaker selection

`select_group_speaker` handles the deterministic methods. A forced speaker or a mention source wins if it is an eligible participant; an unknown or ineligible explicit speaker is an error, never a panic. `Director` and `DirectorAction` then require an explicit speaker, and `Llm` requires an external decision.

- Heuristic scores every eligible, unmuted participant: a base score, a bonus for speaking less than an even share and a penalty for speaking far more, then a recency adjustment (never spoke: bonus; spoke last or one message ago: penalty; three or more ago: small bonus). Recency counts the visible messages after the participant's last line, excluding the user message a send is answering. The highest score wins, first on ties.
- Round robin continues after the prior speaker when that speaker is still selectable and otherwise starts at the first selectable member.

`mentioned_participant` parses `@"Full Name"` first (exact, case-insensitive) and then `@Word` (exact, then prefix, case-insensitive, trailing punctuation trimmed). Candidates are in priority order, and within each step the first match wins.

The method is read when the turn's job runs, not when it is queued. Switching to director mode while a turn without a chosen speaker is queued fails that turn as speaker-unavailable. When a conversation switched to LLM selection has no speaker-model snapshot, the application resolves the model live; a speaker-model snapshot is unused while the method is not LLM.

## Tools

The crate owns the provider-neutral side of tool calling: bounded `ToolDefinition`s, a `ToolRequest` with a `ToolChoice` (auto, required or named), `ProposedToolCall`s from a provider, the transcript values `TranscriptToolCall` and `TranscriptToolResult`, and the attempt-scoped `ToolExecution` lifecycle. Handlers and provider JSON live elsewhere: the remote OpenAI-envelope, Anthropic, Gemini and Ollama adapters have family-specific codecs for declarations, transcripts, buffered responses and streams, and application code runs the handlers.

A proposal is persisted before anything executes. `ConversationManager::request_tool_executions` validates the call set against the exact declared request, assigns stable wire ordinals and handler versions, and stores the whole set atomically. Undeclared tools, a named-choice mismatch, duplicate provider call ids, over-limit sets and an empty set under a required choice all fail before storage. A later continuation round appends after the existing executions with an expected next ordinal, so a recovery worker cannot interleave a second copy of the round.

The execution state machine is `Requested → Validated → Running → Succeeded | Failed | Cancelled | Interrupted`, with `Rejected` and `Cancelled` also reachable before running. Terminal states cannot regress. A handler round moves through `transition_tool_execution_batch`, a typed batch CAS that requires one conversation, turn and attempt owner and unique execution ids; a stale revision, invalid transition or storage failure rolls back every execution, so nobody sees a partly running or partly settled round.

The provider's call id is kept apart from the stable `ToolExecutionId`, so providers without native ids do not force invented wire identities. Message parts reference only the execution id. `context_with_settled_tool_round` appends one settled round to a context as assistant calls followed by matching user results, leaving the original untouched if anything is inconsistent. Only memory, creation, companion and lorebook handlers exist; the contract does not imply shell, filesystem, plugin or general chat tools.

## Context and inference contracts

`ports.rs` also defines the values that cross from conversation state to a provider:

- `ProviderNeutralContext`: messages with text, media, tool call and tool result parts, plus prompt, lorebook and memory attributions and a budget report. Validation checks that tool calls sit on assistant messages, results on user messages, and every result matches an earlier call by execution id, name and provider call id.
- `InferenceRequest`: turn, attempt, operation, a `ResolvedInferenceProfile` (chat profile, tool policy, output policy, safety context), the context, the job used for cancellation, an optional stream sink, media grants, tools and a prompt cache key that lets a local runtime reuse the previous turn's prompt. The tool policy and the presence of tools must agree, and a required policy needs a required or named choice.
- `InferenceOutcome`: candidates (parts, tool calls, replay reference and any images returned as `GeneratedMedia`, which finalization stores as assets), usage, the normalized and provider-native finish reasons, the HTTP `provider_request_id`, the separate `provider_response_id` (used by OpenAI-compatible adapters for billing lookups; not an attempt or candidate identity) and warnings. Only a single-candidate outcome may carry tool calls.
- `InferenceUsage` keeps an optional provider-reported cost as `ProviderReportedCost`, a validated finite, non-negative value. JSON round trips keep the f64 precision, an unknown amount stays distinct from zero, and invalid numbers cannot enter the type, so usage records keep their equality.
- `PromptRuntimeFacts` and `PromptRuntimeValues` feed prompt conditions and pre-resolved runtime strings; `None` means unavailable and the assembler must not invent a replacement. `ContextAssemblyError` variants carry no authored text.

The async ports (`ContextAssembler`, `InferencePort`, `UsagePort` and the others) describe the runtime dependencies of a generation workflow. `lettuce-app` implements the ones it uses and composes the flow itself.

## Time

`CompanionClockSettings` lets a direct conversation set its own clock. Time awareness is off by default, and the override is `Live`, `Frozen { anchor_at }` or `Ticking { anchor_at, set_at }`. `effective_now` returns real time when awareness is off or live, the anchor when frozen, and the anchor plus non-negative elapsed real time when ticking; negative anchors are rejected. Clearing or resetting restores the live, disabled default without touching other settings. This is conversation data, not part of a character snapshot.

The database stamps each new message of a companion direct chat with this effective time. The application reads it for prompt time values, history timestamps, memory lines, the memory cycle's time awareness and temporal-range retrieval.

## Scene tags

`extract_scene_prompt` removes every `<img>…</img>` scene tag from a direct chat reply and returns the cleaned text and the first non-blank prompt. A tag closes at `</img>`, `[continue]` or `[/continue]` in any case; an unclosed tag drops the rest of the text.
