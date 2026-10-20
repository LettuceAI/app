# lettuce-tauri

The Tauri 3 alpha shell uses Wry on desktop, Android and iOS. Linux uses GTK3 and WebKitGTK; Windows uses WebView2 and Apple platforms use WKWebView. The Android project lives in `gen/android`; iOS still requires `bun run tauri ios init` in `apps/ui` to generate its native project. It holds no application behavior: it opens the application API from `lettuce-app`, runs its workers, and exposes it to the webview as commands, events and the asset URI scheme. It is the only application crate that depends on Tauri and tauri-specta (`scripts/check-architecture.sh` enforces this), and nothing depends on it.

The workspace currently uses local sibling checkouts of `tauri`, `tauri-specta`, `tauri-plugin-single-window` and `tauri-plugin-android-fs`. These changes are not published dependencies. Specta's unpublished rc.26 crates are pinned to the same upstream revision as the Specta fork. The dialog plugin uses `xdg-portal` with its defaults disabled. CEF is not selected or initialized by the application.

For an isolated debug launch, pass a separate identifier and `app.appDirectoriesOverride` through a Tauri configuration file.

## Startup and shutdown

`run()` installs logging (`lettuce-observability`), builds the tauri-specta command and event registry and starts Tauri. In debug builds it first writes the TypeScript bindings to `apps/ui/src/api/generated/bindings.ts`.

The setup hook resolves the app data directory (identifier `com.lettuceai.app`, the same as the legacy app, so legacy import finds its data) and the resource folder, and opens the API with `ApiContext::open_desktop` over the native `NativeSecretStore`, an event sink that emits Tauri events, the platform's `FileAccess` and the platform's asset URL base. It then runs `lettuce_app::api::startup`, which returns once restart recovery and legacy detection are done, so commands never race recovery; the rest of startup and the workers (conversation generation, the job runner with the job change feed) run on threads `lettuce-app` starts, each with a current-thread Tokio runtime because repositories are called synchronously. The shell manages an `ApiContextSlot` containing the current Arc and the returned `ApiWorkers` as Tauri state, and forwards window focus changes (on Android and iOS also suspend and resume) to `ApiContext::app_focus_changed` for the active-time counter, which writes the counted time when focus is lost.

On exit the shell calls `ApiWorkers::stop`, which tells every worker to take no new work, calls `begin_shutdown` (cancelling running inference, and every job a worker runs through its token linked to the context's shutdown token), waits for the backend's `shutdown` to stop the local diffusion server, records the counted active time, and then joins the threads.

## Commands

Commands live in `src/commands/`, one module per domain. Each command is a `#[tauri::command] #[specta::specta]` one-line wrapper over the `lettuce_app::api` function of the same name, taking one request DTO and returning `Result<Response, ApiError>`; the generated client exposes them as `commands.conversationsList(request)` and so on, with errors as `{ status: "error", error: ApiError }`.

| Command | Request | Response |
| --- | --- | --- |
| `conversations_list` | `ConversationsListRequest` | `ConversationPage` |
| `conversation_open` | `ConversationOpenRequest` | `ConversationView` |
| `conversation_messages` | `ConversationMessagesRequest` | `MessagePage` |
| `conversation_send` | `ConversationSendRequest` and an `on_event: Channel<GenerationEvent>` | `SendAccepted` |
| `generation_cancel` | `GenerationCancelRequest` | `null` |
| `conversation_launch_direct` | `LaunchDirectRequest` | `LaunchDirectResponse` |
| `characters_list` | `CharactersListRequest` | `CharacterPage` |
| `jobs_list` | `JobsListRequest` | `JobPage` |
| `job_get` | `JobGetRequest` | `JobView` |
| `job_cancel` | `JobCancelRequest` | `null` |
| `job_watch` | `JobWatchRequest` and an `on_event: Channel<JobEvent>` | `JobView` |
| `files_inspect` | `FilesInspectRequest` | `FileInspection` |
| `assets_ingest` | `AssetsIngestRequest` | `AssetRef` |
| `files_pick_open` | `FilesPickOpenRequest` | `FilesPicked` |
| `files_pick_save` | `FilesPickSaveRequest` | `FileSavePicked` |
| `app_status` | none | `AppStatus` |
| `app_ui_state_update` | `AppUiStateUpdateRequest` | `AppUiStateView` |
| `purge_notices_list` | none | `PurgeNoticeList` |
| `purge_notice_dismiss` | `PurgeNoticeDismissRequest` | `null` |
| `opened_files_take` | none | `OpenedItem[]` |

`conversation_send` wraps its channel as the turn's `GenerationEventSink` and `job_watch` its channel as the job's `JobEventSink`, so token deltas and job progress for a watcher travel on those channels. The only global event is `AppEvent` (`app-event`), which carries an `ApiEvent`: `generation_settled`, or `job_updated` for every job change.

## Files

`src/files.rs` implements `lettuce_app::api::FileAccess` per platform, and `files::file_access` picks the one the context gets. On desktop a `FileSource` or `FileTarget` URI is a filesystem path opened with `std::fs`, any `scheme://` URI is refused as unsupported, and the pickers are `tauri-plugin-dialog` dialogs run on the blocking pool (the extension filter is the union of the requested kinds; a kind that offers every file removes the filter). On Android, `content://` and `file://` URIs go through `tauri-plugin-android-fs` (Storage Access Framework: name and length for `describe`, a real file descriptor for `open`, `open_file_writable` for `create`, which truncates safely), plain paths into the app's own folders through `std::fs`, and the pickers are the system document pickers filtered by MIME type (a kind with no reliable MIME type, such as backups, chat logs or models, shows every file and the content is checked after picking). Picked URIs keep their grant until the app or device restarts; persistable grants and folder pickers are not wired. Backup export writes shared Downloads through the plugin public-storage API. Other platforms get plain paths only and the pickers answer Unsupported.

## Media

Images and other media never cross IPC as bytes, base64 or data URLs, in either direction. The shell registers the asynchronous `lettuce-asset` URI scheme, which serves a ready asset's bytes with its MIME type from the media store (`lettuce_app::api::read_asset`). Every `AssetRef` carries its `url`, built by the API from the base the shell passes at startup: `http://lettuce-asset.localhost/` on Windows and Android, `lettuce-asset://localhost/` on Linux, macOS and iOS (chosen by `cfg`). The frontend uses that URL as is and never builds one. A response carries `Content-Length`, `Accept-Ranges: bytes` and `X-Content-Type-Options: nosniff`. A single `Range` (`bytes=a-b`, `a-` or `-n`) gets `206` with `Content-Range`, reading only those bytes; an open-ended `a-` returns at most 4 MiB, and the client asks again for the rest. A range past the end answers 416, a missing asset 404, a bad id 400, an unavailable store 503; any other request gets the whole asset with 200. Commands that accept user media later take a file path from the dialog or drag and drop (a `content://` URI on Android), which Rust reads, validates and ingests.

## Bindings

`tauri-specta` generates `apps/ui/src/api/generated/bindings.ts` from the command signatures and the `lettuce-contracts` types (their `specta` feature is enabled only here). Export trims trailing spaces on each line and ends with one newline. Regenerate after changing a command or contract with `cargo run -p lettuce-tauri --bin export-bindings`. The test `committed_bindings_are_current` exports to a temporary file and fails when the committed file differs, so `cargo test --workspace` catches stale bindings.

## Configuration

`tauri.conf.json` points `devUrl` at the frontend's Vite dev server on `http://localhost:1420` (started with `bun run dev` in `apps/ui/`) and `frontendDist` at `../ui/dist` (built with `bun run build`). The desktop window is undecorated and uses the frontend title bar and resize handles. The content security policy allows scripts only from the app, styles from the app plus inline styles, images and media from the app, the asset scheme, `blob:` and `data:` (small inline SVGs from CSS), fonts from the app and `data:`, and connections only to the IPC endpoints. `apps/ui/index.html` must not contain an inline `<style>`: Tauri would add its hash or a nonce to `style-src`, which disables `'unsafe-inline'` and breaks React's style attributes. `useHttpsScheme` must stay unset, because the asset URL bases assume `http` on Windows and Android. The `default` capability grants the main window `core:default` only; application commands need no plugin permission.

## Builds

The product ships two desktop builds; the GPU features select how llama.cpp and whisper are compiled. They forward to `lettuce-app`'s `llama-*` and `asr-*` features, and none is on by default.

| Build | Platforms | Features | Command |
| --- | --- | --- | --- |
| Normal (CPU and Vulkan in one binary) | Linux, Windows | `vulkan` | `cargo tauri build --features vulkan` |
| CUDA | Linux, Windows | `cuda` | `cargo tauri build --features cuda` |
| Normal (Metal) | macOS | `metal` | `cargo tauri build --features metal` |
| CPU only | any, for development | none | `cargo tauri dev` |

The Tauri CLI passes `--features` through to Cargo (`cargo tauri dev --features vulkan` works the same way), and release builds turn on `custom-protocol` themselves. Mobile builds use no GPU feature. `app_version` reports the `-cuda` suffix whenever `cuda` is on, because it follows `lettuce-local-llm`'s `cuda` feature.

Building on Linux needs the WebKitGTK 4.1 development packages. The `vulkan` feature also needs the Vulkan headers, `glslc` and the SPIRV-Headers CMake package; `cuda` needs the CUDA toolkit.

## Platform notes

The shell is written generic over `R: Runtime` and `run()` carries `#[cfg_attr(mobile, tauri::mobile_entry_point)]`, so Android and iOS builds (`gen/android`, `gen/apple`) come from this crate, which is built as `staticlib`, `cdylib` and `rlib`.

Single instance: on desktop `tauri-plugin-single-window` is registered first, so a second launch hands activation to the running instance (which focuses its window) and exits before any other setup; it is not registered on Android or iOS.

Opened files and navigation: the inline plugin `lettuce-host` (`src/host.rs`) keeps an in-memory inbox of `OpenedItem`s, either `{ kind: "file", source: FileSource }` or `{ kind: "navigate", path }`. Sources are the first launch's arguments, a second launch's arguments and working directory (the single-window activation payload, with relative paths resolved against that directory), macOS `RunEvent::Opened`, and on Android the VIEW and SEND intents plus shortcut intents. Desktop arguments are kept only when they name an existing regular file ending in `.lettuce` or `.uec` (any case). A new item fires the `opened-files` event (`OpenedFiles`, no payload) and the webview takes the inbox with `opened_files_take`, which returns and clears it; nothing is persisted, and the shell does not sniff content, so the UI decides what an opened file is through `import_preview` or `backup_inspect`. Navigation paths are limited to `/characters/new`, `/library` and `/`; anything else is dropped with a warning on the Rust side. The second-instance window focus uses the plugin's `Config::new()`, which focuses the existing window.

User files on mobile: Android uses `tauri-plugin-android-fs` (the plugin the legacy app used; 29.0.0, default features off, so no JavaScript commands are exposed), registered only on Android; desktop uses `tauri-plugin-dialog`, registered only on desktop. An upload is the picked URI handed to Rust, which opens, validates and ingests it, matching the no-bytes-over-IPC rule. iOS has no picker in this shell yet.

The Linux entry point sets `WEBKIT_DISABLE_COMPOSITING_MODE=1` and `WEBKIT_DISABLE_GPU_PROCESS=1` before startup unless the user already set them. These retain the previous workaround for WebKitGTK crashes on Wayland and NVIDIA.

Microphone capture uses CPAL on desktop and Android. Android checks the Activity's `RECORD_AUDIO` permission before selecting an input device; denial reaches the API as the typed microphone permission failure. The Android manifest declares `RECORD_AUDIO` and `MODIFY_AUDIO_SETTINGS`. Not yet wired: the runtime `RECORD_AUDIO` request prompt; the shell checks permission only. `lettuce-app` still reports dead-code warnings on Android for desktop-only image and local-model helpers. Capture and permission prompts have not been exercised on a device.

Branch list, fork, rename and select commands forward their typed requests to the application API and export their DTOs through the shared command registry.

The `conversation_duplicate` command delegates transactional copying to the application API and returns its stable conversation identity and revision.

`conversation_branch_delete` forwards branch revision and operation-key validation to the application API and returns its typed errors and stable replay result.

The four conversation copy wrappers pass typed requests to lettuce-app. The shell transports IDs and managed asset references; it performs no copy planning, SQL or model calls.

The memory and companion command modules forward typed requests to lettuce-app one for one: manual memory edits, the memory read, forced cycle controls, the cycle log, revert and error dismissal, Soul reads and growth edits, the Soul writer job and scheduled notes. Forced cycles and the Soul writer return a job id; their progress, text deltas and results arrive through `job_watch`.

Generation channels transport typed speaker selection events along with text deltas and settlement. Late stream attachment receives the resolved speaker from the application API; the shell does not perform speaker selection.

Generation and job channels also transport typed local runtime notices. Job watches carry throughput separately from text. The global application event transports local runtime report changes with model ids; the application composition performs the request routing and model resolution.

Generation and job channels forward typed `ModelLoading` events, including retry and terminal load status, model name and GPU progress. The shell does not reconstruct load state or throttle it.

Lorebook, staged project and prompt wrappers forward typed requests to the application API. Draft and stage commands return durable job identities for `job_watch`; source documents arrive as managed asset ids. Trigger and prompt previews are backend reads, and the shell performs no matching or prompt rendering.

Provider control wrappers forward catalog, account listing, saved-or-draft verification, public OpenRouter endpoint discovery and certificate metadata listing to lettuce-app. Verification request credentials cross inbound IPC only; responses carry key presence or redacted typed error details. The shell owns no provider URL or authentication policy.

Provider control commands forward account save/delete, model listing/verification and certificate import/remove to lettuce-app. The shell transports FileSource locations and metadata views; it never receives secret values in results.

Model profile and NanoGPT usage commands forward the catalog, profile reads, revisioned save/delete/duplicate/default requests and usage reads to the composition root. Duplicate names are supplied by the UI; the shell builds no localized text. ModelsChanged and ProviderQuota use the shared application event channel and generated contracts.

The settings wrappers expose settings_get, settings_update, settings_sampler_defaults_update, content_filter_log and content_filter_clear. Their requests and results are owned contract types; the shell performs no settings merging or filter work. SettingsChanged and ContentFilterHit travel through the shared application event channel.

Startup resolves the log directory and creates it before installing local output, then installs the chained panic hook and retains the output guard as managed state. The log command module delegates list, page, search, relevant lines, delete, clear, export and append to lettuce-app; app_usage_days returns the current focused-time read. DeveloperLogLine and AppUsageChanged use the shared application event channel. Log-mirror delivery failures go directly to stderr so they cannot create another mirrored warning.

The metrics command module forwards cursor listing, detail, per-message detail and receipt-backed explicit clearing to lettuce-app. Generated DTOs contain summaries and samples, with no native model file paths.

Log exports check reserved database and control-file locations before creating a target, open native paths and Android document targets without truncation, then check source and protected-file identities before truncating through the same descriptor. The app holds the database-file lifecycle lock through the write. Providers that cannot support this operation return a typed file error.

`usage_clear_before` exposes the application ledger maintenance command with a timestamp cutoff and operation id; database deletion and retry semantics remain in the application and database crates.

The shell exposes `storage_database_files_list` and `storage_database_file_delete` through the app API. File identifiers are basenames and deletion is guarded by the app's lifecycle lock, active-file check, live-handle lease and durable retry receipt.

Database-file inventory carries nullable creation time and measured modification time as Unix milliseconds. The shell does not substitute discovery time for an unknown creation time.

`media_save_to` forwards an asset identifier and FileTarget to the app. Native export writers check protected media identities, hold an exclusive operating-system target lease until close and refuse a concurrent writer with Busy before truncation. Process exit releases the lease.

The storage module delegates media_library_list and media_library_remove to the application API. Library items use the shell asset URL base, while removal returns typed reference details and accepts a durable operation identity.

The diagnostics command returns the app support report string. The backend redacts home paths and performs the embedding checks; the shell registers the command alongside the log tools.

Usage query, statistics and CSV wrappers forward owned contracts to the app. Statistics carry an IANA timezone, and CSV exports write through the selected FileTarget with the native target lease and protected-file checks.

The storage optimization wrapper returns the application maintenance job without running database work on the shell thread.

The usage recalculation command admits a durable missing-cost job. The shell returns its job identity and forwards typed completion and failures through the existing jobs API.

`storage_summary` exposes database, media, model-folder and log byte totals through the generated contracts. Storage inspection lives in the application API; the shell supplies no native paths in the result.

The reset host stops application workers, clears the main webview localStorage and sessionStorage with an acknowledged script, and starts the desktop executable before exiting. The database lifecycle lock holds the child at startup until the parent stores the terminal reset result. Worker shutdown joins other worker threads while allowing the current reset job to finish. Android and iOS reset run the same steps and then exit without relaunching; the user reopens the app.

The legacy_state command exposes the application's typed storage detection, and AppStatus includes the same state. The shell never reads legacy bin payloads.

Each shell command takes an Arc snapshot from ApiContextSlot before calling the application API. File-backed backup inspection and index list, forget and delete run through this same adapter. Desktop file removal preserves its error kind. An Android metadata failure is treated as missing only after the plugin confirms public-storage permission and an available primary volume; permission and storage failures retain the entry.

Backup export creates an exclusive temporary file and synchronizes it before publication. Desktop temporary locations and publication resolve the target parent directory to its canonical path, retaining the selected final filename. Parent-directory aliases share one path while a selected final symlink is replaced as a directory entry. Publication protects database and media files, then atomically replaces the requested target with std::fs::rename. The complete file is synchronized first, and parent-directory synchronization is used where supported; interrupted publication is checked against the recorded full header before index recovery. Android creates a pending file in Download/LettuceAI, renames through the existing FS plugin, clears pending state and records its actual URI. Recovery can finish pending publication; after an interrupted older Android MediaStore update it discovers candidate files through the same plugin and the application verifies the complete recorded header and size before indexing one.

Restore calls the same open_context constructor used during initial setup, runs ordinary startup, publishes the context slot and stops old workers and listeners before returning the fresh context. A failed rebuild or startup marks the slot RecoveryRequired and stops the remaining workers until the application is reopened. The slot lock is held only while cloning or replacing the Arc; commands retain their own snapshot and the database write fence rejects stale writes.

Restore rebuilds through the launch constructor and the application context slot runs ordinary startup before publishing the replacement context. The shell retires old listeners and workers after publication. Backup replacement uses the application lifecycle lock and protected-file identity checks without requiring an advisory lease on the previous target.

Context retirement uses one shared routine for listeners, workers and backend shutdown. The shell reload integration test runs the real ResetHost, constructor, startup and old-worker shutdown with a mock Tauri runtime, isolated application directories and the existing in-memory secret store. Reload keeps the device-local secret store passed to the ordinary constructor.

The shared application event channel forwards SyncConflictsChanged alongside LorebooksChanged and PromptsChanged. Their source is the application database commit feed, including incoming sync writes.

The sync command module delegates sharing, approval, begin, connect, stop, status, conflict listing, conflict resolution, fork acknowledgment and legacy dismissal to the active application context. The shared event channel carries SyncStatusChanged and SyncConflictsChanged. Exit joins the sync session before backend shutdown even when no worker handle was installed.

files_inspect returns ChatTemplate and ModelFile for entity files, Lorebook for World Info and USC, and Other for ZIPs without a supported v1 public manifest. Classification belongs to the application layer.

The file command adapter exposes import_preview, import_commit and the seven entity export commands through one context snapshot each. It forwards FileSource and FileTarget DTOs, including format choices and typed warnings; the application owns parsing, replay, jobs, media and protected publication. Export commands take no operation IDs.

Entity export targets are required on desktop and absent on Android. Android uses the existing pending-file writer in Download/LettuceAI, and the application returns its published URI without changing the plugin collision suffix.

Desktop chrome uses Crisp's title bar. Windows and Linux create a transparent undecorated window so the UI can clip its rounded frame; macOS uses an opaque decorated window with an overlay title bar and native traffic lights. The main-window capability permits minimize, toggle maximize, close, dragging and edge resizing. A native restart is required after changing these window flags.

The asset scheme also serves the API-provided `?preview=512` image URLs through `read_asset_preview`. Preview responses are whole static WebP images; byte ranges continue to apply to original URLs. Successful media responses use a private one-day browser cache, while failed reads use `no-store`.

The media_references command forwards an on-demand usage read to the application API. Library list requests may explicitly omit eager references; media removal keeps its existing guarded command.

Linux packages (deb and rpm) use `linux/lettuceai.desktop`, the bundler's default desktop template with `%F` added to `Exec`, so a file manager passes the opened files as arguments, and install `linux/lettuceai-mime.xml` as `/usr/share/mime/packages/com.lettuceai.app.xml`, which defines `application/vnd.lettuceai.backup` (`*.lettuce`) and `application/vnd.lettuceai.uec+json` (`*.uec`, a kind of JSON); the package manager's shared-mime-info trigger registers them. The AppImage cannot register MIME types and opens files only from arguments.

## Android project

`gen/android` is the Gradle project `tauri android init` generated, meant to be committed to the repository, with these changes: the LettuceAI launcher icons (made with `bun run tauri icon` from `icons/icon.png`, written to a scratch folder and copied into `res`, so the desktop icons stay as they are), a black splash screen with the leaf, `adjustResize` so the keyboard resizes the webview, the audio permissions above, `res/xml/locales_config.xml` listing the app's 20 locales so Android 13 and later offer them as the app language (the frontend follows that choice), and Android backup turned off: `allowBackup="false"` and `data_extraction_rules.xml` exclude every domain from cloud backup and device transfer, so chats and settings never leave the device that way. The minimum is Android 11 (API 30): public Downloads are written through MediaStore without a storage permission, and AAudio, which CPAL uses for the microphone, is always present.

Release builds are signed when `gen/android/keystore.properties` exists (`keyAlias`, `password` for both the store and the key, `storeFile` as an absolute path). The file is ignored by git and must never be committed; without it a release build is left unsigned. The key is the same upload key that signed the legacy app, so an install updates the existing app in place. The application ID defaults to `com.lettuceai.app`; the `lettuceApplicationId` Gradle property or `LETTUCE_ANDROID_APPLICATION_ID` changes it, so a development build can be installed next to the release one.

Back button and back gesture: the frontend registers Tauri's back-button listener on Android (`apps/ui/src/app/shell/useAndroidBack.ts`). A press closes the topmost open overlay first, then goes back in history (route blockers such as unsaved-changes confirmations still apply), opens Home from a page with no history, and exits the app on Home, another main screen or onboarding. `core:app:allow-exit` in the default capability allows that exit.

Opened files on Android: `HostPlugin.kt` is the Kotlin side of `lettuce-host` (registered with `register_android_plugin`, so no extra crate). Rust subscribes once during plugin setup by passing a `Channel`; the plugin reads the launch intent at that moment, receives later intents through `Plugin.onNewIntent` (MainActivity stays `singleTask`) and sends batches of items over the channel. An intent delivered from the recent-apps list is ignored so a finished import is not offered again. `content://` URIs are passed on as they are; their read grant lasts as long as the app task. The manifest has two intent filters on MainActivity: VIEW for `content` and `file` URIs and SEND and SEND_MULTIPLE for streams, both with the MIME types `application/octet-stream`, `application/json`, `application/vnd.lettuceai.backup` and `application/vnd.lettuceai.uec+json` and without `*/*`. Building also lets `tauri-build` add intent filters for `bundle.fileAssociations` between its own `tauri-file-associations` marker comments in the manifest. They add nothing: they carry only the two custom MIME types and a `pathPattern` without a scheme, which Android ignores, so the hand-written filters are what receive files.

Background work: `BackgroundWorkService` is a foreground service of type `dataSync` that only keeps the process alive. It shows one ongoing low-importance notification in its own channel (title and body in `strings.xml`) that opens the app. `lettuce-app` raises an active-work signal (`ApiContext::watch_active_work`) when the count of running jobs plus in-flight conversation generations moves between zero and non-zero; a job counts from its claim until it settles, a generation from its claim until its turn settles. The shell ORs the signal of every context it opens (including the one a restore swaps in; a context's entry disappears when its signal closes) in one aggregate (`src/active_work.rs`) and drives the service from it once: `true` is applied at once and `false` only after the aggregate stayed false for five seconds, so the gap between one job settling and the next being claimed does not stop and restart the service, which Android refuses to start again from the background. A change the plugin call did not apply is tried again at the next change of the aggregate. The plugin's `setBackgroundWork` starts or stops the service, which enters the foreground in both `onCreate` and `onStartCommand`, so a second start never leaves it without a notification. On Android 13 and later the plugin asks for `POST_NOTIFICATIONS` once per process, only while the activity is resumed; work that starts in the background defers the request to the next resume while work is still active. A denial still runs the service, Android just hides the notification. A start that Android refuses (for example while in the background) is logged and ignored, and `onTimeout` stops the service when Android 15 ends a `dataSync` run. The foreground service permissions are `FOREGROUND_SERVICE` and `FOREGROUND_SERVICE_DATA_SYNC`.

App shortcuts: `res/xml/shortcuts.xml` declares three static shortcuts (New character, Library, Chats). Each is a VIEW intent on MainActivity with data `lettuceai://open/characters/new`, `lettuceai://open/library` or `lettuceai://open/`, which the plugin turns into a `navigate` item. The shortcut `targetPackage` is the default application ID, so a build with another `lettuceApplicationId` does not get working shortcuts.
