# lettuce-context: legacy parity notes

Facts about how `lettuce-context` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- `LorebookMatcher` preserves the verified legacy matching: the latest ten messages for recent-message detection, the newest user message for latest-user detection, punctuation normalized for literal matching, word boundaries for one-word keywords, substring matching for multiword keywords, trailing `*` as a word-prefix operator, and normalized substring matching for CJK/Thai/Lao/Myanmar/Khmer/Hangul text.
- `LegacyV1` lorebooks ignore entry priority and order active entries by the legacy ordinal, then creation time, then source order.
- Lorebook entry titles may be empty, as legacy blank entries are.
- Direct and companion chat rendering moves a `Relative` entry that names a volatile turn value to in-chat depth 0, as legacy did.
- Legacy also resolved identity tokens inside lorebook text, author notes, summaries and memories, and in every message of a one-to-one request (`apply_identity_placeholders`, `sanitize_placeholders_in_api_messages`). Callers do that with `PromptRenderValues::resolve_identity` and `resolve_names` before rendering and on history.
- `{{#if}}` blocks: legacy only used them for `current_draft`; catalog fragments now also use them for optional lines. A malformed, unknown or nested block and a stray `{{else}}` or `{{/if}}` stay in the text because legacy sent such text verbatim.
- Regex keywords are validated in exactly the form the matcher compiles, matching the legacy matcher, which silently treated an uncompilable keyword as never matching.
- Like legacy, nothing is limited by count (entries, keywords, active books and entries, prompt entries). The 8 MiB prose limit on entry titles and keywords lets a long World Info key or title import.
- `count_tokens_batch` uses the o200k encoding (`tiktoken-rs` 0.11, special tokens read as plain text), as the old `tokens_count_batch` did for the lorebook trigger preview.

## Deliberate differences from legacy

- Prompt and lorebook libraries are paged by `updated_at DESC, id ASC` with a keyset cursor. This is an intentional library redesign from the legacy prompt created-order listing.

## Fixed bugs

- Earlier explicit 256 KiB regex limits and raw-keyword validation rejected patterns legacy matched (`\w{100}`, a case-insensitive `hero\Z`) and accepted ones the matcher could never compile. Validation now compiles the keyword the way the matcher does.

## Not in this crate

Import/export, sessions, sync/backup, hard purge, legacy id conversion, database/schema types, events, full-text search, async ports and conversation assembly belong to their owning crates.

## Not decided yet

- `DeterministicV2` lorebooks use the same ordering as `LegacyV1` until a product decision makes entry priority take part (comment in `LorebookMatcher::activate`).
- The previous README said conversation snapshots "can pin the resolved revision later"; snapshot activation (`resolve_lorebook_snapshot_activation`) exists now.
- `PromptRepository::lookup_exact` leaves fallback and app/session precedence to conversation assembly.
