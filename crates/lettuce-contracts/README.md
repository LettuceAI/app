# lettuce-contracts

The serde types that cross the IPC boundary between the Rust backend and the frontend. They are owned transport DTOs, kept separate from the domain and provider types so that an internal change never silently changes what the frontend receives, and so the frontend never has to guess a provider's wire values or feature combinations.

Only the composition crate (`lettuce-app`) and the Tauri shell (`apps/tauri`) depend on it; domain crates must not. It depends on nothing but `lettuce-types` and `serde`, and it holds no behavior.

The `specta` cargo feature derives `specta::Type` on the application API contracts so the Tauri shell can export them as TypeScript bindings (`apps/ui/src/api/generated/bindings.ts`). Only the Tauri shell enables it; no other crate sees specta.

## Application API contracts

The application API (`lettuce_app::api`) speaks only these types. Ids are UUID strings and timestamps are unix milliseconds (exported to TypeScript as `number`, which holds them exactly). No domain type appears in them.

Database-file inventory uses relative file identifiers, nullable creation timestamps and always-present measured modification timestamps. Creation is unknown only for pre-existing files whose filesystem does not expose it; newly created files retain the timestamp recorded before creation.

Media never crosses IPC as bytes, base64 or a data URL, in either direction: a contract names media only by `AssetRef`, and a request that brings in user media will carry a file path (a `content://` URI on Android) that the backend reads, validates and ingests. No contract type has a bytes or base64 field.

Media export takes an asset identifier and FileTarget. Its typed details distinguish missing assets, missing blobs or objects, unavailable media storage, invalid stored metadata and protected export targets.

- `ApiError` is what every call returns on failure: a stable `ApiErrorCode` (`not_found`, `conflict`, `invalid_input`, `unsupported`, `unavailable`, `cancelled`, `busy`, `internal`), an English diagnostic `message` that is never shown to the user, and optional `ApiErrorDetails` (the invalid field, the missing model, the Hugging Face failure (`HfFailure`), or what keeps the local models folder busy). The frontend localizes by code and details.
- `ApiEvent` is the application-wide event the host broadcasts. `GenerationSettled` names a conversation and turn whose generation reached a terminal state, so list views can refresh.
- `AssetRef` names a stored media asset by id; the host serves its bytes under the `lettuce-asset://` URI scheme.
- Conversations (`src/conversations.rs`): `ConversationsListRequest` and `ConversationPage` of `ConversationSummary` (kind, title, character avatars, last message preview, update time); `ConversationOpenRequest` and `ConversationView` (participants as `ParticipantView`, the active `BranchHead`, the newest `MessagePage`, the pending turn and `can_send`); `ConversationMessagesRequest` for older pages; `TimelineMessage` with text and media parts (`MessagePartView`), reasoning and the shown candidate's ordinal and count; `ConversationSendRequest` (its `client_operation_id` is the idempotency key) and `SendAccepted`; `GenerationEvent` (`started`, `delta`, `completed`, `failed` with a `GenerationFailureCode`, `cancelled`), each carrying its turn id; `GenerationCancelRequest`; `LaunchDirectRequest` and `LaunchDirectResponse`; `CharactersListRequest` and `CharacterPage` of `CharacterSummary`.

- Local models (`src/local_models.rs`, `src/hugging_face.rs`, `src/ollama.rs`): the llama.cpp devices, fit estimate and embedded template for a model file with the editor's unsaved `LlamaSettingsDraft` or a saved model (`LlamaModelTarget`); the models folder's `LocalModelFile`s with the models using each one, deletion, adoption and the folder switch; `LocalFileRunnability`; the Hugging Face browser's search, files, README, author, avatars, runnability, recommendation (with the download planner's limits), download request (`HfDownloadRequest` with its setup and `client_operation_id`) and `HfTokenStatus`; the Ollama model store and pull request. Jobs carry a `JobSubjectDetail` and the local model results.

## Provider contracts

The provider contracts in `src/lib.rs` describe the catalog, model discovery and saved-account verification:

- `ProviderCatalogContract` is the list the provider settings screen renders. Each `ProviderDescriptorContract` gives a provider kind's display name, `ProviderProtocolContract`, aliases, default endpoint and whether it can be edited, whether an API key is required, optional or unused and which header carries it, and what the provider supports: streaming, native tool translation, structured output, signed tool replay, reasoning together with tools, model listing, key verification, the `ReasoningSupportContract` (none, effort, budget only, dynamic), the `PromptCachingSupportContract` (none, supported, automatic) with the exact `PromptCacheRetentionContract` choices it accepts, which sampling parameters it takes (`ProviderParameterSupportContract`) and the extra request body keys it allows. Features are listed separately so the frontend never infers an unsafe combination from a protocol name or a model capability, and retention choices are typed so it never infers provider wire values.
- `ProviderAccountRequest` names an account for account-scoped calls.
- `ProviderModelsContract` returns the models a provider account lists, each a `RemoteModelContract` (id, display name, description, context length, input and output modalities, supported endpoints, prices).
- `KeyVerificationContract` reports whether an account's key verified, with the HTTP status when there was one.

`lettuce-app` builds these from the provider catalog in `lettuce-providers` (`generation/provider_runtime.rs`).

Branch management DTOs describe creation-ordered branch rows and revisioned fork, rename and select requests. Mutation responses return the branch id and the revision from the committed operation, so replay returns the same response after later writes.

Conversation views return nullable origin conversation and message ids for provenance. These fields are plain identities and do not require the source to remain present.

`ConversationDuplicateRequest` selects the source, optional title and whether messages are copied. `ConversationCopyResult` carries the stable new conversation identity and the revision committed with its receipt.

Character-copy requests carry source conversation, selected message and target character IDs plus an operation key. Whole-group copies omit the message boundary. New-group copies carry an ordered distinct member list, including the source owner. All return the stable new conversation ID and revision.

Manual memory requests carry the conversation, expected memory revision and operation key. Update uses explicit keep/set choices for nullable categories and observed time; summary editing distinguishes set from clear. Mutation results return the original committed revision and item id for exact replay.

Memory views expose nullable counts, authored origins, cycle labels and status with typed model, embedding, provider and lease-loss failures. The read request identifies a conversation; its active branch determines the selected own space or pool.

Forced memory cycles take the conversation and an operation key; a retry may name a summarisation model. A cycle that cannot start is refused with `ApiErrorDetails::MemoryGate`, naming whether the conversation lacks dynamic memory, the global switch is off, there is no dialogue to summarise, or a cycle already runs. Cancelling stays `job_cancel`.

`memory_cycles` pages the activity log newest first: each cycle has its window label, outcome actions, published summary, status, typed failure, `reverted`, `revertable` and the later cycle that blocks a revert. `memory_cycle_revert` takes the run, the expected memory revision and an operation key; a cycle a later cycle started from is a `Conflict` with `ApiErrorDetails::MemoryCycleDependent` naming that cycle. `memory_error_dismiss` hides the failure the status shows.

Companion commands return the authored Soul configuration as a document next to the growth facts of the Soul in effect (`companion_soul_get`), edit growth by fact id, queue the Soul writer as a job whose result is a `CompanionSoulDraft` document, and manage scheduled notes with API-assigned ids and timestamps. A missing character is `NotFound` and a character that is not a companion is `Unsupported`.

A revert refused because the user edited an item the cycle changed carries `ApiErrorDetails::MemoryCycleUserEdited` with that memory's id.

`companion_soul_get` returns the authored Soul configuration as a typed `CompanionSoulConfigView` (identity text, baseline affect, regulation style, authored facts, relationship defaults, prompting and the sharing toggles) derived from the companion domain type, and the Soul writer takes and returns the typed `CompanionSoulDraft`. Contracts hold no free-form JSON for them.

`ApiEvent::MemoryChanged` names a conversation whose `memory_get` result changed.

Generation streams include `SpeakerSelecting` and `SpeakerSelected` with the resolved character id before text deltas. The API replays the resolved speaker to a late stream attached to a running turn.

Local runtime notices carry a typed code on a generation or job stream. `JobEvent::Throughput` carries generated tokens and tokens per second without duplicating text deltas. `ApiEvent::LocalModelRuntimeReportChanged` names the local model profiles whose stored report changed.

Turn and job `ModelLoading` events carry typed `ModelLoadStage` and `ModelLoadStatus`, an integer overall percentage, the model name and optional `ModelLoadGpuProgress` rows. Loaded and Failed remain explicit terminal load statuses, separate from generation or job settlement.

Lorebook and prompt DTOs carry aggregate revisions and operation keys, typed configured-source failures, preview explanations, staged project state and historical source names with deleted markers. Generator commands return existing job DTOs rather than inline inference results.

Provider control commands export the catalog, secret-free account views, saved-or-draft verification requests, verification results and public OpenRouter endpoint metadata to TypeScript. Verification errors carry HTTP status and a redacted provider message in ApiErrorDetails so the UI can display the provider's reason. TrustedCertificateView contains identity, filename, import time and validity metadata.

Provider control requests expose draft or saved verification, revisioned account writes with operation ids, cascading deletion, model listing and existence checks, public OpenRouter endpoints and FileSource certificate import. Views carry key presence and certificate metadata with its list revision. InUse and Malformed are explicit error categories; provider errors redact credentials.

Stored certificate views include validity and a typed InvalidPem reason; runtime client construction skips invalid stored roots so users can list and remove them. New imports validate strictly before writing.

Duplicate certificate imports return Conflict with CertificateAlreadyImported and the existing certificate id, separate from operation-id conflicts.

Provider verification preserves string messages, string error types and JSON-stringified error values with credential redaction. Missing provider text uses typed MissingApiKey or InvalidApiKey details and an absent provider_message.

Model profile contracts expose catalog/default revisions, profile configuration, declared modality scopes and optional listing metadata. Save, duplicate, delete and default selection carry operation ids and CAS revisions. Duplicate requires the display name authored by the caller. NanoGPT usage returns account metadata and nullable quota/subscription fields; ProviderQuota carries a typed warning level and account id, and ProviderQuota error details carry a typed failure plus redacted provider status/message.

ProviderQuotaLevel maps NearLimit to 75 percent, AlmostExhausted to 90 percent and Exhausted to 100 percent. Only the highest crossed threshold is delivered for one check.

Model views derive input and output scopes from Supported capability statuses. Saves without remote metadata replace Supported declarations; declaring an explicit Unsupported modality fails typed before writing. Reported metadata replaces the side it supplies. Model deletion publishes a models settings change when it clears or promotes a stored selection.

NanoGPT usage requests choose refresh explicitly: cached reads reuse a successful account result within 300 seconds, stale reads fetch, and refresh forces a fetch; both join an active check.


Capability statuses are the only model scope source. Editor saves reject declared Unsupported modalities before writing. A reported metadata side marks listed modalities Supported and unlisted modalities Unknown unless explicitly Unsupported; an unreported side uses editor declarations.

Explicit cached quota reads retry failures immediately, allowing credential repairs to take effect. Background completion checks still coalesce failures for 300 seconds; a running check is shared by both paths.

Settings commands use closed section variants rather than JSON patches. SettingsView returns global preferences, device configuration metadata, model defaults and the shared settings revision. UI preference changes name a known key and its typed value; a null value removes that key. The creation-helper section is readable but has no update variant. Sampler defaults carry the complete typed model settings layer.

ContentFilterLogView contains redacted hit records with the level, score, terms and timestamp. ContentFilterHit announces a coalesced change through the application event channel. Settings error details distinguish revision conflicts, invalid stored data, missing model references, storage failure and the developer-mode gate.

Settings command inputs retain deserialization failures until the API maps them to InvalidInput with field details, so unknown keys and invalid enum choices use the same typed error channel as range validation. Their exported TypeScript shape stays the closed request DTO.

SettingsDeviceView.revision is the CAS token for DeviceEmbedding patches. Global section patches and sampler defaults use SettingsView.revision; the device revision never changes the synced app-settings identity.

SettingsChanged.section uses the global SettingsPatch section names and sampler_defaults for global settings, models for model-selection writes, device for every device_settings write (embedding, certificates and folder relocation), and ui_state for device UI state. Catalog-only writes publish ModelsChanged without a settings event.

Log viewer contracts carry names, paged lines and match indices; exports use FileTarget. DeveloperLogLine carries a written line, and AppUsageChanged invalidates the app-time read. AppUsageDaysView carries per-day milliseconds including the current focused stretch.

LlmMetricView holds the generation id, creation time, summary and optional samples. Metrics listing uses an opaque cursor and bounded page size; clearing takes a client operation id and returns its committed count.

Usage clearing carries a timestamp cutoff and client operation id and returns the removed row count. Usage storage failures use typed error details; contract values do not expose database paths.

Storage file contracts expose basenames, creation kind and time, size, active status and deletion availability. Explicit deletion carries a client operation id; database-file error details name only the requested basename, with no native path.
