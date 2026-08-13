# lettuce-context

This crate owns provider-neutral prompt documents, lorebooks, typed bindings,
validation, legacy matching, activation previews, and pure rendering. It does
not read a database or settings, call an inference engine, assemble a
conversation, or mutate history.

Repository ports are bounded and synchronous. Prompt and lorebook library
queries take `lettuce_types::PageRequest`, filter by lifecycle status first,
and use an adapter-owned keyset cursor with deterministic
`updated_at DESC, id ASC` ordering. This is an intentional library redesign
from the legacy prompt created-order listing. There are no unbounded library
list ports.
`LorebookDetails { book, entries }` is the snapshot returned by a lorebook
`get`; each entry mutation returns the complete updated details and book
revision for the next CAS.

Mutation inputs are authored drafts. Prompt metadata drafts contain only name,
purpose, condensation, and behavior version. Prompt entry drafts contain only
entry content; add allocates identity and order, update/remove/reorder require
an existing typed ID, and replace supplies the complete ordered draft set.
Lorebook entry create uses `Append` or an explicit insertion index, while
update/remove/reorder target an existing entry. Successful prompt mutations
return the complete document with one root revision bump.

Built-in prompts use a separate seed/reconcile port. Seeds have stable unique
keys, explicit purposes, non-zero versions, and a typed digest of the closed
metadata-plus-entry seed input. `PromptProvenance::BuiltIn` persists that
baseline digest, the current authored digest, and required/protected policy;
reconciliation returns per-key actions for created, refreshed, preserved, or
reset documents. Identity, revisions, and timestamps remain adapter-owned.
The digest is derived from the closed seed DTO and is never caller-supplied.
Protected built-ins remain editable for entry/name/condense content, but
purpose/provenance identity edits and archive return `Protected`; required
built-ins are recreated/restored by bootstrap and user archive returns
`Required`. `protected=false, required=true` is valid: the prompt is editable
but cannot remain missing or archived. Prompt lookup is exact by requested ID and purpose
and returns typed missing, archived, purpose-mismatch, or available results. It
never chooses app/session fallback precedence.

Character, persona, and group lorebook binding ports remain separate. A
binding create command contains only the lorebook ID and insertion target.
Bindings follow the latest book revision; an archived book may remain
referenced but is skipped by activation. Conversation snapshots can pin the
resolved revision later. Binding records require ordered, non-reversed
timestamps and return the owner revision with the updated bindings.

`resolve_lorebook_activation` is a pure, bounded multi-book resolver. It keeps
source provenance plus exact book revision, reports missing/archived/duplicate
sources, deduplicates by book ID, and globally bounds active entries and
rendered content bytes. It orders active entries by legacy ordinal, creation
time, then source order. LegacyV1 ignores entry priority. The generic renderer
skips blank text without a payload, but retains blank payload entries; the
`PromptDocument.condense` flag is only a signal for later conversation
assembly, where consumer-specific condensation belongs.

`LorebookMatcher` preserves verified legacy matching: recent-message detection
uses the latest ten messages, latest-user detection uses the newest user
message, punctuation is normalized for literal matching, one-word keywords
use word boundaries, multiword keywords use substring matching, trailing `*`
is a word-prefix operator, and CJK/Thai/Lao/Myanmar/Khmer/Hangul text uses
normalized substring matching.

Import/export, sessions, sync/backup, hard purge, legacy ID conversion,
database/schema types, events, FTS, async ports, and conversation assembly are
deferred to their owning crates.
