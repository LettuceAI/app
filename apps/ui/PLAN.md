# LettuceAI Frontend Refactor Plan

## Decision

Rebuild the frontend as feature-owned vertical slices over a generated backend client. Preserve the product's visible capability while removing the current architecture in which pages import global storage schemas, call hundreds of Tauri commands, pass native paths, and coordinate backend transactions themselves.

This is not a visual redesign mandate. Existing interaction patterns that work can survive. The architectural redesign makes those screens easier to understand and safer to evolve: one conversation feature for direct and group modes, one asset system, one operation center, capability-driven model controls, and settings grouped by user intent.

## Current application behavior

The current frontend has broad product areas for onboarding, character/persona authoring, direct chat, group chat, memories/companion state, lorebooks/prompts, discovery, image playground/library, model/provider/runtime management, speech, backup/sync, usage, host API, a now-dead Engine integration and developer support. Engine is removal scope, not a target feature.

The largest concentration is settings, with many pages that are actually full product workflows: model discovery/install, Stable Diffusion, LoRA library, Kokoro studio, embedding tests, companion downloads and lore generation. Direct and group chat duplicate layout, history, search, memories, settings and appearance. `App.tsx` owns a very large flat route table. Core storage wrappers expose broad schemas and command-shaped methods. Numerous UI components carry fields named `path` for avatars, attachments, models and LoRAs, making native filesystem representation part of UI state.

The target preserves every feature but changes where it lives and how it communicates.

## Target source structure

```text
apps/ui/src/
  app/
    bootstrap/
    router/
    providers/
    shell/
    error-boundaries/
  api/
    generated/             # generated only; never hand-edited
    client.ts              # transport/error/event setup
    query-keys.ts
    mocks/
  features/
    onboarding/
    characters/
    personas/
    groups/
    conversations/
    memory/
    companions/
    lorebooks/
    prompts/
    models/
    model-hub/
    image-generation/
    media-library/
    speech/
    creation/
    discovery/
    transfer/
    backup/
    sync/
    usage/
    host-api/
    settings/
    diagnostics/
  entities/
    asset/
    job/
    model-profile/
    conversation-summary/
  shared/
    ui/
    forms/
    hooks/
    i18n/
    validation/
    formatting/
    testing/
  styles/
```

`features` may import `entities`, `shared` and generated API modules. `entities` may import `shared` and generated contract types. `shared` cannot import a feature. Features do not import each other's internals; a small public `index.ts` exports supported components/use cases. ESLint dependency rules enforce this.

## Backend access

All backend communication goes through generated feature clients from `lettuce-contracts`. There is no raw `invoke`, command-name constant, handwritten backend response type or generic `storageRepo`.

Each generated operation exposes:

- Typed request and response.
- Stable error code union and safe details.
- Abort/cancellation integration where supported.
- Query/mutation metadata used to create consistent hooks.
- Event correlation types for jobs and streaming.

Feature API files wrap generated calls only to add frontend cache behavior or compose view models. They cannot rename fields into a second domain schema. Runtime validation remains at the external boundary for persisted browser state, URLs and compatibility data; generated IPC results should not be parsed again with a parallel Zod schema.

## State ownership

Use three explicit classes of state:

1. Server/domain state: query cache keyed by IDs/revisions. Lists, details, settings, jobs and catalog pages use generated query hooks. Mutations invalidate or update precise keys.
2. Route/workflow state: selected entity, wizard step, filters and shareable search terms live in typed route params/search params.
3. Ephemeral UI state: open sheets, draft text, hover/selection and unsaved form state stay local or in a feature-scoped store.

Do not mirror the whole backend into a global store. Do not use a context provider as a hidden service locator. Persisted drafts have a named feature repository and schema version. Optimistic updates require an expected backend revision and a conflict rollback path.

## Application shell and routing

Replace the giant route declaration with lazy feature route modules. The shell owns top navigation, global command palette, job center, connection/offline status, update notice and fatal-error boundary. Route loaders prefetch the one read model needed for the screen.

Proposed primary information architecture:

- Home/search.
- Chats: unified conversation list with direct/group filters.
- Characters: characters, personas and groups as adjacent libraries.
- Create: manual and assisted creation.
- Discover.
- Studio: image playground, media library, voices and prompt/lore authoring.
- Models: provider connections, model profiles, installed models, downloads and runtimes.
- Settings: application behavior, appearance, privacy/security, sync/backup, usage, integrations and diagnostics.

Model installation and voice/image studios are no longer buried under “Settings” merely because they configure something; they are workflows. Redirect aliases preserve old deep links during migration.

## Conversation experience

Direct and group screens become one `conversations` feature with mode-specific participant controls. Shared components include conversation shell, virtualized message timeline, composer, attachment tray, generation state, history/search, branch tree, memory panel, settings and diagnostics.

The screen loads a single `ConversationView` containing header, participants, effective feature summary and the initial message page. Older messages paginate by cursor. Search returns message/revision hits with navigation anchors. A group adds participant/speaker controls; it does not switch to a different message component or API.

Sending is one mutation. The UI creates a local optimistic user-message placeholder keyed by the returned operation/correlation ID. Backend events move it through committed -> generating -> candidate available -> selected/complete or failed/cancelled. Streaming deltas live in a bounded external store so every token does not rerender the whole page. On reconnect, the UI queries the generation turn/job rather than assuming the event stream was complete.

Editing creates a revision; regeneration creates a candidate; branching selects a path. The UI labels these concepts consistently instead of treating them as destructive overwrites. Failure cards use stable error codes with retry/copy-report actions. The composer keeps unsent text on failure and never has to manually save attachments into a session folder.

## Media and image behavior

Every persistent visual/audio value is an `AssetRef { id, kind, revision? }`. A shared asset component resolves a controlled asset URL, requests the appropriate derivative and handles pending, quarantined, missing and deleted states. UI code never converts native paths, joins filenames, or asks whether a string is base64 versus a path.

File selection returns an external-file grant token. Upload calls media ingestion and receives an `AssetId` plus validation result. Crop/round/banner editing stores a transform specification and requests a derivative; it does not maintain three independent avatar paths. Drag/drop, clipboard and remote-import entry points share the same ingestion mutation.

The media library queries catalog assets by kind, source, association, date and retention class. It can show “used by” references and a safe cleanup preview. Generated images appear as job outputs and become conversation attachments only through an explicit attach action. The image playground builds its form from `ImageCapabilities`, disables unsupported fields with an explanation, displays per-output progress/failure and preserves the exact request snapshot for “reuse settings.” LoRAs and checkpoints are model artifacts referenced by ID, not paths.

Audio follows the same rule. A synthesized preview may be a rebuildable cached asset; attaching speech to a message promotes/associates it persistently. Playback components receive asset streams and metadata, not managed filesystem locations.

## Character, persona and group authoring

Forms edit explicit draft DTOs and submit expected revisions. Character authored content and appearance are separate form sections but one domain draft. Publishing shows validation problems and creates an immutable version; the UI can compare versions and choose whether conversations pin or follow latest.

Persona authoring clearly distinguishes the application default from a conversation's pinned persona. Group editing manages reusable membership and speaker policy. Session-local muting or participant state is edited in the conversation, not accidentally saved back into the group template.

Import first displays an `ImportPlan`: new entities, conflicts, media, unsupported fields and privacy warnings. The user chooses conflict resolutions, then one commit operation runs. Creation helper produces a reviewable proposal in the same forms, so AI-created and manually-created entities use identical validation and save paths.

## Lore, prompts, memory and companions

The lore editor is a feature shared by character bindings, group templates and standalone library routes. Its trigger preview calls the exact backend evaluator and shows included/excluded reasons and token usage. The frontend does not recreate keyword matching.

Prompt editing uses structured sections with schema-aware controls, validation and a runtime-equivalent preview trace. Advanced raw editing can exist as an expert view but still compiles to the typed template representation.

Memory screens distinguish authored memories, extracted suggestions, archived/superseded facts and retrieval explanation. Users can see source message, scope and why a memory was selected. Companion relationship/soul screens render the event-derived projection and milestone explanation; they do not recalculate domain state in React.

## Models, providers and local runtimes

Split the current settings maze into four concepts:

- Connections: endpoint, authentication status and provider health.
- Model profiles: a user-named configuration used by features.
- Catalog/installations: discover, download, verify, update and remove artifacts.
- Runtimes: installed engines, device configuration, loaded state and health.

Editors render fields from capability/config schemas. Unknown/unverified capabilities are visible. Testing a connection/profile is a job with a result report, not a boolean toast. Downloads appear in the global job center and survive navigation. Removal first shows dependency/lease information.

## Settings and onboarding

Settings pages load one typed section at a time, show whether a value is default or overridden, validate before save, and identify restart-required changes. Save bars are consistent and conflict-aware. Secret fields display configured/unconfigured status and replacement controls but never round-trip the value.

Onboarding is a resumable state machine backed by actual readiness checks: provider connection verified, model profile usable, optional local runtime installed, optional memory/sync configured. It recommends paths but does not create a parallel set of setup APIs. A user can leave and resume without losing verified steps. Existing users migrating to epoch 2 see a dedicated import/reconciliation screen rather than normal onboarding.

## Jobs, progress and errors

A global job center subscribes to one typed event stream and reconciles from `list_jobs` on startup/reconnect. Job views support nested stages, determinate/indeterminate progress, byte rates where meaningful, retry notes, cancellation and recovery-required actions. Feature pages reference job IDs instead of maintaining their own queues.

Errors use stable codes mapped to localized titles/actions. Raw backend strings may appear only in an expandable diagnostic detail if marked safe. Toasts are for completed transient actions, not long failures or decisions. Expected empty/offline/unconfigured states have designed page states; fatal boundaries capture a correlation ID and recovery options.

## Performance and accessibility

- Lazy-load routes and heavy editors/runtimes; do not bundle every settings workflow at startup.
- Virtualize long conversation/catalog/log lists and use cursor pagination.
- Keep streaming deltas outside broad React context; batch visual updates to an animation-frame or fixed cadence.
- Use stable selectors and normalized cache entries to avoid invalidating the full app after one message.
- Every operation works by keyboard and exposes progress/cancellation through accessible live regions without announcing every token.
- Focus returns predictably after dialogs/routes, reduced-motion is honored, contrast is token-tested, and media has meaningful labels/captions where available.
- All user text is locale-backed. Contract error codes map to localized copy with a safe fallback; backend English strings are not UI copy.

## Testing strategy

- Generated-client contract tests against the Rust schema and recorded compatibility fixtures.
- Feature unit tests for view-model transformations and form validation.
- Component tests using generated API mocks, including loading/empty/error/conflict states.
- End-to-end vertical journeys: onboarding; create/import character; direct/group conversation; edit/regenerate/branch; attach media; image generation; speech; model install; backup/restore; sync conflict.
- Accessibility automation plus keyboard-only smoke journeys.
- Visual regression for shared shell, message types, forms, job center and critical responsive layouts.
- Performance fixtures with thousands of conversations/messages/assets/models.

## Migration sequence

### 1. Boundary and shell

Generate API clients around current commands, introduce query/error/job infrastructure, modularize routes and ban new raw invokes. No feature behavior changes yet.

### 2. Assets and shared entities

Introduce `AssetRef`, common media rendering/ingestion, job components, IDs/revisions and model-profile selectors. Compatibility adapters translate legacy paths while screens migrate; adapters are deleted after media cutover.

### 3. Conversation spine

Build the unified conversation feature against new backend read models and mutations. Migrate direct chat, then group chat by enabling participant controls. Retire duplicate group components only after parity for history, search, memories, lore, appearance and settings.

### 4. Authoring and knowledge

Move characters, personas, groups, lorebooks, prompts, memory, companions and creation proposals. Replace storage-shaped forms with domain drafts/import plans.

### 5. AI studios and model operations

Move providers/profiles, model hub, downloads, local runtimes, image playground/library, ASR and TTS to capability/job APIs.

### 6. Operational features

Move discovery/transfer, backup, sync, usage, host API and diagnostics. Delete Engine routes, API wrappers, schemas, onboarding/settings entries and localization made unreachable by that removal. Remove the last generic storage schema/repository and raw native path type.

## Frontend completion gates

- Zero imports of Tauri `invoke` outside the generated transport.
- Zero UI/domain state fields representing managed native paths.
- Zero imports from another feature's internal modules.
- Direct and group conversations use the same timeline, composer and history APIs.
- Every long action is visible and recoverable through the global job model.
- No backend record has a second handwritten TypeScript schema.
- Every route has loading, empty, recoverable error and unavailable-capability behavior.
- Existing locale coverage is retained for all changed user-facing copy.
- Old `core/storage`, duplicate group-chat implementation and compatibility asset-path adapters are deleted before the refactor is declared complete.
