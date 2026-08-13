# lettuce-platform

Operating-system capabilities, application lifecycle, managed paths, confined
filesystem operations, updater integration, and user-intent ports.

## Boundary

Does not expose a global Tauri/shell handle or arbitrary application root path.
Filesystem consumers receive purpose-specific capabilities.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Implemented filesystem kernel

Phase 1 provides an immutable `DirectorySnapshot`, named non-overlapping leaf
roots, explicit checked `ObjectKey` segments, and separate read/write
capabilities. `FilesystemAuthority` is the only factory: the app composition
root constructs it from its attested native locations and passes its
`ManagedFiles` facade plus purpose capabilities onward. Its constructor is
public solely so a separate composition adapter can perform that wiring;
architecture checks will restrict construction to adapter code, while domain
code receives only the facade and purpose capabilities. `ManagedFiles` has no
constructor or capability-minting shortcut.

Managed reads and metadata operate on descriptor-relative `cap-std`
directories. Listing has an explicit caller limit and uses no-follow entry
file types, so symlinks are returned as `Other`. Existing intermediate and
final symlinks are rejected for file operations; no operational method accepts
a caller-supplied native path. The platform container directory is internal and
is never grantable.

Writes are streamed to an exclusively-created sibling stage file, bounded when
requested, and synced before commit. Replacement uses same-root rename and
reports parent-directory durability. Create-new uses an atomic hard-link
no-replace operation; filesystems that cannot provide it return `Unsupported`.
Once that link succeeds, the target is committed exactly once: the receipt
reports whether sibling-stage cleanup was completed or needs bounded recovery,
so cleanup failure must not be retried as the write itself.
Windows replacement of an existing target is reported as a
recoverable replacement failure rather than being described as atomic. Failed
commits retain their stage artifact for recovery; ordinary dropped writers
clean up their own stage file. A single authority-wide mutation lock
serializes operations within that authority; it is not a cross-process lock.

Generic root deletion is not exposed. File removal is an in-process move to an
opaque trash receipt with collision-safe, retryable restore. Receipts are
authority-bound, validate opaque internal names, and include source/destination
durability statuses. Durable trash journals, retention purge, a quarantine
workflow, and ambiguous-commit reconciliation are later work; this slice does
not claim generic trash is crash-restorable or retention-managed. Recovery is
bounded, idempotent, scans each leaf once, and identifies artifacts by full
leaf plus relative location; it only inspects tool-owned stage names and does
not guess at destructive cleanup.

This slice intentionally excludes Tauri, lifecycle, archive, grants, updater,
domain formats, cryptography, and secret-envelope policy. The private
persistent root permits known-key reads and atomic writes for a settings-owned
secret adapter, but generic listing/export is unavailable. Root construction
uses a no-follow final open; ambient ancestor attestation remains the
application adapter's responsibility.
