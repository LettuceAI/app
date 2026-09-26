# lettuce-local-llm

The local llama.cpp runtime: planning how a GGUF model fits on the machine, loading it, building the prompt, sampling, speculative decoding (MTP and DFlash), and running generation requests on one worker thread.

The crate knows nothing about conversations. It receives a `LlamaGenerationRequest` (model path, messages as JSON, sampling and runtime settings, tools) and reports through observer traits. The provider adapter that turns an inference request into one of these lives in `lettuce-providers` (`llama_cpp.rs`); the host that stores runtime reports and metrics and forwards events lives in `lettuce-app` (`models/local_llama.rs`).

## Build

The bindings are the `llama-cpp-2` and `llama-cpp-sys-2` crates from the `MegalithOfficial/llama-cpp-rs` fork with the `mtmd` feature, pinned by commit in the workspace `Cargo.toml` (currently `779747a`, llama.cpp b11157). They build on desktop only: Android and iOS never link llama.cpp, and every module that touches the bindings is compiled out there, leaving the pure planning, request and tool-call code. GPU backends are the `cuda`, `cuda-no-vmm`, `rocm`, `vulkan` and `metal` features. whisper.cpp (`lettuce-speech`) links into the same binary.

## Frozen formulas

The offload planner, the context sizing and the runnability scoring (in `lettuce-model-hub`) are ported with their formulas, constants and tie-breaks unchanged, and their unit tests are carried over. They encode measured behavior of llama.cpp; changing one silently changes which models load where. The three constants that were never measured (a 90% VRAM budget, a 5% or 256 MiB compute reserve floor, a safety factor of 2) stay as they are. Where the fork or llama.cpp changed underneath, the difference is documented rather than compensated in the formulas.

## Modules

| Module | Role |
| --- | --- |
| `offload` | GPU offload planning: layer counts, KV geometry, multi-GPU distribution. Pure. |
| `context` | Context sizing: fallback ladder, OOM classification, effective VRAM. Pure. |
| `llama` | Process-wide backend and per-model metadata, measured through the bindings. |
| `hardware` | RAM, GPU devices and memory. |
| `context_info` | The model editor's fit estimate. |
| `engine` | The model slot: loading, reuse, fallbacks, projector and draft models. |
| `generation` | The worker thread and the request handler. |
| `request` | One request's raw values and the rules that resolve them. Pure. |
| `prompt` | Chat template resolution and prompt building. |
| `sampler`, `sampler_profile` | The sampler chain. |
| `mtp`, `dflash` | Speculative decoding. |
| `tool_calls` | Parsing tool calls from a local reply. Pure. |

## Planning

`offload` decides how many layers go to the GPU. Offload units are the repeating blocks plus the output layer, which is offloaded first and can cost many times a block. Per-unit weights come from the GGUF tensor index; a tied output head charges the embedding to the output unit. KV cache bytes come from per-layer geometry: sliding-window layers are capped at `PAD(min(ctx, n_swa + n_ubatch), 256)`, and bytes per value include the block scales of quantized types. The recommended context is found by bisecting against the real KV curve. `plan_smart_gpu_offload` is pure: it takes model metadata, unit costs and KV geometry, and asks once for a measured compute buffer. `plan_multi_gpu_distribution` turns a strategy (manual, priority, proportional, balanced) into `n_gpu_layers`, `tensor_split` and `main_gpu`.

`KvCacheTypes` carries separate K and V cache types (`kv_type_k` and `kv_type_v`, both or neither, never beside `kv_type`). With one shared type every formula runs the original expression; with different types each half is billed at its own type's bytes per value, which is how llama.cpp allocates the two tensors. The context params, compute probe, drafter, context key and runtime report (`k=<type>,v=<type>`) follow the pair.

`llama` measures what the planner needs: model metadata with weights kept on the CPU, KV geometry, GGUF unit costs, head dimensions and the compute buffer projected without allocating. Each is cached per model path for the life of the process.

`context` sizes the context itself: the (context, batch) fallback ladder, the OOM classifier and its error detail, the effective VRAM (backend free memory capped by the Windows DXGI budget), per-device VRAM alignment (missing devices imputed from the smallest reported one), the recommended context and the CPU fallback limits. It keeps its own older KV-per-value table. Formulas read a `ModelShape` rather than the model.

`hardware` reports available RAM, the largest free GPU memory ggml reports, the DXGI local-memory cap on Windows, the device list, per-device memory and unified memory. Integrated GPUs are listed (`IntegratedGpu`) and one can be the selected single device, with its budget read from that device; multi-GPU takes discrete devices only. On AMD APUs (Ryzen AI, handhelds) the iGPU's reported memory follows the carve-out the user set, and available RAM shrinks with it.

`context_info` is the model editor's estimate: maximum and recommended context, memory, GPU layers and multi-GPU placement. The draft model reserve (`offload::drafter_reserve_path`) is the DFlash drafter (or the MTP path) when DFlash is enabled, else the MTP draft when MTP is, and counts unless the draft is placed on the CPU.

## Loading

`LlamaEngine` (`engine`) holds the model slot:

- A load reuses the loaded model when the path and the full parameter key match. A smart-offload model already on the GPU is kept when only the candidate list changed. Otherwise the hot context is discarded and the old model dropped before the new one loads.
- Smart offload walks the GPU layer candidates down; a fixed layer count is tried once. Both fall back to the CPU (with a "switched to CPU" notice) unless strict mode forbids it.
- Multi-GPU loads select the devices, layer split, tensor split and main GPU on the raw params.
- The multimodal projector and the MTP draft model (GPU, with a CPU retry when allowed) reload with the model or when their own settings change.
- Load progress reports stage and status codes and per-GPU percentages through `EngineObserver`.
- llama.cpp's own fitter (`fit_model_params`, `measure_mmproj_fit_margins`) is ported but stays behind its gate.

## Generation

`LlamaRuntime::start` spawns the `lettuce-llama` thread. Requests run one at a time on it, because the thread owns the model slot and the hot context cache; `generate` queues a request and `unload` drops the model. For each request the handler:

1. Resolves the request (`request`): sampler profile defaults and filters, deduplicated devices, MTP and DFlash draft bounds, and the thinking switch (`/think` or `/no_think` in the messages, then the explicit flag, then a requested reasoning format).
2. Plans, using the per-model smart offload cache, the native fitter behind its gate, multi-GPU distribution and draft model placement.
3. Loads through the engine and creates a context, trying attempt groups in order: KV on the GPU, then KV in RAM, then smaller contexts, with one reload at the KV-aware layer estimate. A K/V type combination llama.cpp refuses fails with llama.cpp's own message instead of being treated as out of memory.
4. Reuses the longest cached prompt prefix from the hot context cache (1 GiB, oldest evicted first), rebuilding the MTP carry. Reuse follows llama-server's sliding-window rule (`prompt_cache_reuse_blocked`): when the target's cache, or an own-KV MTP or DFlash drafter's, no longer holds the cells a windowed SWA layer needs before the resume position, the whole prompt is evaluated again instead of trimmed.
5. Builds the prompt (`prompt`), samples (`sampler`), and decodes, with MTP or DFlash when enabled.
6. Streams text with stop-sequence hold-back and thinking-tag splitting (`lettuce-inference`'s parser), flushing every 32 ms or 256 bytes.
7. Parses tool calls from the reply (`tool_calls`), with raw-text recovery first.
8. Writes the runtime report through `RuntimeReportStore` and hands a `LlamaMetricsRecord` to the host.

Events go to a `GenerationObserver` (deltas, reasoning, tool calls, heartbeats, notices such as MTP being disabled for a vision request or the KV cache moved to RAM, runtime report updates). The caller persists metrics and ends the stream. An abort is a typed error. `LlamaHost` is what the application implements: runtime report storage, the metrics sink and the frontend events.

## Prompt

`prompt` resolves the chat template from the explicit override, then the GGUF's embedded template, then a named preset. Tool definitions or tool messages take the OpenAI-compatible template path: `tool_choice` `auto`, `none` or `required`, a named tool narrows the list and becomes `required`, and a tool-marker heuristic and missing native parser metadata go into diagnostics. Plain chats try the same path, then llama.cpp's basic template call. A `role: content` transcript ending in `assistant: ` is the opt-in fallback when resolution or application fails. Media parts become mtmd markers; inline images are normalized to PNG. BOS is never added to templated prompts; raw completions follow `tokenizer.ggml.add_bos_token`, defaulting to on. Errors are typed.

Gemma 4 forced reasoning adds `<|think|>\n` at the start of the first system message (or adds a system message), appends `<|channel>thought\n` to the built prompt, and splits streamed and final text as starting inside reasoning that ends at `<channel|>`.

## Sampler

`sampler` builds the chain: a profile (balanced, creative, stable, reasoning) and its defaults, the stage order (default or the user's, deduplicated; an explicit empty list means no stages), penalties only when a penalty is set, DRY, XTC, typical and min-p parameters, the template's lazy grammar forced to the front, then `dist` (seeded, random seed when none) above zero temperature, or `greedy`. `adaptive_p` is the tenth default stage; its position in the order is ignored, and when the order includes it and the target is in (0, 1] it replaces the final `dist` or `greedy` step with `adaptive_p(target, decay or 0.95 clamped to 0..0.99, seed)`.

DRY sequence breakers are decoded once (`decode_llama_sequence_breaker`), so a `\n` breaker survives.

## Speculative decoding

### MTP

`mtp` detects bundled NextN layers and discovers external `mtp-*.gguf` drafts. A draft context sits beside the target: with its own KV when the draft is as wide as the target, or sharing hidden state when its output width matches. Drafting is greedy and stops below 0.75 top-10 probability or at end of generation; verification uses the request's sampler, and the KV is rolled back at the first mismatch. The prompt cache is trimmed or rewound to match. The draft length (default 4, at most 8) halves under 50% acceptance and grows at 80% or more, judged every 8 rounds. A round never drafts more than one verification batch can carry within the target's `n_batch` (the drafts alone with an own KV, one fewer with shared hidden state). Empty draft logits stop drafting. Greedy MTP output is checked token for token against plain greedy decoding on a real model.

### DFlash

`dflash` runs DFlash, DFlash2 and DSpark drafters, following llama.cpp's `common/speculative.cpp` (b11157).

Drafter selection:

- A drafter is a GGUF whose architecture is `dflash` or that carries `dflash.block_size` (16 when absent). The configured `dflash_model_path` is used only when it is one. Otherwise a sibling file whose stem contains `dflash` is used, but only when its stem matches the model's once the marker and its separators are dropped (a bare `dflash.gguf` names no model); the first by name wins.
- A drafter with `markov_w1.weight` is DSpark; otherwise one with a selector (`dflash_selector_top_k` > 0) is DFlash2; otherwise DFlash.
- The drafter takes the MTP draft slot (placement, VRAM reserve, engine load), runs only without media, and wins over MTP, with a warning when both are enabled.

Setup checks, each of which turns into a warning and a run without DFlash: the drafter's target layers exist in the target; its `n_embd` equals the target's (llama.cpp sizes the injection input as target layers times the drafter's `n_embd` while the features arrive at the target's width); its vocabulary passes llama.cpp's speculative compatibility check (`LlamaModel::is_speculation_compatible`); its mask token is inside its vocabulary; a DFlash2 selector fits the output rows; a DSpark drafter whose `dflash.has_confidence_head` is anything but `true` runs only with `dflash_min_probability` 0 (a missing key counts as having the head); and the target batch can verify a draft.

The run:

1. The drafter context is a default context bound to the target (`ctx_other`), without recurrent snapshots, sized for one noise block of outputs.
2. Every target batch, prefill chunks and verification alike, writes the target's inputs at the drafter's `target_layer_ids` into the drafter's cache with one embedding decode per drafter micro-batch. These batches are embeddings-only (`LlamaBatch::new_embeddings_only_with_position_rows`), because the injection graph has no token input and crashes on a batch carrying tokens. For an M-RoPE drafter they carry four position rows (the position three times, then 0), since llama.cpp reads every row of an embedding batch; token batches share one row.
3. The first round samples the anchor from the prompt's logits. Each later round decodes `[anchor, mask...]` in the drafter (non-causal unless `dflash.attention.causal` is `true`) and reads greedy tokens by their top-10 softmax until one falls below `dflash_min_probability` (default 0.55). DFlash2 walks its dense nextn selector lattice slot by slot instead of logits. DSpark's Markov head biases the block's logits inside the drafter graph; it reads greedy top-10 tokens and stops at the first slot whose confidence (the sigmoid the confidence head writes into the nextn row) is below `dflash_min_probability`, reading no confidence at 0.
4. With `dflash.sample_from_anchor` (default `true`) a DSpark block is `[anchor, mask...]` of exactly the draft length and slot 0 is already a draft; otherwise, and always for DFlash and DFlash2, the block is one longer and is read from slot 1. Blocks stay within the trained `dflash.block_size`, since the Markov bias is skipped for a longer one.
5. The block is dropped from the drafter's cache, anchor and drafts are verified in one target batch with the request's sampler (one sample per row), the accepted prefix is kept in both caches, and the sampled token becomes the next anchor.

The draft length is 1 to 15 (default 4), capped at the block size for a DSpark drafter sampling from the anchor and one below it otherwise, kept below the target's `n_batch`, and adapts like MTP's. The hot context key includes DFlash and its draft length; a reused drafter takes the request's `dflash_min_probability`, and a reused DSpark drafter without a confidence head is dropped for a request above 0. Stats use the MTP shape: `usage.mtp_stats` and the runtime report's `dflashStats`. Draft tokens are picked on the CPU; upstream's backend draft sampler (`llama_set_sampler`) is not used.

Known limits:

- Once a DFlash drafter resolves, bundled MTP is not loaded and the drafter holds the draft slot, so if DFlash setup then fails the run has neither. A DSpark drafter without a confidence head at the default `dflash_min_probability` of 0.55 is such a failure. Falling back to MTP would need the target reloaded with its NextN layers, or the draft slot reloaded with the MTP model and a target context built for MTP's draft length.
- The VRAM reserve (`offload::drafter_reserve_path`) reserves the MTP path when DFlash is enabled without a drafter path, even with MTP off, and the offload plan's `bundled_mtp_draft` still counts bundled NextN layers that DFlash keeps from loading, so both can over-reserve.
- DSpark shares `dflash_min_probability` as its confidence cutoff, so at 0.55 it stops drafting earlier than upstream's default of 0.
- The draft length stops at 15, so a DSpark drafter trained on a 16-token block never drafts its full block.
- DeepSeek-V4 DSpark drafters are untested.

Ignored tests compare greedy DFlash output with plain generation on a counting prompt (`LETTUCE_DFLASH_MODEL`) and greedy DSpark output on a 64-token explanation (`LETTUCE_DSPARK_MODEL`), each requiring at least half of the drafts accepted.

## Tool calls

`tool_calls` parses tool calls from a local reply, including raw-text recovery for models that write them inline: `<tool_call>` blocks, JSON, `<function=...>` tags and `<parameter=...>` arguments.

## Tests on real models

Ignored tests run against real files: `LETTUCE_PLAN_MODEL` prints the offload plan, an engine test loads a model on the CPU, and the generation path has been checked on a real model for CPU generation, prompt-cache reuse and MTP.
