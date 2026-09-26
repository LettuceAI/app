# lettuce-jobs: status notes

Facts about `lettuce-jobs` that are not architecture. The crate README describes the current design.

## Not wired yet

- The previous README described the crate as the "Slice 0" domain-independent foundation, with policy, recovery, retention, handle and resource-admission vocabulary "for later database/application adapters". The SQLite adapter and startup recovery now exist; `JobRegistry` and `scheduler::ResourceClaim` still have no callers outside this crate, and there is no scheduler here beyond `claim_next`'s priority and resource selection.

## History

- `ConversationGeneration` was added after the other kinds; its registry key was appended at the end rather than inserted, which is why the registry uses its own key instead of the enum order.
