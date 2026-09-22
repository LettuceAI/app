# lettuce-image-generation

Capability-aware remote and local image requests, Stable Diffusion runtime,
LoRA application, upscale, jobs, output ingestion, and provenance.

## Boundary

Remote orchestration and the local SD runtime remain separate internal modules.
Permanent outputs exist only after media validation.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

Domain core (legacy `image_generator/commands.rs` + `types.rs`):

- `ImageGenerationRequest`: model profile, prompt, per-request
  `StableDiffusionSettings` laid over the model's (legacy callers spread their
  settings over the model's `advancedModelSettings`; the sd.cpp binding always
  stays the model's), input and mask image assets, request LoRAs, size,
  quality, style, count, source (legacy `usage_source`: none, scene,
  playground, creation helper), attribution and output retention.
- Legacy prompt composition, unchanged: `merge_loras` (a request LoRA replaces
  the model's entry for the same file and noise stage), `lora_keywords`
  (trimmed, case-insensitive dedupe, first spelling kept) and
  `compose_image_prompt` (pre-prompt, then keywords the prompt does not
  already contain, then the prompt, joined with `", "`), with the legacy tests.
- `resolve_image_profile`: an image model is any model whose output image
  capability is Supported (legacy output scopes), on an enabled account with a
  valid connection; text output capability becomes legacy's "text" output
  modality.
- `ImageProviderPort`: one generation with the composed prompt, effective
  settings, merged LoRAs and input bytes; outputs come back as bytes (adapters
  fetch URLs), with optional token usage. `ImageProviderError` keeps the
  message legacy showed.
- `ImageMedia` for `LocalMediaBlobStore`: reads input images and ingests each
  output as a `GeneratedImage` asset with producing-job and model provenance.
- `ImageGenerationRecord` / `ImageGenerationRepository`: the durable request
  and terminal state.

The app's `ImageGenerationCoordinator` runs each request as an
`ImageGenerate` job (interrupted, never re-run, after a crash), records job
usage for success and failure like legacy, and settles the record once. A job
that ended without running to completion (interrupted, or cancelled while
queued) has its record settled as failed ("Image generation was interrupted.")
or cancelled, and its open usage settled as failed, on the next admission or
claim. A usage write that fails after the provider answered is logged and the
images are kept, as legacy did.

Deferred to the stable-diffusion.cpp LoRA library slice: legacy filled sdcpp
LoRA keywords from the stored LoRA library before composing the prompt.

Deliberate corrections:

- Outputs pass media validation. An output that is not a valid image is
  dropped and counted (`rejected_outputs`) instead of being saved as a broken
  file; the request fails only when no output is valid.
- Base LoRAs and the pre-prompt come from the effective settings for every
  caller. Legacy sdcpp read base LoRAs from the stored model while the
  creation helper sent no settings at all, so its prompts skipped the model's
  pre-prompt and LoRA keywords.
- Request bounds legacy lacked: at most 10 images, 16 input images, a 64 KiB
  prompt, and trimmed size/quality/style values.

stable-diffusion.cpp (in progress):

- `resources/stable-diffusion-cpp-catalog.json`: the legacy one-click model
  catalog (8 profiles, variants, pinned repository/revision/size/SHA-256 of
  every component, the bundle markers legacy used to recognise user-picked
  files) and the pinned RealESRGAN upscaler, extracted mechanically from the
  legacy source. `catalog.rs` types and validates it and keeps legacy's
  lookups and error texts.
- `sd_runtime`: the frozen auto-fit placement estimate and `--backend`
  specs, the per-build compute policy (legacy name-based files migrate),
  engine device matching, the native `img_gen` payload with legacy defaults
  and reference rules, console output (240-line tail, OOM signatures,
  throttled progress per stream), GitHub release filtering per platform, the
  legacy on-disk layout (engine builds, archives, content-addressed
  components, active build and policy files) so legacy installs are reused,
  and LoRA path normalization with the FLUX.2 Klein tensor alias cache.

- `sd_runtime::server::LocalDiffusionEngine`: the managed sd-server
  (legacy arguments and order, reuse while the model/build/policy key is the
  same, five-minute readiness, native job API polled every 500 ms for ten
  minutes, one retry with `--offload-to-cpu` after an out-of-memory failure
  under the automatic policy, cancel through the engine job or by stopping
  the server, shutdown). It implements `ImageProviderPort` for the managed
  `sdcpp` account; the job's cancellation token cancels the engine job.
  Verified against the real engine (Vulkan build, FLUX.2 Klein 4B, a Klein
  LoRA whose compatibility cache equals legacy's byte for byte).

The app composes it (`AppBackend::with_local_diffusion`): starting the image
server unloads llama.cpp, and every llama.cpp request stops the image server
first (a failed stop fails that request, as legacy did).

Next: the remote provider adapters and ComfyUI, the LoRA library, upscale,
runnability, then the scene, playground and creation-helper callers.
