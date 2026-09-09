# lettuce-speech

Owns speech recognition and synthesis workflows, voice configuration, learned
corrections, local speech runtimes, and typed audio results.

ASR and TTS remain separate internal modules. Persistent audio is handed to
`lettuce-media`; model artifacts and long-running work use injected ports.

The first ASR backend slice defines a durable transcription request/result
contract, a runtime port, learned-prompt/correction port, and a media-backed WAV
decoder. Audio is opened by asset identity through managed media handles,
bounded to thirty minutes of interleaved finite PCM, downmixed and linearly
resampled to 16 kHz. Requests retain the selected model artifact hash, language,
scope, prompt, translation, GPU and cache choices. Results preserve raw and
corrected text, merged prompt, detected language, timed segments and applied
corrections.

This preserves the legacy chat and group-chat flow while removing caller-owned
native paths and unchecked PCM byte payloads. English-only model/language
mismatches, malformed WAV frames, oversized audio and invalid runtime results
fail before settlement.

`WhisperCppRuntime` implements `AsrRuntime` with the pinned whisper-rs binding
and accepts a model only after the installed manifest is reverified and matches
the durable descriptor. Its process cache is keyed by artifact hash, effective
CPU/GPU choice, flash-attention choice and GPU device. CUDA, ROCm, Vulkan and
Metal remain explicit build features. It preserves the legacy greedy sampling,
thread, translation, context, timestamp, split, length, token, offset,
duration, temperature, language-detection, prompt and segment conversion
inputs. An `auto` language request triggers detection, including for
English-only models. Nonfinite temperatures and invalid device/thread values
now fail before reaching native code instead of relying on lossy casts.

The job cancellation token is read by whisper.cpp's abort callback during
inference, and a post-call check maps native abort failures back to the durable
cancelled outcome. The adapter routes whisper.cpp and GGML logs through
`tracing`, supports verified preload and explicit cache clearing, and never
exposes the verified native path outside the runtime boundary. A deterministic
native-loader test uses a verified invalid model fixture to exercise the real
FFI without downloading model data.

The ASR learning library owns typed vocabulary and correction records plus its
repository port. Vocabulary queries preserve legacy language-neutral matching,
scope filtering, priority/use/update ordering, normalized deduplication, the
24-term limit and the 240-byte prompt budget. Corrections preserve the legacy
longest-phrase, confidence, use-count and ID order, case-insensitive word
boundaries, whitespace-flexible phrases and one application record per match.
Replacement text is now treated literally, correcting the legacy regex
replacement bug that interpreted dollar signs as capture expansion.

`AsrLearningLibrary` supplies the concrete `AsrPromptLibrary` used by the
durable coordinator and also exposes validated vocabulary/correction CRUD.
Authored values are stored without trimming or truncation; normalized lookup
values remain separate. Chat and group-chat edits now use the same learning
boundary to derive bounded correction pairs through the legacy tokenization and
LCS grouping rules. Five-word and low-value filters, vocabulary and phonetic
signals, the confidence formula, pair deduplication and ignored-pair suppression
remain unchanged. Accepting a pair retains its counters and promotes
conversation scope to project after two acceptances and global after four;
ignoring a pair increments durable scope-specific memory, with global ignores
also suppressing narrower scopes. Legacy row transfer remains a later learning
slice.

Voice examples now retain a managed audio asset identity, authored and
normalized expected/Whisper text, optional language and scope, and optional
vocabulary/correction links. They list newest-first through the learning port,
support update and deletion, and reuse the edit-learning algorithm to return at
most one suggested correction. Raw filesystem paths are no longer part of the
record. The learning repository also accepts a fully validated four-class batch
in one transaction and exposes filtered ignored suggestions for export.

Versioned learning-library export preserves the legacy language and scope
filters plus every counter and optional link, using managed audio asset IDs in
place of native paths and retaining content and redacted provenance evidence.
Import rejects unknown versions, oversized or malformed documents, incomplete
link graphs and changed managed audio, allocates fresh learning IDs, remaps
voice links and commits all rows only when every managed audio asset is valid.
The application compatibility adapter converts retained version-2 exports into
this boundary after safely ingesting their referenced audio files.

Pinned, resumable installed-model downloads now compose through model-hub,
network, platform and durable artifact-install jobs. Managed removal clears the
process-wide Whisper context cache before deleting verified owned bytes.
Microphone IPC and file-format expansion remain later ASR slices.

The first TTS slice defines the six legacy audio-provider kinds as a closed
typed configuration and persists provider metadata through a repository port.
API keys are represented only by a scoped secret reference and owner identity;
plaintext credentials are not part of the domain or SQLite schema. User voices
retain their provider, name, model, voice and optional authored prompt through
the same port. Both aggregates use revision compare-and-swap, retain creation
timestamps across updates and validate stored data when read. Provider deletion
returns its secret metadata for separate native-secret cleanup and atomically
cascades its user voices. Provider HTTP adapters, voice discovery, synthesis,
preview caches and Kokoro execution remain later TTS slices.

The application coordinator now owns native-secret creation, generation-safe
rotation and deletion around this repository. A failed secret cleanup after an
already committed provider deletion produces an opaque exact retry receipt.
Provider configuration updates cannot switch kinds, which avoids silently
reusing one provider's credential under a different protocol; changing kinds is
a delete-and-create operation.

The synthesis contract freezes the provider metadata, model, voice, optional
prompt, text, output asset identity and retention choice in one bounded request.
Remote credentials are loaded only through the scoped secret reference and are
borrowed by the provider-neutral async runtime port. Runtime audio bytes are
redacted from debug output and must pass the existing media header and MIME
validation before becoming a synthesized-speech asset. Preview output is
temporary with an admitted expiry; message audio is persistent. The durable
result retains only managed asset identity, content hash, detected MIME, size
and completion time. Provider-specific HTTP transports, voice discovery,
preview cache policy and Kokoro execution remain later TTS slices.

`OpenAiCompatibleTtsRuntime` implements the first remote synthesis transport
through the central bounded network client. It preserves the legacy bearer-auth
request, configurable endpoint/path, MP3 response request, authored input and
trimmed optional instructions. The adapter retains a bounded response MIME with
the legacy MP3 fallback, maps retryable HTTP and transport failures to runtime
unavailability, and drops the in-flight request when the job cancellation token
fires. Provider error bodies and credentials never enter runtime errors or logs.

`ElevenLabsTtsRuntime` preserves the hosted ElevenLabs synthesis contract: the
voice remains a path segment, `output_format=mp3_44100_128` is a typed query,
the scoped credential is sent only as `xi-api-key`, and the JSON body contains
the authored text and selected model. ElevenLabs does not consume the optional
voice prompt. The response remains MP3 as in the legacy flow, with cancellation
and retry classification delegated through the same bounded network client.
Voice identifiers containing path separators or encoded traversal are now
rejected before transport instead of being interpolated into the URL.

The same ElevenLabs adapter implements typed voice-design previews through
`POST /v1/text-to-voice/design`. It preserves the legacy authored sample,
description, optional design model, omitted loudness and optional preview
count, then decodes provider base64 inside the adapter. Preview identity,
finite duration and MP3 media type remain typed while raw bytes are exposed
only to the media sink. Input validation follows the current provider limits:
100 to 1,000 sample characters, 20 to 1,000 description characters, one of
the two catalogued design models when supplied, and at most three previews.
This deliberately rejects the legacy editor's empty description before a
billable request. The contract is documented by the official
[ElevenLabs voice-design API](https://elevenlabs.io/docs/api-reference/text-to-voice/design).

The same typed runtime creates the selected preview through `POST
/v1/text-to-voice`. It preserves the authored voice name, generated preview ID,
description, omitted labels and scoped `xi-api-key`, and returns only a validated
provider voice ID from the private response DTO. Creation requires the current
20-to-1,000-character description contract and the local user-voice name bound
before the billable call. Provider errors remain redacted and use the existing
retryable HTTP classification. The wire contract follows the official
[ElevenLabs create-voice API](https://elevenlabs.io/docs/api-reference/text-to-voice/create/).
Persisting the returned ID as a user voice remains owned by the existing TTS
configuration boundary.

`FishTtsRuntime` preserves the hosted Fish Audio synthesis request, including
bearer authentication, the selected model header and reference voice, MP3 at
44.1 kHz and 128 kbps, normalization, normal latency, zero volume and loudness
normalization. Its prompt-derived speed formula is unchanged: parse a positive
finite JSON number, cast it to `f32`, clamp it to `0.7..=1.3`, and otherwise use
`1.0`. The response remains `audio/mpeg`, and the bounded client owns retries,
timeouts and cancellation. Durable admission requires an explicit model, so a
caller that wants the legacy fallback must select `s2-pro` before admission
instead of leaving mutable fallback selection inside the transport. Fish voice
discovery and cache refresh remain a later TTS slice.

`FishSpeechTtsRuntime` preserves the self-hosted Fish Speech contract with the
legacy `http://127.0.0.1:8080` and `/v1/tts` defaults, configurable endpoint and
path, optional bearer authentication, exact authored text, reference voice and
MP3 output. The model is selected by the server at startup and the protocol does
not consume the stored model label or optional prompt. The response remains
`audio/mpeg`; retry, timeout and cancellation behavior stays inside the bounded
network client. Durable admission requires an explicit reference voice instead
of preserving the legacy ambiguous request that omitted `reference_id` for a
blank selection. Health verification and server-default model presentation
remain later configuration slices.

`GeminiTtsRuntime` preserves the Vertex AI location/project/model route, bearer
access token, `x-goog-user-project`, fixed `en-us` speech configuration, legacy
prompt-plus-text composition, lowercase voice resolution with `preview`
selecting `kore`, and base64-decoded WAV output. Project, location and model
segments are validated before URL construction. The response parser now finds
the first actual inline-audio part across returned candidates instead of
assuming the first part contains audio, which avoids rejecting valid metadata
parts without changing the selected audio. Malformed JSON/base64 and missing
audio reject without exposing the provider body. Static model and voice
catalogs plus credential verification remain later configuration slices.

`RemoteTtsRuntime` is the single provider-dispatching implementation used by
the application boundary. It routes each frozen remote provider configuration
to its existing adapter and shares one host-configured bounded network client.
It also routes ElevenLabs voice design through that adapter and contains no
wire logic. Kokoro rejects explicitly until its native runtime is installed
behind the same port.

The built-in TTS catalog preserves the exact legacy model IDs and display names
for all six provider kinds, the separate ElevenLabs voice-design models, and
the ten Gemini voice IDs with gender and description labels. Kokoro exposes all
three variants on desktop and only Int8 on mobile as before. Other built-in
voice lists stay empty because ElevenLabs and hosted Fish require discovery,
Fish Speech uses a server reference, and Kokoro lists installed voice assets.
Kokoro catalog entries now come from the same model-hub variant definitions used
by managed asset inspection, avoiding a second platform matrix while preserving
the existing IDs and labels exactly.

Configured ElevenLabs voice discovery uses the bounded central client and
scoped `xi-api-key`. It preserves response order, voice ID, name, optional
preview URL and provider labels, with category and description overwriting the
same legacy label keys. Duplicate IDs, oversized fields and paginated partial
responses reject before persistence, correcting the legacy behavior that could
replace a complete cache with an incomplete first page. The orphan provider
search command remains separate.

Configured hosted Fish discovery preserves the legacy authenticated `GET /model`
request for up to 100 account models with `sort_by=created_at`. It keeps TTS and
missing-type models, excludes singing-conversion and failed models, and maps
state, tags, languages, trimmed description, library category and Fish engine
labels. A response with `has_more` rejects before cache replacement so the first
page cannot erase older configured voices. The wire contract follows the
official [Fish Audio model-list API](https://docs.fish.audio/api-reference/endpoint/model/list-models).

ElevenLabs credential verification uses the legacy `GET /v1/voices` probe with
the scoped `xi-api-key`. Any successful HTTP status verifies the credential;
other HTTP statuses return `false`, while bounded-client transport failures stay
distinct typed errors.

Hosted Fish credential verification uses the legacy bearer-authenticated
`GET /model?self=true&page_size=1` probe. It shares the same successful-status
boolean and distinct bounded-client failure semantics.

Gemini credential verification probes the project-scoped Vertex publisher-model
path with bearer authentication and `x-goog-user-project`. It uses the provider's
validated configured location, correcting the legacy verifier that silently
forced `us-central1`, and retains the shared status and transport semantics.

Self-hosted Fish Speech verification probes `/v1/health` below the configured
base URL and sends bearer authentication only when the provider has a secret.
It preserves the shared successful-status boolean and typed transport failure.

OpenAI-compatible credential verification corrects the legacy unconditional
success by issuing the standard bearer-authenticated `GET /v1/models` probe
below the configured base URL. A compatible server without that route rejects
verification rather than reporting fabricated success. The endpoint follows the
official [OpenAI models API](https://platform.openai.com/docs/api-reference/models/list).

Kokoro phonemization preserves the legacy voice-prefix language mapping,
markdown normalization, inline IPA and stress annotations, punctuation
segmentation, lexicon replacement, batched eSpeak fallback and the complete
upstream character-to-token table. Inputs, lexicon entries and token output are
bounded. Process execution is supplied only through the purpose-specific
platform phonemizer capability; ONNX and audio synthesis remain separate.
Kokoro voice style loading preserves the 256-float little-endian row format,
first-seen duplicate merge order, positive-weight normalization, weighted
per-row blending, shorter-voice last-row extension and token-count row clamp.
The boundary rejects malformed rows, unsafe IDs, nonfinite samples and weights,
and arithmetic overflow. It returns normalized blend metadata plus bounded
style rows; ONNX execution and audio synthesis remain separate.
Native Kokoro inference loads the verified ONNX model at optimization level 3,
accepts either `input_ids` or `tokens`, preserves float or Int32 speed input,
pads each token chunk with boundary zeros and supports cooperative ONNX run
termination. The legacy 510-token punctuation-aware splitting, token-count
style lookup, 240-sample linear crossfade and 24 kHz mono PCM16 WAV encoding
are preserved. Nonfinite or oversized inference output fails before media
ingestion. Routing persisted Kokoro TTS requests into this runtime remains the
next application slice.
