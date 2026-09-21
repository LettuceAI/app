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

Next: the model/context engine and its worker thread, prompt templates and
sampler, MTP, request handling and the inference port.
