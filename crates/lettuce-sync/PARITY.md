# lettuce-sync: legacy parity notes

Facts about how sync relates to the legacy app (2.2.x), and how the design got to its current form. The crate README describes the current design; this file keeps the comparison and the history.

## Legacy parity

- Memory item payload version 4 preserves manually chosen dates with `user` precision and optional source attribution, matching `old-code/src-tauri/src/storage_manager/sessions.rs:4561-4568`. Its version changes the handshake schema fingerprint.

- Pairing is session-scoped, matching the legacy UI. No persistent peer-trust model was invented.
- The state scan stamps deletes with the time this device deleted the entity, like legacy's per-write capture.
- Deletes follow legacy: a delete beats a concurrent update on both sides.
- Two concurrent first messages of an empty conversation are handled like legacy's root siblings: the lower first message keeps the conversation and the other chain moves to a conversation of its own. A separate conversation is the only way to keep both first messages, which is what legacy's branch sessions did.
- Retrieval embeds every memory without a current vector first, like legacy, skipping superseded ones and continuing past failures.
- Pricing caches stay local, as in legacy.
- The change journal is not compacted, as in legacy.

## Deliberate differences from legacy

- Canonical domain changes replace legacy raw SQLite changesets and schema-position coupling. Session hello uses a fingerprint of the supported canonical payload schemas instead of legacy's positional SQLite schema equality.
- Peers on 2.2.5 or older cannot sync with this build: their pairing handshake differs, and a current client reports the legacy host's version (see the `lettuce-app` transport).
- State scanning replaces legacy's per-write capture, so edits between sessions collapse into one change.
- Image generation, speech synthesis and transcription records stay device-local. Legacy had no such tables: its image history was the playground history (now synced) and its voice cache only listed provider voices. Generated images reach other devices through the playground history and messages.
- No legacy database, source or user asset is read, rewritten or deleted by this crate.

## User decisions

- Local model files (sync S13, 2026-09-21): for llama.cpp and stable-diffusion.cpp profiles, the files the runtime loads and the installed runtime build are device-local; everything else in the profile syncs.

## Fixed bugs

- Backlog #19: outbound reads used to cover only this device's own changes. They now cover every origin in the local frontier, so a peer relays changes it received from a third device.
- Backlog #20: two cases used to stay pending forever and now commit. An update for a persona this device never received (created before journaling existed on its origin) adopts the complete snapshot, and a default change whose persona is archived or missing here is journaled with the local default kept and an unresolved conflict recorded.
- Backlog #21: local persona operation identities were derived from the revision; a remote winner could move the revision backwards and turn the next local edit into a permanent stale-revision replay. Identities are now scoped to the latest remote change that won locally.
- The referenced-media scan used to miss message media, so messages with images never became ready to journal. Media used by messages and voice examples is now journaled like other referenced media.

## History

The sync design was built in stages. The stage names are still used in commit messages and handoff notes.

- Canonical change boundary, local journal, persona codec with explicit journaling, persona default singleton, archive/restore, outbound batches and acknowledgements, incoming batches, persona conflicts, session hello, frame values, persona-referenced media. At that point every other aggregate still needed explicit journal wiring; the state scan replaced that plan.
- S2: state-scanned aggregates (provider accounts, model profiles).
- S3a (protocol version 2): media without a catalog. The media phase used to exchange a catalog of every referenced asset, capped at 256, which cannot hold a character library. Each side now fetches by content hash only the blobs its deferred media changes wait for.
- S3b: characters.
- S4: lorebooks and bindings; personas and the persona default joined the scan.
- S5: prompts, with deterministic built-in document and entry ids.
- S6: groups.
- S7: application settings.
- S8a: conversations part one (snapshot artifacts, roots, messages, forked branches).
- S8b: concurrent replies and forks.
- S9a: memory. S9b: companions.
- S10: plain rows through the generic row codec.
- S11: usage.
- S12 (protocol version 3): secrets.
- S13: local model files.
- Protocol version 4: hard deletes. Payloads are unchanged (the schema fingerprint stays); the protocol version marks peers that accept these deletes.
- Protocol version 5: large entities (256 MiB single-entity payload limit, oversized changes in batches of their own, `not_synced` notices).
- Playground history and Creation Helper sessions joined sync after that.

## Corrections to the previous README

- It listed the scan order as accounts, models, personas, persona default, characters, lorebooks, character bindings, persona bindings. The code (`SCANNED_CODECS`) scans prompts and app settings before characters and groups before lorebooks; the new README gives the full order.
- It said the media phase fetches only what the latest pending batch from the connected peer waits for. `pending_media` now reads deferred `media.asset` changes (up to 256 per phase), whatever batch they came from.

## Not wired yet

- Frontend status flow for sync sessions was listed as later work.
- Other aggregate blob families and legacy sync-state migration were listed as later slices.
- Applying a remote model delete clears app defaults pointing at it locally; the previous README said this side effect must be reconciled "when settings sync lands". Settings sync (S7) exists now and clears missing defaults itself; check whether anything is left to reconcile.
- Persona is the only kind with a conflict resolution surface; other kinds keep their evidence without one.
- Follow-up: launch snapshot artifacts travel inside their snapshot entity as base64. Moving them to the blob phase (chunked, content-addressed like media) would remove the largest payloads and the memory cost of the 256 MiB limit.

Character snapshot schema version 2 includes provider-account voice preferences. The session fingerprint changes with that version, so a peer with the old character payload cannot silently pair under the same schema contract.

Own memory entities identify both conversation and branch; received user forks start with an empty own space and receive their seeded items and summaries through normal changes. Legacy represented a fork as another session with copied memory (`old-code/src/core/storage/repo.ts:547-586`). Shared pools retain character ownership and exchange a separate cursor for each conversation and branch.

Branch labels use the same last-writer-wins entity policy as titles while parent, fork message and creation time remain immutable. Legacy branches were sessions with their own title (`old-code/src/core/storage/repo.ts:1683`; rename caller `old-code/src/ui/pages/chats/ChatTreePage.tsx:213-220`). The separate label edit timestamp prevents local head changes from promoting an older label.

Conversation lineage travels with its root as creation metadata and stays valid after source deletion. Legacy direct-to-character branches recorded parent-session and branched-message fields (`old-code/src/core/storage/repo.ts:1744-1748`); sync no longer needs a surviving source to retain that provenance.

Branch deletion exchanges a retained topology payload with a monotonic tombstoned lifecycle, including when deletion happens before the first sync. An active payload cannot resurrect it, and a concurrent label edit cannot suppress deletion. Parent and fork identities remain immutable; labels retain their independent last-writer-wins behavior.

Conversation copies carry immutable protected source-scene provenance separately from launch origins, preserving copied scene messages under different target defaults (`old-code/src/core/storage/repo.ts:1898-1978`). Backup and sync retain the reference after source purge; launch-origin validation remains unchanged.

Companion session payload version 2 carries the private relationship rows of a duplicated companion conversation, so a peer reproduces independent relationship state instead of resolving to the shared character relationship. A payload whose private ownership differs from the receiving device is a conflict, and ownership never reverts. Legacy had no companion duplicate.

Memory summary and inherited-summary payloads preserve user/model/import provenance, including a user summary with no source coverage; category and observed-time metadata are retained in item payloads.

Item and own-summary payload version 4 and inherited-summary version 3 preserve nullable token counts. Their schema versions change the handshake fingerprint; derived counts remain conditional on the current text.

Manual edit history is exchanged as canonical entities, retaining the legacy edit anchor semantics (`old-code/src-tauri/src/storage_manager/sessions.rs:3936-3993`). Local sequence, space and short ids are rebound rather than copying legacy SQLite row positions between devices.

Memory cycle reverts and dismissed memory failures are not synced. They qualify the device-local dynamic-memory runs they refer to (runs, attempts and their outcomes never leave the device); the item and summary changes a revert makes sync like any other memory change.
