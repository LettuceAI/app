# lettuce-creation: legacy parity notes

Facts about how `lettuce-creation` relates to the legacy app (2.2.x). The crate README describes the current design; this file keeps the comparison.

## Legacy parity

- The creation-helper progressive-edit and finalization behavior is pinned in `fixtures/legacy-import/creation-helper-tool-scenarios-v1.json`.
- The helper's tool names and schemas are legacy's creation-agent tools (`old-code/.../creation_helper/agent/tool_defs.rs`, `verbs.rs`) for every operation the proposal supports. As in legacy, the same tools are offered at every stage and the user can keep chatting after a confirmation request. Tool names match case-insensitively (names with spaces never pass provider validation), and the lenient argument reading is legacy's.
- Undeclared tools are answered with legacy's `unknown tool: NAME`.
- `list_creation_dialogue` leaves out turns without a succeeded attempt because legacy never persisted a turn whose reply failed.
- A retry re-reads the current helper model and settings, like legacy.
- The tool fallback setting is legacy's `creationHelperToolFallback`; `parse_creation_fallback` accepts legacy's envelope aliases, and a reply that does not parse ends the turn with its raw text, like legacy.
- Regenerate is legacy's `regenerate_response`.
- Existing-persona apply keeps the known-id resume boundary legacy's get-by-id had.
- Single-entry, keyword and staged lorebook generation behavior is pinned in `fixtures/legacy-import/lorebook-generation-tool-scenarios-v1.json`. That fixture is the legacy baseline, not a statement that its original first-slice exclusions are still unimplemented.
- The single-entry writer's contract is copied from legacy: ordinary and force modes, later entry beats `no_entry`, the normalization and 24-keyword cap, the exact JSON and XML fallback prompts and parsers, the six source/force final instructions and the two-attempt native/structured-fallback decisions.
- The keyword contract is legacy's: one required tool, exact final instruction and fallback prompts, first matching call, first-spelling deduplication capped at 24.
- The staged generator copies legacy's 5-to-50 target clamp, 20 000-character excerpts, planner tool and final instruction (field aliases, defaults, returned entry count), the pending-draft initialization on approval, the writer tool and final instruction (trimmed non-blank title and content, keywords trimmed without an added cap or deduplication, both always-active aliases with a false default), the batch size of three in outline order, the permissive manual editing and approval, refinement prompt inputs and instruction, and the coherence tool schema, four change kinds and their application rules.
- Like legacy, a planner source reference that names no extracted source is kept, and the writer then reads every excerpt.
- Source preparation copies legacy's ordered `src_NN` ids, 50 MiB per-source and 200 MiB total bounds, 20 000-character excerpts with the exact truncation marker, and legacy's source order. PDF text uses legacy's `pdf-extract` 0.7.
- The staged commit keeps legacy's explicit-name then initial-name precedence.

## Deliberate differences from legacy

- Legacy never left its preview status. Here any successful change returns the proposal to drafting, so confirmation has to be requested again.
- Legacy gave `write_lore_entry` a default `New entry` title; a missing title is now rejected.
- The legacy `edit_scene` texts promised `sc_*` ids that never existed; they now point at the draft summary.
- The lorebook description tool was removed because legacy had none.
- In the structured fallback, a call whose name is not a valid tool name is dropped instead of answered, and an envelope with neither calls nor reply keeps its raw text instead of an empty reply.
- Legacy's helper preloaded only the root name of an existing lorebook and silently discarded every reviewed entry at final apply. Existing-lorebook apply now applies the complete reviewed entry graph.
- Process-local job state, permissive free-text tool inference, positional draft identity, partial commit and cancel-by-deletion are not preserved.
- Per the pinned scenario correction, only approved staged drafts are committed.
- Staged commit appends with contiguous ordinals; legacy left ordinal gaps and collided with existing entries.
- Legacy's staged `pipeline.rs` called the provider without recording usage; the dispatches are now recorded. Legacy keyword generation also omitted usage recording. Legacy single-entry generation recorded both primary and fallback requests before checking response success.

## Not wired yet

- Legacy tools that need draft fields or services the proposal does not have yet: model, prompt, gradient, lorebook attachment, list reads, images, persona and lorebook deletion. Administrative tools and image or media leases were also listed as later slices.
- Resolving which model and settings a creation turn runs with (helper model then default model, streaming toggle, creation feature sampling slot) is caller-side wiring deferred to the frontend and command phase; the coordinator takes a resolved profile.
- Automatic host scheduling, lease renewal, progress IPC and frontend integration for lorebook generation are deferred. A project browser or host persistence of the selected staged project id is not implemented; resume works by known id.
- The single-entry generator's move to the dispatch ledger was listed as a separate follow-up; the current code records entry and keyword dispatches through the shared helper.

## Approved removals

- Remote catalog discovery, named in the crate description, is not ported (approved removal 2026-09-23).

## Test coverage

| Scenario group | Evidence |
| --- | --- |
| Text, Markdown and PDF sources and byte/excerpt bounds | `staged_lorebook_sources` keeps legacy limits, order and Unicode truncation. |
| Planner outline, required tool, approval and edits | Reducer and domain tests and the app staged SQLite scenario cover non-empty plans, stable ids and review CAS. |
| Three-entry batches, failure, retry and partial recovery | Batch checkpoint plus per-entry runs keep completed drafts, retry failed ids under new jobs and reject stale batch writes. |
| Draft edits, refinement history and coherence acceptance | Domain transitions and the SQLite execution scenario. |
| Explicit accepted-draft commit | The SQLite repository applies the book, accepted entries and receipt in one transaction; stale requests and duplicate commit intent are checked. |
| Cancellation and durable replay | The app scenario covers job settlement, kept review state, late response cancellation, frozen inputs and replay without redispatch. |

- The staged scenario uses scripted inference and repository reloads, not a process-kill test or a live provider. Backend scenario coverage does not mean the user-facing rewrite is complete.
- A barrier-backed SQLite regression verifies concurrent partial-retry writer execution, independent settlement and no redispatch of terminal jobs; resource denial leaves jobs unchanged, duplicate batch requests are rejected before inference, and a cancelled project's queued writer settles cancelled without inference.
- The on-disk source scenario saves an outline awaiting approval, closes the database, reloads it by request identity, replays configured admission without resetting review, and approves the restored outline.
- The staged lifecycle scenario covers usage in all four stages, invalid planner usage, cancellation after a response, independent concurrent writer failure evidence and replay without duplicated charges. Fault injection shows admission failure sends zero requests, settlement failure sends one, no false checkpoint is written, and a later retry keeps the pending evidence.
- Usage JSON tests cover old and new documents for all four usage types.

## History

- The previous README said protected binary and PDF extraction were still deferred, while also describing PDF extraction as done. The code extracts PDF text (`pdf_extract::extract_text_from_mem`) during source preparation; protected document reading is wired in `lettuce-app`.
- The creation-helper slice was delivered in stages: targets, turns and proposals; the native tool contract; attempts and rounds; remote dispatch and continuation through the `lettuce-app` coordinator; persona, character and lorebook finalization; structured fallback; regenerate.
- Staged runs gained configured input provenance, per-operation overrides and admitted prompt documents after the first slice; older run documents read these as absent.
