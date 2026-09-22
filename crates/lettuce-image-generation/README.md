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

Next: the stable-diffusion.cpp runtime, the remote provider adapters and
ComfyUI, then the scene, playground and creation-helper callers.
