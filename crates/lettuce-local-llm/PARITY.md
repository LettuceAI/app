# lettuce-local-llm: legacy parity notes

Facts about how `lettuce-local-llm` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- Android and iOS never link llama.cpp, as in legacy.
- `offload` is legacy's GPU offload planner with its formulas, constants and tie-breaks unchanged (user rule: runtime formulas are frozen). All legacy unit tests are carried over.
- The per-model metadata, KV geometry, unit costs and compute buffer are cached per model path for the life of the process, as legacy did.
- `engine` is ported from legacy's `load_engine`. Load progress keeps legacy's stage and status codes and per-GPU percentages; Tauri events became an `EngineObserver` and logs go to `tracing`.
- `context` is legacy's context sizing, unchanged, including its own older KV-per-value table.
- `hardware` reports what legacy reported (sysinfo 0.33 for RAM).
- `mtp` is legacy's draft/verify runtime unchanged.
- `sampler` is legacy's sampler chain unchanged.
- `prompt` is legacy's prompt builder unchanged. Errors keep legacy's wording, without legacy's module/line prefix.
- `tool_calls` is legacy's tool-call parser.
- `generation` is legacy's request handler, including the per-model smart offload cache, the native fitter behind its unchanged gate, the context attempt groups, the 1 GiB hot context cache and the metrics record. Inline images, the `image` 0.25 PNG normalization and base64 handling are unchanged.
- The DFlash settings, the shared draft slot, stats and the adaptive draft length come from release 2.2.5. The adaptive-p sampler and Gemma 4 forced reasoning are release 2.2.5 additions.
- The VRAM reserve keeps legacy's drafter selection in `offload::drafter_reserve_path`.

## Deliberate differences from legacy

- Legacy panicked on empty MTP draft logits; drafting now stops there.
- Legacy could draft more than one verification batch can carry when the batch size was at or below the draft length, which aborts llama.cpp. A round is now bounded by the target's `n_batch`. This only bounds the drafted count; the draft context is sized as before.
- Aborts are a typed error; legacy matched the word "aborted" in any error text.
- DRY sequence breakers are decoded once. Legacy decoded them in the chat layer and again in the runtime, and the second `trim()` turned a decoded newline into an empty breaker that was dropped, so the default `\n, :, ", *` list never had its newline and a `\n`-only list fell back to the four defaults. llama.cpp's own default breakers include the newline.
- AMD APUs (user requirement 2026-09-21): legacy left iGPUs out of the device list, refused an iGPU as the selected device and skipped it in per-device VRAM, so an APU had nothing to pick. iGPUs are now listed and selectable as a single device. Whether the budgets need more for unified memory (for example memory the iGPU can borrow beyond the carve-out) is still open: it needs measurement on the hardware and the user's approval, since the formulas are frozen.
- Separate K and V cache types were a user request (2026-09-21). With one shared type all legacy tests pass as before. Checked on a real model: K q8_0 34 MiB and V q4_0 18 MiB at 2048 cells, as predicted.
- DFlash follows llama.cpp's `common/speculative.cpp` instead of legacy's runtime, because legacy's never ran. Legacy never reached DFlash from chat (its provider field allowlist lacked the `llamaDflash*` keys), and its runtime could not have run: it built an MTP-type drafter context (null for a model without NextN layers, and the DFlash graph aborts on the MTP decoder), ran a separate encoder pass that the fused injection decode replaced, kept one target output (a llama.cpp abort on the first verification batch), sampled matched rows twice, wrote only the bonus token's features into the drafter, never bounded the draft by the target batch, and discovered any `*dflash*.gguf` beside the model.
- Upstream refuses a DSpark drafter without a confidence head; this runtime runs it with `dflash_min_probability` 0.

## Fit estimate

The fit estimate (`context_preview`, shared by the model editor through `context_info` and the Hugging Face browser) is not the load planner, and the user approved changing its memory formulas where they disagreed with llama.cpp. Measured on a Gemma 4 E4B Q4_K_M file with an RTX 4060, the estimate was 77% over on model bytes and 75% over on the KV cache, and a 32K context was called impossible where the load used about 4.0 GiB of 6.6 GiB free. The changes, each checked against llama.cpp's own allocation:

- Input embeddings stay on the host. `token_embd*` and `per_layer_token_embd*` belong to no offload unit (`llama-arch.cpp` 720 and 913 give both the input layer). The per-layer projection (`per_layer_model_proj`, `per_layer_proj_norm`) is created with block 0 (`models/gemma4.cpp` 59-60), so it follows block 0. A model that ties its head puts a copy of the embedding on the output unit while the CPU keeps the original, so host bytes are the file size minus the offloaded units plus that copy once the output unit is offloaded. The Gemma file's host and device model bytes then match the projection (host exactly, device within 16 KB).
- The last `shared_kv_layers` layers hold no cache (`llama-hparams.cpp` 320-329, `models/gemma4.cpp` 10). The count is read from `<arch>.attention.shared_kv_layers` in the file, by the native loader and by the header parser alike. Gemma 3n hard-codes its 20 (`models/gemma3n.cpp` 7) with no key in the file, so it is still estimated with a full cache.
- `swa_full` defaults to false in the estimate, as `LlamaContextParams::default()` does at load; the editor and the browser both took true. The estimate applies it itself (`preview_context`), so a caller cannot diverge. The load plan takes it too (`OffloadRequest::swa_full`, from `rt.swa_full == Some(true)`): it sizes the cache from the same geometry and projects the compute buffer with the same `swa_full`, so the plan and the preview agree and a `swa_full` context is planned for the cache it will really allocate.
- The header parser (`canonical_model`, mobile and the browser's fallback) reads the sliding window pattern (a bool array), the sliding layers' own key and value lengths, per-layer `head_count` and `head_count_kv` arrays and `shared_kv_layers`, so Gemma 4 gets a verdict on mobile. A sliding window without a declared pattern (Gemma 3 derives it in code) or a shared count that would leave no cache stays unknown. The parser and the native loader produce identical metadata, geometry and unit costs for the Gemma file (ignored test `the_header_parser_and_the_native_loader_describe_a_real_model_alike`).
- Working memory is the compute buffer llama.cpp projects for the shape (`llama::project_memory`, the call the load plans with) wherever the editor can ask for it: the GPU pool takes the device compute buffer plus the sidecars, the RAM pool the host compute buffer, and the load plan's layer count is made against the measured buffer, as at load. llama.cpp builds the projection from the tensor index with no weights, so a metadata-only copy projects exactly like the full file (ignored test `llama_cpp_projects_the_same_memory_for_a_metadata_only_copy`). A projection costs about a second per new shape (cached afterwards), so the Hugging Face browser, which plans each candidate file at many shapes, and mobile keep the reserve formula; there the Gemma estimate is 6% low at 32K because the formula's floor is below the real buffer. The projection of a CPU-only placement on a GPU build still allocates a device compute buffer (1.1 GiB for the Gemma file); the estimate does not count it.
- On the Gemma file at 4K and 32K the estimate and the measurement agree to the byte on the KV cache (109,051,904 and 578,813,952 bytes at f16), on host model bytes (2,862,612,480) and, with the projection, on the working memory (579,883,008 and 609,243,136); device model bytes differ by 15,960 of 3,007,371,008.

How the preview treats the settings the load handles differently (the load is `generation.rs`, `engine.rs`):

| Setting | Preview | Load |
| --- | --- | --- |
| Single selected GPU with no free memory or not found | falls back to the machine's largest free GPU memory | the same (`generation.rs` 1240-1247) |
| Several selected GPUs | taken in the requested order; a missing, integrated or repeated id leaves no estimate and `placement_fits` false | refused when the device is resolved (`engine.rs` 467-491, 529) |
| Multi-GPU enabled with fewer than two ids | plans as no multi-GPU | `multi_gpu_active` needs two ids (`generation.rs` 1176-1178) |
| Zero layers with several GPUs | all on the CPU, no distribution | zero layers distribute to nothing (`generation.rs` 1713-1728); a manual distribution wins over the layer count (1387), and so it does in the preview |
| Explicit layer count | the distribution is made for that count with the load's inputs (no unit costs, free memory unreserved) | `generation.rs` 1713-1728 |
| Smart layer count | exact unit costs and the free memory less the draft reserve | `generation.rs` 1540-1552 |
| Compute buffers per device | taken out of each device only to judge whether the split fits (`placement_fits`), never to choose it | no such check; the load tries the split and falls back |
| Projector with a CPU-only backend | counted in RAM | the projector loads on the GPU whenever a GPU backend exists (`mtmd.cpp` `use_gpu` true, `engine.rs` 1170), so with one it is not counted at zero layers (the pool would be a GPU pool) |
| GPU pool with a device that reports no capacity | kept, capacity 0 | n/a (display only: dropping it would hide the bytes in use) |
| Device order in a pool | the selection order | `with_devices` order (`engine.rs` 529) |
| Unified memory | one pool: the sum of the pools against physical RAM; the GPU sub-budget is still checked | the load budgets RAM and VRAM separately (`is_unified_memory` is only read by the estimate); the sum is what the machine has |
| Draft placement text | trimmed, lowercased, anything but `auto`, `gpu` and `cpu` is `auto` (`request::normalized_mtp_placement`) | the same, now one function (`request.rs` 424, 459) |
| `auto` draft placement | compares the draft reserve with 90% of the budget of the selected GPUs | `generation.rs` 1327-1331 |
| Projector path | trimmed before measuring | `request.rs` 412 |
| Flash attention | the caller's choice, else auto; the editor and browser pass the ROCm-aware default | `generation.rs` 1201-1207 |
| `swa_full` | applied inside `preview_context`, default false | see above |

The projector: the mtmd context is created after the model and before the context, kept while consecutive requests carry media, and dropped by the next text-only request (`engine.rs` 1151-1186). A text-only request that loaded the model first used to plan without the projector, and the next image request reused that model (`engine.rs` 861-870, a smart-offload model with GPU layers is kept when only the candidates change) and then put the projector on top of a plan that never reserved it. The load plan (`generation.rs`, `sidecar_vram_reserve_bytes`, the planning-config key and the native-fit margins) now reserves the projector whenever one is configured, as the estimate does, whether or not the request carries media.

## Fork updates

- The fork was first pinned to the commit legacy shipped with (`07a8b3b`, `mtmd` feature). It is now pinned to `779747a` (llama.cpp b11157).
- With that update `LlamaContextParams::default()` gives SWA layers a windowed cache (`swa_full` false, as llama-server does); the sizing formulas already assumed it, and `swa_full` stays a user setting.
- A K/V cache type combination llama.cpp refuses (quantized V without flash attention, mixed types on MLA models, a block size that does not divide a head) now fails context creation with its own message instead of a null context, so it is not retried as an out-of-memory fallback and reaches the user inside the context error unchanged.
- Known and left alone because the formulas are frozen: with YaRN and a custom `rope_freq_scale`, llama.cpp now rewrites the loaded model's `n_ctx_train` when a context is created (upstream dfc29b64e), so later reads of the training context on that model see the scaled value; and the target context no longer allocates KV for bundled NextN layers on any architecture (upstream 5cdd3d1da), so the bundled-MTP KV reserve in the offload plan over-reserves slightly.

## History

- The previous README listed as next steps: report and metrics storage and host events in the app, the runtime commands (devices, embedded template, unload, context info), then stable-diffusion.cpp. `lettuce-app` now has `DatabaseLlamaHost` and the device, embedded template, unload and context info commands (`models/local_llama.rs`).
- DSpark note: DeepSeek's Qwen3-4B block-7 drafter barely drafts terse counting, upstream included, which is why its live test uses a 64-token explanation.

The host notice envelope now carries the request id for application routing. The notice itself and the once-per-load KV fallback rule remain unchanged (`old-code/src-tauri/src/llama_cpp/mod.rs:2598-2607,3544-3556`; `old-code/src-tauri/src/llama_cpp/desktop/engine.rs:1399-1441`). No permanent model-path deduplication is added.

Explicit KV placement now controls attention/KV offload on a single GPU as well as multiple GPUs. Pin and Split enable offload; System RAM disables it; Automatic retains the explicit offload flag and existing platform defaults. Legacy gated placement behind multi-GPU (`old-code/src-tauri/src/llama_cpp/mod.rs:1698-1733`) and ignored it when choosing the final context attempts (`:3067-3078`). Preview planning and context creation now share the placement resolution; CPU and OOM fallbacks remain unchanged. Sizing formulas are unchanged.

The context-info response exposes the already resolved single-GPU layer count through estimated_placement, including a zero-layer result for CPU execution. This lets the model editor render GPU/CPU distribution without changing the offload planner or sizing formulas. Multi-GPU placement remains aligned to selected_gpu_device_ids.
