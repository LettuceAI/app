# lettuce-context

Prompt documents, lorebooks and the bindings that attach lorebooks to characters, personas and groups, together with the pure code that works on them: validation, condition evaluation, prompt rendering, keyword matching and lorebook activation.

The crate is pure. The renderer and the matcher take complete snapshots and return values; the repository traits describe synchronous operations without exposing a database, settings store or transaction. It does not read settings, call a model, assemble a conversation or change history. Conversation assembly and prompt selection live in `lettuce-app`, storage in `lettuce-database`, import and export in `lettuce-transfer`, and replication in `lettuce-sync`.

## Prompt documents

A `PromptDocument` is an ordered list of `PromptEntry`s plus metadata:

- `purpose` says what the prompt is for: a chat template (`DirectChat`, `CompanionChat`, the two group modes), one of the background jobs (memory summarizer and manager, reply helper, lorebook writers, image prompts, companion Soul writer, growth and consolidation), or `RuntimeText`.
- `provenance` is `BuiltIn`, `User`, `Derived { source }` or `Imported`.
- `behavior_version` is `LegacyV1` or `DeterministicV2` and changes entry selection (below).
- `condense` is a flag for conversation assembly; the renderer keeps it but never condenses anything itself.

Each entry has a role, content, an enabled flag, an optional condition, an optional image-slot payload and an injection position:

- `Relative`: part of the prompt block the caller places before history.
- `InChat`: inserted into history at `depth`.
- `Conditional`: in-chat, only once the conversation has at least `conditional_min_messages` messages.
- `Interval`: in-chat, every `interval_turns` messages.

Conditions are a closed vocabulary (`PromptEntryCondition`): chat mode, scene and avatar generation state, whether memory, lorebook content, an author note or a scheduled note is present, message and participant counts, keyword tests, provider id, reasoning and vision, time awareness, companion mode, and `All`/`Any`/`Not` combinators. `PromptConditionContext` holds the facts they are evaluated against. Trees are bounded to depth 16 and 256 nodes.

A conversation launch freezes its prompt as a `PromptSnapshot`: the entries and the fields rendering needs, without lifecycle, provenance or live timestamps. `render_prompt_snapshot` renders it with exactly the same code as a live document.

## Rendering

`render_prompt(document, context)` turns a document into a `RenderedPrompt` with two lists, `relative` and `in_chat`. The steps:

1. Check the render values. `PromptRenderValues` has fixed fields (character, persona, user, scene, lorebook, author note, summary, key memories and so on) and a map of purpose-specific values keyed by `PromptVariable`, a closed enum. A variable not allowed for the document's purpose is an error, and values are bounded at 1 MiB each and 8 MiB in total.
2. Select entries. Under `LegacyV1` an entry runs when it is enabled or marked `system_prompt`; under `DeterministicV2` only when enabled. Either way its condition must hold.
3. Resolve `{{#if name}}…{{else}}…{{/if}}` blocks. A block keeps its first branch when the named variable is non-empty. Blocks do not nest; a malformed, unknown or nested block, and a stray `{{else}}` or `{{/if}}`, stay in the text as written rather than failing the render.
4. Substitute placeholders in a single pass. A substituted value is never scanned again, so a `{{date}}` or `{{content_rules}}` inside a character description reaches the model verbatim. Name tokens (`{{char}}`, `{{persona}}`, `{{user}}`) inside the descriptions and scene text are resolved before substitution. An unknown placeholder is left as it is.
5. Drop entries whose text is blank, unless they carry a payload.
6. Place each entry by its position. In `DirectChat` and `CompanionChat`, a `Relative` entry whose content names a value that changes every turn (lorebook, memories, summaries, companion state, scheduled notes, dates or times) is moved into the chat at depth 0. Other purposes keep the authored positions.

Text the renderer substitutes verbatim, such as lorebook content, author notes, summaries, memories and history messages, is not rendered again, so callers resolve identity tokens in it themselves with `PromptRenderValues::resolve_identity` and `resolve_names`.

`explain_prompt` returns why each entry was or was not selected (`disabled`, `condition_false`, `injection_schedule`, `selected`), and `preview_prompt` returns both the render and the explanation for the prompt editor.

`render_prompt_text(purpose, content, values)` renders a single fragment with the same placeholder and conditional rules, without selection or placement.

### Why prompt text lives in documents

Every text a model receives is prompt content: chat templates, background job instructions, tool descriptions and the small runtime fragments the application injects (chat fallbacks, operation notes). The built-in catalog ships as a JSON resource in `lettuce-app` and is seeded as ordinary prompt documents; fragments sit in `RuntimeText` documents and are looked up by entry key. Rust code decides whether and where a piece of text goes in, never what it says. Built-ins stay editable in place, and one renderer applies the same rules to all of them. `RuntimeText` documents accept every render variable, including `{{regenerate_guidance}}`, and are never used as chat templates.

## Built-in prompts

Built-ins are seeded and reconciled through `PromptBootstrapPort`, kept apart from the user-facing `PromptRepository` so seeding policy cannot leak into normal edits.

A `BuiltInPromptSeed` has a stable unique key (plus optional aliases), a seed version, metadata, entries with stable unique entry keys, and `required` and `protected` flags. The adapter stores `PromptProvenance::BuiltIn` with the digest of the seed and the digest of the current authored content; both digests are computed from the closed seed input, never supplied by a caller. Comparing them tells whether the user has edited the document.

`reconcile_built_ins` runs in one of two modes. `RefreshUnedited` updates a document only while it is unedited; `ResetToSeed` overwrites it. Each seed reports `Created`, `RefreshedUnedited`, `PreservedEdited` or `ResetEdited`. Identity, revisions and timestamps belong to the adapter.

A protected built-in can still have its entries, name and condense flag edited, but changing its purpose or provenance, or archiving it, fails with `Protected`. A required built-in is recreated or restored at bootstrap and cannot be archived (`Required`). `required` without `protected` is valid: the prompt is editable but can never stay missing or archived.

## Prompt library

`PromptRepository` covers creating user drafts, reading, paging, revising metadata, mutating entries, archiving and restoring. Every mutation is a CAS on the document revision and returns the complete document with the revision bumped once.

Mutation inputs are authored drafts. `PromptMetadataDraft` holds only name, purpose, condense and behavior version. `PromptEntryDraft` holds entry content and an optional `built_in_entry_key` that only seeds may set; normal mutations leave it unset, and update or reorder keep an existing key. `PromptEntryMutation` is explicit: `Add` allocates identity and order, `Update`, `Remove` and `Reorder` need an existing entry id, and `Replace` supplies the whole ordered set.

Library queries (`PromptLibraryQuery`) take a `lettuce_types::PageRequest`, filter by lifecycle status first and order by `updated_at DESC, id ASC` with an adapter-owned keyset cursor. There is no unbounded listing.

`lookup_exact(id, purpose)` returns a typed `PromptLookupResult`: `Missing`, `Archived`, `PurposeMismatch` or `Available`. It never falls back to another prompt; choosing between app and session defaults is the caller's policy. `PromptDependencyReader` lists the characters, groups and starters that reference a prompt.

## Lorebooks

A `Lorebook` has a name, a lifecycle status, an optional icon, a `DetectionPolicy` and a behavior version. Its `LorebookEntry`s each have a title (may be empty), keywords, a case-sensitivity flag, a `KeywordMatchMode` (`Literal` or `Regex`), content, `enabled`, `always_active`, a priority and a dense ordinal. `LorebookDetails { book, entries }` is the aggregate a `get` returns. Entry mutations create at `Append` or an explicit index, or target an existing entry, and return the complete updated details together with the book revision for the next CAS. Library paging works like the prompt library.

### Matching

`LorebookMatcher` decides which entries of one book are active for a turn. The text it searches depends on the book's detection policy: the latest ten messages joined together (`RecentMessageWindow`), or only the newest user message (`LatestUserMessage`). An entry is active when it is enabled and either `always_active` or one of its keywords matches.

Literal keywords are matched after normalizing punctuation to spaces and, unless the entry is case-sensitive, lowercasing:

- a one-word keyword must equal a whole word;
- a multi-word keyword matches as a substring;
- a trailing `*` makes the keyword a word prefix;
- text in scripts written without spaces (CJK, Thai, Lao, Myanmar, Khmer, Hangul) always matches as a substring.

Regex keywords are trimmed, lowercased unless case-sensitive and compiled with the `regex` crate's default size limits. `validate_regex_keyword` validates a keyword in exactly the form the matcher compiles, so a keyword that saves is a keyword that runs.

### Activation across books

A turn usually draws on several books: the character's, the persona's, the group's, a starter's and the conversation's own selection. `resolve_lorebook_activation` takes them as ordered `LorebookActivationSource`s with typed provenance and:

1. skips duplicate book ids (the first source wins), missing books and archived books, and reports each as a skipped source;
2. matches every remaining book and records the exact book revision used;
3. orders all active entries globally by entry ordinal, then creation time, then source order. Priority does not take part;
4. fails if the active content exceeds `MAX_ACTIVE_LOREBOOK_CONTENT_BYTES` (4 MiB).

`resolve_lorebook_snapshot_activation` does the same for lorebooks frozen in a conversation snapshot. Snapshot entries have no timestamps or lifecycle, and this path does not invent them, so a replay depends only on data the snapshot actually captured. Matching goes through the same keyword code.

`LorebookMatcher::explain` gives the reason for each entry (`disabled`, `always_active`, `no_keywords`, `no_keyword_match`, `keyword_match`), and `preview` adds the active content joined by `format_lorebook_for_prompt` (trimmed, non-empty contents separated by blank lines).

## Bindings

Character, persona and group bindings have separate repository traits. A create command carries only the lorebook id and an insertion target; the adapter assigns revision, ordinal and timestamps. A binding follows the latest book revision. An archived book can stay bound and is skipped at activation; a conversation snapshot pins the revision it resolved. Binding lists are ordered, timestamps may not run backwards, and every mutation returns the owner revision it used as its CAS token along with the updated list.

## Limits

Counts are not limited: a lorebook holds any number of entries and keywords, a turn activates any number of books and entries, and a prompt holds any number of entries. Only bytes are bounded:

- entry titles, keywords and contents by the 8 MiB prose limit;
- a lorebook's authored text by 64 MiB;
- a turn's active lorebook content by 4 MiB;
- prompt, entry and lorebook names by the 1 KiB label limit.

The message text keywords are matched against is not bounded. Condition trees are the one structure with count limits (depth, nodes, children).

## Token counts

`count_tokens_batch` counts each text with the o200k encoding from `tiktoken-rs`, reading special tokens as plain text. The lorebook editor uses it for the trigger preview.
