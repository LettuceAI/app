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
