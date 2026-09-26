# lettuce-image-generation: legacy parity notes

Facts about how `lettuce-image-generation` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- The domain core ports legacy `image_generator/commands.rs` and `types.rs`.
- Per-request settings over the model's: legacy callers spread their settings over the model's `advancedModelSettings`.
- `ImageGenerationSource` is legacy's `usage_source` (none, scene, playground, creation helper).
- `merge_loras`, `lora_keywords` and `compose_image_prompt` are legacy's prompt composition, unchanged, with the legacy tests.
- `resolve_image_profile` follows legacy's output scopes; text output capability becomes legacy's "text" output modality.
- `ImageProviderError` keeps the message legacy showed.
- `shrink_for_upload` is legacy's `input_images` (2048 px, 4 MiB, PNG or JPEG 90/82/74, mask follows, GIFs untouched).
- The coordinator records job usage for success and failure like legacy, and keeps the images when a usage write fails after the provider answered, as legacy did.
- The stable-diffusion.cpp catalog and the RealESRGAN upscaler pin were extracted mechanically from the legacy source; `catalog.rs` keeps legacy's lookups and error texts.
- `sd_runtime` keeps legacy's frozen auto-fit estimate and `--backend` specs, the per-build compute policy (legacy name-based files migrate), engine device matching, the native `img_gen` payload with legacy defaults and reference rules, the console handling (240-line tail, OOM signatures, throttled progress), GitHub release filtering per platform, and legacy's on-disk layout, so legacy installs are reused.
- `LocalDiffusionEngine` keeps legacy's arguments and their order, server reuse, five-minute readiness, 500 ms polling for ten minutes and the single `--offload-to-cpu` retry. It was verified against the real engine (Vulkan build, FLUX.2 Klein 4B, a Klein LoRA whose compatibility cache equals legacy's byte for byte).
- Starting the image server unloads llama.cpp and every llama.cpp request stops the image server first; a failed stop fails that request, as legacy did.
- The LoRA library is legacy's `image_loras`, including the single CivitAI lookup by hash. Generations on the local engine take LoRA keywords from the library before composing the prompt, as legacy did for sdcpp.
- Runnability is legacy's `sdcpp_runnability`: verdict strings, reason texts, check order and the probe payload are legacy's.
- `hf_bundle` holds legacy's bundle rules and writes the bundle manifest in legacy's JSON format.
- `civitai` holds legacy's LoRA browsing rules and status texts.
- The playground lists 30 entries by default, 1 to 200, like the old playground.

## Deliberate differences from legacy

- Outputs pass media validation; legacy saved an invalid output as a broken file.
- Base LoRAs and the pre-prompt come from the effective settings for every caller. Legacy sdcpp read base LoRAs from the stored model, while the creation helper sent no settings at all, so its prompts skipped the model's pre-prompt and LoRA keywords.
- Request bounds legacy lacked: at most 10 images, 16 input images, a 64 KiB prompt, trimmed size, quality and style.
- Input images are turned upright by their EXIF orientation before resizing. Legacy ignored it and dropped the EXIF on re-encode, so rotated photos went out sideways and the mask followed the sideways size. A same-size re-encode that is not smaller keeps the original bytes.
- Uninstalling a catalog variant checks each model's stored engine build instead of legacy's active one before removing an unused build.
- Runnability transport errors carry the shared HTTP client's texts and limits (64 MiB requests, a poll URL must be a plain path).

## History

- The previous README listed as not yet ported: the runnability probe, Hugging Face image bundles and the component library, and importing legacy `image_loras` and `playground_generations` rows; and as next steps the scene, playground and creation-helper callers. Runnability, bundles and the CivitAI rules are in the crate now, the legacy import reads `image_loras` and playground rows, and the scene and playground callers exist in `lettuce-app`. Check the code before treating the component library or the creation-helper caller as open.
- The legacy LoRA keyword fill was first deferred to the LoRA library slice and is now done.
- App commands built on this crate: installed catalog variants, uninstall (shared files kept, unused engine build removed on request), registration repair, the LoRA library, and upscaling a stored image into a new asset.
- The previous README described the request's output retention; it is now `ImageOutputPolicy` (`Retained`, or `Preview` with an expiry).
