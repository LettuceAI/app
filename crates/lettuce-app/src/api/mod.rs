//! The application API the desktop shell exposes as commands. Every call is
//! a plain async function over an `ApiContext` that takes and returns
//! `lettuce-contracts` DTOs and fails with an `ApiError`; repository work
//! runs on the blocking pool.

mod assets;
mod characters;
mod context;
mod conversations;
mod error;
mod events;
mod mapping;
mod worker;

#[cfg(test)]
mod tests;

pub use assets::{AssetBytes, read_asset};
pub use characters::characters_list;
pub use context::{ApiContext, ApiContextParts, ApiMediaStore};
pub use conversations::{
    conversation_launch_direct, conversation_messages, conversation_open, conversation_send,
    conversations_list, generation_cancel,
};
pub use events::{ApiEventSink, GenerationEventSink};
pub use worker::ConversationGenerationWorker;
