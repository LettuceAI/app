use lettuce_types::{ConversationId, GenerationAttemptId, GenerationTurnId, JobId, UsageEventId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UsageTombstone {
    Conversation {
        event_id: UsageEventId,
        conversation_id: ConversationId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        job_id: Option<JobId>,
    },
    Dispatch {
        event_id: UsageEventId,
        attempt_id: GenerationAttemptId,
        job_id: JobId,
    },
    Legacy {
        run_id: String,
        source_id: String,
    },
}

impl UsageTombstone {
    pub fn key(&self) -> (&'static str, String) {
        match self {
            Self::Conversation { event_id, .. } => ("conversation", event_id.to_string()),
            Self::Dispatch { event_id, .. } => ("job", event_id.to_string()),
            Self::Legacy { run_id, source_id } => (
                "legacy",
                serde_json::to_string(&(run_id, source_id)).expect("strings serialize"),
            ),
        }
    }
}
