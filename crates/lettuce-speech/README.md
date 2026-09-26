# lettuce-speech

Speech recognition and synthesis: the transcription and synthesis contracts, the Whisper runtime, the ASR learning library (vocabulary, corrections, voice examples), audio provider and user voice configuration, the remote TTS adapters, the built-in TTS catalog, voice discovery and credential checks, ElevenLabs voice design, the speech cache port, and the Kokoro phonemizer, voice blending and ONNX runtime.

ASR and TTS are separate modules that share the crate's artifact, media, job and platform contracts. Audio enters and leaves as managed media assets (`lettuce-media`), never as native paths or unchecked byte payloads. Model files come verified from `lettuce-model-hub`, HTTP goes through `lettuce-network`'s bounded client, credentials are `SecretRef`s resolved through `lettuce-settings`, and eSpeak NG runs through `lettuce-platform`'s purpose-specific capability. Durable coordination (jobs, secrets, caching, routing Kokoro requests) lives in `lettuce-app`; persistence behind the repository ports lives in `lettuce-database`.

## ASR

### Contract

A `TranscriptionRequest` (`asr.rs`) names the audio asset and freezes the selected model (`AsrModelDescriptor` with the artifact hash), language, scope, prompt, translation, GPU and cache choices. A `TranscriptionResult` keeps the raw and corrected text, the merged prompt, the detected language, timed segments and every applied correction. `TranscriptionRepository` stores the request and its terminal state.

`AsrAudioSource` opens audio by asset identity through managed media handles. The WAV decoder accepts at most thirty minutes of interleaved finite PCM, downmixes and linearly resamples it to 16 kHz. An English-only model with a non-English language, a malformed WAV frame, oversized audio or an invalid runtime result fails before the request settles.

### Whisper runtime

`WhisperCppRuntime` (`whisper_runtime.rs`) implements `AsrRuntime` with the pinned `whisper-rs` binding (CUDA, ROCm, Vulkan and Metal are explicit build features; whisper.cpp links into the same binary as llama.cpp).

- A model is accepted only when its file is present at the installed size and its manifest matches the durable descriptor. The content is hashed once, at install time.
- The process cache of loaded contexts is keyed by artifact hash, effective CPU or GPU choice, flash attention and GPU device. Preload and explicit cache clearing are supported; managed removal of a model clears the cache before deleting its bytes.
- Decoding uses greedy sampling with the configured threads, translation, context, timestamps, splitting, length, token, offset, duration, temperature, prompt and segment conversion. A non-finite temperature or an invalid device or thread value fails before reaching native code.
- An `auto` language (or `detect_language`) runs whisper.cpp with the `auto` language, which detects and then transcribes, also for English-only models.
- The job's cancellation token is read by whisper.cpp's abort callback during inference, and a post-call check maps a native abort back to the cancelled outcome.
- whisper.cpp and GGML logs go to `tracing`, and the verified native path never leaves the runtime. A native-loader test exercises the real FFI with a verified invalid model fixture, without downloading a model.

Whisper model downloads are pinned and resumable, composed from `lettuce-model-hub`, `lettuce-network`, `lettuce-platform` and a durable artifact-install job in `lettuce-app`.

### Learning library

`learning.rs` holds what the user teaches the recognizer, behind `AsrLearningRepository`. Records use UUID ids from `lettuce-types`, never SQLite row ids.

- Vocabulary terms feed the Whisper prompt: queries match language-neutrally, filter by scope, order by priority, use count and update time, deduplicate on the normalized value, and stop at 24 terms and a 240-byte prompt budget.
- Correction rules rewrite the transcript: longest phrase first, then confidence, use count and id; matching is case-insensitive on word boundaries with flexible whitespace inside phrases, each match records one application, and replacement text is literal.
- Authored values are stored without trimming or truncation; normalized lookup values are kept separately.
- Learning from edits: when the user edits a transcript in chat or group chat, `AsrLearningLibrary` derives bounded correction pairs by tokenizing and grouping with LCS, filtering five-word and low-value pairs, weighing vocabulary and phonetic signals into a confidence, deduplicating pairs and suppressing ignored ones. Accepting a pair keeps its counters and promotes it from conversation scope to project after two acceptances and to global after four. Ignoring a pair increments a durable per-scope counter, and a global ignore also suppresses narrower scopes.
- Voice examples keep a managed audio asset id, the authored and normalized expected and Whisper text, optional language and scope, and optional links to vocabulary and corrections. They list newest first, can be updated and deleted, and run the same edit-learning algorithm to suggest at most one correction.

`AsrLearningLibrary` is also the `AsrPromptLibrary` the transcription coordinator uses, and exposes validated CRUD. The repository accepts a fully validated batch of all four record kinds in one transaction and lists ignored suggestions for export.

Export is versioned and keeps the language and scope filters, every counter and optional link, and managed audio asset ids with content and redacted provenance evidence. Import rejects unknown versions, oversized or malformed documents, incomplete link graphs and changed audio; it allocates fresh ids, remaps voice links, and commits only when every referenced audio asset is valid.

## TTS

### Configuration

`AudioProviderConfig` (`tts.rs`) is a closed set of six provider kinds: OpenAI-compatible, ElevenLabs, Fish Audio (hosted), Fish Speech (self-hosted), Gemini (Vertex AI) and Kokoro (local). An `AudioProvider` holds the typed configuration and, for API keys, only a secret reference and owner identity. A `UserVoice` keeps its provider, name, model, voice and an optional authored prompt. Both go through `TtsConfigurationRepository` with revision CAS, keep their creation time across updates and are validated when read. Deleting a provider cascades its user voices atomically and returns its secret metadata so the secret can be cleaned up separately.

The coordinator in `lettuce-app` creates, rotates (generation-safe) and deletes the native secret around this repository; if secret cleanup fails after the provider deletion committed, it returns an opaque retry receipt. An update cannot change a provider's kind, so one provider's credential is never reused under another protocol; changing kinds is delete and create.

### Synthesis

A `SynthesisRequest` freezes the provider configuration, model, voice, optional prompt, text, output asset id and `TtsOutputPolicy` in one bounded request. The runtime receives the credential only as a borrowed secret value through the async `TtsRuntime` port. Returned audio bytes are redacted in `Debug` and must pass media header and MIME validation before becoming a synthesized-speech asset (`TtsAudioSink`). Preview output is temporary with an expiry; message audio is persistent. The durable `SynthesisResult` keeps only the asset id, content hash, detected MIME type, size and completion time.

`RemoteTtsRuntime` (`remote_tts.rs`) is the single dispatcher the application uses for remote providers: it routes each configuration to its adapter over one shared network client and contains no wire logic itself. Kokoro requests are rejected there; `lettuce-app` routes them to the local runtime.

### Remote adapters

| Adapter | Request | Response |
| --- | --- | --- |
| `OpenAiCompatibleTtsRuntime` | bearer auth, configurable endpoint and path, the authored input and trimmed optional instructions, MP3 requested | the response MIME type (bounded), MP3 if missing |
| `ElevenLabsTtsRuntime` | voice as a validated path segment, `output_format=mp3_44100_128` as a typed query, key only as `xi-api-key`, JSON body with text and model; the voice prompt is not used | MP3 |
| `FishTtsRuntime` | bearer auth, model header, reference voice, MP3 at 44.1 kHz and 128 kbps, normalization, normal latency, zero volume, loudness normalization; speed from the prompt (a positive finite JSON number as `f32` clamped to 0.7..1.3, else 1.0) | `audio/mpeg` |
| `FishSpeechTtsRuntime` | default `http://127.0.0.1:8080` and `/v1/tts`, configurable, optional bearer auth, exact text, reference voice, MP3; the server picks the model, so the stored model label and prompt are not sent | `audio/mpeg` |
| `GeminiTtsRuntime` | Vertex AI location, project and model route (segments validated), bearer token, `x-goog-user-project`, fixed `en-us` speech config, prompt plus text, lowercase voice with `preview` meaning `kore` | the first inline audio part across candidates, base64-decoded; headerless PCM is wrapped in a WAV header using the rate and channels its `mimeType` names (24 kHz mono by default) |

All adapters share the bounded client's retries, timeouts and cancellation: retryable HTTP and transport failures map to runtime unavailability, the in-flight request is dropped when the job's cancellation token fires, and provider error bodies and credentials never reach errors or logs. Durable admission requires an explicit model for hosted Fish (choose `s2-pro` for the usual default) and an explicit reference voice for Fish Speech, so no fallback choice is hidden inside a transport.

### Catalog, discovery and verification

`tts_catalog.rs` lists the built-in model ids and display names for all six kinds, the ElevenLabs voice-design models and the ten Gemini voices with gender and description. Kokoro offers all three variants on desktop and only Int8 on mobile, from the same `lettuce-model-hub` variant definitions that asset inspection uses. The other built-in voice lists are empty: ElevenLabs and hosted Fish need discovery, Fish Speech uses a server reference, and Kokoro lists installed voices.

Discovery (`tts_discovery.rs`, `VoiceDiscovery`, `DiscoveredVoiceRepository`) refreshes a provider's voice cache:

- ElevenLabs uses `xi-api-key` and keeps response order, voice id, name, optional preview URL and labels, with category and description written into the same label keys.
- Hosted Fish sends an authenticated `GET /model` for up to 100 account models sorted by `created_at`, keeps TTS and untyped models, excludes singing-conversion and failed ones, and maps state, tags, languages, trimmed description, library category and engine labels.
- A duplicate id, an oversized field or a response that says more pages exist rejects before the cache is replaced, so a first page never erases a complete cache.

`VoiceSearch` searches the ElevenLabs voice library by text (`/v1/voices?search=`). A search result may be one page of many and is returned without being cached; an OpenAI-compatible provider returns nothing.

`AudioProviderVerifier` checks credentials; any successful status verifies, other statuses return `false`, and transport failures stay typed errors:

- ElevenLabs: `GET /v1/voices` with `xi-api-key`.
- Hosted Fish: bearer `GET /model?self=true&page_size=1`.
- Gemini: the project-scoped Vertex publisher-model path in the configured location, with bearer and `x-goog-user-project`.
- Fish Speech: `/v1/health` below the base URL, with bearer only when a secret exists.
- OpenAI-compatible: bearer `GET /v1/models` below the base URL; a server without that route fails verification.

### Voice design

`voice_design.rs` and the ElevenLabs adapter implement voice design. `POST /v1/text-to-voice/design` takes the authored sample (100 to 1000 characters), description (20 to 1000 characters), an optional design model (one of the two catalogued ones) and up to three previews, omitting loudness; the adapter decodes the base64 previews itself and exposes typed preview ids, finite durations and MP3 type, with raw bytes going only to the media sink. `POST /v1/text-to-voice` then creates the chosen preview with the voice name, preview id and description (labels omitted) and returns only a validated provider voice id; the description and local name are checked before the billable call. Saving that id as a user voice goes through the configuration repository. The wire contracts follow ElevenLabs' [voice-design](https://elevenlabs.io/docs/api-reference/text-to-voice/design) and [create-voice](https://elevenlabs.io/docs/api-reference/text-to-voice/create/) API references.

### Speech cache

`SpeechCacheRepository` replaces a file cache with durable syntheses. `find_reusable` returns the most recent finished, retained synthesis with the same `SynthesisReuseKey` (provider, model, voice, text and prompt; no prompt and an empty prompt are the same) whose audio is still stored and unexpired. `cached_blobs` lists synthesized audio blobs that only their syntheses keep: every asset on the blob is non-library synthesized speech bound to a synthesis, and nothing else refers to any of them. `release` marks such a blob's bytes missing, which lets `lettuce-app`'s TTS audio cache free space through `lettuce-media`.

## Kokoro

Kokoro runs locally in three stages.

1. Phonemization (`kokoro_phonemizer.rs`). The voice id's prefix selects the language. Text goes through markdown normalization, inline IPA and stress annotations, punctuation segmentation and lexicon replacement, and the remaining text is phonemized by eSpeak NG in batches through the platform's `EspeakPhonemizer` capability; the result maps to tokens with the upstream character-to-token table. Inputs, lexicon entries and token output are bounded. A user lexicon may be flat or split into `global` and per-language sections; they merge in a fixed order with the selected language overriding global entries, and malformed or oversized documents are rejected.
2. Voice style (`kokoro_voice.rs`). Voice files are rows of 256 little-endian floats. A blend of several voices merges duplicates in first-seen order, normalizes positive weights, blends row by row, extends a shorter voice with its last row, and picks the row by token count. Malformed rows, unsafe ids, non-finite samples or weights and arithmetic overflow are rejected. The result is normalized blend metadata plus bounded style rows.
3. Inference (`kokoro_runtime.rs`). `OnnxKokoroRuntime` loads the verified ONNX model at optimization level 3, accepts either an `input_ids` or `tokens` input and a float or Int32 speed input, splits token streams at 510 tokens on punctuation, pads each chunk with boundary zeros, looks up the style by token count, runs with cooperative ONNX termination, crossfades chunks over 240 samples, and encodes 24 kHz mono PCM16 WAV. Non-finite or oversized output fails before media ingestion.

Kokoro never initializes ONNX Runtime. The composition root commits the process's single environment through `lettuce-embeddings` and only then creates the `OnnxRuntimeCommitted` evidence that `OnnxKokoroRuntime::load` requires. That constructor is `unsafe`: evidence created before a successful commit would let a session read uninitialized `ort` state, and a Kokoro-owned commit could pin a wrong library path for the whole process or build a session over a failed setup. `ort` loads the library dynamically everywhere except iOS, which links it statically.
