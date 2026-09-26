# New frontend

The clean frontend will be built here according to [PLAN.md](PLAN.md). The legacy React application remains in `old-code/src`.

Do not import code directly from `old-code`. Shared behavior should first receive a characterization test, then be redesigned behind generated contracts or deliberately reimplemented in this tree.

Frontend package/tooling is intentionally not initialized in the crate-scaffolding step; it will be selected when the generated Rust/TypeScript contract boundary is implemented.
