//! Shared limits for stored user content. Runtime prompt assembly has no
//! separate text-size cap; model context and transport limits apply there.

pub const MAX_LABEL_BYTES: usize = 64 * 1024;
pub const MAX_TEXT_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_DOCUMENT_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_TOOL_ARGUMENT_BYTES: usize = 64 * 1024 * 1024;
