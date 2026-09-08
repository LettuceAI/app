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
now fail before settlement. Runtime execution remains behind `AsrRuntime`;
the embedded whisper.cpp adapter, installed-model discovery/downloads, the ASR
learning repository, microphone IPC and file-format expansion remain later ASR
slices. TTS has not started.
