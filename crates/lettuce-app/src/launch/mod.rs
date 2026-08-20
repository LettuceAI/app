//! Deterministic conversation launch planning.
//!
//! The planner resolves a caller's launch request against the authored
//! character graph, copies every resolved source into an immutable snapshot
//! document, and hands the closed bundle to the conversation creator. All
//! identities are derived from the operation key, so a retried request lands
//! on the same conversation row instead of creating a second one.

mod digest;
mod documents;
mod error;
mod identity;
mod planner;
mod policy;
mod request;

pub use error::{DirectLaunchError, LaunchSourceError};
pub use identity::launch_conversation_id;
pub use planner::{ConversationLaunchPlanner, DirectLaunchSources};
pub use request::{
    DIRECT_LAUNCH_REQUEST_FORMAT_V1, DirectConversationLaunchRequest, DirectUserParticipant,
    LaunchSelection,
};

#[cfg(test)]
mod tests;
