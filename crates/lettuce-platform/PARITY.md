# lettuce-platform: status notes

Facts about `lettuce-platform` that are not architecture: what the previous README listed as deferred, and where it was out of date. The crate README describes the current design.

## Not wired yet

- The crate description names application lifecycle, updater integration and user-intent ports. None of that exists yet; the filesystem kernel was delivered first and deliberately left out Tauri, lifecycle, archives, grants, the updater, domain formats, cryptography and secret-envelope policy.
- Trash has no durable journal, no retention purge, no quarantine workflow and no reconciliation of an ambiguous commit. Generic trash is therefore not crash-restorable or retention-managed.
- The previous README said architecture checks would restrict `FilesystemAuthority::new` to adapter code. `scripts/check-architecture.sh` only restricts the `cap-std` and `cap-primitives` dependencies to this crate; nothing checks where the authority is constructed.

## History

- The previous README said create-new always publishes through an atomic hard link. The code now uses `renameat2(RENAME_NOREPLACE)` on Linux and a per-directory lock with a plain rename on Android (to avoid the `SIGSYS` older Android seccomp policies raise for `renameat2`); only other targets use a hard link.
- The previous README described the private persistent root as serving a settings-owned secret adapter. Its only caller today is `AppDatabaseLocation` in `lettuce-app`, which keeps the active database pointer there; `lettuce-settings` does not use this crate.

Database-file lifecycle coordination now has a confined cross-process file lock. The existing per-authority mutation lock remains separate; no private-persistent removal permission or filesystem constraint was relaxed.

Slice 12 adds a separate database-directory capability for explicit kept-file deletion. It refuses traversal, unexpected file identity and symlink sidecars without relaxing the generic private-persistent deletion restriction. The app owns active-file checks, live-handle reservations and durable operation receipts.

Slice 12 media protection adds confined identity checks and uncapped internal inventories. The existing bounded listing API and private-persistent listing refusal remain intact; symlinks do not grant access to outside files.
