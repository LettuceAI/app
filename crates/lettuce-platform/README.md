# lettuce-platform

Confined filesystem access for the rest of the workspace: a fixed set of managed directories, capabilities that grant read or write access to one of them, crash-safe writes, a move-to-trash, a resumable install store for large model files, and a narrow eSpeak NG process wrapper.

It is the only crate allowed to depend on `cap-std` and `cap-primitives` (`architecture.toml`, enforced by `scripts/check-architecture.sh`). It has no Tauri dependency and never hands out a global shell handle or an application root path. Callers name files with checked `ObjectKey`s under a root they were given a capability for; no operational method accepts a native path.

## Structure

- `FilesystemAuthority` (`authority.rs`) is the only factory. The composition root in `lettuce-app` builds it once from a `DirectorySnapshot` and passes on the `ManagedFiles` facade plus the capabilities each consumer needs. Its constructor is public only so that composition code can do that wiring.
- `DirectorySnapshot` (`directories.rs`) holds the trusted native locations: the app data directory and the private persistent directory. The paths must be absolute, free of `.` and `..`, and must not overlap the internal `platform-v2` container. No accessor returns them.
- `ManagedRoot` lists the grantable leaf roots: `Diagnostics`, `ImportStaging`, `JobStaging`, `MediaBlobs`, `Quarantine`, `Trash` under `platform-v2/`, and `PrivatePersistent` (by default `private-persistent-v2/` next to it). The container directory itself is never grantable.
- `ReadCapability` and `WriteCapability` pair one root with the authority that issued them. `ManagedFiles` checks with `Arc::ptr_eq` that a capability belongs to its own authority and fails with `WrongCapability` otherwise. `ManagedFiles` has no constructor and cannot mint capabilities.
- `ObjectKey` (`keys.rs`) is a list of explicit segments. There is no string or path parser. A segment may not be empty, `.` or `..`, contain `/`, `\` or control characters, look like a drive prefix, or start with one of the crate's reserved prefixes (`.lettuce-stage-`, `.lettuce-trash-`, `.lettuce-recovery-`, `.lettuce-journal-`). Keys are capped at 64 segments, 255 bytes and 255 characters per segment, and 4 KiB in total. Unicode is kept as given, not normalized.
- `PlatformError` is a small copyable enum with no paths in it, safe to send over IPC or into diagnostics.

## Reading

All operations are relative to descriptors opened once at construction. The roots themselves are opened without following a final symlink; trusting the ancestors of those paths is the composition adapter's job. Below a root, every directory on the way to a file is opened no-follow, so an existing symlink anywhere on the path fails with `SymlinkEscape`.

- `read` loads a whole file, up to `MAX_MANAGED_READ_BYTES` (2 GiB, the same as the media blob limit, so any stored object can be read whole).
- `open_read` returns a `ReadHandle` that implements `Read` but cannot be turned into a path.
- `metadata` stats without following symlinks.
- `list` needs an explicit limit (1 to 1024). It uses the directory entry's own file type, so a symlink is reported as `Other` and its target is never touched. Listing the private persistent root is refused: that root only supports reads and writes of keys the caller already knows.

## Writing

Writes go through a `StagedWrite`:

1. `stage`, `stage_new` or `stage_bounded` exclusively creates a sibling stage file named `.lettuce-stage-<uuid>` next to the target. A bounded stage rejects writes past its limit.
2. The caller streams bytes into it (`Write`).
3. `commit` syncs the stage, takes the authority's mutation lock and publishes it. `Replace` renames over the target within the same root; a failed rename is `ReplaceFailed` (on Windows replacing an existing file is reported this way instead of being passed off as atomic). `CreateNew` publishes without ever overwriting: `renameat2(RENAME_NOREPLACE)` on Linux, a per-directory lock plus existence check plus rename on Android (older Android seccomp policies kill a process that calls `renameat2`), and a hard link elsewhere. An existing target is `Conflict`; a filesystem that cannot do it is `Unsupported`.
4. The parent directory is synced and the `CommitReceipt` reports it (`Synced`, `Unsupported` or `Failed`) along with the byte count and the stage cleanup status.

Once a create-new publish has succeeded the write is done, even if removing the stage afterwards fails. The receipt then says `RecoveryNeeded` for the cleanup, and the caller must not retry the write. A failed commit keeps its stage file for recovery; a `StagedWrite` dropped without commit removes its own stage. `write_atomic` is the one-call version for a byte slice.

The mutation lock is per authority and serializes mutations within one process. It is not a cross-process lock.

## Removing files

`remove_file` deletes one regular file under a write capability and reports whether it existed. `remove_to_trash` instead renames the object into the `Trash` root under an opaque `.lettuce-trash-<uuid>` name and returns a `TrashReceipt` with the sync status of both directories. `restore_from_trash` moves it back; the receipt holds a weak reference to its authority, so it only works on the authority that issued it and only for the root it came from, and it refuses to overwrite an object that has since appeared at the old key. Neither operation is allowed on the private persistent root, and there is no way to delete a root.

Trash is in-process only: there is no journal, so a trashed object is not guaranteed to be restorable after a crash, and nothing purges old trash.

## Recovery scan

`recover_incomplete` walks every root (depth 16, at most 256 entries) and counts leftover stage files it owns, identified by root and full relative location so equal names in different roots stay distinct. It only inspects names matching the crate's own stage pattern and deletes nothing; the `RecoveryReport` says how many were found and whether the scan was cut short.

## Install store

`ConfinedInstallStore` (`install.rs`) exists for large files that must stay resumable across restarts and that a native runtime may later open by path, such as model artifacts. It is opened on one absolute directory, which is canonicalized once; after that every name is an `ObjectKey` and parent directories are checked for symlinks as they are created.

- `prepare(partial, target, max_bytes)` returns `Installed` if the target file already exists (within the size limit), or `Resume` with a `ResumableInstall` positioned at the end of the partial file, so a download continues from the bytes that survived a restart.
- A `ResumableInstall` can `append` (bounded by `max_bytes`), `restart` from zero, be read and seeked for verification, `sync`, `discard`, and `commit` (rename over the target) or `commit_new` (hard link, `Conflict` if the target exists). Verifying the bytes is the caller's job before commit.
- Only a committed file exposes its native path (`InstalledFile::native_path`, the path returned by `commit`), for building a verified runtime manifest. Partial paths and the root are never returned.
- `inspect` opens a known key read-only, `list` gives a sorted, bounded inventory of one directory (missing directory means empty), `remove_installed` deletes a target only when the caller's expected path matches, `discard` removes a partial or installed file by key, `discard_partials_like` clears stale `.part` siblings with a given prefix, and `owns_installed_path` tells whether a path belongs to a key in this store.

`lettuce-model-hub` uses it for pinned artifacts (Whisper, Kokoro, the companion emotion model), `lettuce-media` for the sync media store, and `lettuce-transfer` for backup archives and restore workspaces. Of the managed roots, only `MediaBlobs` (the local media blob store in `lettuce-media`) and `PrivatePersistent` (the pointer to the active database file, `AppDatabaseLocation` in `lettuce-app`) have callers outside tests so far.

## eSpeak NG

Desktop Kokoro phonemization needs eSpeak NG. `EspeakNgProcess` implements the `EspeakPhonemizer` trait and runs `espeak-ng --ipa --stdin -q -v <language>` with fixed arguments, from `PATH` (`from_path`) or from an absolute executable with an optional data directory (`with_managed_paths`, both must exist). Input is capped at 64 KiB and may not contain NUL; the language must be 1 to 16 ASCII letters, digits, `-` or `_`; stdout and stderr are drained on separate threads and capped at 1 MiB. On Linux the executable's directory is put on `LD_LIBRARY_PATH` for a bundled build, and on Windows the process is started without a console window. There is no generic process runner in the crate.
