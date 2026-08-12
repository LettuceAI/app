use serde_json::Value;
use tauri::AppHandle;

use crate::chat_manager::types::{Character, Model, ProviderCredential, Session, Settings};
use crate::conversation_manager::{
    execute_generation, ConversationExecutionInput, ConversationExecutionOutput,
};
use crate::storage_manager::group_sessions::GroupSession;
use crate::usage::tracking::UsageOperationType;

pub(super) struct GroupGenerationRequest<'a> {
    pub app: &'a AppHandle,
    pub session: &'a GroupSession,
    pub character: &'a Character,
    pub model: &'a Model,
    pub credential: &'a ProviderCredential,
    pub settings: &'a Settings,
    pub messages: &'a Vec<Value>,
    pub request_id: &'a str,
    pub stream: bool,
    pub operation_type: UsageOperationType,
}

pub(super) async fn execute_group_generation(
    request: GroupGenerationRequest<'_>,
) -> Result<ConversationExecutionOutput, String> {
    let request_session: Session = serde_json::from_value(serde_json::json!({
        "id": request.session.id,
        "characterId": request.character.id,
        "title": request.session.name,
        "createdAt": request.session.created_at.max(0) as u64,
        "updatedAt": request.session.updated_at.max(0) as u64
    }))
    .map_err(|error| format!("Failed to prepare group request settings: {error}"))?;

    let output = execute_generation(ConversationExecutionInput {
        app: request.app,
        session_id: &request.session.id,
        request_session: &request_session,
        settings: request.settings,
        model: request.model,
        credential: request.credential,
        messages: request.messages,
        stream: request.stream,
        request_id: Some(request.request_id.to_string()),
        operation: request.operation_type.as_str(),
        log_scope: "group_chat_response",
        tool_config: None,
    })
    .await
    .map_err(|failure| {
        super::record_group_failed_usage(
            request.app,
            &failure.usage,
            request.session,
            request.character,
            request.model,
            request.credential,
            request.operation_type,
            &failure.message,
        );
        failure.message
    })?;

    super::record_group_usage(
        request.app,
        &output.usage,
        request.session,
        request.character,
        request.model,
        request.credential,
        &output.api_key,
        request.operation_type,
        "group_chat_response",
    )
    .await;

    Ok(output)
}
