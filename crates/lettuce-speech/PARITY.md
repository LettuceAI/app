# lettuce-speech: legacy parity notes

Facts about how `lettuce-speech` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- The transcription contract preserves the legacy chat and group-chat dictation flow while removing caller-owned native paths and unchecked PCM byte payloads.
- `WhisperCppRuntime` preserves legacy's greedy sampling, thread, translation, context, timestamp, split, length, token, offset, duration, temperature, language-detection, prompt and segment conversion inputs. Legacy only checked that the model file existed; the runtime now also checks the installed size and the manifest.
- Vocabulary queries preserve legacy's language-neutral matching, scope filtering, priority/use/update ordering, normalized deduplication, the 24-term limit and the 240-byte prompt budget.
- Corrections preserve legacy's longest-phrase, confidence, use-count and id order, case-insensitive word boundaries, whitespace-flexible phrases and one application record per match.
- Edit learning keeps legacy's tokenization and LCS grouping, the five-word and low-value filters, vocabulary and phonetic signals, the confidence formula, pair deduplication and ignored-pair suppression unchanged.
- Learning export preserves legacy's language and scope filters and every counter and optional link. The application compatibility adapter converts retained legacy version-2 exports into this boundary after safely ingesting their referenced audio files.
- The six audio provider kinds are legacy's. The built-in catalog keeps legacy's exact model ids and display names, the ElevenLabs voice-design models and the ten Gemini voices with their labels; Kokoro exposes all three variants on desktop and only Int8 on mobile, as before.
- OpenAI-compatible synthesis keeps legacy's bearer request, configurable endpoint and path, MP3 request, authored input, trimmed instructions and MP3 fallback MIME.
- ElevenLabs synthesis keeps legacy's voice path segment, `mp3_44100_128`, `xi-api-key` and JSON body; the response stays MP3 as in the legacy flow.
- ElevenLabs voice design keeps legacy's authored sample, description, optional design model, omitted loudness and optional preview count; voice creation keeps the voice name, preview id, description and omitted labels.
- Hosted Fish synthesis keeps legacy's request and its prompt-derived speed formula unchanged. Legacy fell back to `s2-pro` inside the transport; callers now select it before admission.
- Fish Speech keeps legacy's `http://127.0.0.1:8080` and `/v1/tts` defaults.
- Gemini keeps legacy's Vertex route, headers, fixed `en-us` config, prompt-plus-text composition and voice resolution (`preview` selects `kore`).
- ElevenLabs discovery keeps legacy's response order, fields and label keys; hosted Fish discovery keeps legacy's `GET /model` request for up to 100 models and its filters and labels. The orphan provider search command stays separate.
- Credential verification keeps legacy's probes for ElevenLabs (`GET /v1/voices`), hosted Fish and Fish Speech.
- `TtsVoiceRefreshCoordinator::search` returns nothing for an OpenAI-compatible provider, like legacy.
- The speech cache key treats no prompt and an empty prompt as the same, as legacy's cache key did.
- Kokoro phonemization preserves legacy's voice-prefix language mapping, markdown normalization, inline IPA and stress annotations, punctuation segmentation, lexicon replacement, batched eSpeak fallback and the complete upstream character-to-token table. Flat and `global` plus language-scoped legacy lexicon documents keep their exact merge order.
- Kokoro voice loading preserves legacy's 256-float row format, first-seen duplicate merge order, positive-weight normalization, per-row blending, shorter-voice last-row extension and token-count row clamp.
- Kokoro inference preserves legacy's 510-token punctuation-aware splitting, token-count style lookup, 240-sample linear crossfade and 24 kHz mono PCM16 WAV encoding.
- iOS links ONNX Runtime statically, as legacy did.

## Deliberate differences from legacy

- Legacy set whisper.cpp's `detect_language` flag for `auto`, which returns right after detection, so an `auto` dictation came back with no text. `auto` now detects and transcribes.
- Legacy interpreted `$` in correction replacement text as a regex capture expansion. Replacement text is now literal.
- Voice examples no longer store raw filesystem paths; they reference managed audio assets.
- ElevenLabs voice ids containing path separators or encoded traversal are rejected before transport instead of being interpolated into the URL.
- Voice design validates the current provider limits (100 to 1000 sample characters, 20 to 1000 description characters, at most three previews), which rejects legacy's empty description before a billable request.
- Fish Speech admission requires an explicit reference voice instead of legacy's ambiguous request that omitted `reference_id` for a blank selection.
- Legacy labeled Gemini's inline audio `audio/wav` as returned, but the Gemini TTS models answer unary requests with headerless 16-bit little-endian PCM (`audio/L16;codec=pcm;rate=24000`, 24 kHz mono, per Google's speech-generation docs), except newer models that already send RIFF WAV. Non-RIFF audio is now wrapped in a WAV header, so media ingestion accepts it.
- The Gemini parser finds the first inline-audio part across candidates instead of assuming the first part is audio, which avoids rejecting responses that start with metadata parts without changing the selected audio.
- ElevenLabs and hosted Fish discovery reject paginated partial responses, duplicates and oversized fields before persistence. Legacy could replace a complete cache with an incomplete first page. The Fish contract follows the official [Fish Audio model-list API](https://docs.fish.audio/api-reference/endpoint/model/list-models).
- Legacy's Gemini verifier silently forced `us-central1`; verification now uses the configured location.
- Legacy's OpenAI-compatible verification always succeeded. It now probes `GET /v1/models` per the [OpenAI models API](https://platform.openai.com/docs/api-reference/models/list), and a server without that route fails verification.
- The legacy per-file TTS disk cache becomes durable syntheses. Its files were never in backups, and their keys cannot be recomputed after audio provider ids are remapped on import.

## Approved removals

- The legacy `tts_audio` cache files are not imported.

## Not wired yet

- Microphone IPC and additional audio file formats were listed as later ASR slices.
- Legacy learning row transfer was listed as a later learning slice.
- Fish Speech health presentation and server-default model display were listed as later configuration slices.

## History

- The previous README said provider HTTP adapters, voice discovery, synthesis, preview caches and Kokoro execution were later TTS slices, and that Kokoro was rejected until its native runtime existed. All of these exist now; `RemoteTtsRuntime` still rejects Kokoro by design, and `lettuce-app` routes Kokoro requests to `OnnxKokoroRuntime` (`speech/kokoro_native_synthesis.rs`).
