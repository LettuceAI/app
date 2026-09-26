# lettuce-contracts

The serde types that cross the IPC boundary between the Rust backend and the frontend. They are owned transport DTOs, kept separate from the domain and provider types so that an internal change never silently changes what the frontend receives, and so the frontend never has to guess a provider's wire values or feature combinations.

Only the composition crate (`lettuce-app`) depends on it; domain crates must not. It depends on nothing but `lettuce-types` and `serde`, and it holds no behavior.

## Provider contracts

Everything in `src/lib.rs` today describes providers:

- `ProviderCatalogContract` is the list the provider settings screen renders. Each `ProviderDescriptorContract` gives a provider kind's display name, `ProviderProtocolContract`, aliases, default endpoint and whether it can be edited, whether an API key is required, optional or unused and which header carries it, and what the provider supports: streaming, native tool translation, structured output, signed tool replay, reasoning together with tools, model listing, key verification, the `ReasoningSupportContract` (none, effort, budget only, dynamic), the `PromptCachingSupportContract` (none, supported, automatic) with the exact `PromptCacheRetentionContract` choices it accepts, which sampling parameters it takes (`ProviderParameterSupportContract`) and the extra request body keys it allows. Features are listed separately so the frontend never infers an unsafe combination from a protocol name or a model capability, and retention choices are typed so it never infers provider wire values.
- `ProviderAccountRequest` names an account for account-scoped calls.
- `ProviderModelsContract` returns the models a provider account lists, each a `RemoteModelContract` (id, display name, description, context length, input and output modalities, supported endpoints, prices).
- `KeyVerificationContract` reports whether an account's key verified, with the HTTP status when there was one.

`lettuce-app` builds these from the provider catalog in `lettuce-providers` (`generation/provider_runtime.rs`).
