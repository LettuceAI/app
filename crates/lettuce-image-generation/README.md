# lettuce-image-generation

Image generation: the request and its durable record, prompt composition and LoRA merging, input image preparation, output ingestion as media assets, and the local stable-diffusion.cpp runtime (model catalog, engine builds, compute placement, the managed `sd-server`, LoRA library, upscaling). It also holds the rules for assembling image models from Hugging Face files and for browsing LoRAs on CivitAI, and the playground history types.

Remote image providers (the OpenAI-style, Gemini and other adapters and ComfyUI) live in `lettuce-providers` (`RemoteImageProviders`). Job admission, usage recording and the callers (scenes, playground, creation helper) live in `lettuce-app`, whose `AppImageProviders` routes `sdcpp` accounts to the local engine and every other kind to the remote adapters. The local runtime and the remote orchestration are separate modules that meet only at `ImageProviderPort`.

## Requests

`ImageGenerationRequest` (`request.rs`) is one generation: the model profile, the prompt, per-request `StableDiffusionSettings` laid over the model's own (the stable-diffusion.cpp binding always stays the model's; an empty `base_loras` drops the model's base LoRAs), input and mask image assets, request LoRAs, size, quality, style, count, an `ImageGenerationSource` (direct, scene, playground, creation helper), an `ImageAttribution` (conversation or character, for usage), and an `ImageOutputPolicy` (`Retained`, or `Preview` with an expiry). Validation bounds it at 10 images, 16 input images and a 64 KiB prompt, and trims size, quality and style.

`ImageGenerationRecord` and `ImageGenerationRepository` hold the durable request and its terminal state.

### Model

`resolve_image_profile` (`profile.rs`) accepts any model whose output image capability is `Supported`, on an enabled account with a valid connection. A model that also outputs text reports "text" as an output modality.

### Prompt

`prompt.rs` composes the prompt that reaches the provider:

- `merge_loras`: a request LoRA replaces the model's entry for the same file and noise stage.
- `lora_keywords`: the trigger keywords of the merged LoRAs, trimmed and deduplicated case-insensitively, first spelling kept.
- `compose_image_prompt`: the pre-prompt, then the keywords the prompt does not already contain, then the prompt, joined with `", "`.

Base LoRAs and the pre-prompt come from the effective settings for every caller, so every caller gets the model's pre-prompt and LoRA keywords. On the local engine, LoRA keywords are filled from the LoRA library before the prompt is composed.

### Input images

`shrink_for_upload` (`input_images.rs`) prepares reference images. Pixels are first turned upright by their EXIF orientation. An image larger than 2048 px (Lanczos3) or 4 MiB is re-encoded: PNG when any pixel is transparent, else JPEG at quality 90, 82, then 74 until it fits. Whenever the first image is resized or re-encoded, the mask follows its upright size (Nearest, PNG). GIFs pass through untouched, and a same-size re-encode that is not smaller keeps the original bytes.

### Provider port and outputs

`ImageProviderPort` (`port.rs`) runs one generation with the composed prompt, effective settings, merged LoRAs and input bytes, and returns output bytes (adapters fetch URLs themselves) with optional token usage. `ImageProviderError` carries the message shown to the user.

`ImageMedia` (`media.rs`) is the port to `lettuce-media`: it reads input images and ingests each output as a `GeneratedImage` asset with the producing job and model as provenance. Outputs go through media validation: an output that is not a valid image is dropped and counted in `rejected_outputs`, and the request fails only when no output is valid. A permanent output exists only after that validation.

### Running a request

`lettuce-app`'s `ImageGenerationCoordinator` runs each request as an `ImageGenerate` job that is interrupted, never re-run, after a crash. It records job usage for success and failure and settles the record once. A job that ended without running to completion (interrupted, or cancelled while queued) has its record settled as failed ("Image generation was interrupted.") or cancelled, and its open usage settled as failed, on the next admission or claim. If the usage write fails after the provider answered, the error is logged and the images are kept.

## Local stable-diffusion.cpp

`sd_runtime` runs stable-diffusion.cpp as a managed `sd-server` process.

- Catalog. `resources/stable-diffusion-cpp-catalog.json` is the one-click model catalog: 8 profiles with their variants, the pinned repository, revision, size and SHA-256 of every component, the bundle markers that recognize user-picked files, and the pinned RealESRGAN upscaler. `catalog.rs` types and validates it and provides the lookups and error texts.
- Layout (`layout.rs`). Engine builds, archives, content-addressed components, LoRAs, upscalers, and the active build and policy files, in the on-disk layout existing installs use, so their builds and models keep working.
- Releases (`releases.rs`). Engine builds come from the upstream GitHub releases, filtered per platform and resolved at runtime; no engine version is pinned.
- Policy (`policy.rs`). Per-build GPU selection, stored next to each installed build, with engine device matching.
- Fit (`fit.rs`). The auto-fit placement estimate and `--backend` specs, mirroring upstream `src/core/backend_fit.cpp`; catalog file sizes stand in for tensor byte counts that are unknown before download. The estimate is frozen like the llama.cpp formulas.
- Payload (`payload.rs`). The native `img_gen` request built from fixed defaults and reference-image rules.
- Output (`output.rs`). Console handling: a 240-line tail kept for out-of-memory signatures, and step lines turned into throttled progress events per stream.
- Inventory (`inventory.rs`). What the local image settings page shows: catalog entries with install state, engine builds, the active build, compute policies, model file detection and disk usage.

### The engine

`LocalDiffusionEngine` (`server.rs`) implements `ImageProviderPort` for the managed `sdcpp` account:

1. Resolve the engine build and compute policy for the model.
2. Start `sd-server` with its fixed argument list, or reuse the running one while the model, build and policy key is unchanged. Readiness may take up to five minutes.
3. Submit the job through the native job API and poll it every 500 ms for up to ten minutes.
4. Under the automatic policy, retry once with `--offload-to-cpu` after an out-of-memory failure.
5. Cancel through the engine job or by stopping the server; the job's cancellation token cancels the engine job.

The local engine and llama.cpp never run at once: `AppBackend::with_local_diffusion` wires it so starting the image server unloads llama.cpp, and every llama.cpp request first stops the image server (`stop_for_llama`); if that stop fails, the llama.cpp request fails.

### LoRAs

`loras.rs` gives sd-server library-relative LoRA paths and rewrites FLUX.2 Klein tensor aliases into a compatibility cache. `lora_library.rs` is the local LoRA library: keywords and base architecture read from safetensors headers, reuse of a copy's discovery by hash, a CivitAI lookup by hash (one request), user keywords that are never replaced, import with a same-name check, and deletion refused while a local model uses the LoRA.

### Runnability and upscaling

`runnability.rs` answers whether a model runs here. `LocalDiffusionEngine::catalog_runnability` gives a catalog variant a compute-policy placement estimate before it is installed and a real generation probe (one step, or the full request) once it is. `remote_bundle_runnability` estimates a Hugging Face bundle from its file sizes before download. Transport errors carry the shared HTTP client's texts and limits.

`upscale.rs` holds the upscaler library and upscales a stored image once through `sd-cli`, leaving the server as it is.

## Hugging Face bundles

`hf_bundle.rs` holds the rules for assembling a local image model from Hugging Face files: repository compatibility including declared base-model ancestry, per-role default queries and listing matches, file format and quantization, exclusion of training artifacts, role compatibility, the GGUF encoder hint markers and image-role inference. A bundle manifest is written under `<image root>/huggingface/bundles`, and a verified identical file from another bundle is hard-linked instead of downloaded again.

## CivitAI

`civitai.rs` holds the LoRA browsing rules: the `/api/v1/models` query (LoRAs only, sort, period and base-model filters, and no NSFW when Pure mode is on), what a page and a model show (supported base models only, NSFW models and images hidden in Pure mode), the status texts, and the checks on a download (a single safetensors file name and an https URL on `civitai.com` or a subdomain).

## Playground

`playground.rs` has the playground history types: generated and imported entries with their images, listed 30 at a time by default (1 to 200).
