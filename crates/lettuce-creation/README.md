# lettuce-creation

Resumable AI-assisted authoring plus remote catalog discovery, inspection, and
import preparation.

## Boundary

Creation and discovery remain separate internal modules and commit only through
normal transfer, media, and authored-domain use cases.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Status

The verified creation-helper progressive-edit and finalization behavior is
pinned in
`fixtures/legacy-import/creation-helper-tool-scenarios-v1.json`. The first
implementation slice is active: versioned new/edit targets, durable user turns,
immutable draft proposal revisions, bounded pure target-specific text, scene,
and lorebook-entry operations, ordered typed outcomes, and explicit
awaiting-review/awaiting-confirmation stages are owned here. Failed operations
do not roll back later valid operations in the same provider order. The
repository port persists input before inference and advances proposal lineage
with workflow revision CAS; exact retries return the immutable stored records.

The SQLite implementation lives in `lettuce-database`. No operation in this
slice writes character, persona, or lorebook aggregates. Final authored-domain
apply, provider continuation, image/media leases, administrative tools, and
frontend events remain later slices.

The native proposal-tool contract is also active. Drafting exposes only the
versioned mutators valid for the current target plus `show_preview`; review
exposes only `request_confirmation`, and confirmation exposes no tools. Exact
declaration/version checks reject undeclared or malformed calls before
reduction. Valid calls reduce in provider order into one proposal and one typed
result per call, including operation errors without stopping later calls.
Application-generated scene and lorebook-entry IDs are deterministic across an
exact proposal retry, and the repository CAS commits the proposal before any
later provider-continuation work.

Creation turns also own durable inference attempts before provider dispatch.
Each attempt pins its immutable base and planned proposal identities, retry
parent, target/stage-specific tool request, job identity, exact resolved-profile
fingerprint, ordinal, lifecycle, and failure state. Retry children must keep the
profile binding and use a distinct job. Native calls are admitted atomically in
provider order before reduction, including their exact definition version,
provider identity, arguments, raw arguments, and protected replay reference.
Exact retries return the stored evidence; stale bases, changed retries,
cross-turn owners, undeclared tools, version drift, reused jobs, profile drift,
and duplicate identities fail closed.

Each attempt additionally checkpoints up to eight immutable provider-response
rounds. Round evidence preserves mixed visible text/reasoning, candidate replay,
and the exact contiguous call range, including text-only terminal responses.
This makes batch ownership and continuation recovery explicit instead of
guessing round boundaries from timestamps or a flat call list.

Remote provider dispatch and recursive continuation are active through the
`lettuce-app` coordinator. User-turn/first-attempt admission is one atomic port
operation bound to the exact workflow revision; it never exposes a durable turn
without its dispatch attempt. The explicit interrupted-parent recovery contract
preserves parent evidence and admits an empty immediate child atomically; retry
children never inherit a partial provider sequence. Successful terminal
settlement likewise advances the proposal workflow and attempt together through
one repository operation.

Confirmed persona finalization is now an explicit application-coordinated
authored-domain apply. New-persona workflows map complete confirmed drafts
through the normal create invariants. Existing-persona workflows additionally
pin the authored revision and revise only name/description while preserving all
other authored fields, media, lifecycle, identity, timestamps, and default
selection. Both mutations atomically persist an immutable workflow receipt, so
crash retries return the original result after the persona revision advances
while stale or changed commands conflict.

Confirmed new-character finalization is also active. A complete name and
definition plus the ordered draft scenes map into the normal character-plan
contract, retaining proposal scene IDs, text, direction, and order. The
resulting graph deliberately starts with default character policies and empty
provenance, presentation customizations, media, variants, and starters. The
complete graph and its character-specific immutable receipt commit atomically.

Confirmed existing-character finalization is revision-pinned and applies the
reviewed name, definition, and authoritative ordered scene list in one root
CAS. All character fields absent from the helper draft remain unchanged.
Retained scene IDs preserve lifecycle, variants, selected variant, assets, and
creation metadata while reviewed text/direction/order changes bump only affected
scene revisions; new IDs start as minimal active text scenes and omitted IDs
are removed. Removal is rejected when it would invalidate an unrepresented
default-scene or starter reference instead of silently rewriting that setting.

Confirmed new-lorebook finalization is active through the normal lorebook
aggregate path. It preserves the confirmed root name and exact entry IDs,
titles, content, and order. Because neither the legacy nor current authored
lorebook aggregate has a description field, the helper description remains
proposal context and is not fabricated into durable content. Creation-helper
entries have no keyword-policy fields, so finalization makes them explicitly
enabled and always-active with empty keywords, literal matching, case-insensitive
matching, and priority zero; this avoids persisting inert entries while keeping
all hidden policy defaults deterministic.

Confirmed existing-lorebook finalization is also active and revision-pinned.
It preserves root lifecycle, detection, icon, behavior, identity, and creation
time while applying the reviewed name and complete entry graph in one root CAS.
Retained proposal entry IDs keep their activation, keyword, matching, priority,
and creation metadata; reviewed title/content/order changes bump only affected
entry revisions. New IDs use the explicit new-lorebook defaults, and omitted
IDs are removed because the confirmed proposal is authoritative. This
deliberately corrects the legacy helper, which preloaded only the root name and
silently discarded every reviewed lorebook entry at final apply.

Single-entry, keyword, and staged lorebook-generation behavior is pinned in
`fixtures/legacy-import/lorebook-generation-tool-scenarios-v1.json`. Generation
owns durable reviewed proposals; `lettuce-context` remains the only lorebook
domain owner and final apply must use its expected-revision use cases atomically.
Process-local job state, permissive free-text tool inference, positional draft
identity, partial commit, and cancel-by-deletion are not preserved.

The single-entry writer's pure provider contract is now copied from legacy:
ordinary mode requires `write_lorebook_entry` or `no_entry`, force mode exposes
only `write_lorebook_entry`, a later valid entry wins over an earlier no-entry
call, and title/content/always-active plus case-insensitively deduplicated
keywords use the same normalization and 24-keyword legacy cap. Its exact JSON
and XML fallback prompts and parsers are also present, with force-mode
`no_entry` rejected as undeclared. The six source/force final instructions and
two-attempt native/structured-fallback checkpoint decisions are typed here as
well; none of these contracts mutate a lorebook.

The matching immutable run now freezes the direct conversation, lorebook,
character/persona, selected message and memory identities, source mode,
optional-summary choice, force flag, exact resolved inference profile, prompt
revision, rendered input values, fallback format, and generic job identity.
Messages/memory/mixed source gates are validated before admission. The SQLite
adapter rejects changed request replay and the application admission boundary
reuses the generic restart/cooperative-cancellation `CreationRun` job.
Application execution persists the native decision and, only when needed, one
same-profile structured fallback decision with normalized result, usage, and
provider diagnostics. Durable entry-or-none results replay without another
provider request; invalid declared results remain terminal instead of silently
falling through.
