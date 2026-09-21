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
- `mtp`: bundled NextN detection and external `mtp-*.gguf` discovery.

Known gap (user requirement 2026-09-21): AMD Ryzen AI / handheld APUs share
one user-adjustable memory pool between the iGPU and the CPU. Legacy (and
this port, so far) leaves iGPUs out of the device list, refuses an iGPU as the
selected device and skips it in per-device VRAM; offload budgets treat VRAM as
a separate card. Listing and selecting iGPUs is a behavior fix to make; any
budget change for unified memory needs measurement on the hardware and the
user's approval, since the formulas are frozen.

Next: contexts and the hot-context cache, prompt templates and sampler, MTP
runtime, request handling, the worker thread and the inference port.
