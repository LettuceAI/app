# lettuce-inference

The provider-neutral pieces around a running generation: the in-process stream and cancellation registry, Pure mode (the content filter applied to model output), and the parser that splits inline thinking tags from visible text.

The crate does not read conversations, build prompts, persist anything or speak a provider wire protocol. Wire formats and outcome assembly live in `lettuce-providers`, the generation lifecycle and its persisted state in `lettuce-conversations` and `lettuce-jobs`, and orchestration in `lettuce-app`. What stays here is shared by all of them at runtime.

## Streams and cancellation

`InferenceRuntime` is the bridge between a running request and whatever consumes its output, and implements `InferenceRuntimePort`.

- `register_stream(sink_id)` opens a bounded channel (64 events) and returns the `InferenceStreamReceiver`. `emit` validates each `GenerationStreamEventEnvelope` against the previous one on the same sink (same operation, turn and attempt, strictly increasing sequence, via `validate_after`) and then sends it, waiting when the channel is full. Backpressure therefore reaches the provider adapter; there is no detached fan-out task and nothing is buffered durably.
- `register_cancellation(job_id, token)` records the `CancellationToken` of a live execution. `request_cancel` signals one by durable job id and reports whether a live execution was there; `cancel_all` signals every one. `is_cancelled` and the async `cancelled` let adapters check or await it.

Registration and removal belong to the application's execution lifetime. Provider adapters race cancellation against connection setup, socket reads, buffered responses and backpressured delivery. The job and conversation domains remain the authority on persisted state; this registry only holds what is live in the process.

## Pure mode

Pure mode checks model output against a content dictionary and blocks it above the threshold of the selected `PureModeLevel`: `Off`, `Low` (counts only terms of weight 0.8 and up, threshold 2.0), `Standard` (1.5) or `Strict` (1.0).

### Engine

`ContentFilter` (`content_filter.rs`) normalizes the text (invisible characters dropped, leet and homoglyph folding), scores it against the bundled English dictionary (`resources/content-filter-dictionary.json`: explicit sexual, graphic violence and slurs with weights, plus a context allowlist), and keeps a redacted log of the last 200 checks that scored above zero. For streams, `StreamFilterContext` keeps the last 500 bytes and rescans that window on every delta. The level can change while the filter is shared.

### Around a provider

`pure_mode.rs` wires the filter into generation:

- `PureModeRuntime` wraps the stream sink. A delta that pushes its window over the threshold is refused with `ContentBlocked`, which stops the provider's stream.
- `PureModeInference` wraps an `InferencePort` and passes every result through `PureModeGuard::settle`, which checks the final answer as a whole and turns a blocked stream or a blocked final answer into the `CONTENT_BLOCKED` failure ("Response blocked by Pure Mode. Try rephrasing your message.").

The final answer is always checked whole as well as windowed, because a stream window and the whole text cut a mixed passage at different points and either one can catch what the other misses.

### Other languages

The English dictionary always applies. `content_lexicons.rs` adds lexicons for other languages without asking the user what they write in:

- Detection. `whatlang` detects the language of the checked text over windows of the stream-window size that overlap by half, so a passage long enough to be recognized in a stream is also recognized in the whole text. In Latin script only a confident detection of a language other than English selects its lexicon, because short English lines are often misdetected and many foreign terms are ordinary English words. In other scripts, where English is not a candidate, an unsure detection selects every lexicon written in that script (a Russian paragraph is often below `whatlang`'s confidence threshold). The trade-off: a short foreign reply of a sentence or two is often not confidently detected and gets only the English dictionary.
- Lexicons. The LDNOOBW lists for 24 languages (source, revision, license and changes in `resources/content-filter-lexicons.json`). `en` stays the original dictionary; `kab` and `tlh` are left out because they cannot be detected. Every term weighs 0.8, the median weight the English dictionary gives the 91 terms it shares with the LDNOOBW English list, so it counts at every level including Low. A term the English dictionary already counted, or a form of it, is not counted again.
- Matching. Text and terms are lowercased and NFC-composed, a dotted capital I lowercases to a plain i, and curly apostrophes become straight ones, which separate words (so an elision such as `d'x` matches `x`). Single Latin words match on the folded text wherever neither neighbour is a Latin letter or digit, so a romanized term inside Japanese or Chinese text still counts; longer Latin terms match as whole-word sequences. Other spaced scripts match whole words on the unfolded text, so Cyrillic and Greek are not folded into Latin. Terms with Han, kana, Thai, Khmer or Myanmar letters match as substrings of the text with whitespace removed, except that a single-character term counts only when it stands alone, since without word segmentation it is usually part of an innocent word. Terms without letters are skipped.
- Known limits. The `hin` list is romanized while Hindi is only detected in Devanagari, and the Latin transliterations in the `rus` list only count inside Cyrillic Russian text.

## Thinking tags

`thinking.rs` separates reasoning that models write inline from visible text. It knows `<think>`, `<thinking>`, `<reason>`, `<reasoning>` and Gemma's channel markers (`<|channel>thought ... <channel|>`), matched case-insensitively.

- `ThinkingTagParser::feed` handles streamed chunks: a possible partial tag at the end of a chunk is held back until the next one, and `finish` releases what is left. `starting_in_reasoning(close_tag)` starts inside reasoning, for replies whose opener was prefilled.
- `split_thinking_tags` splits a complete text. `normalize_thinking_content` combines the tagged reasoning with reasoning the provider returned separately, appending the latter unless it repeats the former, both trimmed.

`lettuce-providers` and `lettuce-local-llm` use it when normalizing streams and final responses.
