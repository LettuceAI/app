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

## Fork updates

- The fork was first pinned to the commit legacy shipped with (`07a8b3b`, `mtmd` feature). It is now pinned to `779747a` (llama.cpp b11157).
- With that update `LlamaContextParams::default()` gives SWA layers a windowed cache (`swa_full` false, as llama-server does); the sizing formulas already assumed it, and `swa_full` stays a user setting.
- A K/V cache type combination llama.cpp refuses (quantized V without flash attention, mixed types on MLA models, a block size that does not divide a head) now fails context creation with its own message instead of a null context, so it is not retried as an out-of-memory fallback and reaches the user inside the context error unchanged.
- Known and left alone because the formulas are frozen: with YaRN and a custom `rope_freq_scale`, llama.cpp now rewrites the loaded model's `n_ctx_train` when a context is created (upstream dfc29b64e), so later reads of the training context on that model see the scaled value; and the target context no longer allocates KV for bundled NextN layers on any architecture (upstream 5cdd3d1da), so the bundled-MTP KV reserve in the offload plan over-reserves slightly.

## History

- The previous README listed as next steps: report and metrics storage and host events in the app, the runtime commands (devices, embedded template, unload, context info), then stable-diffusion.cpp. `lettuce-app` now has `DatabaseLlamaHost` and the device, embedded template, unload and context info commands (`models/local_llama.rs`).
- DSpark note: DeepSeek's Qwen3-4B block-7 drafter barely drafts terse counting, upstream included, which is why its live test uses a 64-token explanation.
