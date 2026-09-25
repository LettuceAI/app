# lettuce-app

Memory structured fallback and summary fallback preserve cache-read/reasoning
details when both responses supply them. Missing or overflowing detail sums stay
unknown. Input/output aggregation retains the existing fallback behavior.

The sole composition root, application workflows and desktop IPC boundary.
The legacy OpenAI-compatible host API is not ported (user decision
2026-09-23).

`AppBackend::preflight_legacy_database` exposes the read-only legacy SQLite
inventory boundary before any import is attempted. Compatibility types live in
`lettuce-transfer`, while all source SQL remains in `lettuce-database`.
`AppBackend::read_legacy_database_inventory` turns a legacy app data directory
into the same `LegacyBackupInventory` a legacy backup archive decodes to: the
database documents plus every regular file under `lettuce/{images,avatars,
attachments,sessions}` and `generated_images`, in archive order and bounded by
the archive entry, object and total limits. Unlike the legacy exporter, symbolic
links are never followed and names that cannot form a safe archive path are left
out, so the walk cannot read outside the legacy roots.
`AppBackend::plan_legacy_database_import` plans that inventory through the shared
legacy backup planner and projects one `LegacyImportPlan`. Provider/model,
prompt, persona and lorebook plans match the SQLite planners for the same
database (parity test in `lettuce-database`). ASR rows and voice audio come
from the database tables, because the legacy backup ASR document has no row ids
or voice examples; the voice audio candidates merge into the authored media plan
under the same total limit.
`AppBackend::legacy_character_importer` writes the planned characters of an
admitted run through the characters stage. `execute_database_import` takes them
straight from a `plan_legacy_database_import` result. It requires the plan's
source fingerprint and runs after the persona/lorebook, media and
provider/prompt stages. `AppBackend::legacy_group_importer` does the same for
reusable groups and their lorebook bindings once the characters stage finished.
`AppBackend::legacy_audio_importer` first stores each legacy audio provider API
key in the native secret store under its deterministic reference. On retry it
loads and compares the key instead of rewriting it. It then runs the audio stage
for the providers and user voices.
`AppBackend::legacy_settings_importer` runs the settings stage for the planned
global settings and their remapped feature model and prompt selections.
`AppBackend::legacy_direct_conversation_importer` turns each legacy direct
session into a finished conversation. The launch snapshots come from
`ConversationLaunchPlanner::prepare_direct` against the imported character,
persona, scene and default model; conversation, message, candidate, memory,
companion note and audio ids derive from the legacy ids within the run's
`LegacyIdScope`, so importing a different legacy source never collides with an
earlier import.
Each assistant variant becomes a candidate of its own succeeded turn (send or
continue for the first, regenerate for the rest), with a usage event that keeps
the legacy prompt/completion tokens when both are present, and the selected
variant stays active. Without a parent message or a chat model the variants are
kept as ordered revisions instead. The session's selected scene is the
conversation's scene whether or not the chat opens with it (its background,
direction and prompt follow it as in legacy); an opening scene row becomes a
scene message with its selected-scene origin; template rows stay plain
messages; hidden system
rows stay hidden. A blank legacy title falls back to the character name, the
launch key is derived from the session id's UUID, the message reasoning goes to
the active variant (legacy kept it only on the message), and without a selected
variant the variant matching the message content is active (legacy rendered the
content). A replay returns the committed stage receipt before any launch
planning, so sources changed after the stage cannot block it. Branch sessions
stay separate conversations. Companion memory jobs read the summary cursor per conversation
(`MemorySummaryRepository::summary_cursor`), so several companion conversations
feeding one shared pool each summarize their own dialogue while the pool keeps
one cumulative summary.
A group launch accepts companion characters as ordinary members (user decision
2026-09-14); companion state, prompt state, scheduled notes and the companion
clock stay direct-conversation only. `AppBackend::legacy_group_conversation_importer`
does the same for group sessions after the groups stage: it launches from the
imported group profile patched with the session's chat mode, memory policy,
character lorebook switch, member model overrides and own starting scene (a
group-owned scene with session-derived ids, snapshotted only) (`GroupLaunchOverrides`),
passes the session lorebooks only when the session overrode them, and rewrites the cast to the session's members, reusing
planned member snapshots, snapshotting members the profile no longer lists, and
turning speakers whose character is gone into disabled, muted "Unknown" members
(user decision 2026-09-11) so their messages keep an author. Group sessions
whose group profile is gone are recorded as `GroupReference`/`MissingGroup` and
left out of the stage for now. An all-muted legacy cast unmutes one member
(legacy refused to generate in that state), a message's author follows its
active candidate, and members without a model inherit the group model or the
first member override. Both conversation importers also import the session's
memory: legacy memories that fit the rewrite's memory item become the
conversation's memory space with ids derived per session (legacy branches
copied memories with the same ids) and deterministic short ids, stored embeddings of
64-768 dimensions become ready projections under their legacy source version,
memory texts that no embedding carries become memories of their own (legacy
kept texts apart from embeddings; manual memory, branched chats and character
file imports held texts without embeddings that its prompt and memory list
used) with legacy's manual defaults and no token count, but only where legacy
showed them: always on manual memory, on dynamic memory only while the chat
had no embeddings (dynamic = the global switch and the character's memory type
for direct and companion chats, the session's memory type for groups).
Corrected: those texts were dropped. A legacy summary covers the visible user and assistant messages up to
legacy's summary cursor, the last message of the newest memory cycle in
`memoryToolEvents` that still advanced it (not reverted, not an error or user
edit, anchored in this conversation; the pool carrier reads the shared pool's
events). Messages after it stay unsummarized, so the runtime picks them up like
legacy would have. Corrected: the summary used to claim the whole dialogue.
Deviation: without a usable cycle legacy restarted at message 0, which a
summary window cannot express, so the summary covers the first message.
Not yet imported: the per-cycle tool log and its revert (no rewrite
equivalent) and the cursors of non-carrier pool conversations (backups do not
carry per-conversation memory cursors).
A direct session's saved `companionState.preferences` (time awareness, off
when absent, and its frozen/ticking override, read like legacy `temporal.rs`:
an override missing its anchor runs on real time) becomes the conversation's
`companion_clock`; a companion session without saved state takes the
character's `time_awareness`, which legacy seeded on its next save. A session's own background becomes the conversation
`background` (see lettuce-transfer).
A legacy direct session of a companion character is imported as a companion
conversation: it binds the character's shared memory pool, which takes the
legacy companion shared memory when legacy kept one and otherwise the memories
of the character's most recently updated session that has any (user decision
2026-09-14); ready legacy embeddings land with the conversation that creates the
pool and the summary with the carrier. Its companion session state comes from
the legacy `companionState` JSON (clamped into the rewrite's ranges, launch
defaults when unreadable) with the legacy pool relationship of the session's raw
legacy persona key; every conversation of one character and launch persona
shares the latest conversation's relationship. Continuity episodes are placed in
one chain per character and launch persona by start time: legacy created them
lazily and kept sessions without a persona under a separate `__default__` key,
so indexes and previous links are renumbered along the merged chain (legacy
episodes keep their timestamps; sessions legacy never recorded start at their
creation and end where the next one starts). Legacy Soul facts that form an
exact snapshot replace the imported character's authored Soul facts (their
source memory ids mapped to the imported pool memory ids), and legacy
scheduled notes of imported companion characters are written in the same stage
(notes the rewrite rejects, such as blank content, are left out). Conversations
are written in episode order so episode links resolve.
`AppBackend::legacy_creation_importer` seeds a creation workflow with its initial
proposal for every legacy creation helper session whose untouched active draft
fits the rewrite's draft; sessions with chat, tool history, draft history or
images stay in sealed evidence. `AppBackend::complete_legacy_import` marks a
partial legacy run completed only once every stage in `LegacyImportStage::ALL`
has committed. Memories in an incompatible shape stay in sealed evidence. Session settings become
current conversation settings: the author note, the session prompt override
(snapshotted when the imported prompt is available with the conversation's
purpose, otherwise the launch prompt stays like legacy's fallback; a session
imported as a companion chat, because its character is a companion, imports none
since release 2.2.5 never read it, while a session whose own mode alone was
companion runs as a direct chat and keeps it), the lorebook
override (an empty legacy list disables lorebooks) and, for groups, a speaker
selection that differs from the profile. Legacy per-session sampler values,
background, voice autoplay and group chat mode or starting scene differing from
the profile have no destination yet. Candidates always carry the
launch model snapshot; the legacy message model is kept only in usage. Session settings
(author note, prompt and lorebook overrides, generation settings, background,
voice autoplay), memory fields, companion state and attachments are not mapped
yet.
`AppBackend::plan_legacy_personas` exposes the bounded persona candidates and
their explicit default owner while leaving all source media unresolved.
`AppBackend::plan_legacy_lorebooks` exposes the bounded root and ordered-entry
plan needed to resolve later persona and character bindings.
`AppBackend::plan_legacy_provider_models` exposes the read-only provider-account
and model-profile plan. Credentials remain pending secret-store inputs represented
only by API-key presence and header names; no secret value or destination row is
written during planning.
`AppBackend::plan_legacy_prompts` reads the bounded legacy prompt library and
app-wide prompt selection. Import admission seals those source identities, and
the provider/model completion transaction creates validated imported documents,
preserves entry order and legacy execution flags, maps the valid app default,
and replays after reopen without duplicate prompts.
`AppBackend::plan_legacy_media` resolves planned persona and lorebook references
below an explicitly supplied legacy storage root plus legacy ASR voice audio,
which may be an external file selected by the user. The application adapter
validates and hashes bounded regular files without copying them or exposing
ambient filesystem access to domain crates. Admission seals each original voice
locator, safe logical key, content hash and stable destination assignment. The
media executor revalidates and hashes each sealed source immediately before
ingestion, stores voice examples as persistent `OtherAudio` assets, supports
mixed image/audio plans, and replays the immutable completion after reopen.

## Boundary

`prepare_staged_lorebook_documents` reads ready SourceDocument assets through
the existing protected media handles, enforces legacy per-source/total byte
limits, verifies content hashes, and prepares ordered bounded text/PDF excerpts.
It returns extracted snapshots only; raw bytes and filesystem paths are not
persisted in workflow state. Source assets retain their existing library or
temporary retention policy; project-owned source associations remain pending.

Staged lorebook settings selection combines explicit overrides, persisted
generator settings and packaged prompt IDs. Model-owned lorebook-generator
overrides use the existing operation parameter layer. Unset temperature/top-p
use legacy 0.3/1.0; unset output tokens use the generator setting (default 4096),
not ordinary chat output tokens. Other unset fields inherit model parameters.
Reasoning is disabled with effort/budget cleared as in legacy feature requests.
Staged, single-entry and keyword prompt conditions treat an explicit Disabled
mode as reasoning off.
`admit_configured` loads persisted settings,
the selected model/account and planner prompt through existing ports, resolves
the profile with the shared model resolver, applies legacy target-count defaults
and bounds, then uses normal durable planner admission. Missing model/prompt
references and incompatible profiles fail before job creation.
`start_configured_batch` reuses that selection/resolution path with the writer
prompt, then starts the durable draft batch. Configured refinement and coherence
admission use the same path with their own stage prompts. Each admission freezes
its resolved inputs. Explicit project model/prompt overrides are persisted;
follow-up selection uses operation override, project override, then live settings
and defaults. Planner admission replay uses its saved inputs/profile even after
settings or model changes, rejecting changed request inputs. Configured refinement
and coherence also persist per-operation overrides and replay before live
configuration resolution; changed feedback, ownership, time, safety or overrides
conflict. Configured writer batches persist profile, prompt identity/revision and
operation overrides atomically with batch start. Partial admission resumes from
that checkpoint without resolving current settings, including entries that have
no job yet. Resolved low-level callers cannot replace a frozen batch profile.
Older low-level batches without this checkpoint still use their resolved API.
Execution checkpoint replay checks job ownership and project cancellation but
does not require the live prompt revision: no new inference uses that prompt.
New admissions also persist the full validated prompt document in the existing
run JSON (and configured batch checkpoint before per-entry admission). First
inference uses that frozen document instead of the caller's live document;
prompt edits affect newly admitted work only. Older runs without a snapshot
still require the caller's exact admitted prompt revision. This deliberately
replaces legacy's execution-time live template lookup with stable durable inputs.
Automatic host
wiring remains pending.
Writer batches validate prompt and text modalities before changing the project
or creating jobs, so invalid inputs leave the draft checkpoint unchanged.

May wire all crates; no crate may depend on it.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The composition root initializes storage and built-in prompts, plans direct and
group conversation launches, assembles provider-neutral context, and can build
a reusable remote-provider runtime. Provider composition requires the host to
supply its real `SecretStore` and current TLS policy; the app never substitutes
an in-memory credential store in production. The runtime loads provider
accounts through `lettuce-database` repositories and exposes catalog, model
discovery, and key verification through `lettuce-contracts` DTOs. It also owns
and exposes the matching `InferenceRuntime`, so later generation workflows can
register bounded stream receivers and cancellation tokens against the same
runtime used by the provider adapters.

Direct companion launch is now a usable storage-initialization path. The
planner freezes the validated authored companion defaults and effective
persona scope in a non-serializable prepared launch, and the database commits
the conversation graph, artifacts, operation/outbox, fresh session emotion,
carried character/persona relationship, and the next character/persona
continuity episode in one transaction. The prior open episode is closed by the
new launch. Exact launch retries revalidate the initial-state request without
resetting relationship continuity or advancing the episode sequence. Group
companions and group growth scheduling remain deferred.

Single-entry lorebook generation now has a restart-safe admission boundary.
Application preparation resolves the direct conversation's character and
persona owners, reads the selected branch messages, conversation memory and
summary, and target lorebook through their existing ports, then copies the
legacy chronological message, durable memory, relative-time, existing-entry,
and `(none)` formatting into immutable prompt values. Missing or foreign
selected message and memory IDs fail closed instead of being silently omitted.
One interactive generic creation job and one immutable request-owned run bind
those inputs before provider dispatch. The claimed-job executor renders the
frozen prompt with the exact source/force instruction, tries the native tool
contract, and uses exactly one same-model JSON/XML fallback only when no usable
declared native result exists. Each decision, usage counters, and provider
diagnostics are checkpointed before job settlement, so restart replays the
durable entry-or-none result without dispatching again. Changed admission
replay conflicts, and neither preparation nor execution mutates the lorebook.
Keyword-draft preparation separately freezes the legacy title/content,
existing-keyword, and direction values with the active keyword prompt and
resolved text profile, then admits one restart-safe request-owned creation job.
Exact replay returns the same run and changed replay conflicts; provider
execution uses the exact required native contract and one same-profile
structured fallback, checkpoints usage and provider diagnostics, and settles
the generic job. Completed keyword drafts replay without dispatch and no
lorebook is mutated.
Staged lorebook admission accepts only already-extracted bounded excerpts,
freezes the resolved planner profile and active planner prompt, and persists one
stable project with a generic restart-safe creation job. Exact admission and
created-to-planning CAS replay without duplicating state; source extraction
remains a separate input boundary.
Staged admission requests can prepare pasted text or already-read UTF-8 TXT/MD
bytes through `with_sources`; validation and legacy excerpt truncation happen
before job creation, and raw source bodies are not retained in the request.
Writer batch requests derive identity from the stable plan ID and persisted
batch revision, and replay with the frozen batch start time. Restart after a
partial completion reuses unfinished runs; retrying failed drafts admits new
jobs while completed drafts remain unchanged.
Failed refinement closes its job while preserving the existing reviewed draft.
Late failures from older draft batches close only their job; they cannot mark
the newer retry's draft failed.
Explicit planner retry admits a new job only after the previous planner job
failed. The same project, sources, resolved profile and prompt are retained;
retry admission and checkpoint replay do not issue duplicate provider calls.
Claimed planner execution renders the frozen prompt with the exact legacy
brief, decimal target count, source-excerpt formatting, final instruction, and
required tool declaration. It makes one native request, accepts only typed tool
calls, durably checkpoints calls, usage, and provider diagnostics, then applies
the planning-to-outline-review CAS. Restart replays that checkpoint without
redispatch, and the planner job succeeds while the project waits for review.
Outline approval then performs one restart-safe project CAS that creates the
ordered pending drafts from the reviewed plans and enters drafting. It does not
dispatch writer inference or reopen the completed planner job.
The coordinator also saves pre-approval outline edits through project revision
CAS, preserving planner evidence and supporting exact edit replay.
The coordinator also exposes restart-safe stable-ID draft edits and approval
toggles as project revision operations without dispatching inference.
Refine admission freezes the selected draft, trimmed feedback, exact legacy
render values, resolved profile, and active refine prompt. The shared writer
executor checkpoints one typed attempt before applying the revised draft and
revision-history append through the project CAS.
The staged project coordinator also persists coherence proposals and applies
only the accepted stable change IDs before returning to drafts-ready. Coherence
provider admission and execution remain separate.
Coherence admission now freezes the exact legacy drafted-entry rendering,
resolved profile, active prompt revision, project revision, and generic job.
Claimed execution uses the required native tool contract, checkpoints one
attempt, then submits the proposals; restart replays both checkpoints without a
second provider request.
Final staged commit accepts a new destination/name or an existing destination
revision, and delegates one atomic apply to the creation repository. The
committed project retains the result receipt for exact retries.
Project cancellation also cancels its queued jobs and requests cancellation of
claimed jobs. All three staged executors check the durable project before and
after inference; dispatch settlement recognizes cancellation that races with
provider completion. The active host can additionally signal the job handle to
abort transport promptly; durable cancellation alone discards a late response.
Claimed staged-writer execution renders those frozen values, appends the exact
legacy final instruction, and makes one required native tool request. Its
checkpoint is durable before the matching pending draft CAS; restart replays
without redispatch, and the request-owned creation job settles normally.
Batch admission selects the first three pending/failed drafts in outline order,
marks them durably as drafting, and creates deterministic stable-ID writer
runs/jobs. An active batch replays without selecting later work; terminal
writer failure marks its draft failed, while transient job retry leaves it in
the active batch.
A pending stable plan can be prepared into one restart-safe writer admission.
Preparation copies the legacy full-outline, proposed-key, and relevant-excerpt
formatting into immutable values with the active writer prompt and resolved
profile, then binds one generic creation job. Foreign/non-pending plans and
changed request replay fail before provider execution.

Direct user sends can now pass through `CompanionTurnCoordinator`. It detects a
companion from its normalized runtime state, resolves the stored direct
character/persona owner, reads the current authored companion config like the
legacy completion flow, classifies ordered user text through a narrow engine
capability, applies the copied pure signal and turn math, then submits the
atomic prepared companion send. Blank, missing, and unavailable classification
use the legacy neutral update; cancellation stops before persistence. Exact
send replay bypasses classification and never reapplies state. Roleplay and
group sends retain the ordinary conversation repository path.

Prompt launch follows legacy `build_system_prompt_entries`. A companion
character resolves the companion chain (`launch::policy::companion_prompt`): its
companion template, then the app default prompt (`default_prompt_document_id`),
each only when it is an active companion-chat document (legacy accepted an
app-wide template in companion mode only when its type was companion chat), then
the bundled companion prompt; a starter's prompt is ignored (release 2.2.5
`221fe1aa`: the companion template wins over any session template). A direct
character uses the starter's prompt, then its direct prompt, then
`launch::policy::direct_app_default_prompt`: the app default prompt when it is
an active direct-chat document, else the bundled app default prompt (disabled
only when that is missing too). The strict launch rule stays: an explicit
starter prompt fails on any resolution error and an inherited direct prompt
fails when dangling or of another purpose, while an archived inherited one now
falls through to the app default like legacy's not-found fallback instead of
disabling the prompt. Legacy used an app-wide template of any type in a direct
chat; only a direct-chat document is used here. A direct chat without its own
model override resolves its chat model live on every new turn like legacy
`select_model_with_credential`: the character's current default model, then
the app default model; a live model whose revisions differ from the launch
snapshot gets a conversation snapshot that preparation attaches, and the turn
records the model it used. Launch only chooses and
validates: context assembly resolves the system prompt of every one-to-one turn
from live sources (`live_direct_prompt`), so each turn uses the current content
and revision of the current documents. A companion chat (`companion_clock`
decides companion mode) follows the companion chain from the live character and
app settings, ignoring any chat selection, so a chat launched before its
character became a companion and a template changed after launch both follow the
current template. Any other direct chat follows `launch::policy::direct_prompt`,
legacy's non-companion order: the chat's selection when it is an active
document of a chat purpose, then the live character's direct prompt, then the app default
chain; a selection that is missing, archived or of another purpose falls through,
and a prompt the chat disabled yields none. The selection is a current prompt
override, else the prompt the launch pinned the way legacy pinned
`session.promptTemplateId` at creation (`repo.ts` 1586-1609): a starter's
explicit prompt, or an inherited launch prompt that is the character's direct
prompt. Editing a pinned prompt's content therefore reaches existing chats, while
pointing the character at another prompt does not; a chat whose launch fell back
to the app default chain pinned nothing (legacy's null session template) and
keeps following the character live. Whether an inherited launch prompt is the
character's pin is derived from stored fields, not a new flag: it is when its
source equals `direct_prompt_id` in the conversation's frozen launch character
snapshot, which the launch planner only inherits from that field and otherwise
fills from the app default chain. Branches are forks inside one conversation and
share its launch and settings, so they keep the pin as legacy's branch copy did;
the rewrite has no branch-to-another-character flow. The legacy direct importer
pins only what legacy pinned: a session template that imports becomes the
current override, and a session whose template was null or did not import gets a
disabled launch prompt (its launch snapshot draft is dropped), which is "no
selection", so its turns follow the live character like legacy's null session
template; that is distinct from a disabled current-settings prompt, which means
no prompt. A session template imports, and a chat selection resolves, when it
is an active direct-chat, companion-chat, group-conversational or group-roleplay
document (`policy::DIRECT_SELECTION_PURPOSES`): legacy set the session template
from a chat template, whose picker (`ChatTemplateEditorPage.tsx` 276-281) offered
direct and group chat templates, and rendered it whatever its type, so a group
prompt selected by a one-to-one chat renders there and render values its purpose
does not admit (scene LoRA keywords) are left out instead of failing the turn,
like legacy's placeholder replacement. Feature prompts (memory, reply helper,
lorebook, image) could never be a session template and still fall through.
The frontend's "reset chat prompt" must send `PatchValue::UseLaunchDefault`
(back to the launch pin or the live chain), not `Clear`: `Clear` stores a
disabled prompt, a state legacy never had. The launch and override snapshots now
only record which prompt the chat selected: their content is never rendered for
a one-to-one turn, the stored override stays as the chat's selection, and a
legacy session override still imports that way. `Database::set_default_prompt_document`
selects the app default prompt under the settings revision. A group turn also
resolves its prompt live (`live_group_prompt`, `launch::policy::group_prompt`):
the conversation's current override, the speaker's current group prompt for the
chat mode, then the group's, each only when it is an active document of the
mode's purpose, else the bundled group prompt; a disabled prompt yields none.
Lorebooks are read live on every turn the way legacy re-read them
(`prompt_engine.rs` 2309-2366, `group_chat_manager/mod.rs` 4897-4945): a direct
chat uses its own selection (a current override, or a starter's explicit books),
else the character's current enabled bindings followed by the current
persona's; a group chat activates its own selection or the group's current
bindings first, then the speaker's current bindings unless the group disables
character lorebooks, and an empty or disabled group selection (legacy's empty
`lorebookIds`) removes only the group's books. Entry edits, new bindings and
archiving therefore reach existing chats, and an archived or missing book stops
injecting. Each tier orders its active entries by legacy display order, then
creation time, then binding order, and a group turn keeps the first occurrence
of an entry. The turn records every book it used with the revision it read and
the activated entry ids. Persona, characters and group are read live too
(`generation::live_sources`). A one-to-one chat uses its chosen persona, or the
current default persona when it chose none or its persona no longer exists or is
archived (legacy `choose_persona`, `storage.rs` 509-521); a disabled persona is
none, and a launch that found no default persona stays without one. A group
chat uses its own persona, else its launch's explicit one, else the group's
current selection, and a persona that no longer exists is none (legacy
`load_persona`). Every character body is its current record, and
`{{group_characters}}` and `{{@"Name"}}` mentions use the group's current
members in cast order. A group's chat mode and character-lorebook switch follow
the group (`group_sessions.rs` 509-600); a launch bakes a legacy session
override of either into its snapshot with no separate marker, so the snapshot
value is used while the group is unchanged since launch and the group's value
once it changes, which drops such an imported override after the first group
edit. A group turn in conversation mode sends no scene. The live document goes through the launch
snapshot conversion, and prompt attribution carries its current id and
revision. Because the prompt is live, a one-to-one turn whose prompt (or any
document its chain fell back to) changed between its first context assembly and
inference admission is reassembled with a different context, conflicts with the
admitted request and fails closed, as a turn does when its model profile
changed; this is intended.
Context assembly reads the current authored Soul/prompting config, character-owned
Soul state, conversation/persona-scoped runtime state, and current persona
name, then renders the legacy prompt-state block with the stored continuity
episode at the source message's effective clock through the existing typed
`companion_state` placeholder. The bundled companion continuity entry drops its
"Scheduled Background Context" sub-section through an `{{#if scheduled_notes}}`
condition when no scheduled notes exist, where legacy stripped the rendered
heading from the string afterwards (`prompt_engine.rs` 4349-4350) and left four
newlines behind; that whitespace-only difference is deliberate.
A group-purpose prompt selected in a direct chat renders `{{group_characters}}`
as empty text; the old direct engine left the literal placeholder in the prompt
(corrected).

`companion_clock` resolves legacy `is_companion_mode` and the session clock for a
conversation: a direct chat is a companion chat when its live character is a
companion or it has companion state, and its clock counts only while time
awareness is on. Generation input renders every time placeholder the caller left
unset from the clock's effective now (legacy `time_placeholder_values`), sets the
time-awareness prompt fact that gates the catalog `# Time` entry, and formats
memory lines against the clock. With time awareness the assembler prefixes each
direct user and assistant history message with its stored `<time>` stamp, as
legacy `push_user_or_assistant_message_with_context` did, and the post-turn
memory host freezes the same flag into the memory run (legacy `flow.rs`), which
was hardcoded off before. Group prompts also get these time values, where
legacy left the literal placeholders in group prompts; that is a deliberate
correction. Legacy applied the clock whenever time awareness was on without
checking companion mode, but only companion sessions could turn it on, so the
companion requirement changes nothing reachable. A replayed attempt formats its
memory lines with the clock settings current at replay. With time awareness the
generation runner also strips timestamps the model echoed from every text part
of the final candidate before the empty-reply check (legacy
`strip_echoed_time_stamps`: `<time>` tags and the old bracket stamp anywhere, one
invented leading stamp, then trim); the replay input constructor resolves the same
flag, treating a character deleted after dispatch as no time awareness so the
recorded request still recovers. Legacy stripped after its empty-response check,
so a reply that was only a timestamp was saved as an empty message; stripping
first makes it an empty-reply failure, a deliberate correction. For a direct
companion chat with time awareness, dynamic-memory retrieval detects a calendar
phrase in the retrieval query against the clock's effective now (`temporal_query`,
legacy `detect_temporal_query_range`); when one is found only memories observed
inside that half-open window are candidates (none means no retrieved memories),
the similarity threshold drops to -1 and the recent and frequent fill slots are
skipped, while the cold keyword fallback still searches the filtered set.
A memory whose stored vector has zero norm scores 0 like legacy, so the -1
threshold still admits it, while a vector with a NaN component never matches.
Before the query is embedded, every memory of the space without a ready vector
for the current embedding model and its current text (new, edited, synced from
another device or embedded by an older model) is embedded and stored, like
legacy `migrate_session_memory_embeddings_if_needed`; an unavailable model
leaves them for the next retrieval and a failure to store one is only logged.
Legacy's 90-second per-memory timeout and progress toast are not ported (the
embedding call follows the job's cancellation instead).
Retrieval scores go through the active model's `SimilarityCalibration` before
the cold multiplier and the threshold: v4 keeps raw cosine and the configured
`min_similarity`; Eidos compares its calibrated score with its published
default threshold (0.50), falling back to its published fallback (0.35) when no
memory reaches the default, unless `min_similarity` is set, which is then
compared with the calibrated score as-is. Unset `min_similarity` on v4 means
0.35. The temporal -1 threshold is unchanged. The x0.7 cold-memory penalty
applies to the calibrated score, as legacy applied it to raw cosine. Semantic duplicate evidence also compares
the calibrated score with the duplicate threshold setting (strictly greater,
as legacy) and stores that score.
Calendar arithmetic is checked: an amount that overflows a date yields no range,
where legacy panicked on inputs like "200000000000 days ago".
Roleplay and group assembly do not read companion state. Missing or corrupt
companion state fails assembly closed, and the composition root exposes the
fully wired assembler over the shared database ports.

For dynamic-memory companion sends, the coordinator also derives the legacy
turn-effect seed from the already computed transition. Storage keeps that seed
with the admitted turn and creates the visible processing effect atomically
with assistant finalization; companion continuation uses the legacy zero-delta
seed without a user-message owner, and exact replay cannot duplicate it. Typed
ready/failed settlement is available for post-turn dynamic-memory coordination,
and the terminal coordinator now assigns authoritative added/updated memory IDs
to each coalesced effect through its exact user/assistant source window. It
copies the legacy largest-delta summary selection, percent rounding, wording,
pluralization, and three-part cap; no-op success is ready, terminal failures use
bounded stable reasons, and exact settlement replay is idempotent. Processing
effects can now be rediscovered from SQLite in stable order. Automatic
admission waits for the resolved legacy message interval and takes the oldest
ready effect prefix; a user/assistant effect remains atomic when it crosses the
interval boundary. The batch's logical idempotency key is derived from its
ordered durable effect identities and frozen interval, so
duplicate discovery reuses one runtime job and a fresh in-memory job store after
restart rebuilds the same logical batch. Empty discovery is the
non-companion/dynamic-memory-disabled no-op. The application bridge maps that
logical batch deterministically to one durable background memory run, resolves
the conversation-owned memory space, and freezes ordered visible user/assistant
messages with each message's exact active revision or candidate plus the
resolved inference profile. Same-job rediscovery replays the processing attempt;
a replacement runtime job restart-recovers it into one child instead of creating
a duplicate run. The first-round inference coordinator materializes those exact
revision/candidate bodies, renders the existing editable dynamic-memory prompt
with the copied legacy budget variables and runtime-input wording, dispatches
the run's frozen profile and required tool contract, and atomically admits the
single provider tool candidate as round zero. A committed round replays without
provider I/O; cancellation settles before admission, and rejected, empty,
mixed-content, undeclared, or inconsistent signed-replay outcomes do not become
durable calls. One admitted background round now reuses the existing create
preparation and typed reducer with the cycle's hard-delete budget (the run's
starting item count and the hard deletes settled by earlier rounds), then
atomically settles its ordered results with
the memory CAS; exact retry returns those results without embedding or reducing
again, and `done` preserves the existing short-circuit behavior. When a native
tool request errors or returns no calls, the coordinator copies the legacy
second request with tools disabled and the frozen JSON/XML format instruction,
parses its text into the same typed calls, aggregates both request usages, and
admits them through the same durable round path. Empty operations become an
explicit no-change `done` checkpoint. Before round zero, the runner renders the
editable dynamic-summary prompt over the frozen source window and prior
cumulative summary, requires `write_summary`, accepts valid same-response text,
and copies the legacy tool-disabled retry and validation. The validated summary,
token count, provider context, usage, and cursor commit atomically against the
memory root; recovery replays that checkpoint without provider I/O, and the
following memory-tool phase consumes its stored text. A settled background
round now appends its admitted native calls and their results to the exact
durable request context, stops before provider I/O on `done`, or dispatches and
atomically admits the next bounded round with the frozen profile/tool contract.
Each result is rendered in the legacy payload shape (`memory_tool_result`):
`status`/`name`, six-digit `memoryId`, `deletedMemoryId`/`deletedText`, the
`updatedMemories` list from the settled outcome, and for skipped calls the
echoed `arguments`, `repairQueued` for category skips and a human-readable
`reason`; the `[id] text` line and every reason are `prompt_app_memory_runtime`
catalog entries (`memory_id_line`, `memory_skip_*`), which the first-round
memory list uses as well. `done`, `stopped_after_done` and rejected results keep
their typed shape.
An already admitted next round replays without provider I/O. Debounce/startup
wiring remains a later slice.
The composition root now exposes its SQLite database as the durable generic
`JobStore`. The background loop connects the
existing round executor and continuation coordinator until the durable `done`
result. It resumes from the latest admitted checkpoint, skips seed generation,
embedding, reduction, and provider dispatch for already settled/admitted work,
and keeps create IDs, token counts, and timestamps as explicit caller inputs.
Effect discovery also accepts the exact authored `auto`, `askFirst`, or
`manual` run mode. `manual` admits no automatic job; `askFirst` waits for the
existing interval gate and records/replays a durable conversation approval
without provider dispatch; `auto` preserves the existing oldest-prefix cadence
and clears any pending approval when it admits the job. The approval query
returns only a currently pending count. Skip clears that pending flag while
retaining the prompted-count baseline, and approval bypasses the ordinary gate
to freeze the newest interval-sized whole-effect suffix. Older unsummarized
effects remain attached for terminal settlement while only that suffix is sent
to summary and memory inference, matching the legacy forced-cycle cursor move.
The same coordinator exposes explicit direct/group trigger admission without
consulting the authored run mode or interval gate. Direct trigger/retry selects
the recent suffix; the legacy group trigger keeps the oldest cursor prefix.
Both accept a partial window when fewer than one interval of messages exists,
keep whole companion effects, and reuse the same idempotent job/claim path.
Direct retry may freeze an explicit model profile into that same recent-window
job. The run rejects a different resolved profile, and an opted-in default
change happens only on the first successful terminal settlement. Failed or
cancelled retries leave the configured dynamic-memory model unchanged. Group
retry intentionally has no model-override entry point, matching the legacy API.
The run freezes whether companion time awareness is enabled and uses the matching
legacy tool contract. In enabled runs, a known create source is retained and an
unknown or omitted ID falls back to the latest frozen user/assistant source. In
disabled runs, source attribution is cleared. Each source effective time is
frozen with the run; enabled transcript lines use the copied legacy
`[message:ID] role: <time>YYYY-MM-DD HH:MM</time> content` format, and created
memories persist the selected role, observed time, and `turn` precision.
Companion runs additionally freeze the legacy supersession capability: their
create schema exposes `supersedes`, valid active targets are linked during the
existing atomic reduction, and disabled direct/group execution clears that
field before reduction.
The existing eight-round and 64-call admission limits remain authoritative.
Terminal success now feeds the run's durable starting snapshot and current
memory snapshot into the existing companion effect coordinator, settles every
coalesced effect idempotently, then marks the attempt succeeded. Failure and
cancellation use the existing bounded effect summaries and attempt failure
codes. Effect-first ordering makes a crash retry converge without leaving a
succeeded attempt whose effects are still processing.
The same terminal read now exposes the ordered prefix of at most sixteen
nonblank memory items whose IDs were absent from the run's durable starting
snapshot. This is the legacy growth trigger/evidence boundary; it does not run
growth or alter memory settlement.
Successful direct-companion results can now admit one restart-policy
`CompanionGrowth` job keyed by the durable memory-run ID. Admission freezes the
successful attempt, resolved memory profile, current Soul revision, character
owner, and bounded fresh-memory evidence; empty and non-companion results
create no job. A claimed growth job renders the editable growth prompt with the
frozen legacy values, requires the exact `record_growth` tool, checkpoints the
first parsed provider proposal, and applies it through the existing Soul policy
and repository with one stable operation ID. Restart skips inference after the
checkpoint and exact Soul apply replays its receipt. The same SQLite job claim
then settles success, cancellation, terminal rejection, or a retryable provider
or storage failure without changing the already-settled chat or memory run.
Only a succeeded growth job that actually added facts can admit its separate
`CompanionConsolidation` follow-on, and only when the authoritative post-growth
Soul has at least twelve active changeable facts. Admission freezes the exact
profile, authored context, Soul revision, character owner, and deterministic
apply identity in SQLite; repeated admission loads the same run and its one
immutable parsed-proposal checkpoint. A claimed consolidation job renders the
editable legacy prompt from that frozen snapshot, requires the exact
`consolidate_soul` tool, checkpoints its first parsed proposal, and passes it
unchanged through the existing consolidation policy and Soul repository with
the run's stable operation ID. Restart skips inference after the checkpoint and
replays the same Soul receipt; the SQLite job then settles success,
cancellation, terminal rejection, or retryable provider/storage failure.
Explicit Soul-writer preview admission now freezes the selected primary and
optional fallback profiles, editable prompt identity/revision, exact legacy
prompt values, normalized starting draft, structured fallback format, and
request identity in SQLite before inference. Repeating the request ID returns
the stored run and its interactive restart-policy job, and ordered round
checkpoints are idempotent; admission does not mutate the character or durable
Soul. The concrete dispatcher claims and settles that job; its executor renders
the same editable prompt only at its frozen ID/revision, appends the exact
legacy authoring instruction, and requires the six-tool contract. Fillers and
the authoring instruction render from `prompt_app_companion_runtime` at
admission (before the job exists) and are frozen in the run; fallback
instructions and tool descriptions render when a request is built, as do the
growth and consolidation lines, fillers and tool descriptions. It reduces
and persists every native round
before provider-neutral continuation, suppresses calls after `done`, resumes
incomplete checkpoints, and returns completed previews without provider I/O.
No-call responses use one tools-disabled JSON/XML fallback request with the
copied legacy instructions; a failed primary target may continue through the
frozen fallback profile. Cancellation, the eight-round hard cap, later
provider failures, and generic job settlement never silently promote an
incomplete draft. Character Soul mutation and frontend IPC remain outside this
preview workflow.
Direct companion context assembly now resolves scheduled notes from the
character-owned repository at the source message's immutable effective time.
The resulting block and `has_active_scheduled_note` condition are derived from
that authoritative active set, replacing caller-supplied scheduled-note text;
when an editable prompt does not consume `{{scheduled_notes}}`, the same legacy
block is inserted at in-chat depth zero. Roleplay and group contexts do not read
or inject companion scheduled notes.
The claimed-job runner composes the existing run admission/recovery,
first-round inference, bounded execution/continuation loop, and terminal
settlement into one application path. Prompt/profile/policy inputs and stable
create seeds stay caller-owned. First-round and loop failures are classified
through the same terminal coordinator before returning their typed error.
One shared startup/post-finalization bridge now rediscovers those durable
processing effects, reuses their deterministic per-conversation admission,
claims each exact job with the ordinary lease/resource lifecycle, starts it,
and returns the admission, claim, and cancellation handle needed by that
runner. Repeated dispatch while a conversation job is active is a no-op, and a
fresh job store rebuilds and claims the same logical effect batch after
restart. Host-specific runtime inputs and terminal job progress/completion
remain outside this bridge.
The matching settlement boundary keeps runtime lifecycle evidence on that same
claim: success records complete progress and the conversation outcome,
provider/tool failures use bounded job errors after the runner has settled the
attempt and effects, and cancellation follows request, cleanup, and terminal
steps. Admission or terminal-settlement errors return the existing job to its
retry queue so processing effects are not stranded behind a terminal
idempotency record.
Conversation launch now creates the authoritative normalized memory space in
the same transaction for every resolved manual/dynamic memory policy, and the
memory repository resolves it directly from the conversation identity. This
removes an external `MemorySpaceId` input from the pending post-turn worker.

The delete-after coordinator preserves the selected anchor message and applies
the existing branch-local tombstone policy to its suffix. It derives removed
IDs from the durable tombstone outbox record, restores memory and summary from
the earliest intersecting immutable run, invalidates removed companion effects,
and immediately admits retained effects from the rewound run suffix with a
restart-stable rebuild job key. Tombstone replay first checks the rewind receipt,
closing the crash gap between the conversation mutation and memory rollback.

The composition layer can also verify an installed embedding manifest through
`lettuce-model-hub`, load one serialized ONNX embedding runtime, and execute
request-scoped inference with a `lettuce-jobs` cancellation token. Hosts remain
responsible for scheduling this synchronous CPU/model-load work on the job
worker rather than a UI or async-runtime thread.

Dynamic-memory create preparation requires a claimed job carrying model-load,
disk-read, and CPU resources. It embeds candidates at the engine's configured
dimension (legacy `embeddingDimensions`, default 768), compares
only live exact-revision projections, supplies qualified duplicate evidence to
the reducer, and persists a projection only after the memory CAS confirms the
item survived. ONNX unavailability does not discard authoritative memory: the
create proceeds without semantic evidence and leaves rebuildable repair state.
Cancellation still stops preparation instead of degrading to an unembedded
write. The companion background rounds use this preparation.

Native creation inference now has its own bounded application coordinator. It
builds each request from the durable turn, base proposal, attempt-owned tool
contract, and resolved provider profile; dispatches through the shared
`InferencePort`; and admits mixed assistant text/reasoning plus native calls as
immutable creation rounds. Each admitted round preserves usage, provider finish
metadata, request identity, and replay evidence. Recovery reconstructs the
cumulative proposal and exact call/result continuation without redispatching
completed rounds. As in the legacy agent loop, rounds continue until a
response carries no tool calls (a preview or confirmation request no longer
ends the turn, so the model's closing message reaches the user), and after
eight tool rounds the attempt settles with the proposal accumulated so far
instead of failing (legacy stopped iterating and kept the draft); once the
attempt reaches the 64-call cap (a round keeps only the calls that fit) it
settles the same way. A request that cannot be built (missing runtime text,
unrenderable prompt, invalid profile) fails the attempt instead of leaving it
running. A text-only
response stops without fabricating a proposal; tool-driven completion appends
exactly one planned proposal; cancellation and provider failures settle the
attempt. Host commands and frontend surfaces remain a later
slice.

Dispatch is additionally bound to the attempt's unique claimed job and a
BLAKE3 fingerprint of the complete resolved inference profile. A stale caller,
reused retry job, or profile drift is rejected before the attempt starts or any
provider request is sent.

The creation admission use case records one user turn and its first bound
attempt in a single repository transaction against the exact workflow revision
and base proposal. Exact request replay returns the original pair; changed
identity or content, stale workflow state, and a job collision roll back without
leaving a turn that has no attempt.

Interrupted creation recovery is an atomic parent/child boundary. It preserves
the parent's partial rounds and calls for audit, settles that running parent as
interrupted, and creates an empty immediate child with the same base/profile
binding, a new planned proposal, and a distinct job. Partial provider rounds are
not transplanted into a new request. Successful continuation settlement now
commits the exact reduced proposal, workflow advance, and attempt success in one
SQLite transaction, closing the proposal-committed/running-attempt crash window.
Confirmed new- and existing-persona commands are also exposed through
application use cases and delegate to one creation-owned atomic apply port;
callers do not coordinate persona mutation and receipt persistence themselves.
The confirmed new-character use case follows the same boundary and delegates
the complete character graph plus receipt transaction to the creation apply
port.
Confirmed new-lorebook finalization is exposed through the same application
boundary and delegates its ordered aggregate plus receipt commit atomically.
The revision-pinned existing-lorebook use case uses that boundary as well, so
callers never coordinate root CAS, entry reconciliation, and receipt storage.
Confirmed existing-character finalization follows the same rule for profile,
scene reconciliation, dependency checks, root CAS, and receipt persistence.

## Bundled prompts

The app owns the closed, versioned built-in prompt catalog in
`resources/built-in-prompts/v1.json`. It is embedded with `include_str!`, so
bootstrap and reset work from the application binary on desktop and Android
without depending on installation paths or writable files.

`BuiltInPromptService` reconciles unedited seeds during startup and supports
explicit reset of one or every protected prompt. Consumers keep the returned
`BuiltInPromptIds` instead of hard-coding database UUIDs. The stable legacy
catalog keys and entry keys are compatibility identities; user-facing names
and content remain editable, while purpose and lifecycle protection do not.

Avatar image prompts (`avatar_image_prompt`, 2026-09-22) read the active
`prompt_app_avatar_generation` / `prompt_app_avatar_edit` documents the way
legacy's avatar sheet did: enabled non-blank entries in document order whose
`hasSubjectDescription` / `hasCurrentDescription` conditions hold (other
conditions pass), joined by blank lines, then legacy's nine placeholders
replaced in order with trimmed values; nothing else is substituted. With no
entry selected the bundled seed's enabled entries stand in for legacy's
template text. Local stable-diffusion.cpp models get the request as typed.
The creation helper's Rust path (template text, untrimmed, no conditions) is
not ported yet.

`AppBackend::open` is the supported production database-open path. It opens the
caller-provided SQLite path, applies database migrations, bootstraps the
catalog, and returns the database together with the complete typed prompt ID
set. This prevents a host from accidentally serving an initialized database
without the required app prompts. `AppBackend::open_in_memory` provides the
same invariant for tests.

Startup recovery is one explicit bounded application pass over expired durable
claims. It returns every recovered job with its persisted policy action and
current snapshot, including kinds whose executors are not wired yet, so a host
cannot silently consume unrelated work. `AppBackend` also exposes the existing
companion memory discovery/claim coordinator with the same SQLite database as
both effect repository and job store. Worker identity, resource admission,
runtime inputs, polling, and execution remain host-owned.

The adjacent `v1.snap` pins a canonical digest for every individual entry,
including its stable key, role, content, scheduling, conditions, payload, and
system-prompt flag. A change to one legacy field therefore identifies the
specific prompt entry instead of appearing only as a catalog-wide checksum.
Legacy removed empty sections by string replacement in Rust. The catalog
expresses the same rule as entry conditions instead: the app default's
`entry_world_info` requires `hasLorebookContent` and `entry_context_summary`
requires `hasMemorySummary`, so an empty lorebook or a non-dynamic chat no
longer sends a bare heading.

`AppBackend::usage_costs` exposes `UsageCostCoordinator::capture_job` for
caller-triggered OpenRouter cost capture by job and dispatch ID. It reads
immutable response identity, resolves the same-revision enabled OpenRouter
account, fetches generation metadata and the actual model's endpoint prices,
then writes an immutable basis using the existing usage ledger. Replays return
before network/account lookup; another writer's completed basis wins a race.
Provider errors leave raw usage and cost state untouched for retry. Missing
account/identity/usage/native totals/auxiliary counts or ambiguous pricing
returns no cost. No host polling, automatic post-inference scheduling or cache
was added. The narrow app-owned billing port is implemented by RemoteProviders.

SQLite scenarios cover native/normalized disagreement, preserved raw response
amounts, routed endpoint selection, stored generation/price provenance, old
basis JSON, changed-account gating, missing/error/ambiguous lookup retry and
reopen with account deletion and no network replay.

Staged lorebook planner, writer/refinement and coherence execution now reuse
the existing job-dispatch usage ledger before provider invocation and before
response reduction/checkpointing. Every actual dispatch preserves its response
ID and optional usage, including invalid responses and project cancellation
during inference; returned transport failures remain explicit failed evidence.
Checkpoint replay creates no second dispatch record. Existing successful
checkpoint usage stays unchanged. The staged SQLite lifecycle scenario covers
all four stages, invalid planner usage, cancellation after response, independent
concurrent-writer failure evidence and replay without duplicated charges.

Legacy staged pipeline.rs called the provider without recording usage; recording
these dispatches corrects that accounting omission. Legacy single-entry primary
and fallback requests recorded usage before checking response success; their
dispatch-ledger integration is described below. No new schema,
worker, pricing formula or host scheduling was introduced.

Lorebook entry and keyword final instructions, JSON/XML fallbacks, tool
descriptions and the preparation fillers (`(none)`, `[empty message]`, numbered
message/memory lines, existing-entry lines) render from the built-in
`prompt_app_lorebook_runtime` document with legacy wording. Fillers are frozen
into the run at admission together with the filler that marked an absent input
(`none_marker`), so later catalog edits cannot change a run's source gates;
instructions, fallbacks and tool texts are read when a request is built.
Selected memory lines are numbered contiguously over non-blank memories, as
legacy did.
Staged lorebook final instructions, tool descriptions and fillers (`(none)`,
`(empty)`, outline lines, source excerpt lines and the coherence drafted-entry
block) render from the
same document. Writer/refinement and coherence values are rendered at admission,
before the job is created, and stored in the run together with the `(none)`
filler they used (`none_marker`), which the run validator compares against;
the planner's excerpts, every final instruction and the tool descriptions render
when a request is built.
Lorebook entry and keyword native/fallback executions also use job dispatch
evidence. The shared helper distinguishes evidence persistence failure from
provider failure. Entry/keyword stop on evidence failure without writing a
false failed-native checkpoint or dispatching a fallback; provider failures
retain the existing fallback policy. Primary/fallback IDs and optional usage
remain separate, and successful replay adds no dispatch. Legacy entry generation
recorded both requests before response validation; legacy keyword generation
omitted that recording, which this corrects.

The same distinction fixes Soul-writer alternate-model fallback after evidence
or run-persistence/replay-cleanup failure. Other companion/staged callers retain
their existing public provider-error mapping. Fault-injection scenarios prove
admission failure sends zero requests, settlement failure sends one, no false
checkpoint is written, and later retry preserves the pending evidence. Existing
entry tests also prove provider-error fallback, cancellation and replay.

Background companion memory summary, native tool requests, structured fallbacks
and continuation now retain each dispatch through the same job ledger. Responses
are saved before summary/tool validation or checkpoint aggregation; transport
failures and cancellation remain distinct. Summary/round totals keep their
existing meaning and are not additional billable dispatches. Evidence failures
stop fallback; a cancelled primary response or job cancellation during a
tool-less response also stops fallback. The job store
and evidence repository must share durable job ownership.

SQLite scenarios cover summary fallback totals versus separate raw responses,
failed memory primary/fallback calls, continuation and terminal replay, missing
usage, storage failure without fallback, and cancellation with retained response
evidence. Legacy memory/flow.rs recorded summary and memory responses before
validation; this restores that boundary. No schema or scheduler was added.

Creation-helper continuation rounds retain each actual provider dispatch in the
existing job usage ledger before response validation. Successful round totals
remain unchanged; rejected/cancelled responses retain raw usage and response
identity even without a round checkpoint. Evidence storage failure stops the
run without classifying it as a provider failure. The caller must supply a job
persisted in the same store as the usage repository. Completed replay adds no
dispatch. SQLite tests cover these boundaries and separate two-round evidence
from the existing aggregate. Legacy creation_helper/service.rs also accounted
for initial/continuation responses before accepting their generated content.
Each creation request follows the legacy agent path
(`creation_helper/agent/run.rs`): the seven `prompt_app_creation_helper`
system entries (role, intake, creative principles, output contract, tool use,
examples, `<current_draft>`) with `{{target_label}}` and the legacy `DRAFT (...)`
view rendered from `prompt_app_creation_runtime` fragments, then every earlier
turn that succeeded (its user message and the trimmed visible text segments of
its attempt joined with blank lines; reasoning is not replayed), then the
current message; in-chat entries of an edited document go in at their depth.
When the resolved profile uses the local llama.cpp protocol, every entry
(relative, then in-chat) is trimmed, empty ones are dropped and the rest are
joined with a blank line into one leading system message, as release 2.2.5
does, because chat templates such as Gemma reject a second system message;
remote providers, Ollama included, keep one message per entry.
Lorebook drafts also list their entries with ids, which legacy never showed
because its entries were written straight to the database, and a lorebook
description is never shown (legacy showed `<unset>`). Not yet shown: the
avatar, background, model, prompt, gradient and image-gallery lines, whose
fields do not exist yet, and the non-native fallback-protocol entry (the legacy
UI never enabled it). Deviation: legacy had no history cap; the oldest turns
are dropped so the request stays under the 512-message provider-context limit
with room for eight rounds. Corrected: legacy flattened tool arguments and
turned every `"` into `'`, which broke the dialogue block the output contract
asks for; arguments are kept verbatim. Tool descriptions come from the
same runtime document. A call to an undeclared tool is recorded as an
`UndeclaredTool` operation and answered `{success: false, error: "unknown tool:
NAME"}` as legacy did, instead of failing the attempt.
Tool results follow the legacy agent shape `{success, message, error?, ...}`
rendered from `prompt_app_creation_runtime` (`Name set to 'X'`, `Scene added`
with `scene_id`, `entry written`/`entry updated` with the entry's id, title and
content, legacy argument errors such as `SET_NAME requires args: name=<text>`
with `message` `error: ...`, preview and confirmation with `action` and the
draft as it stood at that call). Differences: the `entry` and `draft` values
use the rewrite's draft shape, not legacy's stored rows, and the draft is left
out when the result would pass the 1 MiB tool-result limit. Corrected: legacy
always sent an empty preview/confirmation message (it read `note`), which is
now the model's message or the legacy default text, and it reported "scene
updated" on a failed scene edit. Rewrite-only failures (blank text, limits,
unknown or duplicate ids, a missing lore-entry title, a tool unavailable for
the draft) have their own texts. An error while replaying a stored round other
than a storage failure fails the attempt.

`ConversationInitialInferenceCoordinator` supplies the initial provider
dispatch boundary for a running conversation generation attempt. It reloads the
durable turn, verifies the turn/attempt/job/request identities and requires the
request's model and context attributions to match the prepared turn.
Each dispatch is admitted as a durable pending checkpoint before the provider is
called, using the job usage event that retains the raw response evidence. The
provider response, an invalid response, a provider failure or a cancellation
observed after the response settles that checkpoint once; a cancellation after
visible text streamed settles the streamed reply with a `Cancelled` finish
reason (the provider adapters return the text and reasoning emitted before the
stop), which generation finalizes as the stopped reply; a later call with the
same request replays the stored outcome or error without another provider
request, including after the attempt has been finalized or the database was
reopened. Changed context, profile, tools or media grants conflict with the
admitted request. A concurrent caller, or a caller that finds a pending record
whose settlement did not complete, receives a pending error instead of a second
dispatch; the attempt then follows the existing interruption and recovery path
and the child attempt dispatches fresh under its own job while the parent record
and evidence are retained. Cancellation before admission produces no evidence.
Response interpretation, tool admission and terminal aggregation stay with their
existing coordinators.

`ConversationGenerationDispatchCoordinator` and `ConversationGenerationJobRunner`
compose those pieces into one claimed-job pipeline for a conversation attempt.
Admission creates or reuses the `ConversationGeneration` job keyed by
the attempt's job idempotency key and attaches it; claim starts the job and its
stage; the runner then stages Preparing, prepares the turn from the supplied
model and attributions, stages Running, dispatches through the initial
coordinator, and records usage and finalizes the reply with the usage timestamp
frozen to the dispatch checkpoint. Chat replies declare no tools, as legacy
sends them with no tool config, so a reply carrying tool calls is rejected as
`ProviderRejected` instead of being executed. Every mutation the runner owns uses
an operation token derived from conversation, turn, attempt, job and step, so a
re-run replays instead of conflicting; a finalized attempt replays its candidate
and usage event without touching the provider. Settlement maps the run outcome
onto the job store: success, cancellation through the two-phase turn cancel,
failure through `fail_generation`, and a pending dispatch through interrupt and
recover into a child attempt with its own job (linked as a child of the parent).
Non-success settlement records a real usage event for the attempt from the
complete ordered dispatch evidence (known counters only when every admitted
response reported them, otherwise an unavailable reason) so no attempt ever
references a fabricated usage id or sums the aggregate event as another charge.
A recovered child on an already prepared turn moves straight to Running and
never re-prepares. Turn-side
settlement errors schedule a job retry instead of leaving the claim running.
Before each Preparing or Running stage append, the runner reads the latest
durable checkpoint sequence for its attempt and allocates the next value. A
pre-existing streaming progress checkpoint and process reopen therefore do not
collide with runner-owned stages; the database still enforces contiguous
uniqueness and operation replay. Streaming progress emission and frontend
commands remain outside this runner. The prepared runner exposes one synchronous
command-facing execution operation for an already durable turn and attempt. It
admits the existing idempotent job, claims that exact job with caller-supplied
resource availability and a shared cancellation token, runs the prepared
pipeline, and settles through the same dispatcher. A succeeded job returns its
durable candidate and usage as an exact replay without another claim or provider
call; other terminal jobs and temporarily unclaimable jobs are distinct typed
outcomes. `AppBackend` also owns the inference runtime shared by these executions
and its provider runtime. Cancelling by durable job ID first records the job
cancellation and then signals the exact registered token. An unclaimed queued job records the
turn's cancelled-before-response usage, settles the turn and job immediately;
claimed work remains with the runner until its response evidence and cleanup are
settled. Completion unregisters the live token. Unknown and already-terminal IDs
return explicit idempotent outcomes. The caller remains responsible for creating the turn, supplying
dynamic-memory create seeds, and deciding when to invoke the operation; no
second scheduler or background loop is introduced.
The dispatcher can schedule the exact `BeginGeneration` returned by send,
continue, regenerate or retry. A single worker operation lists the oldest
highest-priority queued conversation-generation job, resolves its immutable
creation event back to the attached turn and attempt, and executes it through
the same prepared runner. One invocation handles at most one job and returns
`Idle` when none is queued, leaving polling cadence and host lifetime outside
the application workflow.
`PreparedConversationGenerationJobRunner`, exposed by `AppBackend`, now owns the
reconstructible input boundary for an ordinary direct or resolved group turn. It
first reads an admitted initial-dispatch request for the current attempt or its
interrupted parent. That durable request preserves the resolved profile, exact
provider-neutral context, tools and media grants across restart and live model or
context drift; recovery rewrites only the current attempt/job cancellation
identity and installs the caller's runtime stream sink. Dynamic execution is
reconstructed from the launch-frozen policy, current conversation-owned memory
space and immutable settled round plans. When no dispatch exists, the runner
loads the durable turn and branch ancestry (timeline pages arrive newest first
and are reversed into oldest-first order, which speaker selection and memory
queries rely on; the whole branch is read, so long chats no longer fail, and the
context request carries only pinned and scene messages, the source message and
the recent window the assembler can select, so its omitted-message count reflects
that trimmed input), trims a finalized replay back to the
turn's original source message, resolves the selected participant's effective
settings and exact snapshotted live model/account, assembles provider-neutral
context, derives unique media grants from that context, and invokes the
claimed-job runner. Stored explicit decisions, including muted mention targets,
run directly. Automatic selection, the director check and the method recorded
for a forced speaker use the conversation's current speaker-selection method
(its own setting, else the group's method at launch). LLM selection without a
launch speaker-model snapshot (a conversation switched to LLM later) uses the
live group speaker model, else the default model, as legacy resolved it per
call; its decision records no model snapshot. For a user-message send in a non-director group, a mention of an
enabled character in that message becomes the explicit speaker before any
heuristic, round-robin or LLM selection, so no speaker-selection call is made
and mute is bypassed as in legacy. Mentions resolve against each character's
current record: its display name (nickname, or name) first, then its real
name. Display names are cosmetic otherwise; the LLM speaker-selection prompt
and group transcript labels use real character names. A participant whose
character record no longer exists is ineligible for every selection path and
is labelled "Unknown". A director continuation uses its forced participant, and
group regeneration retains the original candidate author; neither path fabricates or
persists an automatic-selection rationale. An unresolved group turn fails with
`SpeakerUnavailable` before provider dispatch. Only
the stream sink and prompt runtime values remain caller-supplied; the sink stays
outside the durable initial-dispatch fingerprint, so replay may use a new sink
without another provider call. Context or model preparation failures map into
the existing run settlement categories. Every turn reads live global settings.
A direct conversation launched with dynamic memory runs as manual memory while
the global `dynamic_memory.enabled` switch is off (legacy
`is_dynamic_memory_active`); groups follow only their memory mode, as legacy
groups did. The history window is `summary_message_interval` for dynamic
memory (the group override for groups) and `manual_mode_context_window`
otherwise (legacy only loaded the latest 120 messages; the window is now
bounded by the 512-message context policy instead). The lorebook scan (last
ten messages plus the latest user message) and the prompt message count use
the whole branch, not the sent window; the context timeline keeps those scan
messages. Unreadable settings documents fail the turn instead of retrying. A
dynamic direct turn resolves the
persisted global policy, loads its authoritative
conversation memory space and summary, embeds the latest visible user message
(or the enriched last two visible messages; a group regeneration leaves out the
reply being replaced, as legacy did), and selects current projections with the legacy threshold, cold-memory
penalty, category diversity, and smart recent/accessed fallbacks. Direct Smart
retrieval restores score order after category-overflow fill, then fills empty
slots with the newest and most-accessed unselected hot memories. Group Smart
retrieval reserves up to two slots for those hot picks before filling with
remaining diverse semantic matches. Equal semantic and cold-keyword scores
retain memory-space order, independent of projection query order. The selected
IDs also drive the existing retrieval-access receipt and cold promotions.
Companion Smart retrieval applies the legacy lexical overlap and sequence-anchor
bonuses after semantic candidate selection, preserving the selected set and
threshold while reordering its prompt lines and recorded IDs. This follows the
live character's companion mode; roleplay, group and Cosine queries receive no
bonus. Companion temporal-range filtering remains pending with effective-clock
parity; these retrieval checks do not establish temporal-query parity. Memory
reaches the prompt as in release 2.2.5: in a dynamic direct or companion chat
`{{key_memories}}` and `HasKeyMemories` use only the memories retrieved for the
attempt (send, continue and regenerate all retrieve), each with its legacy
observed-at suffix, and no separate retrieved-memory message is sent; 2.2.0
filled them with every hot or pinned memory and added a `Relevant memories:`
block on send. Templates without the placeholders get depth-0
`# Context Summary` and `# Key Memories` fallbacks, the latter holding the
same retrieved lines, suppressed by any raw template entry that names the
placeholder. Memory bullets, the `observed DATE, RELATIVE` note and
the relative-time words (`just now`, `yesterday`, `N units ago`, ...) render
from `prompt_app_memory_runtime`; the legacy buckets are computed as a typed
`RelativeTime`, and the lorebook entry writer uses the same note through its
`lorebook_selected_memory_observed` line. Deviation: legacy stripped leading
`- ` from the whole numbered line, so a memory whose text is only dashes lost
the space before its note; the rewrite strips the memory text only. Groups put `Important facts to remember in this
conversation:` plus the retrieved items into `{{key_memories}}` and get no
fallbacks. A group's `{{context_summary}}` is the stored summary in manual
mode too, as legacy group chats always used it. Manual memories render once
(legacy duplicated them in a relevant block); pinned history messages stay chronological. A stable identity derived
from the exact space revision enters the context. A rebuild of the same
attempt after its retrieval access reads the selection back from the
receipt, so the rebuilt context matches the first build.

Every section the turn injects outside the selected template comes from the
required built-in `prompt_app_chat_runtime` document (purpose `runtimeText`),
read live and rendered once per turn: the summary, key memory, world
information, author note, companion state and scheduled-note
fallbacks (a group-chat author-note variant as legacy had), swap places, and
the regenerate, continue, group begin and group continue-same-speaker
instructions, all with legacy wording. It renders with the turn's swapped
names, so under swap places `{{char.name}}` is the side the model plays. The group
`{{key_memories}}` prefix is its `runtime_group_key_memories` entry. Rust
only decides whether a section applies (placeholder absent, operation, last
message, group or companion mode); its text, role and depth come from the
entry, so a user edit or a disabled entry is honored (runtime entries are not
system-prompt entries, which LegacyV1 would render even when disabled). Direct chats now get the
legacy swap-places note and companion-state fallback, and groups the legacy
begin and continue-same-speaker notes instead of a generic continue line. The
author note and swap note name `{{persona.name}}` (the persona, or the user's
display name when there is none, where legacy wrote "user" / "the user
persona"). A missing runtime document
fails the turn with `RuntimeTextUnavailable`.
Chat history is sent in conversation order (the parent chain), never sorted by
message time, so a companion clock moved back keeps the whole context. In-chat
entries and runtime sections count their depth within the conversation
messages only, as legacy's `insert_in_chat_prompt_entries` did, so a depth
beyond the history lands right after the system prompt. A condensed direct
prompt merges every relative entry (any role) and the relative runtime sections
into one system message and every depth-zero in-chat entry except conditional
and interval ones into one turn-context message; a condensed group prompt
merges all of it into one system message (legacy
`condense_entries_into_single_system_message`, direct and group). A one-to-one
send outside dynamic memory adds legacy's "Relevant memories" block
(`runtime_relevant_memories`) first among the depth-zero entries. Identity
tokens resolve like legacy: `{{user}}` is the persona title, `{{char.desc}}` the
definition, else the description, and `{{persona.desc}}` the persona
description; lorebook text, summaries, memories and author notes resolve their
tokens before rendering, and every message of a one-to-one request resolves
`{{char}}`, `{{persona}}` and `{{user}}`. A one-to-one chat with no scene drops
entries naming `{{scene}}`. The reasoning condition follows the turn's
reasoning setting, not model capability. `{{group_characters}}` renders one
catalog line per member other than the speaker (definition, else description,
else the name), and `{{@"Name"}}` in a group's starting scene becomes the
member's name.
The companion state block and the scheduled-notes block render line by line
from the built-in `prompt_app_companion_runtime` document (bands, emotion
labels, Soul lines, regulation cues, note line, truncation marker and
heading, byte-identical to legacy by default); `lettuce-companions` decides
which lines apply. A disabled or blank line or emotion item is left out, a
disabled word fragment (band, label, partner fallback) renders empty inside its
line, and a disabled note line or block drops the scheduled-notes section.

Post-turn memory runs freeze the legacy memory tool contract with the
descriptions of the built-in `prompt_app_memory_runtime` document (group runs
use the legacy group wording and no `source_message_id`), and the memory
manager input lists memories as `[six-digit id] text`. The memory manager
runtime input, `none`, the JSON/XML fallbacks (legacy `123456` examples) and the
summarizer's final instruction, plain-text fallback, `write_summary` texts and
`No previous summary provided.` also come from that document, with the legacy
group variants (group summaries label transcript lines `[User transcript
line]:` / `[Character transcript line]:`). Protocol formats (`<time>…</time>`,
`[message:ID]`, `[id] text`, `role: text`) are data formats and stay in code.
The memory manager's and summarizer's prompt conditions see companion time
awareness (never for groups), and time-aware memory runs fill the date/time
values their entries use, as legacy did. A runtime-text key missing from a
stored document (a user-edited copy kept across a catalog update) renders the
bundled catalog text; a disabled entry stays off. Memory cycles follow the
legacy recursive-loop settings read live when the job runs (the group override
for groups): without `recursive_memory_loops` a cycle makes one memory request;
with it, rounds continue until `done` or `recursive_memory_loop_hard_cap`
(bounded by the 64-round storage limit), and reaching the cap ends the cycle
normally instead of failing it. A recursive round that returns no tool calls (even after the
structured fallback) also ends the cycle normally, as legacy did.
After the loop and before the run settles, every create the rounds skipped for a
missing or invalid category is re-tagged once through the memory-owned
`retag_memory` contract, rendered from the same `prompt_app_memory_runtime`
document (repair rules, numbered input lines, the category list and its
JSON/XML fallback). Answered candidates are admitted as one final round of
`create_memory` calls that the round executor applies like any other round;
candidates the request leaves unanswered are dropped, the last answer for a
text wins as with legacy's map insert, a tool response with no usable
`retag_memory` call still runs the structured fallback as legacy re-checked,
and a request that answers with nothing usable falls back to legacy's keyword
guess for all of them. The repair round is stored with
`DynamicMemoryRoundKind::Repair`: the loop treats it as terminal on a same-job
resume instead of continuing the manager loop out of the repair prompt, an
attempt that already holds one is never repaired again, and a failed repair
round execution logs and keeps the cycle as legacy did; only cancellation
propagates. After the repair pass the runner applies the reducer's
`finish_cycle` (trim to `max_entries`, then demote to the hot token budget)
through the memory CAS before the attempt settles; a replay finds no change. A
revision conflict re-reads and retries twice, then keeps the cycle with a
warning (legacy saved last-write-wins); any other storage failure settles the
attempt as a recovery failure. At the other end, `admit_or_recover` runs the
reducer's `start_cycle` (legacy's pre-summary decay) on the space and admits the
resulting change with the run, so `starting_memory` is the post-decay space and
a recovered or replayed run never decays again; legacy's turn-effect diff was
taken before decay, which only matters for an existing memory sourced from the
current window whose importance changed.
`ReplyHelperCoordinator` (`reply_helper`) is legacy `chat_generate_user_reply`
("help me reply") for direct conversations: gated by `help_me_reply.enabled`
before any job exists; the live character (definition, else description) and
the effective persona (title, description; "user" and empty without one),
swapped when `swap_places` as legacy `swapped_prompt_entities`; the last
`history_count` visible user/assistant messages of the active branch; the
`help_me_reply` model, else the default model, with legacy's temperature 0.8 and
top_p 1.0 applied only where the model declares the parameter and the settings'
output cap; the per-style prompt override (active, right purpose) or the
built-in roleplay/conversational document rendered with the character, persona
and `{{current_draft}}` values; the runtime user entry from
`prompt_app_chat_runtime` (`runtime_reply_helper_line`,
`runtime_reply_helper_input`). It runs as a `CreationRun` job keyed by the
request id (usage evidence, cancellation), streams to the request id when
`streaming` is on, cleans the completion like legacy (trim, quotes, leading
"{user}:") and settles the job. Group conversations follow legacy
`group_chat_generate_user_reply`: every character participant is listed as
"Name (definition or description)" in place of the character name, the
character description is the catalog filler "participants in a group
conversation", transcript lines name the message's character ("Character"
when unknown), places are never swapped, and the group input/closing text comes
from `runtime_group_reply_helper_*`; unlike legacy's hardcoded ten messages the
group history honors `history_count`. Legacy's per-model HelpMeReply feature
override has no destination.
The reply itself carries no memory tools: legacy writes memories in
a separate post-turn cycle (`enqueue_post_turn_dynamic_memory`). Plain direct
and group conversations now admit that cycle through
`admit_plain_after_turn_and_claim`: the visible user and assistant messages on
the active branch are counted from the summary cursor, `manual` admits nothing,
`ask_first` records a pending approval, and `auto` admits exactly one
interval-sized window per run, as legacy did. The run reuses the existing
background summary, memory-round and terminal pipeline with a message-window
source instead of companion effects. `CompanionMemoryHostCoordinator`
(`companion_memory_host`) is the host entry point legacy's
`enqueue_post_turn_dynamic_memory` provided: `after_turn` admits and claims the
cycle a finished send or continue earns (never a regenerate; direct chats need
the global `enabled` flag and a dynamic session, groups only a dynamic session,
as legacy gated them; the memory mode comes from `effective_memory`, which
needs no selected speaker), admitting this conversation's processing effects
for companion conversations and the interval window for plain ones;
`resolve_runtime_inputs` reads the live direct or group settings into the runner
inputs (model: the admission override, then `dynamic_memory_model_profile_id`,
then the default model, else the job is rescheduled so the same window runs
once a model is configured, like legacy's "Summarisation model not configured"
retry on the next turn; the user's `dynamic_memory_prompts` override when it
is an active document of the right purpose, else the local manager prompt for
llama.cpp accounts or the built-in document, as legacy fell back to its
built-in entries for a missing template; on llama.cpp accounts the memory
call drops the creative sampler like legacy (`top_k` 40, zero frequency and
presence penalty, repetition penalty 1.0) unless
`dynamic_memory_llama_sampler_overwrite_enabled` is off; legacy's per-model
DynamicMemory feature override, sampler profile/order, min_p, typical_p and
DRY resets have no destination before the llama.cpp runtime slice; policy, duplicate threshold and fallback format from the
settings; supersession for companion conversations); `run_claimed` seeds
creates with the embedding tokenizer (zero on failure, as legacy), runs the job
runner and settles the job. `trigger` is legacy `trigger_dynamic_memory` /
`retry_dynamic_memory`: the same gate, then a forced cycle over the most recent
interval-sized slice of the unsummarized dialogue (companion conversations use
their effects, an optional model override with update-default-on-success is
carried on the batch); `skip` and `pending_approval_count` are legacy
`skip_dynamic_memory_cycle` and `dynamic_memory_pending_approval`. A forced
window never reaches back into already summarized messages, which legacy's
`next_window(force)` could. When a run already exists for the batch, its frozen
profile, flags and fallback format win over live inputs that changed meanwhile
(legacy's in-progress cycle kept the settings it had read); only the
conversation, space, source messages and interval must still match. Companion time awareness has no destination yet, so
the host passes it disabled, as generation does. Retrieval embedding
unavailability preserves
the legacy behavior of continuing without retrieved keys. A nonempty selection
now atomically promotes selected cold items and records the legacy access count,
time and importance updates exactly once under the preparation attempt without
advancing the memory revision; the revision the retrieval read is the context
attribution.
Terminal replay bypasses input reconstruction, so it does not repeat embedding
or access side effects; changed or stale retrieval input fails closed. Manual
mode now reads every active item from the conversation-owned memory root in
stored order and renders the legacy `- text` lines without summary, tools,
embedding, promotion, or access-count changes. Its exact root revision is the
turn attribution for send, continuation, and regeneration. Legacy manual memory
has no selected-revision concept, and the current snapshot field has no writer
or content authority, so a nonempty `selected_revision_ids` still fails closed
instead of inventing a second store. Direct launches freeze the global direct
policy, while group launches freeze the complete group override when present
and otherwise inherit the direct policy. Prepared
generation requires and uses those frozen values, so later global setting
changes cannot alter retrieval, access mutation, or tool-round policy for the
conversation. Older dynamic snapshots without the optional policy remain
readable but fail closed if generation tries to use them. Durable request-body
recovery remains a later slice.
For unresolved group turns using the frozen heuristic or round-robin policy,
the prepared runner now derives participation counts and the prior speaker from
the durable active-branch timeline, stages `SelectingSpeaker`, applies the
existing pure policy, and persists the decision through `ResolveGroupSpeaker`
before preparing context. Disabled participants are ineligible and muted
participants are excluded from automatic choice. Group invariants keep at least
one enabled unmuted member available; the pure policy retains its defensive
no-speaker error. The persisted selection and completed generation replay after
process reopen without selecting or dispatching again. Group launch freezes the
dedicated speaker-selection model when configured, otherwise the application
default, including its exact model/account revisions and protected artifact.
LLM selection uses that frozen identity when the live revision supports tools,
the legacy narrator prompt rendered from the built-in
`prompt_app_group_speaker_selection` catalog document (participant blocks with
JSON-quoted fields, participation share, turns since the participant last
spoke, the last ten messages as 512-character previews, the quoted new user
message, the legacy guidelines and muted note), and a required
`select_next_speaker` tool whose descriptions come from the same document and
whose enum contains only enabled unmuted participants. Group transcripts prefix
the user's and other characters' messages with `[Name]: ` from the chat runtime
entry `runtime_group_message_prefix`, as legacy did.
The provider call is admitted under a separate immutable selection checkpoint
and job-usage record before dispatch. A valid call persists its participant and
bounded rationale; invalid output or non-cancellation provider failure retains
that usage and falls back to the existing heuristic. Cancellation stops instead
of starting fallback. The settled decision and subsequent generation replay
after reopen without repeating either dispatch. Clearing or changing the live
setting after launch does not change that conversation; a missing or unusable
frozen model falls back heuristically without provider usage. The legacy final
fallback to the first configured model is deliberately omitted because it had
no explicit selection provenance.
The selection and generation job-usage rows are raw dispatch evidence for the
same attempt. Their counters overlap the terminal `UsageLedger` aggregate and
must not be summed with it as independent charges. Required-tool output is
validated structurally; missing, malformed or foreign calls use the heuristic
fallback instead of accepting arbitrary response text as a speaker identity.
`PrepareGeneration` records the resolved model and prompt/lorebook/memory
attributions atomically before moving a preparing turn to ContextPrepared.
The existing speaker-resolution mutation continues to own group speaker choice.
Preparation also accepts the explicit director target or original regeneration
author without requiring a new selection decision; mentions remain explicit
decisions rather than automatic selector requests.
Exact operation replay preserves the stored preparation; changed input conflicts.
Recovery can reuse matching preparation under the child's attached job, while
changing the prepared model or attributions requires a new turn. Full automated
send/regenerate/retry orchestration and durable request-body replay remain later.
Generation finalization derives a regenerated candidate's persisted ordinal
from the prior candidate instead of trusting the provider-local response index;
this preserves dense alternatives while new-assistant turns still begin at zero.
Both the post-turn admission and explicit triggers list pending effects with
`list_processing_for_conversation`, which filters status, conversation and
invalidations before the 512-effect page limit; the earlier status-agnostic
page stopped admitting after 512 settled effects and a global page could be
filled by other conversations.
The reply helper's streaming setting is a preference, not a requirement: the
request streams only when the account has streaming enabled and the model does
not declare streaming unsupported, otherwise it is sent as a plain request, as
legacy `effective_streaming_enabled_with_override` downgraded it.
An explicit trigger always clears the pending approval, as legacy did, and a
trigger while another cycle of the conversation is still running returns
`CycleInProgress` instead of an empty result, so callers can tell it apart from
gated memory; legacy started a second concurrent run instead.
A reply helper job that cannot be claimed is cancelled while still queued, so it
never lingers, and repeating a request id whose job already settled reports
`AlreadySettled`.
The running-cycle check pages through every MemoryExtraction job of the
conversation instead of only the first 200, so a long job history never hides a
running cycle.

The legacy import admission coordinator fingerprints the complete preflight inventory and provider/model/prompt/persona/lorebook/media plans, derives the closed source ID set, and admits it through the transfer-owned repository port. It allocates no IDs itself and performs no filesystem ingest, secret read, secret-store write, or domain creation; SQLite returns sealed account, profile, prompt, secret-reference, graph, and media mappings so retries and reopen use the same destination identities. The deterministic legacy llama.cpp account participates in the same assignment path. The legacy database and storage tree remain untouched and retained.

The provider-secret import coordinator requires the live read-only source set to
match every sealed secret assignment. It binds each value to the assigned
destination reference and destination account owner, writes only through
`SecretStore`, and records non-secret progress after each value. Retry and reopen
verify purpose, generation and exact secure-store value. An already-correct value
without a receipt is treated as a recoverable write-before-receipt crash; a
changed, missing or foreign value stops without overwrite or deletion.

The provider/model import coordinator verifies every assigned secure-store
reference, purpose and generation against its immutable completion receipt before
asking SQLite to materialize metadata. It recomputes the complete sealed plan
fingerprint, exposes no secret values to the transaction, and returns the same
provider/model receipt on replay after reopen. Overall import completion follows
only after both graph and provider/model receipts exist.

The legacy media import coordinator reopens only confined regular files under the read-only source root, rejects symlinks and path drift, and rechecks byte length plus BLAKE3 before ingest. It catalogs each file under the admission's assigned asset ID, records durable completion after the ready blob exists, and resumes partial progress by replaying completed objects. Shared bytes deduplicate to one blob while their logical asset identities remain distinct. It never removes or rewrites the legacy source.

The legacy graph execution coordinator recomputes the sealed plan fingerprint and asks the transfer repository to materialize it. SQLite creates lorebooks and their assigned entries, personas and assigned media links, ordered active-lorebook bindings, and the legacy default owner in one transaction only after all media receipts verify. Legacy entry display order becomes the contiguous stored order already established by preflight sorting. Exact retry returns the original completion receipt, including after reopen; any mismatch leaves no partial graph. Source cleanup is intentionally absent pending full migration verification and explicit user-approved deletion.

The legacy ASR import coordinator independently materializes every admitted
vocabulary, correction, ignored suggestion and voice example in one transaction.
It preserves validated learning metrics and timestamps, resolves optional links
through assigned typed IDs, and binds examples only to completed managed audio.
Its immutable receipt replays after reopen without overwriting live learning
data; graph import can proceed before voice audio is available.

The ASR learning transfer coordinator exports the four filtered learning record
classes as one bounded versioned document. Import validates every record and
link before allocating fresh IDs, verifies each ready managed audio asset
against its content and redacted provenance evidence, remaps voice links and
delegates one atomic batch to the speech repository. Missing or changed audio
and malformed later records leave earlier records uncommitted.

Legacy version-2 ASR JSON import validates every source identity, normalized
value, timestamp and optional link before opening audio. Relative voice paths
are confined to the selected document directory; absolute paths preserve the
old file-picker workflow, while symlinks and non-files are rejected. Each
distinct file is ingested once as managed library audio, then the complete
converted version-3 graph uses the same atomic learning import. Source JSON and
audio are never modified or removed.

The speech-transcription coordinator admits an idempotent interactive
`SpeechTranscribe` job, binds it to one immutable audio/model request, claims it
through the generic worker lifecycle, and coordinates managed audio decoding,
prompt construction, runtime execution, correction application and durable
settlement. Successful results replay after reopen without another runtime
call. Cooperative cancellation reaches the runtime token and settles through
job cleanup; unavailable audio, learning storage, models or runtimes schedule a
retry, while malformed inputs fail terminally. `AppBackend` owns one
process-lifetime `WhisperCppRuntime` backed by the installed-model repository so
verified contexts can be reused across coordinator calls. It also exposes the
SQLite-backed ASR learning library and a composed run method that supplies both
the real learning and Whisper adapters to a claimed transcription. Host
scheduling and microphone IPC remain later integration work.

The remote Whisper catalog resolves the current Hugging Face repository head to
an immutable commit before reading its file tree, through the shared
`model_pin_request` and `parse_pin_listing`; Whisper, Kokoro, artifact-install
and GGUF-header downloads build their file URLs with the shared
`pinned_resolve_url` / `resolve_url`. It exposes only bounded model
metadata with coherent LFS size and SHA-256 evidence, retains the legacy
recommendation classifications, and rejects mutable or incomplete entries
before download admission.

The Whisper download coordinator admits the pinned remote identity as a durable
artifact-install job, reports bounded byte progress, preserves matching partial
bytes across restart, and cooperatively cancels before the irreversible install
stage. It accepts a resumed response only at the requested byte offset, safely
restarts when a server returns the complete object, verifies upstream SHA-256,
atomically installs, and admits the existing BLAKE3 manifest afterward. A crash
between rename and manifest admission recovers from the verified final file;
completed jobs replay without another network request or a public native path.

The Whisper-model coordinator scans the retained legacy model directory through
the bounded model-hub inspection contract, records each verified manifest, and
replays exact admissions after reopen without changing source bytes. Resolution
preserves the legacy first-filename default or accepts an explicit installed
model ID, then revalidates size and BLAKE3 before producing the speech request
descriptor and runtime-only artifact handle. The runtime independently repeats
that verification before loading native code. Managed removal validates model
ownership, clears every cached Whisper context before deleting bytes, then
conditionally deletes the exact immutable manifest. Missing retries are no-ops;
retained legacy files cannot enter this removal path.

The TTS configuration coordinator creates remote-provider credentials only in
the injected native secret store under `AudioApiKey` ownership, then persists
their reference through the speech repository. Key rotation uses secret
generation compare-and-swap. Provider metadata and user voices use repository
revision compare-and-swap, and provider kind cannot change during an update.
Provider deletion first validates the exact revision, atomically removes its
voice graph, and deletes the matching secret generation. A native-store failure
after metadata deletion returns an opaque retry receipt; callers cannot forge
its secret identity. Kokoro rejects credentials. Remote provider transports are
constructed with the host's current TLS policy. ElevenLabs and hosted Fish
configured-voice refresh load only the provider's scoped secret, reject partial
or invalid responses, and atomically replace that provider's bounded ordered
cache after the complete fetch succeeds. Preview caching remains a later TTS
slice.

ElevenLabs, hosted Fish and Gemini provider verification load the existing
provider and its exact `AudioApiKey` secret scope before calling the typed
verification transport. HTTP credential rejection returns `false`;
configuration, secret-store and transport failures remain distinguishable
errors. Gemini verification also requires the stored project and location. Fish
Speech verification supports both scoped-secret and unauthenticated servers.
OpenAI-compatible verification requires its stored base URL and scoped secret
and performs a real standard models probe instead of returning success locally.

The ElevenLabs voice-design coordinator admits only a stored ElevenLabs
provider, freezes its provider snapshot, trims and validates the active editor's
sample and description, and resolves only that provider's scoped `AudioApiKey`.
It passes cancellation into the shared remote runtime and sends decoded preview
bytes directly to the existing media sink. Each result retains the generated
voice ID, finite duration, detected MP3 MIME, content hash, size and temporary
managed asset identity; provider base64 and plaintext credentials never cross
that boundary. Voice-design preview requests are interactive and transient;
their audio assets carry an admitted expiry instead of adding another durable
job or request table. The same coordinator admits saved-voice creation from one
generated preview, resolves the same scoped secret, calls the remote runtime and
returns the validated provider voice ID. The active legacy flow then passes that
ID to the existing user-voice configuration save; it did not refresh the
provider cache, so this boundary does not add an unrelated refresh.

The TTS synthesis coordinator admits an idempotent interactive
`SpeechSynthesize` job and persists its provider, text, voice, prompt and output
policy before execution. It claims work through the generic job lifecycle,
loads the exact audio-provider secret when required, calls an injected runtime,
and admits returned bytes through the media boundary under a deterministic
asset identity. Preview output is temporary and retained chat audio is
persistent. Cancellation is checked before secret access, before dispatch and
after the provider response so late audio is not stored. Transient provider,
secret, media or repository failures requeue the same immutable request;
invalid inputs and audio fail terminally. Successful results replay after
reopen without another provider call. The application composition root exposes
one remote TTS runtime that routes the frozen provider kind through the five
completed HTTP adapters. Kokoro remains unavailable until its native runtime
is routed through the durable synthesis job.

The Kokoro asset inventory coordinator is constructed once from a host-resolved
managed root. Its operations accept only a variant and optional selected voice,
return model/config/tokenizer presence plus bounded content identities, and list
installed voices without exposing native paths. This replaces the legacy
caller-supplied asset-root commands. Missing assets remain a usable status;
unsupported variants, empty files, symlinks and inspection races fail. The
legacy automatic `system-kokoro` database row is not reproduced because local
providers already use the ordinary typed configuration boundary and no longer
persist an editable asset root.

Kokoro model bundles install through one durable interactive artifact job. The
application accepts a host-constructed confined install store, never an
operational asset path, and downloads the four pinned files through the central
ranged artifact client. Partial bytes and completed shared files survive retry;
success requires all four files to reverify, and completed jobs replay without
network access. Progress aggregates the bundle with bounded checkpoints.
Cancellation remains available until the first verified file commit, after
which the job finishes or retries the remaining bundle under its irreversible
stage.
The Kokoro voice catalog reads the same immutable repository revision through
the central JSON client, with the shared `model_revision_pin_request` and
`parse_pin_listing`. It accepts only bounded `voices/<safe-id>.bin` entries
with coherent LFS size and SHA-256 metadata, sorts and deduplicates IDs, and
merges exact installed flags from the managed inventory. Catalog reads do not
download, remove or expose native voice paths.
Selected Kokoro voices install as one stable-ID artifact job for either a
single voice or a batch such as the legacy starter pack. Selection is sorted
and deduplicated before admission. Each voice resumes from an identity-bound
confined partial, verifies pinned size and SHA-256 before atomic commit, and is
reverified on successful replay without network access. Progress covers the
whole batch and cancellation ends at the first verified voice commit.
Managed Kokoro removal accepts only the pinned model or voice identity, verifies
the current bytes and confined target before deletion, and treats missing bytes
as an idempotent replay. Removing a model variant preserves its shared JSON
assets. A later install receives a deterministic successor job instead of
replaying an earlier success whose artifact was explicitly removed.
Kokoro phonemization runs only after the complete pinned model bundle is
reverified through its confined install store. The application supplies the
purpose-specific eSpeak capability to the speech boundary and accepts no
operational model, tokenizer, config or executable path per request.
Kokoro voice blending resolves each normalized voice ID to one pinned remote
descriptor, revalidates and materializes its confined installed bytes, then
hands only bounded voice material to the speech boundary. Missing or ambiguous
descriptors and changed assets fail before style selection; native paths do not
cross the coordinator. Completed installs persist an immutable bounded
descriptor sidecar, so persisted voice selections can resolve and reverify
their blend after restart without fetching the remote catalog. Sidecar-free or
invalid retained binaries remain untouched and unavailable to this verified
offline path.
The Kokoro native synthesis coordinator revalidates the complete pinned model
bundle, selects its exact model artifact and runs cancellation-aware ONNX chunk
inference into a bounded WAV. It consumes already validated phonemization and
voice styles. The application TTS runtime now routes persisted Kokoro requests
to this native path while every other provider keeps the existing remote
runtime. It preserves the legacy model override, single or camel-case JSON voice
blend document and positive speed from the voice prompt, resolves only verified
offline voice descriptors, runs blocking native work outside the async worker
and returns the WAV through the existing durable media settlement. Missing
installable assets and runtime libraries retry; malformed requests and changed
artifacts fail before audio ingestion. The optional legacy `lexicon.json` is
read as a bounded pathless snapshot, parsed in its flat or global-plus-language
shape and merged for the primary normalized voice before eSpeak runs. Missing
files preserve the empty lexicon; malformed, oversized, unsafe or changing files
fail without rewriting or deleting user data. Android eSpeak hosting remains a
later platform slice.

The application sync hello coordinator initializes the database-owned device
identity before any change exists and builds a bounded current-protocol hello
with a caller-owned fresh session ID. Device identity survives reopen while
session identity rotates. Transport code will supply its authenticated peer
identity to the sync-domain negotiator; the application does not persist the
legacy one-time PIN or invent a trusted-peer registry.

The sync exchange coordinator runs the canonical change phase over a narrow
already-authenticated transport. It negotiates hello compatibility, exchanges
frontiers, sends and durably stages/applies bounded batches in both directions,
records monotonic peer acknowledgements and finishes only after both sides are
quiescent. An acknowledgement must name and causally cover the sent batch.
Cancellation is available to every transport await and checked before each
storage boundary. Unsupported input remains staged and returns a pending outcome
without acknowledgement, so an upgraded retry can resume without data loss.
Sockets, secure session pairing and status events remain later.

The persona-media sync coordinator exchanges only the bounded catalog of assets
currently referenced by personas and pulls missing content through a typed
authenticated transport. A caller first stages the canonical change batch; a
media dependency returns `Pending`. The media coordinator then resumes and
verifies only catalog entries that exactly match those staged canonical media
facts, commits their logical assets, and the same batch replay materializes the
persona without redispatch or data loss. Cancellation is
checked around every transport wait and mutation boundary.

The production loopback/LAN sync transport now binds a caller-selected socket,
creates or accepts a six-digit session PIN and returns the sync transport traits
only after both peers run SPAKE2 over the PIN (pairing protocol version 2,
backlog #22: version 1 derived the key from the PIN and a cleartext salt, so a
captured handshake allowed an offline PIN search and decryption of the whole
session; now an observer learns nothing to test guesses against and an active
attacker gets one guess per connection) and confirm the resulting key over fresh
challenges. A listener holds one freshly generated PIN and refuses every
connection after three wrong PIN proofs (sharing again binds a new listener with
a new PIN), and like legacy the sharing device's user approves each
authenticated peer once its hello names it (`SyncPeerApprover`); a declined
peer receives an explicit decline. A client that meets a 2.2.x host recognizes
its legacy handshake and reports the host's app version with the
same-version requirement instead of a protocol error; a 2.2.x client cannot
read the current handshake and shows its own error, so both devices must be
updated to sync. The confirmation binds both ephemeral connection roles and durable
device identities; the resulting
ChaCha20-Poly1305 session uses direction-separated monotonic nonces. Every typed
frame is length-prefixed, capped at the canonical payload limit plus 16 MiB
before allocation, decoded with the
same bound and reconstructed through current domain validators. Host/client
ordering prevents two large change frames from filling both socket buffers, and
a bounded early-frame queue lets one peer serve media chunks while entering the
next phase. Cancellation covers accept, connect and every read/write; disconnect
leaves the database batch and confined media partial available for a fresh
session. A pending persona-media batch therefore reconnects, transfers one
shared blob for two logical assets and then converges through the existing
coordinators. Listener discovery, durable peer trust and frontend status events
remain separate frontend work; coordinator completion and errors are the
backend status boundary.

The provider backup coordinator reads one consistent provider/model/prompt
snapshot, derives the exact referenced secret inventory, and loads only those
values from the injected native `SecretStore`. It checks each secret generation
before and after the read and rejects rotating or unavailable values. A value
the store reports missing (a wiped keyring, a key sync could not deliver, a
blank legacy key) no longer blocks the backup: the reference is kept, listed as
missing in the secret section, and the owning provider or audio account is
returned in `ProviderBackupExport::missing_secrets`, so the restored account
asks for its key again like the source device. The Hugging Face and CivitAI
tokens are carried when set; restore writes each one only when the device has
none, after the new database is active, so a token entered on this device is
never replaced; a token whose status cannot be read or whose write fails is
listed in `BackupRestoreReceipt::app_secret_failures`. Metadata and secrets are sealed as separate authenticated
sections. Secret plaintext is held in
zeroizing buffers and never appears in public backup metadata, errors or Debug
output.

That coordinator now includes global settings plus audio providers and user
voices in the same database snapshot. Its derived secret inventory covers both
provider credentials and every referenced `AudioApiKey`; every present value
must be stable before encryption begins.

The same export now carries the complete authored persona, lorebook, character
and group graph plus all logical media and blob metadata. A file-backed scenario
reopens that graph before export and verifies its default persona, roots and
ordered ownership documents. It also reads every unique ready blob through the
confined media store in bounded chunks and lets the transfer layer verify size
and BLAKE3 before encryption. Shared image assets produce one media section;
missing ready content fails the export. The same snapshot includes the complete
canonical ASR learning library across every language and scope, including voice
examples and their links to the already deduplicated audio sections. Restore
remains a separate application slice. The export also seals the complete visible
conversation history shared by direct and group chats, including archived
roots, policy and settings, branches, exact message order, all revisions and
candidates, initial origins and media references. A file-backed reopen scenario
preserves an empty starter message, ordinary starter content and a Director-mode
group that requires no selected speaker, and rejects a broken parent link. The
coordinator derives the complete unique protected snapshot and replay inventory
from that graph and streams each payload through
`ConversationArtifactTransferPort`. It enforces the envelope entry and aggregate
byte budgets before retaining the bytes, then requires the port descriptor,
size and BLAKE3 to match the graph reference. The file-backed scenario decrypts
and verifies the exact starter snapshot payload.

`BackupRestoreCoordinator` restores a version-2 backup as a replacement, never
a merge (user decision 2026-09-14). It decodes and stages the plan, builds the
restore admission request, installs every media object into the shared
content-addressed media root, moves the backup secrets to fresh references and
stores them, then opens a new database file named by the restore id under
`private-persistent-v2/databases`. The graph is written through the restore
writer, device-local state (sync journal, installed Whisper manifests,
discovered TTS voices) is carried from the previously active database file, and
the restored graph must read back equal to the decoded graph (with in-flight
generation settled as interrupted, exactly as the writer restores it) before the
admission is recorded. Only then does `AppDatabaseLocation` atomically point the
active database at the new file; the previous file is never deleted, and secrets
written by a failed attempt (including a failed pointer switch) are removed
again. Legacy import secret assignments and completions inside the backup follow
the same fresh references. Writes to the previous database after the device-local
copy are not carried, so the host stops background work before a restore. A host
opens `AppDatabaseLocation::active_path()` with `AppBackend::open` on the next
launch. Media installs use their own `restore/<hash>.partial` so a concurrent
sync download of the same blob is not truncated.

`LegacyMediaImportCoordinator::execute_from_source` imports legacy media from the
bytes a planned legacy source retained, with the same length and BLAKE3 checks as
the file-based path; candidates the source does not hold (voice audio a live
legacy database plans from its own files) are still read from the confined
storage root.

`LegacyRestoreCoordinator` makes loading a legacy source a replacement too (user
decision 2026-09-14). A version-1 backup is decoded, planned, staged and turned
into a restore admission request; a live legacy app data directory is planned in
place. Either way the whole legacy import chain runs into a new database file in
stage order: admission against the derived source counts, media from the planned
source's retained bytes, provider secrets from the planned configuration, the
authored graph, ASR learning, provider models and prompts, characters, groups,
audio, settings, direct and group conversations, usage records, creation
helper sessions, images (LoRA library and playground history) and local
generation metrics. Only a run that completes carries device-local state from the
previous database, records the backup admission and switches the active database
pointer; the previous file is never deleted. Each restore stages into its own
`<workspace>/<restore_id>` directory, so a later restore of a different backup
never conflicts with an earlier receipt (the version-2 coordinator does the
same). Provider secrets get fresh references at admission and legacy audio API
keys use references derived from the legacy source and the import run, so a
replacement never shares a secret record with the database it replaces and a
failed attempt deletes the secrets it wrote.

The same export includes the conversation-owned generation runtime: turns,
attempts, checkpoint timestamps, speaker and initial inference dispatches,
preparation attribution and tool executions. Its file-backed scenario advances a
turn through three durable stages, settles a tool result, fails the attempt, then
reopens and verifies the exact terminal state. Conversation UsageLedger and cost
bodies remain a later full-backup section while their IDs stay intact.

The export now also includes every generic scheduler job, its ordered lifecycle
events, and immutable job inference evidence with provider response identity,
reported counters and any existing cost basis. A file-backed scenario preserves
a claimed, progressed and retry-scheduled job plus its settled inference record
across reopen and rejects a corrupt event sequence.

The same file-backed export preserves conversation UsageLedger evidence for both
known provider counters with its captured pricing basis and unavailable counters
with their exact reason. Matching job inference is identified as overlapping
audit evidence rather than an additional charge, including after database reopen.

Conversation operation replay records and the complete immutable outbox journal
are exported from the same reopened snapshot. The scenario verifies contiguous
event order, terminal usage linkage, retained operations that emitted no event,
and rejection of a duplicated sequence.

The export also retains normalized companion relationship and session state,
ordered signals, continuity episodes and immutable replacement receipts. Its
file-backed scenario applies a real state transition, reopens the database,
checks exact owner, signal, episode and receipt identity, and rejects a corrupt
episode index. The companion calculation itself is unchanged.

Conversation generation now resolves chat parameters with the conversation's
own model settings as the session layer (`ModelSettingsLayer::chat_overrides`:
a set field overrides, an unset one inherits) and the global model settings as
the app layer, so a legacy session override or app default reaches the request
with legacy's order (conversation, then model, then app; fields the resolver
keeps model-only ignore the app layer). Feature operations (speaker selection,
memory, reply helper, lorebook generator) keep their own operation overrides.

`feature_parameter_input` is legacy `feature_model_overrides` +
`prepare_feature_request` for an app feature: the model's feature slot
(`ModelProfileConfig.feature_parameters`) overrides the model, unset
temperature/top_p (and the output cap when the feature has one) come from the
legacy `FeatureSamplingDefaults` constants, reasoning is off, a conversation's
own model settings do not apply and the app layer fills what the model leaves
unset. `FeatureRequestFields` keeps the fields legacy passed per feature: only
the direct help-me-reply sent top_k, the penalties and prompt caching (the
scene writer sent prompt caching only); every other feature sent temperature,
top_p, the output cap and context length, while Ollama and llama.cpp still got
top_k and the penalties through their request options. Help-me-reply
(settings output cap where the slot sets none), dynamic memory (plus the
llama.cpp sampler strip to top_k 40 and neutral penalties, skipped when the
slot sets its own llama sampler), group speaker selection and the staged
lorebook generator (generator output cap) use it. Companion Soul writer,
companion memory, lorebook entry and creation helper runs receive their profile
from the host, which builds it with the same function and the matching defaults
constant.

`ProviderBackupCoordinator::export_to` writes a v2 backup into any writer one
section at a time (data sections, then each media blob and conversation
artifact read, checked and appended before the next is loaded), so export
memory is bounded by the largest single object instead of the library;
`export` is the in-memory wrapper.

Desktop builds start the embedded llama.cpp worker on first use (legacy
lazy worker) and attach it to `provider_runtime`; `DatabaseLlamaHost` stores
runtime reports and metrics, and the frontend events go to the
`LlamaEventSink` the host attaches with `with_llama_event_sink` (logged at
debug level until then).
The legacy llama.cpp commands are `AppBackend` methods: `llama_backend_devices`
(integrated GPUs included, see lettuce-local-llm), `llama_context_info`,
`llama_embedded_chat_template` (still loads the model on the CPU: llama.cpp
names a template for some models that carry none) and `unload_local_llama`.
`StoredProviderMedia` wraps the local media store as the providers'
`ProviderMediaSource`; the host attaches it with
`ProviderRuntime::with_media_source` so llama.cpp vision can read
attachments.

Scene image protocol in the chat prompt (2026-09-23): the default prompt's
`entry_scene_image_protocol` / `_local` entries carry a `sceneImageProtocol`
condition. Direct chats set that fact only when scene generation is on and the
scene image model resolves (`image_feature_model`), picking the variant of that
model. This is legacy's `append_image_directive_instructions` ID filter as data.
Before this change the remote entry was active in every direct chat. The facts
`sceneGenerationEnabled`/`avatarGenerationEnabled` carry the settings in every
chat. `isSceneGenerationLocalImageModel` and the `{{lora_keywords[...]}}` values
follow the resolved scene model whether or not scenes are on, like legacy
(`scene_loras`). A subject names its LoRA's trigger keywords (blank for a LoRA
without keywords) or the catalog's `scene_lora_primary_subject` /
`scene_lora_secondary_subject` filler when it has none. A character or persona
that cannot be read counts as having no LoRA and does not fail the turn.
Open: an `artifact_id` image recommendation (made by the new app) has no
resolvable LoRA file yet, so it binds as having no LoRA.

Scene images (2026-09-23): `generate_scene_image` is legacy
`chat_generate_scene_image` for a direct chat message:
- The scene model comes from `image_feature_model(Scene)`, the character and
  persona are read live, and the persona is the conversation's effective one.
- A local diffusion model gets the trimmed scene prompt and the character and
  persona recommendation LoRAs. A LoRA is kept when it has no keywords or one
  of them appears in the prompt.
- A remote model gets these input images, in order: character design
  references (else the original avatar), one chat background (the
  conversation's own, none when hidden, else the selected or default scene's,
  else the character's), and persona design references (else the avatar). The
  prompt is the legacy sections rendered from the `scene_image_*` catalog
  entries.
- The size is the model's own when it sets one, else `scene_default_size`,
  else 1024x1024, and is always sent, since most remote adapters read only
  the request size.
- References whose image cannot be read are skipped, as legacy skipped files
  it could not load; a subject whose design references are all unreadable
  falls back to its avatar.
- The request runs as an `ImageGenerate` job with source `Scene` and up to
  three attempts. It retries only when no image came back or the error
  mentions "no image".
- The first image is added to the message as an attachment. The message is
  read again after the generation and its current rendered content revised
  with the image appended (keeping visibility, pin and scene flags), so an
  edit or candidate switch made meanwhile is kept, where legacy overwrote it.
  A message that is generating or changed concurrently yields
  `MessageUnavailable`, and the retained image then stays unattached.
- The caller passes the resources the jobs may use.

Deviations from legacy:
- Legacy's top-level message attachments survived switching variants. Here
  the image belongs to the revised content, so selecting another candidate
  hides it. Attaching to a generated candidate makes it an authored revision,
  which drops that reply's provider replay artifact.
- The image step uses the conversation's effective persona, including the
  inherited default persona. Legacy's image step read only an explicit session
  persona, while its prompt writer used the default. This is corrected to one
  persona.
- The image prompt is bounded at 64 KiB, so very long design notes fail the
  request. Legacy had no bound.
- More than 16 reference images drop persona references first; legacy had no
  limit.
- The scene prompt is not kept as the attachment's filename (legacy's seed for
  "regenerate"). The image job keeps the request prompt.
- Optimistic placeholders and askFirst approval belong to the next slices.

Scene prompts (2026-09-23): `ScenePromptWriter` is legacy
`chat_generate_scene_prompt`. A writer model (`scene_writer_model`, vision
required unless the scene image model is local) turns a direct chat message
into one image prompt:
- The context is the message and the two positions before it in legacy's
  loaded window (latest 120 messages plus older pinned ones, any visibility),
  user, assistant and scene lines only, `{{image:…}}` spans removed.
- The active `prompt_app_scene_prompt_writer` document renders with legacy's
  placeholder values and condition facts; writer-side text (scene request,
  reference notes, hints, background notes, role labels) is the
  `scene_writer_*` catalog runtime text. Reference hints count the stored
  design images; condition flags follow the images that can be read. A remote
  scene model sends those images as multimodal user messages; a local one
  sends the subjects' LoRA bindings and no images.
- Conditional and interval entries are placed by the number of relative
  messages, in-chat entries `depth` from the end with legacy's offsets, and a
  condensing template merges every non-image entry into one system message.
- It runs as a one-shot job (`one_shot_job`, shared with the reply helper):
  non-streaming, `SCENE_WRITER_DEFAULTS` over the model's scene writer slot,
  usage recorded before the answer is cleaned like legacy.
- Deviations: image tokens left in a system or assistant entry are removed
  (legacy's hints there were always empty, and assistant entries leaked the
  raw token); blank messages are not sent, though they still count for
  in-chat placement; `{{char}}`-style names inside descriptions are
  substituted by the shared renderer, where legacy sent them raw;
  `{{image[avatar]}}` / `{{image[references]}}` in a scene writer template
  render empty instead of leaking; usage is also recorded when the answer
  has no text at all.

Design reference notes (2026-09-23): `DesignReferenceWriter` is legacy
`chat_generate_design_reference_description`. The scene writer model (vision
required, scene generation need not be on) reads the subject's avatar and
reference images and drafts design notes from the active
`prompt_app_design_reference` document: relative entries, then every other
entry, each in template order (legacy placed none by depth or message
count); condensing merges non-image entries. The images are assets the
caller already stored (legacy took data URLs from the editor); at least one
is required. The subject name defaults to the catalog's
`design_reference_unnamed_subject`. It runs as a one-shot job with
`SCENE_DESIGN_REFERENCE_DEFAULTS` over the scene writer slot, streams to the
request id when asked and the model can, and cleans the answer like legacy
(quotes, code fences, blank lines). Deviations: usage is recorded (legacy
recorded none); image tokens left in a text entry are removed instead of
sent. `feature_prompt_entries` holds the rendering it shares with the scene
prompt writer.

Scene tags in replies (2026-09-23): a direct chat reply's `<img>…</img>` tag
(legacy `sceneImageProtocol.ts`, run by the old frontend) is handled when the
reply is finalized. The tag always leaves the stored text, which is trimmed
as legacy persisted it; it closes at `</img>`, `[continue]` or `[/continue]`
in any case, and an unclosed tag drops the rest of the reply. The first
non-blank prompt comes back as `ConversationGenerationRunResult::scene_image`
when scene generation is on, its model resolves and the mode is not manual
(`ask_first` for the askFirst mode); the caller then runs
`generate_scene_image` or asks the user. A replayed turn returns none.
Group chats never had the tag. Hiding the tag while a reply streams stays a
frontend job. The legacy `<<image:{json}>>` directive is dropped by user
decision; only the scene tag remains.

Soul growth edits (2026-09-23): `clear_companion_soul_growth` (count of
entries removed), `remove_companion_soul_growth` and
`set_companion_soul_growth_lock` (true when the entry exists) edit the
companion's character-wide Soul. Legacy addressed them through a session,
whose character owned the Soul. The old relationship page called all three
(remove by list position, now by id). A write that conflicts with a
concurrent growth or consolidation run is prepared again on the newer Soul, up
to three times. Deviation: legacy's clear emptied the shared Soul but other
sessions of the character re-persisted their own copies on their next save;
the rewrite's clear really clears the companion's Soul.

Startup recovery (2026-09-23): `AppBackend::recover_after_restart(now)` must
run before any worker starts. It releases every claim the previous process
held (`orphaned_claims`), cancels every waiting conversation generation job
(reason `Recovery`), then settles each live turn so its conversation accepts
new turns again: a turn that had not started or was being cancelled is
cancelled, a recovering turn fails with `RecoveryUnavailable`, any other is
interrupted (usage outcome `Interrupted`); running tools are interrupted and
requested or validated ones cancelled. Nothing is run again, as legacy lost
in-flight generation on a crash and a restore settles it the same way. A job
or turn that cannot be settled is logged, reported and skipped. Other job
kinds keep their recovery policy (`Restart`/`Resume` jobs are queued again)
and wait for their feature drivers.

Post-turn memory driver (2026-09-23): `PostTurnMemoryScheduler` coalesces
cycles per conversation like legacy's scheduler (`enqueue` returns true when
the caller must start `CompanionMemoryHostCoordinator::drive`; turns finishing
during a pass ask for exactly one more). Deviation: legacy slept 1200 ms
before every pass; a finished turn rarely follows another within that time and
the running pass already coalesces later turns, so the rewrite does not wait.
`drive` runs `after_turn` + `run_claimed` per pass, logs a failed pass and
continues, and stops when the conversation is gone; group conversations use
the same driver (legacy awaited group memory inside send/continue).
`resume_after_restart` runs, after `recover_after_restart`, each queued memory
job whose conversation still admits the same window (a processing run attempt
of the same job resumes) and cancels the rest, which would otherwise block the
conversation's next cycle; manually triggered cycles are not resumed, as
legacy lost them on a crash too. A queued job the app stopped during twice
(`LeaseExpired` events) is failed as `LeaseLost`/non-retryable instead of run,
so a job that brings the app down cannot crash every start; its window is then
held (no automatic re-admission) until the user runs memory by hand (a manual
run uses its own key chain); the held companion effects stay processing and
the frontend must say why memory paused. Known limits: a pruned terminal
memory job would restart its key chain onto a stale run id (no memory job is
pruned today); an input error that never clears (settings, missing model)
keeps its job retrying each turn.
Correction: a window whose job failed or was cancelled is admitted again under
a key chained to that job (`retry_idempotency_key`), as legacy retried a
failed cycle on the next turn; before, the terminal job kept the window's key
and the conversation's automatic memory stopped for good. Every input error of
`run_claimed` now settles the job (retry scheduled) instead of keeping the claim.

Every open clears the cached smart-offload layer count
(`actualGpuLayersUsed`) from each llama.cpp runtime report, keeping the rest of
the report, as legacy did on every start (free VRAM differs between sessions);
a failure is logged and the open continues.

Exit: `AppBackend::begin_shutdown` cancels every running inference
(`InferenceRuntime::cancel_all`, legacy `AbortRegistry::abort_all`) and stops
the local diffusion engine from taking or continuing work; `shutdown().await`
also stops its server process. The host calls them on exit request, as legacy
did on `ExitRequested`; the app-usage flush and analytics exit event follow
with their own slices.

App usage (2026-09-23, redesigned with the user): `AppActiveUsageTracker`
counts only the time the window is focused (legacy never paused on blur, so it
counted the time the app was open) and `flush` adds it atomically to the
device-local `app_usage_days` table (`lettuce_usage::AppUsageRepository`, one
row per local day; never synced; a v2 backup carries the days and a restore
writes them, then merges the previous file's days, keeping the larger active
time for a day both hold, like the legacy import). The
host flushes every 30 s and on exit like legacy. Legacy kept a total, a day map,
a started-at and a last-updated field inside the settings JSON with a
read-modify-write; the import writes `appActiveUsageByDayMs` into the table
(max per day, so replays and earlier usage are kept), records an unreadable
day or a total above the days' sum as lost, and drops the two timestamps,
which the table's days and `updated_at` replace.

Companion follow-ups (2026-09-23): after each post-turn memory cycle that
succeeds with fresh memories, `CompanionFollowUpHost` (called by `drive`)
admits and runs the growth cycle with the built-in growth prompt and, when
growth applied changes, the consolidation (legacy ran both inline after the
memory cycle; nothing admitted them in the rewrite before). The prompt is
resolved before admission, so a missing prompt creates no job. As in legacy a
failed follow-up is not retried: a retry-scheduled one is cancelled (its key is
per memory run or growth job, so nothing else is blocked). `resume_after_restart`
runs queued growth and consolidation jobs from their stored runs, fails one the
app stopped during twice and cancels one that can no longer be claimed. Known
gaps: a crash between growth success and consolidation admission loses the
consolidation (legacy lost it too); with a shared
memory pool, overlapping memory passes of two chats can count one chat's new
memories as fresh for the other's growth.

Requested work after a restart (2026-09-23): `recover_after_restart` also
settles work a user asked for instead of running it again: a queued staged
lorebook planner or writer without a saved attempt fails with `AppStopped`
through its own settlement (the draft fails, so a new batch can start; a failed
planner can be retried), one with a saved attempt stays queued so the user's
next action finishes it without new inference, a planner not yet planning is
left alone, refine runs without an attempt, coherence, lorebook entry/keyword
and one-shot helper jobs, speech jobs and image jobs are cancelled, and every
ended image generation whose record is still pending is settled (idempotent, so
a failed reconcile retries on the next start). If a claimed settlement fails,
the job is failed as `LeaseLost` so it never stays running.

Growth (and consolidation, which reuses the growth run's profile) resolves the
memory run's model with its CompanionMemory slot and the companion memory
defaults (temperature 0.3, top_p 1.0, reasoning off, no memory sampler), as
legacy's `feature_model_overrides(CompanionMemory, COMPANION_MEMORY_DEFAULTS)`
did; before, it reused the memory cycle's DynamicMemory sampling.

Lorebook files: `AppBackend::lorebook_files()` imports a World Info file as a
new `LegacyV1` lorebook (entries in file order, literal matching) and exports a
lorebook as World Info or a USC card, entries in their order. The old app's
frontend named a nameless file after its filename; the command layer does that.

Character files: `AppBackend::character_files(media_store)` reads any character
file into a package (`read` for text, `read_file(filename, bytes)` for a file
whose `.png` card is read from its `ccv3`/`chara` text chunk and whose picture
becomes the avatar, like legacy `character_import_preview_from_bytes`) and
imports it as a new character (`import`): avatar, background and scene
background data URLs are stored as media assets first (a failure only leaves
that image off, like legacy), then the bundled lorebooks, the card's embedded
`character_book` as a new lorebook bound last (legacy's character form rules:
keys plus secondary keys, `constant` always active, `insertion_order` as the
order), the character, its lorebook bindings and its scheduled notes are
written in one transaction. Card tags are split on commas, trimmed and blank
ones dropped, blank sources dropped and localized creator notes that are not
strings skipped, as the old character form saved them. A remote background is
skipped like legacy. Images are stored as given instead of re-encoded to WebP.

Deferred to phase (c): downloading a remote (`http`/`https`) avatar URL in a
character or persona file. Legacy downloaded character avatars when
`autoDownloadCharacterCardAvatars` was on and persona avatars always; today
the remote URL is dropped and the file imports without an avatar. The
download policy (the setting, the fetch through the shared network client,
then storing the bytes) belongs in the character and persona file use cases,
not the Tauri shell.
`export(character_id, format)` writes the character as UEC, Chara Card V3 or
V2 with its avatar, background and scene backgrounds inlined as data URLs of
their stored bytes and mime type; an image that cannot be read is left out,
like legacy.

Persona files: `AppBackend::persona_files(media_store)` reads a persona UEC or
the pre-UEC persona package (`read`), imports it as a new persona (`import`:
avatar data URL stored first and left off if it cannot be, the file's
lorebooks bound in order when they are active here and recorded otherwise,
made the default persona when the file says so) and exports a stored persona
as a v2 persona UEC with its avatar inlined and its enabled lorebooks in
binding order (`export`). The persona, its lorebook bindings and the default
change are written in one transaction like legacy, so a failure leaves nothing
and a retry writes one persona. A missing or empty description imports as an
empty description, like legacy. Differences from legacy: lorebook ids that do
not exist are not stored.

Chat files: `AppBackend::chat_files().import_direct(raw, file_stem,
character_id, now)` imports a SillyTavern JSONL transcript as a new chat with
the chosen character (`TARGET_CHARACTER_REQUIRED` without one), titled by the
header's character name, else the file stem, else `Imported Chat`: messages
without content are skipped, swipes become the message's variants with
`swipe_id` (else the first) selected, user and assistant lines keep their time
as the effective time, and the conversation is written in one transaction
through the same history writer the legacy import uses. A blank header name
falls back to the file stem (legacy stored a blank title, which a conversation
cannot have). A companion character gets the chat as its next continuity
episode, ending the open one; legacy saved it as a roleplay session, which the
rewrite cannot represent for a companion character (the legacy backup import
makes the same choice).
`export(conversation_id, now)` writes a chat's active branch as JSONL (file
name and text; the host saves it): the shown content of each message, an
assistant message's candidates as swipes when there are two or more,
speakers by their current character names, the user as the current persona
title or `User`, a `{group: true}` header and the conversation title for group
chats. Deleted (tombstoned) messages are left out; an edited assistant message
shows its edit, with the other candidates as swipes after it.
`import_group(raw, participants, now)` imports a transcript with several
speakers: every named speaker must map to an existing character
(`UNRESOLVED_PARTICIPANTS:<names>` otherwise, `GROUP_CHAT_IMPORT_REQUIRES_CHARACTER_MAPPING`
without any), a new group of those characters is created with legacy's
defaults (conversation mode, LLM speaker selection, manual memory) and named by
the header's character name or `Imported Group Chat`, and its chat is written
with each assistant line authored by its speaker. Every check runs before the
group is created; an archived character counts as unresolved, and a transcript
whose speakers all map to one character becomes a direct chat with it (legacy
made a one-member group, which a group cannot be).

Prompt files: `AppBackend::prompt_files()` imports a USC card or SillyTavern
preset as a new user prompt (the host passes its localized names for an
unnamed entry and preset) and exports a template as `system_prompts_<name>_<date>.usc`
or `.json`; built-in entries export under their catalog keys.

Chat template files: `AppBackend::chat_template_files()` adds a USC or JSON
chat template file as the character's last starter (a scene of another
character, a prompt that is missing or not the character's chat purpose, and
lorebooks that are missing or archived are left off; legacy copied the ids
as they were) and exports a starter as `chat_template_<name>_<date>.usc` or
`.json`.

Model files: `AppBackend::model_files()` imports a model file as a new chat
model on the provider account with the file's provider id and label, else the
provider's only account (`NoProviderAccount` otherwise; legacy stored the model
without credentials, which a model profile cannot be), returning the settings
that were out of range or unknown, and exports a model as
`model_<display name>_<date>.json` or `.usc`. Like legacy, every imported model
is a chat model.

Hugging Face browser: `HuggingFaceBrowser` searches models, lists an author's
models, overview and avatars (cached for the browser's lifetime, six lookups
at a time), a repository's files and its README, signed in with the token in
the secret store. `save_token` stores a token only after whoami accepts it,
`auth_status` reports a missing, invalid or valid token, and `clear_token`
removes it. A token the secret store cannot read is an error rather than an
unauthenticated request.

Runnability: `HuggingFaceBrowser::runnability` and `recommendation` read the
smallest file's GGUF header (512 KiB, then 10 MiB when the essentials were
missing) through `GgufHeaderSource` (`ArtifactDownloadClient` reads only the
requested prefix, signed in with the saved token), and
`local_file_runnability` scores a downloaded file with its projector and GPU
draft model (the DFlash drafter when DFlash is enabled, else the MTP one). Hardware comes from `local_runnability_hardware()` (nothing
on mobile) or a remote host; the llama defaults are passed in.

GGUF downloads: `HuggingFaceBrowser::gguf_install_plan` pins a model file
with its optional projector and MTP draft model to the repository's current
revision, sizes and SHA-256 digests as one `ArtifactInstall` plan below
`llm_models_root` (`DeviceSettings.llm_models_dir`, else
`<app folder>/models/gguf`), and `download_client` signs the downloads in
with the saved token. `register_downloaded_gguf` then adds the llama.cpp
model the old browser created (through the same advanced-settings mapping as
model files; the first model becomes the default, as legacy's
`addOrUpdateModel` did). `gguf_library` lists (flagging DFlash drafters by
their `dflash.block_size` key, desktop only), deletes and adopts model
files and moves the whole folder: files are copied, every stored model path
inside it (llama.cpp and stable-diffusion.cpp, one transaction) is rewired,
then the originals go. Corrected: legacy left stable-diffusion.cpp paths
pointing at the old folder.

Sprout: `sprout_runnability_hardware` asks an Ollama account's active Sprout
probe for the remote machine's memory (bearer key from the secret store), so
Hugging Face runnability and recommendations for that account are judged
against the remote hardware, as legacy did.

ONNX Runtime: `OnnxRuntimeInstaller::ensure` makes the runtime available
before embeddings, the companion emotion classifier or Kokoro load, as
legacy `ensure_ort_init` did: the `ORT_DYLIB_PATH` override, then a bundled
library in the host's resource folder, then an earlier download in
`<app folder>/onnxruntime` (legacy's folder, so its download is reused),
then a fresh download of the official ONNX Runtime 1.22.0 archive. The
archive is an `ArtifactInstall` job below `<app folder>/downloads/onnxruntime`
(byte progress on the job, reported through `OnnxRuntimeInstallEvent`;
cancelled through the caller's token). Unpacking is the job's `install`
stage (`ArtifactInstallCoordinator::run_then`), so the job succeeds only with
a usable runtime; the archive is deleted once unpacked and after any unpack
failure, and macOS then falls back from the architecture archive to
universal2. Unpacking and the disk lookup (with macOS `lipo`/`otool`/
`codesign`) run on blocking threads. Calls are serialized per process; once
the process committed its runtime, `ensure` and `installed` return it without
looking at the disk. `OnnxRuntimeReady` hands the embeddings/emotion link
out, and `initialize` commits the process's one ONNX Runtime environment
and returns the `OnnxRuntimeCommitted` evidence Kokoro sessions require.
`KokoroTtsRuntime` gets that evidence from a `KokoroOnnxRuntimeProvider`
before any Kokoro work, as legacy's TTS commands called `ensure_ort_init`
first on every platform: `ProcessOnnxRuntime` (wired by
`AppBackend::tts_runtime` with the host's `OnnxRuntimePaths`) reuses a
committed runtime, else runs `ensure` and `initialize` itself (the linked
runtime on Android and iOS), and reports the runtime unavailable only when
that fails. Only lettuce-app creates the evidence (`unsafe`
`after_process_commit`), in its init code. The install stage renews the
job lease while it runs. A runtime unpacked before the job record failed is
still returned, and the job is closed (success, else failure) on a best-effort
basis so it does not stay running. Android and iOS return the
linked runtime and never download.
Microsoft publishes no checksum for these archives (GitHub reports no asset
digest either), so the download is checked by the size the server reports,
as legacy checked nothing. For the Tauri host: the macOS bundle needs
legacy's `entitlements.plist` with
`com.apple.security.cs.disable-library-validation`, or the hardened app
refuses the ad-hoc re-signed download.

Image bundles: `HuggingFaceBrowser::bundle_role_search`, `bundle_files`
(files pinned to the repository's revision; GGUF text encoders checked by
their header), `bundle_install` (engine estimate gate, every asset re-checked
against Hugging Face, manifest written, one pinned install below
`<image root>/huggingface/<author>/<repo>/<revision>`) and
`bundle_retry_downloads`; `finish_hf_bundle` marks files in and registers the
model through `register_hf_bundle_model` (existing models keep their name,
settings and binding values), `retry_hf_bundle_registration`,
`downloaded_image_models` and `component_library` follow legacy. The old
in-memory selection map is gone: callers pass the checked assets back and they
are re-verified.

CivitAI: `CivitaiBrowser` searches (up to five fetches per page) and opens
models with the saved token, validates, saves and clears the token like
legacy; `civitai_lora_install_plan` sizes the file with a one-byte ranged
request (the install checks exact size and the listed SHA-256) and
`record_civitai_lora` stores its trained words and base model afterwards.
Pure mode is passed in until the content filter is ported.

Pure mode: `ProviderRuntime` reads the saved level on every request and runs
every provider answer through the Pure mode guard, as legacy's API layer did.
Chat and group prompts get the level's `{{content_rules}}` text and
`default_character_rules` gives a new character the old base and level rules;
both texts live in the chat runtime catalog. The Low level is kept (it was
imported as Standard before).

App version: `app_version(package_version)` appends `-cuda` for the CUDA build
and nothing otherwise. The old app also marked `-rocm` and `-vulkan` builds;
the CPU and Vulkan builds are now one normal build and there is no ROCm build.
The update check itself stays in the frontend.

Avatar gradient: `AvatarGradients::gradient(media, asset, force)` is the old
`generate_avatar_gradient` extraction (about 100 sampled pixels, median-cut to
8 colors, dark muted base, muted companion, dark vibrant accent, CSS and text
colors), proven against the old functions compiled verbatim on 400 sample sets
(`tests/fixtures/legacy_avatar_gradients.tsv`). The caller picks the image: the
round avatar was a separate picker output (a zoomed, circle-clipped crop the
stored crop numbers cannot rebuild), so the round image falls back to the
square avatar until a round asset exists, as the old command fell back when
`avatar_round.webp` was missing; legacy round files are not imported yet.
Results are cached per process by content hash instead of `gradient-*.json`
files. Corrected: a single-color image produced `#hex NaN%` (invalid CSS) and
now spans 0% to 100%; GIF avatars, which failed to decode as `.webp`, now
decode.

## Companion emotion model (Lettuce Thymos)

`companion_emotion_install` replaces legacy's `start_companion_download("emotion")`,
`delete_companion_model("emotion")` and the `companionEmotionInstalled` flag of
`get_embedding_model_info`. `HuggingFaceBrowser::companion_emotion_model`
issues the shared `model_pin_request` for `Zeolit/lettuce-thymos-26m-v1` and
reads it with the shared `pinned_files`, so the install is pinned to the commit
Hugging Face reports. `admit_companion_emotion_install` admits one
`ArtifactInstall` job below `<app folder>/models/thymos` through
`ArtifactInstallCoordinator::admit_labeled`, whose subject carries the
`Lettuce Thymos` display label; download progress, resume and cancellation are
the shared job's. The job store decides whether a Thymos install is active: any
queued or running job with that label, whatever revision it pins.
`active-install.json` is only a hint carrying the admitted job id and pinned
remote, so another request joins that job with its own revision and plan; when
the hint cannot supply them, the request gets
`CompanionEmotionDownloadError::InstallInProgress` with the job to wait on. With
no active Thymos job a corrupt or stale hint is discarded. Two revisions
therefore never download or complete at once.
`finish_companion_emotion_install` (refused as busy while a Thymos job is
active) records the install only after the files, label metadata and optional
model digest verify, replacing the record atomically, then sweeps other
revisions and partial downloads (leftovers are logged and swept again by the
next install or removal). `companion_emotion_status` and admission first
reconcile: a hint whose job succeeded but was never finished is finished the
same way (a failed verification is logged and leaves it not installed), and a
hint whose job ended otherwise is dropped. A succeeded job without a readable
hint cannot be finished this way, since the job store does not record the
revision it pinned; the next admission installs again. `remove_companion_emotion`
is refused as busy while a Thymos job is active and otherwise sweeps the record,
every revision and every partial download. `load_companion_emotion` returns no
classifier, with a warning, when Thymos is not installed, is damaged, fails
verification or ONNX Runtime is unavailable, so companion turns keep legacy's
neutral update; `try_load_companion_emotion` gives the reason for status
reporting.

Host wiring requirement for the frontend/command phase: a Thymos job admitted
but never claimed stays `Queued` (a claimed job that was running at a crash
becomes `Interrupted` through lease recovery), and a queued Thymos job keeps
every later admission, completion and removal busy. At startup the host must
claim and run, or recover, queued `ArtifactInstall` jobs.

Approved removals: the legacy SamLowe `roberta-base-go_emotions-onnx`
classifier is not ported, and its files under
`<legacy app folder>/models/embedding/companion-emotion` are neither imported
nor touched (legacy kept no other companion-model state, so nothing else is
imported); Thymos must be downloaded. The legacy companion NER and router (NLI)
models are not ported either.

## Embedding models (Lettuce Eidos)

Embedding models (`embedding_models`): `EmbeddingModelCatalog::pin` sends the
shared `model_pin_request` to `HUGGING_FACE_ENDPOINT` and reads the answer with
the shared `pinned_files`, pinning the commit it reports; `embedding_install_plan` downloads the family's files below
`<app folder>/models/embedding/<family>/<revision>/` as one `ArtifactInstall`
job (progress, cancellation and resume come from that job). LFS files are
checked against their SHA-256 while downloading; plain git files (Eidos
`calibration.json`, v4 `tokenizer.json`) by size, then by their git blob id
(shared `verify_git_blob`) in `finish_embedding_install`, which also parses the calibration and removes a
file that fails either check before recording the manifest.
`EmbeddingModelCoordinator` loads the chosen family when installed (unset means
Eidos), else any installed family, Eidos first; a completed install becomes the
choice; removing a family keeps the choice so loading falls back.
`adopt_legacy_install` records the legacy `v4-model.int8.onnx` and
`v4-tokenizer.json` in place and, on a device that never chose a model, stores
v4 as the choice so existing v4 users keep v4 until they download Eidos.
Nothing downloads automatically.

Re-embedding is lazy, as legacy's `migrate_session_memory_embeddings_if_needed`:
when retrieval needs a memory space, `embed_missing_memories` embeds its active
memories that have no vector for the active vector-space label and dimension,
which after a model or dimension switch is every memory of that space. Nothing
re-embeds the whole library in the background. Imported legacy `v4` vectors
are kept as they are and stay in use while v4 is the active model. Old vectors
stay stored and become current again when the user switches back. A vector is
stored only while its memory still has the embedded text
(`ProjectionWrite::Superseded` otherwise, embedded by the next retrieval), and
a re-embedded memory also gets its token count recounted with the embedding
tokenizer (no special tokens, no cut, as legacy counted). The recount does not
bump the memory space revision; instead every memory write (change sets and
sync) keeps the stored token count of an item whose text it leaves unchanged,
so an older snapshot cannot restore the old count.

Host wiring still required (phase (c)): nothing calls
`EmbeddingModelCoordinator::adopt_legacy_install`, `load_active` or
`complete_install`, `EmbeddingModelCatalog::pin` or the embedding
`ArtifactInstall` job yet. The host must adopt the legacy v4 files at startup,
load the active service with the device's dimension and token budget, and run
install jobs from the model hub UI, finishing them with `complete_install`.

Local stable-diffusion.cpp: placement and auto-fit see discrete GPUs and
accelerators, as legacy's device list did, plus (user decision) the AMD
integrated GPU of a unified-memory machine, one whose only GPUs are
integrated (an AMD APU such as a Ryzen AI handheld; the test llama.cpp's
`is_unified_memory` makes). That iGPU is budgeted from the memory it reports,
which follows the user's carve-out, as llama.cpp budgets a selected iGPU; the
frozen fit formulas are unchanged. Intel and other integrated GPUs, and an AMD
iGPU next to a discrete GPU, stay excluded as in legacy. Reading the installed list removes the old
`sdcpp:<profile>:<variant>` models whose variant is not installed (legacy
purged every such row on that read); an installed variant keeps its row so
registration adopts its settings. Host wiring still required (phase (c)):
nothing calls `finish_runtime_install` when a catalog or engine download
completes, so the host must run it (extract the engine, then
`register_catalog_model`) from the install job's completion, as legacy's
`sdcpp.rs` did after each download.

Speech and model downloads: a cancelled download, or one that failed for good,
deletes its partial file (legacy deleted its `.tmp` on cancel, stall and
failure); a retried one resumes. Partial names start with a hash of the file
they become, so preparing a download also deletes that file's partials from
older revisions, and nothing outside the managed `downloads`/`.downloads`
folders is touched. A partial that already holds every byte is verified
without another request, and one that fails verification is deleted. Whisper
and Kokoro check at use time only what legacy checked, that the files are
present (here: at their installed size); contents are hashed once, at install.
A Whisper model installed from an older upstream revision (the whisper.cpp
repository head moves) is used as installed instead of conflicting; one whose
file is gone or resized is dropped and downloaded again. A Whisper model
imported from a retained legacy folder can be removed: its file is deleted
when it is inside the managed folder, and otherwise only the app's record goes.

Hard delete. `delete_conversation` and `delete_character` purge the rows
(see lettuce-database) and then collect media. Every media call takes a
`MediaGarbageScope`: the media store, the app's `AppDatabaseLocation` and the
database file this process uses. Each run lists the other database files in
the directory at that moment (a database a restore kept, or one a restore is
writing), and an object any of them catalogs, in any state, is never deleted.
When one of them cannot be read, the run deletes nothing and records a
`media_collection_skipped` notice naming the file. `collect_media_garbage`
runs sync-received deletes still queued, lets the catalog release what
nothing references and, holding the media store's ingestion lock, deletes
those object files only after that commit. Objects a backup export (from
before it reads the catalog) or a restore (for the objects it installs)
holds pinned are left for a later sweep, and restore installs objects under
the same lock. A file that cannot be deleted, or one left by a crash between
commit and deletion, is removed by `sweep_orphan_media_files`, which only
looks at content-addressed files under the media root's `objects` tree and
deletes those neither this catalog (a blob in any state, so a `missing` blob
can become ready again) nor another database file names and no backup has
pinned. The sweep is safe at startup because of these rules. Media
collection failing after a purge leaves the candidates queued. Host wiring
still required (phase (c)): the Tauri commands, the notice list
(`Database::purge_notices`), and running `collect_media_garbage` after each
sync session and `sweep_orphan_media_files` at startup.
