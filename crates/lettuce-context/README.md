# lettuce-context

Owns structured prompt programs, lorebooks, bindings, legacy-compatible
activation, pure rendering, validation, previews, and explanations.

Prompt rendering and lorebook matching remain separate internal modules. The
crate consumes immutable inputs and never queries storage, reads settings,
invokes providers, assembles conversations, or mutates conversation history.

The canonical prompt representation is `PromptDocument.entries`; a legacy
disabled `system_prompt` entry is honored only by `LegacyV1`. `DeterministicV2`
uses the explicit `enabled` flag. The legacy `{{rules}}` and `{{ai_rules}}`
are obsolete import-compatibility placeholders
and always render as empty strings. They do not restore character rules.

`LorebookMatcher` implements the verified legacy matching behavior: recent
message detection uses the latest ten messages, latest-user detection uses the
newest user message, punctuation is normalized for literal matching, one-word
keywords use word boundaries, multiword keywords use substring matching,
trailing `*` is a word-prefix operator, and CJK/Thai/Lao/Myanmar/Khmer/Hangul
text uses normalized substring matching. Activation ordering is ordinal then
creation time; persisted priority remains metadata in both behavior versions
until the unresolved V2 product decision is made.

Repository traits are synchronous lifecycle ports only. Database adapters,
prompt assembly with conversations/memory/companions, import/export, backup,
sync, settings, and provider DTOs belong to later crates.
