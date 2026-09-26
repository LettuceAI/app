# lettuce-contracts

The serde types that cross the IPC boundary between the Rust backend and the frontend. They are owned transport DTOs, kept separate from the domain and provider types so that an internal change never silently changes what the frontend receives, and so the frontend never has to guess a provider's wire values or feature combinations.

Only the composition crate (`lettuce-app`) and the desktop shell (`apps/desktop`) depend on it; domain crates must not. It depends on nothing but `lettuce-types` and `serde`, and it holds no behavior.

The `specta` cargo feature derives `specta::Type` on the application API contracts so the desktop shell can export them as TypeScript bindings (`frontend/src/api/generated/bindings.ts`). Only the desktop shell enables it; no other crate sees specta.

## Application API contracts

The application API (`lettuce_app::api`) speaks only these types. Ids are UUID strings and timestamps are unix milliseconds (exported to TypeScript as `number`). No domain type appears in them.

- `ApiError` is what every call returns on failure: a stable `ApiErrorCode` (`not_found`, `conflict`, `invalid_input`, `unsupported`, `unavailable`, `cancelled`, `busy`, `internal`), an English diagnostic `message` that is never shown to the user, and optional `ApiErrorDetails` (the invalid field). The frontend localizes by code.
- `ApiEvent` is the application-wide event the host broadcasts. `GenerationSettled` names a conversation and turn whose generation reached a terminal state, so list views can refresh.
- `AssetRef` names a stored media asset by id; the host serves its bytes under the `lettuce-asset://` URI scheme, so no media crosses IPC as base64.
- Conversations (`src/conversations.rs`): `ConversationsListRequest` and `ConversationPage` of `ConversationSummary` (kind, title, character avatars, last message preview, update time); `ConversationOpenRequest` and `ConversationView` (participants as `ParticipantView`, the active `BranchHead`, the newest `MessagePage`, the pending turn and `can_send`); `ConversationMessagesRequest` for older pages; `TimelineMessage` with text and media parts (`MessagePartView`), reasoning and the shown candidate's ordinal and count; `ConversationSendRequest` (its `client_operation_id` is the idempotency key) and `SendAccepted`; `GenerationEvent` (`started`, `delta`, `completed`, `failed` with a `GenerationFailureCode`, `cancelled`), each carrying its turn id; `GenerationCancelRequest`; `LaunchDirectRequest` and `LaunchDirectResponse`; `CharactersListRequest` and `CharacterPage` of `CharacterSummary`.

## Provider contracts

The provider contracts in `src/lib.rs` predate the application API and are not exported to TypeScript yet:

- `ProviderCatalogContract` is the list the provider settings screen renders. Each `ProviderDescriptorContract` gives a provider kind's display name, `ProviderProtocolContract`, aliases, default endpoint and whether it can be edited, whether an API key is required, optional or unused and which header carries it, and what the provider supports: streaming, native tool translation, structured output, signed tool replay, reasoning together with tools, model listing, key verification, the `ReasoningSupportContract` (none, effort, budget only, dynamic), the `PromptCachingSupportContract` (none, supported, automatic) with the exact `PromptCacheRetentionContract` choices it accepts, which sampling parameters it takes (`ProviderParameterSupportContract`) and the extra request body keys it allows. Features are listed separately so the frontend never infers an unsafe combination from a protocol name or a model capability, and retention choices are typed so it never infers provider wire values.
- `ProviderAccountRequest` names an account for account-scoped calls.
- `ProviderModelsContract` returns the models a provider account lists, each a `RemoteModelContract` (id, display name, description, context length, input and output modalities, supported endpoints, prices).
- `KeyVerificationContract` reports whether an account's key verified, with the HTTP status when there was one.

`lettuce-app` builds these from the provider catalog in `lettuce-providers` (`generation/provider_runtime.rs`).
