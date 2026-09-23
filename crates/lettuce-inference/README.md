# lettuce-inference

Provider-neutral generation admission, attempts, streaming, tools, approvals,
safety evaluation, cancellation, and terminal outcomes.

## Boundary

Tools and safety remain independent internal modules. The crate does not read
conversations, construct prompts, persist usage, or implement provider wire
protocols.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The runtime foundation provides a bounded in-process stream registry and
notification-driven cooperative cancellation. Stream delivery validates the conversation
operation/turn/attempt identity and monotonic sequence before applying channel
backpressure; no detached fan-out task or durable payload store is involved.
The runtime can signal an exact registered job token by durable job ID and
reports whether a live execution was present; registration and removal remain
owned by the application execution lifetime.
Provider adapters race cancellation against connection setup, socket reads,
buffered responses, and backpressured delivery. Job and conversation domains
remain authoritative for persisted lifecycle state. Provider normalization and
outcome assembly are implemented by `lettuce-providers`; tools and safety
remain later horizontal slices.

Pure mode: `content_filter` is the old content filter engine (normalization,
leet/homoglyph folding, dictionary scoring with the allowlist context, the
500-byte stream window, the redacted 200-entry hit log; dictionaries in
`resources/content-filter-dictionary.json`). `pure_mode` applies it to
provider answers: `PureModeRuntime` refuses a streamed delta that crosses the
level's threshold (which stops the stream) and `PureModeGuard::settle` turns a
blocked stream or a blocked final answer into the `CONTENT_BLOCKED` failure
("Response blocked by Pure Mode. Try rephrasing your message.").

The old filter only knew English. `content_lexicons` adds other languages
without asking the user which one they write in: `whatlang` detects the
language of the checked text over windows the size of the 500-byte stream
window that overlap by half, and the English dictionary keeps applying
unchanged. A stream window and the whole text cut a mixed passage at
different points, so one can catch a foreign passage the other misses, as
the old dictionary's whole-text and windowed scores could differ; the final
answer is always checked whole as well. In Latin script only a confident
detection of a language other than English selects its lexicon, because short
English lines are often misdetected and many foreign terms are ordinary
English words; in other scripts an unsure detection selects every lexicon
written in that script (a Russian paragraph is often below whatlang's
confidence threshold). The cost is that a short foreign reply of a sentence or
two is often not confidently detected and gets only the English dictionary. Lexicons are the LDNOOBW lists (revision `5faf2ba`,
CC BY 4.0, (c) 2012-2020 Shutterstock, Inc.; title, source, license and
changes recorded in `resources/content-filter-lexicons.json`) for 24
languages; `en` stays the old dictionary, and `kab` and `tlh` are left out
because they cannot be detected. Every term weighs 0.8, the median weight the
old dictionary gives the 91 terms it shares with the LDNOOBW English list, so
it counts at every level including Low; a term the English dictionary already
counted, or a form of it, is not counted again. Text and terms are lowercased
and NFC-composed, a dotted capital I lowercases to a plain i, and curly
apostrophes become straight ones, which separate words (so elisions such as
d'x match x). Latin-script single words match on the old folded text wherever neither
neighbour is a Latin letter or digit (so a romanized term inside Japanese or
Chinese text still counts), longer Latin terms as whole-word sequences;
other spaced scripts match whole words on the unfolded text so Cyrillic and
Greek are not folded into Latin; terms containing Han, kana, Thai, Khmer or
Myanmar letters match as substrings of the text with whitespace removed,
except that a single-character term counts only when it stands alone, since
without word segmentation it is usually part of an innocent word. Terms
without letters are skipped. Known limit: the `hin` list is romanized and
Hindi is only detected in Devanagari, and the Latin transliterations in the
`rus` list only count inside Cyrillic Russian text.
