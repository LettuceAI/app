//! Optional memory-model inventory, selection and comparison.
use super::error::{api_error, invalid_field, model_error};
use super::{ApiContext, ModelLoad};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode, RequiredModel};
use lettuce_jobs::{JobStore, handle::CancellationToken};
use lettuce_model_hub::{CompanionEmotionInstallStatus, EmbeddingModelFamily};

fn family(value: dto::EmbeddingFamily) -> EmbeddingModelFamily {
    match value {
        dto::EmbeddingFamily::LettuceEmbV4 => EmbeddingModelFamily::LettuceEmbV4,
        dto::EmbeddingFamily::LettuceEidosV5 => EmbeddingModelFamily::LettuceEidosV5,
    }
}
fn family_view(value: EmbeddingModelFamily) -> dto::EmbeddingFamily {
    match value {
        EmbeddingModelFamily::LettuceEmbV4 => dto::EmbeddingFamily::LettuceEmbV4,
        EmbeddingModelFamily::LettuceEidosV5 => dto::EmbeddingFamily::LettuceEidosV5,
    }
}
fn root(context: &ApiContext) -> Result<std::path::PathBuf, ApiError> {
    context
        .retained_model_roots_for_guard()?
        .and_then(|roots| roots.embedding)
        .map(std::path::PathBuf::from)
        .ok_or_else(|| model_error(ApiErrorCode::ModelRequired, RequiredModel::Embedding))
}
fn embedding_error(error: crate::EmbeddingModelError) -> ApiError {
    match error {
        crate::EmbeddingModelError::NotInstalled => {
            model_error(ApiErrorCode::ModelRequired, RequiredModel::Embedding)
        }
        error => api_error(ApiErrorCode::Internal, error.to_string()),
    }
}

pub async fn embedding_status(
    context: &ApiContext,
) -> Result<Vec<dto::EmbeddingModelView>, ApiError> {
    context
        .blocking(|context| {
            let root = root(context)?;
            let models = crate::EmbeddingModelCoordinator::new(&root, context.backend().database());
            let active = models
                .active()
                .map_err(embedding_error)?
                .map(|model| model.family);
            models
                .installed()
                .map_err(embedding_error)?
                .into_iter()
                .map(|model| {
                    Ok(dto::EmbeddingModelView {
                        family: family_view(model.family),
                        revision: model.source_revision,
                        native_dimensions: u16::try_from(model.native_dimensions).map_err(
                            |_| api_error(ApiErrorCode::Internal, "invalid model dimensions"),
                        )?,
                        active: active == Some(model.family),
                    })
                })
                .collect()
        })
        .await
}

pub async fn embedding_choose(
    context: &ApiContext,
    request: dto::EmbeddingModelRequest,
) -> Result<(), ApiError> {
    context
        .blocking(move |context| {
            let root = root(context)?;
            crate::EmbeddingModelCoordinator::new(&root, context.backend().database())
                .choose(family(request.family))
                .map_err(embedding_error)?;
            context.models_changed();
            Ok(())
        })
        .await
}

pub async fn embedding_remove(
    context: &ApiContext,
    request: dto::EmbeddingModelRequest,
) -> Result<bool, ApiError> {
    context
        .blocking(move |context| {
            let root = root(context)?;
            for (id, install_root) in context.jobs().install_roots() {
                if install_root == root
                    && context
                        .backend()
                        .database()
                        .get(id)
                        .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?
                        .is_some_and(|job| !job.is_terminal())
                {
                    return Err(super::local_models::busy(
                        dto::LocalModelsBusyReason::InstallActive {
                            job_id: id.to_string(),
                        },
                    ));
                }
            }
            let removed =
                crate::EmbeddingModelCoordinator::new(&root, context.backend().database())
                    .remove(family(request.family))
                    .map_err(embedding_error)?;
            context.models_changed();
            Ok(removed)
        })
        .await
}

pub async fn embedding_unload(context: &ApiContext) -> Result<(), ApiError> {
    context.models().unload_embedding();
    Ok(())
}

pub async fn embedding_compare(
    context: &ApiContext,
    request: dto::EmbeddingCompareRequest,
) -> Result<dto::EmbeddingComparison, ApiError> {
    if request.left.trim().is_empty() || request.right.trim().is_empty() {
        return Err(invalid_field(
            "text",
            "both comparison texts must be nonempty",
        ));
    }
    context.models().prepare_embedding(context).await?;
    context
        .blocking(move |context| {
            let _folder_access = context.local_models().folder_access();
            if let Some(root) = context
                .retained_model_roots_for_guard()?
                .and_then(|roots| roots.embedding)
                && std::path::Path::new(&root)
                    .starts_with(super::local_models::models_root(context)?)
            {
                super::jobs::local::folder_move_active(context)?;
            }
            let engine = match context.models().resolve_embedding(context) {
                ModelLoad::Loaded(engine) => engine,
                ModelLoad::NotInstalled => {
                    return Err(model_error(
                        ApiErrorCode::ModelRequired,
                        RequiredModel::Embedding,
                    ));
                }
                ModelLoad::Unavailable => {
                    return Err(model_error(
                        ApiErrorCode::ModelUnavailable,
                        RequiredModel::Embedding,
                    ));
                }
            };
            let dimensions = engine.dimensions();
            let embed = |text| {
                engine
                    .embed_memory(
                        &lettuce_embeddings::EmbeddingRequest { text, dimensions },
                        &CancellationToken::new(),
                    )
                    .map_err(|_| {
                        model_error(ApiErrorCode::ModelUnavailable, RequiredModel::Embedding)
                    })
            };
            let left = embed(request.left)?;
            let right = embed(request.right)?;
            if left.values.len() != dimensions.get()
                || right.values.len() != dimensions.get()
                || left.source_revision != engine.source_revision()
                || right.source_revision != engine.source_revision()
                || left
                    .values
                    .iter()
                    .chain(&right.values)
                    .any(|value| !value.is_finite())
            {
                return Err(model_error(
                    ApiErrorCode::ModelUnavailable,
                    RequiredModel::Embedding,
                ));
            }
            let similarity = left
                .cosine_similarity(&right)
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    model_error(ApiErrorCode::ModelUnavailable, RequiredModel::Embedding)
                })?;
            Ok(dto::EmbeddingComparison {
                cosine_similarity: similarity,
                dimensions: u16::try_from(dimensions.get()).map_err(|_| {
                    api_error(ApiErrorCode::Internal, "invalid embedding dimensions")
                })?,
                source_revision: left.source_revision,
            })
        })
        .await
}

pub async fn companion_emotion_status(context: &ApiContext) -> Result<dto::ThymosStatus, ApiError> {
    context
        .blocking(|context| {
            let root = context
                .retained_model_roots_for_guard()?
                .and_then(|roots| roots.thymos)
                .map(std::path::PathBuf::from)
                .ok_or_else(|| model_error(ApiErrorCode::ModelRequired, RequiredModel::Emotion))?;
            crate::companion_emotion_status(context.backend().database(), &root)
                .map(|status| match status {
                    CompanionEmotionInstallStatus::NotInstalled => dto::ThymosStatus::NotInstalled,
                    CompanionEmotionInstallStatus::Installed { source_revision } => {
                        dto::ThymosStatus::Installed { source_revision }
                    }
                    CompanionEmotionInstallStatus::Damaged => dto::ThymosStatus::Damaged,
                })
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))
        })
        .await
}

pub async fn companion_emotion_remove(context: &ApiContext) -> Result<bool, ApiError> {
    context
        .blocking(|context| {
            let root = context
                .retained_model_roots_for_guard()?
                .and_then(|roots| roots.thymos)
                .map(std::path::PathBuf::from)
                .ok_or_else(|| model_error(ApiErrorCode::ModelRequired, RequiredModel::Emotion))?;
            let removed = crate::remove_companion_emotion(context.backend().database(), &root)
                .map_err(|error| match error {
                    crate::CompanionEmotionDownloadError::Install(
                        lettuce_model_hub::CompanionEmotionInstallError::Busy,
                    ) => api_error(ApiErrorCode::Busy, "a Thymos install is active"),
                    error => api_error(ApiErrorCode::Internal, error.to_string()),
                })?;
            context.models_changed();
            Ok(removed)
        })
        .await
}

pub async fn embedding_install(
    context: &ApiContext,
    request: dto::EmbeddingInstallRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let (root, pin) = context
        .blocking(move |context| {
            let root = root(context)?;
            if root.starts_with(super::local_models::models_root(context)?) {
                super::jobs::local::folder_move_active(context)?;
            }
            let tls = context
                .backend()
                .tls_policy()
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
            let client = lettuce_network::JsonClient::with_tls(&tls)
                .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
            let pin = tokio::runtime::Handle::current()
                .block_on(crate::EmbeddingModelCatalog::new(client).pin(family(request.family)))
                .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
            Ok((root, pin))
        })
        .await?;
    let detail = serde_json::to_value(super::jobs::local::LocalModelJobDetail::EmbeddingInstall {
        root: root.clone(),
        pin: pin.clone(),
        enable_dynamic_memory: request.enable_dynamic_memory,
    })
    .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    let plan = crate::embedding_install_plan(&root, &pin);
    super::jobs::install::admit_install_with_detail(
        context,
        super::jobs::InstallWork::Artifact {
            plan,
            finish: Box::new(super::jobs::InstallFinish::Embedding {
                root,
                pin,
                enable_dynamic_memory: request.enable_dynamic_memory,
            }),
        },
        Some(detail),
    )
    .await
}

pub async fn companion_emotion_install(context: &ApiContext) -> Result<dto::JobAccepted, ApiError> {
    let (root, remote) = context
        .blocking(|context| {
            let root = context
                .retained_model_roots_for_guard()?
                .and_then(|roots| roots.thymos)
                .map(std::path::PathBuf::from)
                .ok_or_else(|| model_error(ApiErrorCode::ModelRequired, RequiredModel::Emotion))?;
            if root.starts_with(super::local_models::models_root(context)?) {
                super::jobs::local::folder_move_active(context)?;
            }
            let browser = context.local_models().browser(context)?;
            let remote = tokio::runtime::Handle::current()
                .block_on(browser.companion_emotion_model())
                .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
            Ok((root, remote))
        })
        .await?;
    super::jobs::admit_install(
        context,
        super::jobs::InstallWork::Artifact {
            plan: crate::ArtifactInstallPlan {
                install_id: String::new(),
                root: root.clone(),
                artifacts: Vec::new(),
            },
            finish: Box::new(super::jobs::InstallFinish::CompanionEmotion { root, remote }),
        },
    )
    .await
}
