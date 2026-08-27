# lettuce-models

Provider connections, model profiles, capability evidence, and selection.

## Boundary

Does not execute inference or install model files.

The public surface is intentionally small. Business invariants belong in domain models and use cases; infrastructure is accessed only through narrow ports owned by the calling crate.

## Foundation contracts

`ProviderAccount` owns a stable connection identity, protocol, endpoint,
enabled state, typed non-secret configuration, and opaque secret references.
Custom header values are never serializable strings; each is a `SecretRef`.

`ModelProfile` always points to an explicit provider account. There is no
provider-label, sole-account, or first-account fallback. Repository ports are
synchronous because the SQLite adapter is synchronous; application composition
places calls on its database worker.

Model deletion dependencies use closed, typed `ModelDependencyReference`
values (for example, `CharacterDefault` and `GroupMemberOverride`) and surface
through `ModelRepositoryError::InUse`. A group-member override identifies both
the owning group and the character whose override retains the model profile.
Future adapters must report `GroupMemberOverride` references in both direct
model-profile deletion and provider-account-with-profiles deletion before
removing the referenced profile or account. The current character/group
persistence slice is not yet responsible for the group SQL; it must add those
typed references when that slice is implemented.

Chat parameters resolve per field in `operation -> session -> model -> global`
order. Provider-neutral controls have one canonical field: legacy Ollama
`num_ctx`, `num_predict`, and `repeat_penalty` import into `context_length`,
`max_output_tokens`, and `repetition_penalty`. The nested `ollama` profile is
reserved for its twelve native controls (`num_keep`, batching/device/thread
controls, tail-free/typical/min-p sampling, mirostat, seed, and stop strings).
This keeps provider-specific wire details typed without duplicating ordinary
chat settings.
