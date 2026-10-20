# lettuce-platform: status notes

Facts about `lettuce-platform` that are not architecture: what the previous README listed as deferred, and where it was out of date. The crate README describes the current design.

## Not wired yet

- The crate description names application lifecycle, updater integration and user-intent ports. None of that exists yet; the filesystem kernel was delivered first and deliberately left out Tauri, lifecycle, archives, grants, the updater, domain formats, cryptography and secret-envelope policy.
- Trash has no durable journal, no retention purge, no quarantine workflow and no reconciliation of an ambiguous commit. Generic trash is therefore not crash-restorable or retention-managed.
- The previous README said architecture checks would restrict `FilesystemAuthority::new` to adapter code. `scripts/check-architecture.sh` only restricts the `cap-std` and `cap-primitives` dependencies to this crate; nothing checks where the authority is constructed.

## History

- Android eSpeak NG is loaded at run time with `libloading` instead of linked at build time like legacy (`old-code/src-tauri/build.rs:200-300`), so building and checking the workspace never needs the bundle. Legacy joined the clause chunks of `espeak_TextToPhonemes` with no separator (`old-code/src-tauri/src/tts_manager/kokoro/phonemizer.rs:733-745`), so a batched call never had one line per segment and every segment fell back to its own call, with multi-clause phonemes glued together. That was a bug: the library output now has one line per clause, like the command line.
- Legacy initialized the Android library with `espeakINITIALIZE_PHONEME_IPA` only (`old-code/src-tauri/src/tts_manager/kokoro/phonemizer.rs:696-712`), so a missing or corrupt `phontab` or `phondata` made eSpeak call `exit(1)` and end the app. Fixed: initialization also passes `espeakINITIALIZE_DONT_EXIT`, so a broken data folder fails the call as `Unavailable`.
- The previous README said create-new always publishes through an atomic hard link. The code now uses `renameat2(RENAME_NOREPLACE)` on Linux and a per-directory lock with a plain rename on Android (to avoid the `SIGSYS` older Android seccomp policies raise for `renameat2`); only other targets use a hard link.
- The previous README described the private persistent root as serving a settings-owned secret adapter. Its only caller today is `AppDatabaseLocation` in `lettuce-app`, which keeps the active database pointer there; `lettuce-settings` does not use this crate.

Database-file lifecycle coordination now has a confined cross-process file lock. The existing per-authority mutation lock remains separate; no private-persistent removal permission or filesystem constraint was relaxed.

Slice 12 adds a separate database-directory capability for explicit kept-file deletion. It refuses traversal, unexpected file identity and symlink sidecars without relaxing the generic private-persistent deletion restriction. The app owns active-file checks, live-handle reservations and durable operation receipts.

Slice 12 media protection adds confined identity checks and uncapped internal inventories. The existing bounded listing API and private-persistent listing refusal remain intact; symlinks do not grant access to outside files.

Slice 12 write-fence sidecars are removed with an explicitly deleted database. They retain the existing all-sidecars-first symlink preflight and confined deletion permission.

Reset preserves and renames the source database instead of clearing its tables (`old-code/src-tauri/src/storage_manager/usage.rs:70-102`). Confined, identity-checked publication never overwrites another database; journal recovery reverses an interrupted move before pointer cutover and finishes it after cutover.
