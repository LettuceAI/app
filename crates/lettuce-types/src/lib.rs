//! Stable vocabulary shared by every LettuceAI crate.

mod error;
mod hash;
mod id;
mod paging;
mod revision;
mod time;

pub use error::{AppError, ErrorCode};
pub use hash::{ContentHash, ParseContentHashError};
pub use id::*;
pub use paging::{Page, PageLimit, PageRequest};
pub use revision::{Revision, RevisionOverflow};
pub use time::{TimeError, TimestampMillis};
