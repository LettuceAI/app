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
  recommended context, memory, GPU layers, multi-GPU placement). The draft
  model reserve (`offload::drafter_reserve_path`) is the DFlash drafter (or
  the MTP path) when DFlash is enabled, else the MTP draft when MTP is, and
  counts unless the draft placement is the CPU.
- `mtp` (desktop): bundled NextN detection, external `mtp-*.gguf` discovery,
  and the legacy draft/verify runtime unchanged: a draft context beside the
  target (own KV when the draft is as wide as the target, shared hidden state
  when its output width matches), greedy drafting that stops below 0.75 top-10
  probability or at end-of-generation, verification with the request's
  sampler, KV rollback on the first mismatch, prompt-cache trim/rewind, and the
  draft length halving under 50% acceptance or growing at 80% or more, judged
  every 8 rounds. Legacy panicked on empty draft logits; drafting now stops
  there instead. A round never drafts more than one verification batch can
  carry within the target's `n_batch` (the drafts alone with an own KV
  cache, one fewer with a shared hidden state); legacy could exceed it with
  a batch size at or below the draft length, which aborts llama.cpp. This
  only bounds the drafted count; the draft context is sized as before.
  Greedy MTP output is checked token for token against plain greedy
  decoding on a real model.
- `dflash` (desktop): DFlash, DFlash2 and DSpark speculative decoding. Settings, the shared
  draft slot, stats and the adaptive draft length come from release 2.2.5;
  the runtime follows llama.cpp's `common/speculative.cpp` (b11157) because
  legacy's never ran (see below).
  A drafter is a GGUF whose architecture is `dflash` (as llama.cpp
  identifies one) or that carries `dflash.block_size` (block size 16 when
  absent). The configured `dflash_model_path` is used only when it is one;
  otherwise a sibling whose stem contains `dflash` is discovered, but only
  when its stem shares the model's once the marker and its separators are
  dropped (a bare `dflash.gguf` names no model), the first by name. A
  drafter carrying `markov_w1.weight` is DSpark (as upstream detects it),
  else one with a selector (`dflash_selector_top_k` > 0) is DFlash2, else
  DFlash; DSpark runs through the same runtime with its own block layout,
  draft limit and truncation. The
  drafter takes the MTP draft slot (placement, VRAM reserve, engine load),
  runs only without media, and wins over MTP (a warning when both are
  enabled). Setup checks, each failing to a warning and a run without
  DFlash: the drafter's target layers exist in the target; its own `n_embd`
  equals the target's (llama.cpp sizes the injection input as target layers
  times the drafter's `n_embd`, while the features arrive at the target's);
  its vocabulary passes llama.cpp's speculative compatibility check
  (`LlamaModel::is_speculation_compatible`: vocab type, BOS/EOS, sizes at
  most 128 apart, token text from id 5); its mask token lies inside its
  vocabulary; a DFlash2 selector fits the output rows; a DSpark drafter
  with `dflash.has_confidence_head` set to anything but `true` runs only
  with `dflash_min_probability` 0 (upstream refuses it; a missing key counts
  as having the head); and the target batch can verify a draft.
  The drafter context is a default context bound to the target
  (`ctx_other`), without recurrent snapshots, sized for one noise block of
  outputs. Every target batch (prefill chunks and verification) writes the
  target's inputs at the drafter's `target_layer_ids` into the drafter's
  cache with one embedding decode per drafter micro-batch, which projects
  and stores them. Those batches are embeddings-only
  (`LlamaBatch::new_embeddings_only_with_position_rows`): the injection
  graph has no token input and crashes on a batch carrying tokens. For an
  M-RoPE drafter they carry four position rows written as upstream does
  (the position three times, then 0), because llama.cpp reads every row of
  an embedding batch; token batches (the noise block, verification) share
  one row as upstream's do. The first round samples the anchor from the
  prompt's logits; each later round decodes `[anchor, mask...]` in the
  drafter (non-causal unless `dflash.attention.causal` is `true`), reads
  greedy tokens by their top-10 softmax until one falls below
  `dflash_min_probability` (default 0.55), drops the block from the
  drafter's cache, verifies anchor and drafts in one target batch with the
  request's sampler (one sample per row), keeps the accepted prefix in both
  caches and makes the sampled token the next anchor. DFlash2 drafters
  (`dflash_selector_top_k` > 0) read their dense nextn selector lattice
  instead of logits and walk it slot by slot. DSpark's Markov head biases
  the block's logits inside the drafter graph. With
  `dflash.sample_from_anchor` (default `true`) a DSpark block is `[anchor,
  mask...]` of exactly the draft length and slot 0 is already a draft;
  without it, and for DFlash and DFlash2 (which ignore the flag as upstream
  does), the block is one longer and is read from slot 1. DSpark reads greedy
  top-10 tokens and stops at the first slot whose confidence (the sigmoid
  the confidence head writes into the nextn row) falls below
  `dflash_min_probability`, and reads no confidence at 0; the token's own
  probability is not checked. Every block stays within the trained
  `dflash.block_size`, since the Markov bias is skipped for a longer one.
  Draft length is 1 to 15 (default 4), capped at the block for a DSpark
  drafter sampling from the anchor and one below it otherwise, and below the
  target's `n_batch`, and adapts like MTP. The hot context key carries DFlash and its
  draft length; a reused drafter takes the request's
  `dflash_min_probability`, and a reused DSpark drafter without a
  confidence head is dropped for a request above 0. Stats use the MTP shape: `usage.mtp_stats` and
  the runtime report's `dflashStats`.
  Known limits: once a DFlash drafter resolves, bundled MTP is not loaded
  and the drafter holds the draft slot, so if DFlash setup then fails the
  run has neither (a DSpark drafter without a confidence head at the default
  `dflash_min_probability` of 0.55 is such a failure, where upstream's
  default is 0); falling back to MTP would need the target reloaded with
  its NextN layers or the draft slot reloaded with the MTP model and a
  target context built for MTP's draft length, which the run does not do.
  The VRAM reserve keeps legacy's selection (`offload::drafter_reserve_path`):
  with DFlash enabled and no drafter path it reserves the MTP path even with
  MTP off, and the offload plan's `bundled_mtp_draft` still counts bundled
  NextN layers that DFlash keeps from loading, so both can over-reserve.
  DSpark shares `dflash_min_probability` as its confidence cutoff, so at the
  default 0.55 it stops drafting earlier than upstream's default of 0.
  Draft length stops at 15, so a DSpark drafter trained on a 16-token block
  never drafts its full block. Upstream's backend draft sampler
  (`llama_set_sampler`) is not used; draft tokens are picked on
  the CPU. DeepSeek-V4 DSpark drafters are untested.
  Legacy corrections: legacy never reached DFlash from chat (its provider
  field allowlist lacked the `llamaDflash*` keys), and its runtime could not
  have run: it built an MTP-type drafter context (null for a model without
  NextN layers, and the DFlash graph aborts on the MTP decoder), ran a
  separate encoder pass the fused injection decode replaced, kept one target
  output (a llama.cpp abort on the first verification batch), sampled
  matched rows twice, wrote only the bonus token's features into the
  drafter, never bounded the draft by the target batch, and discovered any
  `*dflash*.gguf` beside the model. An ignored test compares greedy DFlash
  output with plain generation on a counting prompt when
  `LETTUCE_DFLASH_MODEL` is set, and greedy DSpark output on a 64-token
  explanation when `LETTUCE_DSPARK_MODEL` is (DeepSeek's Qwen3-4B block-7
  drafter barely drafts terse counting, upstream included), each requiring
  at least half of the drafts accepted.
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

- `request`: one generation request with the user's raw values and the
  legacy rules that resolve them (sampler profile defaults and filters,
  deduplicated devices, MTP and DFlash draft bounds, `/think`/`/no_think` over the
  explicit flag over a requested reasoning format), plus the stop matcher,
  stream flush rule, cache eviction count and context key. Fixed legacy bug:
  DRY sequence breakers are decoded once. Legacy decoded them in the chat
  layer and again in the runtime, and the second `trim()` turned a decoded
  newline into an empty breaker that was dropped, so the default
  `\n, :, ", *` list never had its newline and a `\n`-only list fell back to
  the four defaults. llama.cpp's own default breakers include the newline.
- `tool_calls`: the legacy tool-call parser for a local reply, including raw
  text recovery (`<tool_call>` blocks, JSON, `<function=...>` tags,
  `<parameter=...>` arguments).
- `generation` (desktop): the legacy request handler on one worker thread
  that owns the model slot and the hot context cache (1 GiB, oldest first):
  planning with the per-model smart offload cache, the native fitter behind
  its unchanged gate, multi-GPU distribution, MTP drafter placement, the
  context attempt groups (GPU KV, then KV in RAM, then smaller contexts) with
  one reload at the KV-aware layer estimate, prompt-prefix reuse with the MTP
  carry rebuild, streaming with stop-sequence hold-back and thinking-tag
  split, the structured tool-call parse with raw-text recovery first, the
  runtime report (through `RuntimeReportStore`) and the metrics record.
  Events go to a `GenerationObserver`; the caller persists metrics and ends
  the stream. Corrections: aborts are a typed error (legacy matched the word
  "aborted" in any error text); inline images, the `image` 0.25 PNG
  normalization and base64 handling are unchanged. Verified on a real model:
  CPU generation, prompt-cache reuse and MTP.
  Prompt-prefix reuse follows llama-server's sliding-window rule
  (`request::prompt_cache_reuse_blocked`): when the target's cache, or an
  own-KV MTP or DFlash drafter's, no longer holds the cells a windowed SWA
  layer needs before the resume position (its oldest position lies after
  `resume - n_swa`; a recurrent state holds only its latest), the whole
  prompt is evaluated again instead of trimming.

Separate K and V cache types (user request 2026-09-21): `kv_type_k` and
`kv_type_v` on `LlamaCppSettings` (both or neither, never beside `kv_type`)
become `KvCacheTypes`. With one shared type every sizing formula runs the
legacy expression unchanged (all legacy tests pass as before); with
different types each half is billed at its own type's bytes per value,
which is how llama.cpp allocates the K and V tensors (checked on a real
model: K q8_0 34 MiB and V q4_0 18 MiB at 2048 cells, as predicted). The
context params, compute probe, drafter, context key, runtime report
(`k=<type>,v=<type>`) and planning config follow the pair.

`LlamaHost` is what the application provides: the runtime report store, the
metrics sink and the frontend events (model load progress, GPU fallback,
heartbeats, notices, report updates). The provider adapter lives in
`lettuce-providers` (`llama_cpp.rs`).

Next: report/metrics storage and host events in the app, the runtime
commands (devices, embedded template, unload, context info), then
stable-diffusion.cpp.

Release 2.2.5 additions: the adaptive-p sampler (`adaptive_p` is the tenth
default stage; its place in the order is ignored, and when the order asks for
it and the target is in (0, 1] it replaces the final `dist`/`greedy` step with
`LlamaSampler::adaptive_p(target, decay or 0.95 clamped to 0..=0.99, seed)`),
and Gemma4 forced reasoning (`<|think|>\n` opens the first system message, or
a system message is added; `<|channel>thought\n` is appended to the built
prompt; streamed and final text are split starting inside reasoning that ends
at `<channel|>`).

The llama-cpp-rs fork is pinned to `779747a` (llama.cpp b11157). Changes
that reach this crate: `LlamaContextParams::default()` gives the SWA layers
a windowed cache (`swa_full` false, as llama-server does; the sizing
formulas already assumed it, and `swa_full` stays a user setting); a K/V
cache type combination llama.cpp refuses (quantized V without flash
attention, mixed types on MLA models, a block size that does not divide a
head) now fails context creation with its own message instead of a null
context, so it is not retried as an out-of-memory fallback and reaches the
user inside the context error unchanged. Known and left alone because the
formulas are frozen: with YaRN and a custom `rope_freq_scale`, llama.cpp
now rewrites the loaded model's `n_ctx_train` when a context is created
(upstream dfc29b64e), so later reads of the training context on that model
see the scaled value; and the target context no longer allocates KV for
bundled NextN layers on any architecture (upstream 5cdd3d1da), so the
bundled-MTP KV reserve in the offload plan over-reserves slightly.
