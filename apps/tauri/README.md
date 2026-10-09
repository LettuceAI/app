# lettuce-tauri

The Tauri 2.12 shell for every platform: Windows, macOS and Linux today, Android and iOS from the same crate once `cargo tauri android init` and `cargo tauri ios init` generate `gen/android` and `gen/apple`. It holds no application behavior: it opens the application API from `lettuce-app`, runs its workers, and exposes it to the webview as commands, events and the asset URI scheme. It is the only crate that depends on Tauri and tauri-specta (`scripts/check-architecture.sh` enforces this), and nothing depends on it.

## Startup and shutdown

`run()` installs logging (`lettuce-observability`), builds the tauri-specta command and event registry and starts Tauri. In debug builds it first writes the TypeScript bindings to `apps/ui/src/api/generated/bindings.ts`.

The setup hook resolves the app data directory (identifier `com.lettuceai.app`, the same as the legacy app, so legacy import finds its data) and the resource folder, and opens the API with `ApiContext::open_desktop` over the native `NativeSecretStore`, an event sink that emits Tauri events, the platform's `FileAccess` and the platform's asset URL base. It then runs `lettuce_app::api::startup`, which returns once restart recovery and legacy detection are done, so commands never race recovery; the rest of startup and the workers (conversation generation, the job runner with the job change feed) run on threads `lettuce-app` starts, each with a current-thread Tokio runtime because repositories are called synchronously. The shell manages the `ApiContext` and the returned `ApiWorkers` as Tauri state, and forwards window focus changes (on Android and iOS also suspend and resume) to `ApiContext::app_focus_changed` for the active-time counter, which writes the counted time when focus is lost.

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

`conversation_send` wraps its channel as the turn's `GenerationEventSink` and `job_watch` its channel as the job's `JobEventSink`, so token deltas and job progress for a watcher travel on those channels. The only global event is `AppEvent` (`app-event`), which carries an `ApiEvent`: `generation_settled`, or `job_updated` for every job change.

## Files

`src/files.rs` implements `lettuce_app::api::FileAccess` per platform, and `files::file_access` picks the one the context gets. On desktop a `FileSource` or `FileTarget` URI is a filesystem path opened with `std::fs`, any `scheme://` URI is refused as unsupported, and the pickers are `tauri-plugin-dialog` dialogs run on the blocking pool (the extension filter is the union of the requested kinds; a kind that offers every file removes the filter). On Android, `content://` and `file://` URIs go through `tauri-plugin-android-fs` (Storage Access Framework: name and length for `describe`, a real file descriptor for `open`, `open_file_writable` for `create`, which truncates safely), plain paths into the app's own folders through `std::fs`, and the pickers are the system document pickers filtered by MIME type (a kind with no reliable MIME type, such as backups, chat logs or models, shows every file and the content is checked after picking). Picked URIs keep their grant until the app or device restarts; persistable grants, folder pickers and writes into shared Downloads are not wired yet and come with the first command that needs them (log export, backup export). Other platforms get plain paths only and the pickers answer Unsupported.

## Media

Images and other media never cross IPC as bytes, base64 or data URLs, in either direction. The shell registers the asynchronous `lettuce-asset` URI scheme, which serves a ready asset's bytes with its MIME type from the media store (`lettuce_app::api::read_asset`). Every `AssetRef` carries its `url`, built by the API from the base the shell passes at startup: `lettuce-asset://localhost/` on Linux, macOS and iOS, `http://lettuce-asset.localhost/` on Windows and Android (chosen by `cfg`). The frontend uses that URL as is and never builds one. A response carries `Content-Length`, `Accept-Ranges: bytes` and `X-Content-Type-Options: nosniff`. A single `Range` (`bytes=a-b`, `a-` or `-n`) gets `206` with `Content-Range`, reading only those bytes; an open-ended `a-` returns at most 4 MiB, and the client asks again for the rest. A range past the end answers 416, a missing asset 404, a bad id 400, an unavailable store 503; any other request gets the whole asset with 200. Commands that accept user media later take a file path from the dialog or drag and drop (a `content://` URI on Android), which Rust reads, validates and ingests.

## Bindings

`tauri-specta` generates `apps/ui/src/api/generated/bindings.ts` from the command signatures and the `lettuce-contracts` types (their `specta` feature is enabled only here). Export trims trailing spaces on each line and ends with one newline. Regenerate after changing a command or contract with `cargo run -p lettuce-tauri --bin export-bindings`. The test `committed_bindings_are_current` exports to a temporary file and fails when the committed file differs, so `cargo test --workspace` catches stale bindings.

## Configuration

`tauri.conf.json` points `devUrl` at the frontend's Vite dev server on `http://localhost:1420` (started with `bun run dev` in `apps/ui/`) and `frontendDist` at `../ui/dist` (built with `bun run build`). The window is the default decorated window; the custom title bar comes later. The content security policy allows scripts only from the app, styles from the app plus inline styles, images and media from the app, the asset scheme, `blob:` and `data:` (small inline SVGs from CSS), fonts from the app and `data:`, and connections only to the IPC endpoints. `apps/ui/index.html` must not contain an inline `<style>`: Tauri would add its hash or a nonce to `style-src`, which disables `'unsafe-inline'` and breaks React's style attributes. `useHttpsScheme` must stay unset, because the asset URL bases assume `http` on Windows and Android. The `default` capability grants the main window `core:default` only; application commands need no plugin permission.

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

The shell is written generic over `R: Runtime` and `run()` carries `#[cfg_attr(mobile, tauri::mobile_entry_point)]`, so Android and iOS builds (`gen/android`, `gen/apple`) can come from this crate once they are initialized; that step also adds the `staticlib` and `cdylib` library types.

Single instance: on desktop `tauri-plugin-single-window` is registered first, so a second launch hands activation to the running instance (which focuses its window) and exits before any other setup; it is not registered on Android or iOS.

User files on mobile: Android uses `tauri-plugin-android-fs` (the plugin the legacy app used; 29.0.0, default features off, so no JavaScript commands are exposed), registered only on Android; desktop uses `tauri-plugin-dialog`, registered only on desktop. An upload is the picked URI handed to Rust, which opens, validates and ingests it, matching the no-bytes-over-IPC rule. iOS has no picker in this shell yet.

Linux WebKitGTK: on some Wayland and NVIDIA setups the window dies with a Wayland protocol error unless compositing mode and the GPU process are off, so `main` sets `WEBKIT_DISABLE_COMPOSITING_MODE=1` and `WEBKIT_DISABLE_GPU_PROCESS=1` before anything else runs, unless the user already set them. These are the flags the legacy app's webkit-safe scripts used; they become unnecessary once the shell moves to the CEF runtime with Tauri 3.

Microphone capture uses CPAL on desktop and Android. Android checks the Activity's `RECORD_AUDIO` permission before selecting an input device; denial reaches the API as the typed microphone permission failure. An initialized Android shell must declare `android.permission.RECORD_AUDIO` and request that runtime permission through its native permission UI. Not yet wired: the `RECORD_AUDIO` request prompt and manifest entry come with the generated Android shell; this slice checks permission only. This repository has no generated Android shell yet. The whole shell passes `cargo check -p lettuce-tauri --target aarch64-linux-android` with the NDK toolchain (CC/CXX/AR and the CMake Android toolchain file set for the target); `lettuce-app` still reports dead-code warnings on Android for desktop-only image and local-model helpers. Capture and permission prompts have not been exercised on a device.

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
