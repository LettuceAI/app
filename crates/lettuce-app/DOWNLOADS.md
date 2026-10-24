# Runtime download inventory

Audited against the active rewrite on 2026-10-24. `old-code/` is a reference implementation, not a runtime entrypoint. Dependency installation and build tooling are outside the runtime download center.

## Installed files

Transfers in the entries below use persisted jobs. Already-present files can complete registration without a new transfer. The shell enumerates `artifact_install` and `model_pull` jobs through the same paginated query and app-event subscription. Screens consume that inventory rather than querying a second active list. Reopening a screen does not start another transfer; native admission owns deduplication.

| Files | Admission | Execution |
| --- | --- | --- |
| Hugging Face GGUF and optional projector | `api/models/hugging_face.rs::hf_download`, `api/jobs/local.rs::admit_gguf_download` | `InstallWork::Artifact`, pinned artifacts, GGUF registration |
| stable-diffusion.cpp engine builds | `api/image/engine.rs::sd_runtime_install` | Artifact plan, archive extraction and engine registration |
| Catalog image models | `api/image/engine.rs::sd_model_install` | Artifact plan, variant verification and registration |
| RealESRGAN upscaler | `api/image/engine.rs::sd_upscalers_install` | Artifact plan, install finishing |
| Hugging Face image bundles and retries | `api/image/library.rs::hf_image_bundle_install`, `hf_image_bundle_retry` | Artifact plan, bundle manifest and model registration |
| CivitAI LoRA | `api/image/library.rs::civitai_lora_download` | Artifact plan, CivitAI credentials/checksum, LoRA registration |
| Eidos embedding model | `api/memory/models.rs::embedding_install` | Artifact plan, embedding registration and health check |
| Thymos emotion model | `api/memory/models.rs::companion_emotion_install` | Artifact plan and emotion-model registration |
| Whisper model | `api/speech/whisper.rs::whisper_download` | `InstallWork::Whisper`, `WhisperDownloadCoordinator` |
| Kokoro model | `api/speech/kokoro.rs::kokoro_install_model` | `InstallWork::KokoroModel`, `KokoroDownloadCoordinator` |
| Kokoro voice files | `api/speech/kokoro.rs::kokoro_install_voices` | `InstallWork::KokoroVoices`, `KokoroVoiceDownloadCoordinator` |
| ONNX Runtime desktop archive, provisioned on demand | `models/onnx_runtime_install.rs::ensure_with_override` | `ArtifactInstallCoordinator` directly under the process initialization mutex |
| Ollama model pull | `api/models/ollama.rs::ollama_pull`, `api/jobs/local.rs::admit_model_pull` | `ModelPullHandler`, Ollama's streamed pull protocol; files belong to the configured Ollama server |

Paths in the table are relative to [lettuce-app/src](src). The interactive install runner is [api/jobs/install.rs](src/api/jobs/install.rs). GGUF, image, Whisper and Kokoro installs share the sequential `install:downloads` lane; embedding and emotion installs share `install:memory-models`. Ollama pulls retain their provider-specific lanes. ONNX provisioning retains its process initialization lock to avoid loading competing runtime libraries.

[ArtifactInstallCoordinator](src/models/artifact_install.rs) shares pinning, resumable staging, verification and finishing across generic artifact plans. Whisper and Kokoro retain specialized coordinators for their model/voice stores, but use the same job states, runner, cancellation contract and [ArtifactDownloadClient](../lettuce-network/src/lib.rs). These are not thirteen independent HTTP implementations. Protocol-specific registration, credentials, checksums and extraction remain with their owners.

ONNX jobs now carry a display name and respect a persisted cancellation request at existing progress and install-stage boundaries, including lease renewal. Their private provisioning token is not registered with the interactive runner: a stalled source read can still delay cancellation until the next progress update or transport timeout. This is a scheduling/cancellation boundary, not a separate progress format.

## Shared UI

[entities/download](../../apps/ui/src/entities/download) owns the active inventory, measured rate/ETA, canonical job views, cancellation deduplication, cancellation errors/retry, job-watch retry and the reusable `DownloadCard`/`DownloadJob`. There is one application `DownloadItem` adapter. The shell, GGUF browser, image/LoRA pages, speech page, Eidos sheet and welcome-back model installs use it. Non-download tasks retain their own task UI.

Speed uses received byte-counter changes and monotonic client time. A byte total and positive measured speed are required for ETA. Closing a view only detaches its watcher. Terminal shared views remain available to mounted cards after leaving the active menu; replacing the database clears the inventory and rate samples. Initial inventory loading shows a skeleton, and subscription/inventory errors expose retry.

Native `JobSubject.display` now reaches the UI as optional `subject.display_name`. Feature-specific typed details still take precedence. This names Thymos and ONNX jobs and Kokoro model/voice installs without teaching every screen their storage identities. Older persisted jobs without a label still use the generic fallback. New LoRA/Whisper/image details remain available through get, list and watch.

## Other network and save operations

These operations already have an owner with different completion semantics and do not become model-install jobs:

| Operation | Owner and reason |
| --- | --- |
| Generated image URL retrieval and reference images | [providers/images/mod.rs::image_bytes](../lettuce-providers/src/images/mod.rs), using `BulkHttpClient::fetch_url`; part of the image-generation request and managed-media result |
| ComfyUI output retrieval | [providers/images/comfyui.rs::fetch_image](../lettuce-providers/src/images/comfyui.rs), using the configured provider's `/view`; part of generation |
| Remote character-card avatar during entity import | [api/entity_files/mod.rs::download_avatar](src/api/entity_files/mod.rs), using the bounded artifact client and stored TLS policy; controlled by avatar-download preference and import warnings |
| Remote TTS audio response | [lettuce-speech/src/openai_tts.rs](../lettuce-speech/src/openai_tts.rs), [gemini_tts.rs](../lettuce-speech/src/gemini_tts.rs); response to synthesis, ingested by the speech/media workflow |
| Hugging Face/CivitAI catalogs, READMEs, tokens, remote voice lists, provider catalogs and GGUF header probes | `HuggingFaceBrowser`, `CivitaiBrowser`, speech catalogs, provider adapters and `model_runnability.rs`; bounded metadata/probe requests, not installs |
| Remote preview images in cards | Browser image loading, for example [CivitaiView.tsx](../../apps/ui/src/features/settings/image/CivitaiView.tsx); display resources rather than installed assets |
| Save/export/backup, including `mediaSaveTo` | Native managed-media/entity/backup commands write existing local data to a selected destination; no network download is started |

The workspace HTTP dependency is owned by [lettuce-network](../lettuce-network/README.md), enforced by [check-architecture.sh](../../scripts/check-architecture.sh). Provider requests, bounded metadata, artifact streams and media responses intentionally use different client policies within that module. Peer sync uses its own transfer protocol, not HTTP model downloads or `DownloadsProvider`.

## Validation for this audit

The focused download, provider/model, speech, image and memory UI suites passed 108 tests. Typecheck, UI lint, production UI build, architecture checks, binding staleness and whitespace checks passed. Regression coverage includes shared progress, cancellation deduplication and retry, terminal views, subscription recovery, database replacement, native display metadata and persisted cancellation before artifact finishing.

The full UI run passed 562 tests and failed 9 in the character editor; an isolated rerun passed 35 and failed 3 of that file's 38 tests. The Rust workspace run passed 3591, failed 1 and ignored 19. Its failure is the unchanged `memory_is_inherited_from_the_character_policy` test expecting 50 entries while `DynamicMemorySettings::default()` provides 200, both also present in HEAD. These full gates are not green. Real network transfers and native app UX were not exercised in this audit.

Strict Clippy on the touched Rust libraries also encounters the existing `clippy::map_entry` at `api/conversations/mod.rs:158`, unchanged in HEAD. A rerun allowing only that lint passed; the unmodified strict gate remains red.
