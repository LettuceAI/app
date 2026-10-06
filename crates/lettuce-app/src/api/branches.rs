use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{
    BranchStatus, ConversationOverviewReader, ConversationReader, ConversationRepository,
    ConversationRepositoryError, ForkBranch, OperationKind, OperationResultRef, RenameBranch,
    SelectBranch,
};
use lettuce_types::{ConversationBranchId, ConversationId, MessageId};

use super::ApiContext;
use super::error::{IntoApiError, api_error, parse_id};
use super::messages::{committed_revision, expected_revision, on_timeline, operation, replayed};

fn branch_conversation(
    context: &ApiContext,
    branch_id: ConversationBranchId,
) -> Result<ConversationId, ApiError> {
    ConversationOverviewReader::branch_conversation(context.backend().database(), branch_id)
        .map_err(IntoApiError::into_api_error)?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the branch does not exist"))
}

pub async fn conversation_branches(
    context: &ApiContext,
    request: dto::ConversationRequest,
) -> Result<dto::ConversationBranchList, ApiError> {
    let conversation_id = parse_id(&request.conversation_id, "conversation_id")?;
    context
        .blocking(move |context| {
            let (revision, branches) = ConversationOverviewReader::branch_overviews(
                context.backend().database(),
                conversation_id,
            )
            .map_err(IntoApiError::into_api_error)?;
            Ok(dto::ConversationBranchList {
                revision: revision.get(),
                branches: branches
                    .into_iter()
                    .map(|overview| dto::ConversationBranchView {
                        id: overview.branch.id.to_string(),
                        label: overview.label,
                        parent_branch_id: overview.branch.parent_branch_id.map(|id| id.to_string()),
                        fork_message_id: overview.branch.fork_message_id.map(|id| id.to_string()),
                        message_count: overview.message_count,
                        updated_at: overview.branch.updated_at.get(),
                        active: overview.active,
                    })
                    .collect(),
            })
        })
        .await
}

fn is_live(
    aggregate: &lettuce_conversations::ConversationAggregate,
    id: ConversationBranchId,
) -> bool {
    aggregate
        .branches
        .iter()
        .any(|branch| branch.id == id && branch.status == BranchStatus::Active)
}

fn fork_source_branch(
    context: &ApiContext,
    conversation_id: ConversationId,
    aggregate: &lettuce_conversations::ConversationAggregate,
    message_id: MessageId,
) -> Result<ConversationBranchId, ApiError> {
    let active = aggregate.conversation.active_branch_id;
    let on_active = on_timeline(context, conversation_id, active, message_id);
    if let Ok(anchor) = &on_active {
        let owner = anchor.item.message.branch_id;
        return Ok(if is_live(aggregate, owner) {
            owner
        } else {
            active
        });
    }
    let mut live: Vec<_> = aggregate
        .branches
        .iter()
        .filter(|branch| branch.status == BranchStatus::Active && branch.id != active)
        .collect();
    live.sort_by_key(|branch| (branch.created_at, branch.id));
    for branch in live {
        if let Ok(anchor) = on_timeline(context, conversation_id, branch.id, message_id)
            && !is_live(aggregate, anchor.item.message.branch_id)
        {
            return Ok(branch.id);
        }
    }
    on_active.map(|_| active)
}

pub async fn conversation_branch_fork(
    context: &ApiContext,
    request: dto::ConversationBranchForkRequest,
) -> Result<dto::ConversationBranchChanged, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id: MessageId = parse_id(&request.message_id, "message_id")?;
    let expected_revision = expected_revision(request.expected_revision)?;
    let operation = operation(
        request.client_operation_id,
        &[
            b"branch-fork",
            request.conversation_id.as_bytes(),
            request.message_id.as_bytes(),
        ],
    )?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let aggregate = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            let source_branch_id =
                if replayed(context, conversation_id, OperationKind::Fork, &operation)? {
                    let record = ConversationReader::operation_record(
                        database,
                        conversation_id,
                        OperationKind::Fork,
                        &operation,
                    )
                    .map_err(IntoApiError::into_api_error)?
                    .ok_or_else(|| {
                        api_error(ApiErrorCode::Internal, "the fork operation is missing")
                    })?;
                    let OperationResultRef::Branch(id) = record.result else {
                        return Err(api_error(
                            ApiErrorCode::Internal,
                            "the fork result is invalid",
                        ));
                    };
                    aggregate
                        .branches
                        .iter()
                        .find(|branch| branch.id == id)
                        .and_then(|branch| branch.parent_branch_id)
                        .ok_or_else(|| {
                            api_error(ApiErrorCode::Internal, "the fork parent is missing")
                        })?
                } else {
                    fork_source_branch(context, conversation_id, &aggregate, message_id)?
                };
            let commit = database
                .fork_branch(
                    &ForkBranch {
                        conversation_id,
                        source_branch_id,
                        at_message_id: Some(message_id),
                        expected_revision,
                        operation,
                    },
                    context.now(),
                )
                .map_err(IntoApiError::into_api_error)?;
            Ok(dto::ConversationBranchChanged {
                branch_id: commit.value.branch.id.to_string(),
                revision: committed_revision(&commit.outbox),
            })
        })
        .await
}

pub async fn conversation_branch_rename(
    context: &ApiContext,
    request: dto::ConversationBranchRenameRequest,
) -> Result<dto::ConversationBranchChanged, ApiError> {
    let branch_id = parse_id(&request.branch_id, "branch_id")?;
    let expected_revision = expected_revision(request.expected_revision)?;
    let label = request.label.trim().to_owned();
    let operation = operation(
        request.client_operation_id,
        &[
            b"branch-rename",
            request.branch_id.as_bytes(),
            label.as_bytes(),
        ],
    )?;
    context
        .blocking(move |context| {
            let conversation_id = branch_conversation(context, branch_id)?;
            let commit = context
                .backend()
                .database()
                .rename_branch(
                    &RenameBranch {
                        conversation_id,
                        branch_id,
                        label,
                        expected_revision,
                        operation,
                    },
                    context.now(),
                )
                .map_err(IntoApiError::into_api_error)?;
            Ok(dto::ConversationBranchChanged {
                branch_id: branch_id.to_string(),
                revision: committed_revision(&commit.outbox),
            })
        })
        .await
}

pub async fn conversation_branch_select(
    context: &ApiContext,
    request: dto::ConversationBranchMutationRequest,
) -> Result<dto::ConversationBranchChanged, ApiError> {
    let branch_id = parse_id(&request.branch_id, "branch_id")?;
    let expected_revision = expected_revision(request.expected_revision)?;
    let operation = operation(
        request.client_operation_id,
        &[b"branch-select", request.branch_id.as_bytes()],
    )?;
    context
        .blocking(move |context| {
            let conversation_id = branch_conversation(context, branch_id)?;
            let commit = context
                .backend()
                .database()
                .select_branch(
                    &SelectBranch {
                        conversation_id,
                        branch_id,
                        expected_revision,
                        operation,
                    },
                    context.now(),
                )
                .map_err(IntoApiError::into_api_error)?;
            Ok(dto::ConversationBranchChanged {
                branch_id: branch_id.to_string(),
                revision: committed_revision(&commit.outbox),
            })
        })
        .await
}

pub async fn conversation_branch_delete(
    context: &ApiContext,
    request: dto::ConversationBranchMutationRequest,
) -> Result<dto::ConversationBranchChanged, ApiError> {
    let branch_id = parse_id(&request.branch_id, "branch_id")?;
    let expected_revision = expected_revision(request.expected_revision)?;
    let operation = operation(
        request.client_operation_id,
        &[b"branch-delete", request.branch_id.as_bytes()],
    )?;
    context
        .blocking(move |context| {
            let conversation_id = branch_conversation(context, branch_id)?;
            let commit = context
                .backend()
                .database()
                .delete_branch(
                    &lettuce_conversations::DeleteBranch {
                        conversation_id,
                        branch_id,
                        expected_revision,
                        operation,
                    },
                    context.now(),
                )
                .map_err(|error| match error {
                    ConversationRepositoryError::Dependency => {
                        delete_refusal(context, conversation_id, branch_id)
                    }
                    error => error.into_api_error(),
                })?;
            Ok(dto::ConversationBranchChanged {
                branch_id: branch_id.to_string(),
                revision: committed_revision(&commit.outbox),
            })
        })
        .await
}

fn delete_refusal(
    context: &ApiContext,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> ApiError {
    let is_root = ConversationReader::get(context.backend().database(), conversation_id).is_ok_and(
        |aggregate| {
            aggregate
                .branches
                .iter()
                .any(|branch| branch.id == branch_id && branch.parent_branch_id.is_none())
        },
    );
    ApiError {
        code: ApiErrorCode::Conflict,
        message: if is_root {
            "the first branch cannot be deleted".into()
        } else {
            "the selected branch cannot be deleted".into()
        },
        details: Some(dto::ApiErrorDetails::BranchDeleteRefused {
            reason: if is_root {
                dto::BranchDeleteRefusal::RootBranch
            } else {
                dto::BranchDeleteRefusal::SelectedBranch
            },
        }),
    }
}
