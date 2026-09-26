# lettuce-tauri

The Tauri 2.12 shell for every platform: Windows, macOS and Linux today, Android and iOS from the same crate once `cargo tauri android init` and `cargo tauri ios init` generate `gen/android` and `gen/apple`. It holds no application behavior: it opens the application API from `lettuce-app`, runs its workers, and exposes it to the webview as commands, events and the asset URI scheme. It is the only crate that depends on Tauri and tauri-specta (`scripts/check-architecture.sh` enforces this), and nothing depends on it.

## Startup and shutdown

`run()` installs logging (`lettuce-observability`), builds the tauri-specta command and event registry and starts Tauri. In debug builds it first writes the TypeScript bindings to `apps/ui/src/api/generated/bindings.ts`.

The setup hook resolves the app data directory (identifier `com.lettuceai.app`, the same as the legacy app, so legacy import finds its data), opens the API with `ApiContext::open_desktop` over the native `NativeSecretStore` and an event sink that emits Tauri events, and calls `recover_after_restart` before any worker starts. It then starts the conversation generation worker on its own thread with a current-thread Tokio runtime, because the runner calls repositories synchronously, and manages the `ApiContext` as Tauri state.

On exit the shell calls `begin_shutdown` (every running inference is cancelled, so the job being run settles), signals the worker, joins its thread, and waits for the backend's `shutdown` to stop the local servers.

## Commands

Each command is a `#[tauri::command] #[specta::specta]` one-line wrapper over the `lettuce_app::api` function of the same name, taking one request DTO and returning `Result<Response, ApiError>`; the generated client exposes them as `commands.conversationsList(request)` and so on, with errors as `{ status: "error", error: ApiError }`.

| Command | Request | Response |
| --- | --- | --- |
| `conversations_list` | `ConversationsListRequest` | `ConversationPage` |
| `conversation_open` | `ConversationOpenRequest` | `ConversationView` |
| `conversation_messages` | `ConversationMessagesRequest` | `MessagePage` |
| `conversation_send` | `ConversationSendRequest` and an `on_event: Channel<GenerationEvent>` | `SendAccepted` |
| `generation_cancel` | `GenerationCancelRequest` | `null` |
| `conversation_launch_direct` | `LaunchDirectRequest` | `LaunchDirectResponse` |
| `characters_list` | `CharactersListRequest` | `CharacterPage` |

`conversation_send` wraps its channel as the turn's `GenerationEventSink`, so token deltas travel on that channel and never through the global event bus. The only global event is `AppEvent` (`app-event`), which carries an `ApiEvent` such as `generation_settled`.

## Media

Images and other media never cross IPC as bytes, base64 or data URLs, in either direction. The shell registers the asynchronous `lettuce-asset` URI scheme, which streams a ready asset's bytes with its MIME type from the media store (`lettuce_app::api::read_asset`). The frontend builds the URL from an `AssetRef` with `convertFileSrc(assetId, "lettuce-asset")`, which gives `lettuce-asset://localhost/<asset_id>` on macOS and Linux and `http://lettuce-asset.localhost/<asset_id>` on Windows and Android. A missing asset answers 404, a bad id 400, an unavailable store 503. Commands that accept user media later take a file path from the dialog or drag and drop (a `content://` URI on Android), which Rust reads, validates and ingests.

## Bindings

`tauri-specta` generates `apps/ui/src/api/generated/bindings.ts` from the command signatures and the `lettuce-contracts` types (their `specta` feature is enabled only here). Regenerate after changing a command or contract with `cargo run -p lettuce-tauri --bin export-bindings`. The test `committed_bindings_are_current` exports to a temporary file and fails when the committed file differs, so `cargo test --workspace` catches stale bindings.

## Configuration

`tauri.conf.json` points `devUrl` at the frontend's Vite dev server on `http://localhost:1420` (started with `bun run dev` in `apps/ui/`) and `frontendDist` at `../ui/dist` (built with `bun run build`). The window is the default decorated window; the custom title bar comes later. The `default` capability grants the main window `core:default` only; application commands need no plugin permission.

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

User files on mobile: the official `tauri-plugin-dialog` returns `content://` URIs on Android and `file://` on iOS (security-scoped access handled), and `tauri-plugin-fs` opens an Android `content://` URI through the content resolver as a real file descriptor for reading or writing. An upload is therefore the picked path or URI handed to Rust, which opens, validates and ingests it, matching the no-bytes-over-IPC rule. These plugins do not cover folder pickers on mobile, persistable URI permissions, writing into shared storage such as MediaStore Downloads, or access outside the app folder by default. The legacy app used `tauri-plugin-android-fs` for exactly those (backups into Downloads, log export, folder picks, persistable access), so it is still needed there later, unless backups go to a save-dialog `content://` target instead (not yet verified on devices). None of these plugins is added yet; no slice-1 command needs them.
