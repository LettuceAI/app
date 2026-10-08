# LettuceAI UI

The React frontend the Tauri shell (`apps/tauri`) loads. It is built with Vite, React 19 and TypeScript in strict mode, styled with Tailwind CSS v4 and the `@lettuceai/crisp` component library, and managed with bun. [PLAN.md](PLAN.md) describes the target product structure; this file describes what is built.

## Layers

`src/app/` composes the application: `bootstrap/` creates the runtime (API client, query client, i18n instance, router) and renders it, `providers/` holds the provider stack, `router/` builds the code-based route tree, `shell/` holds the root layout and the placeholder status screen, and `error-boundaries/` holds the fatal boundary and the default route error and not-found states.

`src/api/` is the only code that knows how the backend is reached. `generated/bindings.ts` is written by tauri-specta from the Rust command signatures and is never edited by hand. `transport.ts`, `tauri-transport.ts`, `mock-transport.ts`, `client.ts` and `query-keys.ts` wrap it, as described below.

`src/features/` will hold one folder per product feature, each exposing a small public `index.ts` and a lazy route module. `src/entities/` will hold domain building blocks several features render. `src/shared/` holds code with no feature knowledge: `ui/` (shared components such as the typed failure state), `i18n/`, `hooks/`, `formatting/` and `testing/`. `src/styles/` holds the global stylesheet.

## Backend access

A `Transport` has three operations, all typed from the generated bindings: `call` runs a command and resolves to its `{ status: "ok", data }` or `{ status: "error", error }` outcome, `stream` runs a command that takes a channel and passes each event to a callback, and `subscribe` listens to the application-wide `AppEvent`. Command names, request arguments, stream events and results are derived from the generated `commands` type, so a new backend command is available through the transport as soon as the bindings are regenerated. Any command with a channel parameter is a stream command, and callers pass its other arguments without the channel; a type assertion in `tauri-transport.ts` fails the build if a command ever takes its channel anywhere but last.

A stream's events are independent of its call promise: they may arrive before or after the promise settles, and the terminal event (completed, failed, cancelled) is in the stream, not in the promise. The handler stays attached until the caller aborts the `AbortSignal` it passed, which it does on unmount or after the terminal event; the Tauri transport then drops the channel's handler, and a server transport would close its subscription. A signal that is already aborted cancels the stream with a `cancelled` failure before the command is sent, and a call that fails detaches its handler at once because no events follow; after a successful call the abort listener stays, since that is how late events are detached.

`tauri-transport.ts` is the only implementation that touches Tauri: it forwards calls to the generated `commands`, wraps stream callbacks in a Tauri `Channel`, and listens to `events.appEvent`. `mock-transport.ts` is an in-memory implementation with per-command handlers, used by tests and by the browser development mode. A later HTTP or WebSocket transport for a server mode implements the same interface; nothing outside `src/api/` knows which transport is running.

`client.ts` wraps a transport in the `ApiClient` the UI uses. An ok outcome resolves to its data. An error outcome that `isApiError` recognizes (an object whose `code` is one of the generated `ApiErrorCode` values, with a message and nullable details) rejects with an `ApiFailure` carrying that code and details. Anything else, whether the transport threw or Tauri rejected with its own string (an unknown command, a denied permission, arguments that did not deserialize), rejects with an `ApiFailure` whose code is `transport` and whose `cause` is the original value. The backend's `message` is diagnostic text for logs; the UI shows localized copy chosen by `code`.

`selectTransport` picks the Tauri transport when `window.__TAURI_INTERNALS__` exists. Without it, a development build falls back to the mock so the UI runs in a plain browser, and a production build fails with a `transport` failure that the bootstrap renders as the fatal screen, so a missing backend is never hidden behind fake data. The mock branch is guarded by `import.meta.env.DEV`, a build-time constant, so production bundles do not contain the mock at all; the Tauri transport is loaded with a dynamic import, so a browser development session never loads Tauri code.

Media is never passed as bytes: every `AssetRef` from the backend carries the `url` the UI loads as is, and uploads pass a `FileSource` path or URI that Rust reads.

## State

Server state lives in the TanStack Query cache under keys from `query-keys.ts`. Cached data never goes stale on its own (`staleTime: Infinity`), and the query client does not retry failures or refetch on window focus or reconnect: data changes when a mutation or a backend event says so, and a failure is shown with its typed code instead of being retried blindly. `AppEventBridge` subscribes once to the application event stream when the providers mount and invalidates the query keys that `appEventInvalidations` in `query-keys.ts` maps each event type to; an event type missing from the table invalidates nothing. Once the subscription is live the bridge invalidates every query once, so an event that fired while the page was starting is not lost.

Keys nest so that invalidating a prefix refreshes everything under it: `conversations.messages(id)` sits under `conversations.detail(id)`, so the `generation_settled` rule refreshes a conversation's message queries along with its view. `app.status` has no event that refreshes it; the placeholder only shows the version and platform, which cannot change while the app runs. Any other `AppStatus` field (the UI state, legacy detection, conflict and purge-notice counts) must get its own query keyed to the backend event that changes it before a screen uses it. Route and workflow state belongs in typed route params and search params, and ephemeral UI state stays in components.

The router is TanStack Router with a code-based route tree. Its context carries the `ApiClient` and the `QueryClient`, so route components and loaders get the client from the router instead of from a global. Each route's component is loaded lazily through `createRoute(...).lazy(...)`; features will export their route modules the same way.

## Startup

`main.tsx` calls `bootstrap`, which creates the i18n instance, selects the transport and renders the app. If the backend cannot be reached it renders the localized fatal screen instead. If localization itself fails, or anything else rejects before React renders, it writes a plain-text message into the page, the only English text that does not come from a locale file.

## Localization

All user-facing text goes through i18next and react-i18next. Each namespace is a JSON file under `src/shared/i18n/locales/<language>/`; `common` is the only one so far, and each feature adds its own. The resources are registered in `src/shared/i18n/index.ts` and typed through `i18next.d.ts`, so a missing key is a type error. Error codes map to copy under `errors.code`. Tests render the app in i18next's `cimode`, where `t()` returns the key, to prove a screen has no hardcoded strings, and the `react/jsx-no-literals` lint rule rejects literal text in JSX children and in text props such as `title`, `label`, `placeholder` and `aria-label`, while class names, data attributes and the `{" "}` spacing idiom stay free. The restricted props cover every text-bearing prop in crisp's component types; the `cimode` test remains the backstop for anything the list misses.

## Theme

`ThemeProvider` from crisp applies the theme variables and remembers the chosen preset, `MotionConfig` honours the reduced-motion preference, and `src/styles/index.css` imports Tailwind, the bundled Inter font and crisp's stylesheet and points Tailwind's source scanning at crisp's build. The font is bundled rather than loaded from a CDN because the shell's content security policy allows fonts and styles only from the app.

## Boundaries

oxlint enforces the import rules: `@tauri-apps/*` may only be imported inside `src/api/`; outside `src/api/` the generated bindings may only be imported for types and the transports not at all (tests may use the mock); a feature is imported only through `@/features/<name>`; and `shared/` and `entities/` may not import a feature, which also holds for their tests. `__TAURI_INTERNALS__` may not be read outside `src/api/`, as a global or as a property. The tests in `test/lint-boundaries.test.ts` run oxlint on the fixtures in `test/lint-fixtures/` to prove each rule fires, and `test/architecture.test.ts` applies the same rules to relative import paths, which a lint pattern cannot judge, and rejects any mention of `__TAURI_INTERNALS__` outside `src/api/`, including the string form of an `in` check.

## Running

`bun install` installs the dependencies. `cargo tauri dev` in `apps/tauri` starts `bun run dev` here and opens the app on the Vite server at port 1420; `cargo tauri build` runs `bun run build` and bundles `dist/`. On a mobile device the Tauri CLI sets `TAURI_DEV_HOST`, and the dev server then listens on that host with hot reload on port 1421.

`bun run dev` alone serves the UI in a plain browser at `http://localhost:1420` with the mock transport. `bun run check` runs the type check, the linter and the tests; `bun run build` type-checks and writes the production bundle to `dist/`. The build targets ES2023, which every webview Tauri 2 runs on (WebView2 on Windows, WKWebView on macOS and iOS, WebKitGTK on Linux, the system WebView on Android) supports.

`index.html` must not contain an inline `<style>` or script, because the shell's content security policy would then drop `'unsafe-inline'` and break React style attributes.

Provider control errors include `in_use` for referenced records and `malformed` for unreadable provider responses. The API client preserves these typed errors and shared failure screens render their localized copy.

Generated provider contracts expose stored-certificate validity with an InvalidPem reason, CertificateAlreadyImported details with the existing id, and optional redacted verification messages with MissingApiKey or InvalidApiKey reasons.
