# lettuce-local-llm

The local llama.cpp runtime adapter.

## Boundary

Local runtime behavior is isolated from conversation logic. The llama.cpp
bindings come from the `MegalithOfficial/llama-cpp-rs` fork, pinned to the
commit the legacy app shipped with (`07a8b3b`, `mtmd` feature), and build on
desktop only (Android and iOS never link llama.cpp, as in legacy). GPU
backends are the `cuda`, `cuda-no-vmm`, `rocm`, `vulkan` and `metal`
features. whisper.cpp (lettuce-speech) links into the same binary.

## Status

- `offload`: the legacy GPU offload planner, ported with its formulas,
  constants and tie-breaks unchanged (user rule: runtime formulas are
  frozen). Offload units are the repeating blocks plus the output layer,
  which is offloaded first; per-unit weights come from the GGUF tensor index
  (a tied head charges the embedding to the output unit); KV cache bytes come
  from per-layer geometry with sliding-window layers capped to
  `PAD(min(ctx, n_swa + n_ubatch), 256)`; KV bytes per value include block
  scales; the recommended context is bisected against the real KV curve; the
  multi-GPU strategies (manual, priority, proportional, balanced) turn into
  `n_gpu_layers`, `tensor_split` and `main_gpu`. The three unmeasured
  guesses (90% VRAM budget, 5%/256 MiB compute reserve floor, safety factor 2)
  stay as they were. The planner is pure: it takes model metadata, unit costs
  and KV geometry, and asks once for a measured compute buffer. All legacy
  unit tests are carried over.
- `llama` (desktop): the process-wide backend, model metadata (weights kept
  on the CPU), KV geometry, GGUF unit costs, head dimensions and the compute
  buffer projected without allocating, each cached per model path for the
  life of the process, as legacy did. An ignored test prints the plan for a
  real model (`LETTUCE_PLAN_MODEL`).

- `engine` (desktop): the model slot (`LlamaEngine`), ported from the
  legacy `load_engine`: a load reuses the loaded model when the path and the
  full parameter key match (a smart-offload model already on the GPU is kept
  when only the candidate list changed), otherwise the hot context is
  discarded and the old model dropped before loading; smart offload walks the
  GPU layer candidates down, a fixed layer count is tried once, and both fall
  back to the CPU (with a "switched to CPU" notice) unless strict mode
  forbids it; multi-GPU loads select the devices, layer split, tensor split
  and main GPU on the raw params; the multimodal projector and the MTP draft
  model (GPU with CPU retry when allowed) reload with the model or when their
  own settings change. Load progress keeps the legacy stage/status codes and
  per-GPU percentages; Tauri events became an `EngineObserver` and logs go to
  `tracing`. llama.cpp's own fitter (`fit_model_params`,
  `measure_mmproj_fit_margins`) is ported but stays behind its gate. An
  ignored test loads a real model on the CPU.

- `context`: the legacy context sizing, unchanged: the (context, batch)
  fallback ladder, the OOM classifier and its error detail, the effective
  VRAM (backend free memory capped by the Windows DXGI budget), per-device
  VRAM alignment (missing devices imputed from the smallest reported one),
  the recommended context and the CPU fallback limits. It keeps its own older
  KV-per-value table, as legacy did. Model-dependent formulas read a
  `ModelShape` instead of the model.
- `hardware` (desktop): available RAM (sysinfo 0.33), the largest free GPU
  memory ggml reports, the DXGI local-memory cap on Windows, the GPU device
  list, per-device memory and unified-memory detection, as in legacy.
- `context_info` (desktop): the model editor's fit estimate (max and
  recommended context, memory, GPU layers, multi-GPU placement).
- `mtp` (desktop): bundled NextN detection, external `mtp-*.gguf` discovery,
  and the legacy draft/verify runtime unchanged: a draft context beside the
  target (own KV when the draft is as wide as the target, shared hidden state
  when its output width matches), greedy drafting that stops below 0.75 top-10
  probability or at end-of-generation, verification with the request's
  sampler, KV rollback on the first mismatch, prompt-cache trim/rewind, and the
  draft length halving under 50% acceptance or growing at 80% or more, judged
  every 8 rounds. Legacy panicked on empty draft logits; drafting now stops
  there instead. Greedy MTP output is checked token for token against plain
  greedy decoding on a real model.
- `sampler` (desktop): the legacy sampler chain unchanged: profiles
  (balanced/creative/stable/reasoning and their defaults), the stage order
  (default or the user's, deduplicated; an explicit empty list means no
  stages), penalties only when a penalty is set, DRY/XTC/typical/min-p
  parameters, the template's (lazy) grammar forced to the front, then `dist`
  (seeded, random seed when none) above zero temperature or `greedy`.
- `prompt` (desktop): the legacy prompt builder unchanged. The template
  resolves from the explicit override, the GGUF's embedded template, then a
  named preset. Tool definitions or tool messages take the OpenAI-compatible
  template path (tool_choice `auto`/`none`/`required`, a named tool narrows
  the list and becomes `required`, a tool-marker heuristic and missing native
  parser metadata go into diagnostics); plain chats try the same path, then
  llama.cpp's basic template call; the `role: content` transcript ending in
  `assistant: ` is the opt-in fallback when resolution or application fails.
  Media parts become mtmd markers. BOS is never added to templated prompts;
  raw completions follow `tokenizer.ggml.add_bos_token`, defaulting to on.
  Errors are typed with legacy wording, without legacy's module/line prefix.

AMD Ryzen AI / handheld APUs (user requirement 2026-09-21) share one
user-adjustable memory pool between the iGPU and the CPU. Legacy left iGPUs
out of the device list, refused an iGPU as the selected device and skipped it
in per-device VRAM, so an APU had nothing to pick; that is corrected: iGPUs
are listed (`device_type` `IntegratedGpu`), a single selected device may be
an iGPU and its budget is read from that device, while multi-GPU still takes
discrete devices only. The iGPU's reported memory follows the carve-out the
user set, and available RAM shrinks with it. Whether the budgets need more for
unified memory (for example memory the iGPU can borrow beyond the carve-out)
is still open: it needs measurement on the hardware and the user's approval,
since the formulas are frozen.

Next: contexts and the hot-context cache, request handling, the worker thread and the inference port.
