use std::io::{Read, Seek, SeekFrom, Write};

use base64::Engine;
use lettuce_context::{LorebookRepository, PromptBehaviorVersion, PromptMetadataDraft};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::TimestampMillis;
use serde::{Serialize, de::DeserializeOwned};

use super::{
    ApiContext,
    error::{IntoApiError, api_error, invalid_field, parse_id},
    lorebooks::Failure,
};

pub(super) struct ImportFile {
    pub name: String,
    pub kind: dto::FileKind,
    pub bytes: Vec<u8>,
    pub digest: String,
}

pub(super) fn read(context: &ApiContext, source: &dto::FileSource) -> Result<ImportFile, ApiError> {
    read_with_progress(context, source, &mut |_| Ok(()))
}

pub(super) fn read_with_progress(
    context: &ApiContext,
    source: &dto::FileSource,
    progress: &mut dyn FnMut(u64) -> Result<(), ApiError>,
) -> Result<ImportFile, ApiError> {
    if source.uri.trim().is_empty() {
        return Err(invalid_field("source", "source is empty"));
    }
    let description = context
        .files()
        .describe(&source.uri)
        .map_err(IntoApiError::into_api_error)?;
    let mut reader = context
        .files()
        .open(&source.uri)
        .map_err(IntoApiError::into_api_error)?;
    let kind =
        super::file_kind::detect_kind(reader.as_mut()).map_err(IntoApiError::into_api_error)?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|_| super::files::FileAccessError::Io.into_api_error())?;
    let mut bytes = Vec::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        cancelled(context)?;
        progress(bytes.len() as u64)?;
        let length = reader
            .read(&mut buffer)
            .map_err(|_| super::files::FileAccessError::Io.into_api_error())?;
        if length == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..length]);
    }
    progress(bytes.len() as u64)?;
    let digest = blake3::hash(&bytes).to_hex().to_string();
    Ok(ImportFile {
        name: description.name,
        kind,
        bytes,
        digest,
    })
}

pub(super) fn text(file: &ImportFile) -> Result<&str, ApiError> {
    std::str::from_utf8(&file.bytes).map_err(|_| invalid_field("source", "the file is not UTF-8"))
}

fn format_error(error: impl std::fmt::Display) -> ApiError {
    invalid_field("source", error.to_string())
}
impl IntoApiError for crate::ChatFileError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Launch(error) => error.into_api_error(),
            Self::NotFound => api_error(ApiErrorCode::NotFound, self.to_string()),
            Self::Repository(_) | Self::Convert(_) => internal(self),
            _ => format_error(self),
        }
    }
}
impl IntoApiError for crate::PromptFileError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Repository(error) => error.into_api_error(),
            Self::NotFound(_) => api_error(ApiErrorCode::NotFound, self.to_string()),
            _ => format_error(self),
        }
    }
}
impl IntoApiError for crate::ChatTemplateFileError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Repository(error) => error.into_api_error(),
            Self::NotFound | Self::CharacterNotFound => {
                api_error(ApiErrorCode::NotFound, self.to_string())
            }
            _ => format_error(self),
        }
    }
}
impl IntoApiError for crate::ModelFileError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Repository(error) => super::model_profiles::model_error(error),
            Self::NotFound => api_error(ApiErrorCode::NotFound, self.to_string()),
            Self::NoProviderAccount(provider) => {
                let mut error =
                    invalid_field("provider_id", format!("no provider account for {provider}"));
                error.code = ApiErrorCode::NotFound;
                error
            }
            _ => format_error(self),
        }
    }
}
impl IntoApiError for crate::LorebookFileError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Repository(error) => error.into_api_error(),
            Self::NotFound => api_error(ApiErrorCode::NotFound, self.to_string()),
            _ => format_error(self),
        }
    }
}
impl IntoApiError for crate::CharacterFileError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Repository(_) => internal(self),
            Self::NotFound => api_error(ApiErrorCode::NotFound, self.to_string()),
            _ => format_error(self),
        }
    }
}
impl IntoApiError for crate::PersonaFileError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Repository(error) => error.into_api_error(),
            Self::Bindings(_) => internal(self),
            Self::NotFound => api_error(ApiErrorCode::NotFound, self.to_string()),
            _ => format_error(self),
        }
    }
}
fn internal(error: impl std::fmt::Display) -> ApiError {
    api_error(ApiErrorCode::Internal, error.to_string())
}
fn convert<T: Serialize, U: DeserializeOwned>(value: &T) -> Result<U, ApiError> {
    serde_json::from_value(serde_json::to_value(value).map_err(internal)?).map_err(internal)
}
fn warning(code: dto::FileWarningCode, field: impl Into<String>) -> dto::FileWarning {
    dto::FileWarning {
        code,
        field: field.into(),
    }
}
pub(super) fn stem(name: &str) -> Option<&str> {
    std::path::Path::new(name)
        .file_stem()
        .and_then(|name| name.to_str())
}

fn character(
    file: &ImportFile,
    now: TimestampMillis,
) -> Result<lettuce_transfer::CharacterPackage, ApiError> {
    let png = file.bytes.starts_with(b"\x89PNG\r\n\x1a\n");
    let json = lettuce_transfer::character_import_json(
        if png { "card.png" } else { "card.json" },
        &file.bytes,
    )
    .map_err(format_error)?;
    let value = serde_json::from_str(&json).map_err(format_error)?;
    let (mut package, _) = lettuce_transfer::parse_character_import(&value, now.get(), || {
        uuid::Uuid::new_v4().to_string()
    })
    .map_err(format_error)?;
    if png {
        package.avatar_data = Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&file.bytes)
        ));
    }
    Ok(package)
}
fn persona(
    file: &ImportFile,
    now: TimestampMillis,
) -> Result<lettuce_transfer::PersonaPackage, ApiError> {
    lettuce_transfer::parse_persona_import(
        &serde_json::from_str(text(file)?).map_err(format_error)?,
        now.get(),
    )
    .map_err(format_error)
}
fn remote(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}
fn image_preview(
    field: String,
    data: Option<&str>,
    download: bool,
    images: &mut Vec<dto::ImportImagePreview>,
    warnings: &mut Vec<dto::FileWarning>,
) {
    let Some(data) = data else {
        return;
    };
    images.push(dto::ImportImagePreview {
        field: field.clone(),
        remote_url: remote(data).then(|| data.to_owned()),
    });
    if remote(data) {
        warnings.push(warning(
            if download {
                dto::FileWarningCode::RemoteAvatarWillDownload
            } else {
                dto::FileWarningCode::RemoteAvatarNotDownloaded
            },
            field,
        ));
    } else if !base64::engine::general_purpose::STANDARD
        .decode(data.split_once(',').map_or(data, |(_, data)| data))
        .is_ok_and(|bytes| {
            lettuce_media::sniff_media_kind(&bytes) == Some(lettuce_media::MediaKind::Image)
        })
    {
        warnings.push(warning(dto::FileWarningCode::ImageDecodeFailed, field));
    }
}

pub async fn import_preview(
    context: &ApiContext,
    request: dto::ImportPreviewRequest,
) -> Result<dto::ImportPreview, ApiError> {
    context
        .blocking(move |context| preview(context, read(context, &request.source)?))
        .await
}

fn preview(context: &ApiContext, file: ImportFile) -> Result<dto::ImportPreview, ApiError> {
    let now = context.now();
    let mut images = Vec::new();
    let mut warnings = Vec::new();
    let download = context
        .backend()
        .database()
        .load()
        .map_err(|e| api_error(ApiErrorCode::Unavailable, e.to_string()))?
        .settings
        .auto_download_character_card_avatars;
    let content = match file.kind {
        dto::FileKind::CharacterCard => {
            let package = character(&file, now)?;
            image_preview(
                "avatar".into(),
                package.avatar_data.as_deref(),
                download,
                &mut images,
                &mut warnings,
            );
            image_preview(
                "background".into(),
                package.background_image_data.as_deref(),
                false,
                &mut images,
                &mut warnings,
            );
            for scene in &package.character.scenes {
                image_preview(
                    format!("scenes:{}.background", scene.id),
                    scene.background_image_path.as_deref(),
                    false,
                    &mut images,
                    &mut warnings,
                );
            }
            for book in &package.character.lorebooks {
                let mut cursor = None;
                loop {
                    let page = LorebookRepository::page(
                        context.backend().database(),
                        lettuce_context::LorebookLibraryQuery {
                            status: lettuce_context::LifecycleFilter::All,
                            name_contains: Some(book.lorebook.name.clone()),
                            page: lettuce_types::PageRequest {
                                cursor,
                                limit: lettuce_types::PageLimit::new(200),
                            },
                        },
                    )
                    .map_err(IntoApiError::into_api_error)?;
                    if page
                        .items
                        .iter()
                        .any(|item| item.name == book.lorebook.name)
                    {
                        warnings.push(warning(
                            dto::FileWarningCode::BundledLorebookNewCopy,
                            format!("lorebooks:{}", book.lorebook.id),
                        ));
                        break;
                    }
                    let Some(next) = page.next_cursor else {
                        break;
                    };
                    cursor = Some(next);
                }
            }
            let references = lettuce_transfer::CharacterFileRepository::character_file_references(
                context.backend().database(),
            )
            .map_err(internal)?;
            let plan = lettuce_transfer::plan_character_file(
                &package,
                &references,
                now.get(),
                uuid::Uuid::new_v4,
            )
            .map_err(format_error)?;
            let imported = plan
                .import(
                    &references,
                    &lettuce_transfer::CharacterFileAssets::default(),
                    now.get(),
                    uuid::Uuid::new_v4,
                )
                .map_err(format_error)?;
            warnings.extend(
                plan.skipped
                    .into_iter()
                    .chain(imported.skipped)
                    .map(|skip| warning(dto::FileWarningCode::FieldSkipped, skip.source_key)),
            );
            warnings.extend(
                plan.notices
                    .into_iter()
                    .chain(imported.notices)
                    .map(|notice| warning(dto::FileWarningCode::FieldConverted, notice.field)),
            );
            dto::ImportPreviewContent::Character {
                content: Box::new(convert(&package.character)?),
            }
        }
        dto::FileKind::PersonaFile => {
            let package = persona(&file, now)?;
            image_preview(
                "avatar".into(),
                package.avatar_data.as_deref(),
                download,
                &mut images,
                &mut warnings,
            );
            dto::ImportPreviewContent::Persona {
                content: convert(&package.persona)?,
            }
        }
        dto::FileKind::Lorebook => {
            let book =
                lettuce_transfer::parse_lorebook_import(text(&file)?).map_err(format_error)?;
            let entries = book
                .entries
                .into_iter()
                .map(|entry| dto::ImportLorebookEntry {
                    id: String::new(),
                    lorebook_id: String::new(),
                    title: entry.title,
                    enabled: entry.enabled,
                    always_active: entry.always_active,
                    keywords: entry.keywords,
                    case_sensitive: entry.case_sensitive,
                    keyword_match_mode: dto::LorebookKeywordMode::Literal,
                    content: entry.content,
                    priority: entry.priority,
                    display_order: entry.display_order,
                    created_at: now.get(),
                    updated_at: now.get(),
                })
                .collect();
            dto::ImportPreviewContent::Lorebook {
                name: book.name,
                keyword_detection_mode: convert(&book.keyword_detection_mode)?,
                entries,
            }
        }
        dto::FileKind::PromptPreset => {
            let prompt = lettuce_transfer::parse_prompt_import(
                text(&file)?,
                stem(&file.name),
                "Imported entry",
                "Imported preset",
            )
            .map_err(format_error)?;
            if prompt.purpose_defaulted {
                warnings.push(warning(dto::FileWarningCode::FieldConverted, "purpose"));
            }
            dto::ImportPreviewContent::Prompt {
                name: prompt.name,
                purpose: super::prompts::kind(prompt.purpose),
                entries: convert(&prompt.entries)?,
                condense: prompt.condense,
            }
        }
        dto::FileKind::ChatTemplate => {
            let template =
                lettuce_transfer::parse_chat_template_import(text(&file)?).map_err(format_error)?;
            dto::ImportPreviewContent::ChatTemplate {
                name: template.name,
                messages: template
                    .messages
                    .into_iter()
                    .map(|(role, content)| dto::ImportTemplateMessage { role, content })
                    .collect(),
                scene_id: template.scene_id,
                prompt_template_id: template.prompt_template_id,
                lorebook_ids_override: template.lorebook_ids_override,
            }
        }
        dto::FileKind::ModelFile => {
            let model = lettuce_transfer::parse_model_import(text(&file)?).map_err(format_error)?;
            dto::ImportPreviewContent::Model {
                name: model.name,
                provider_id: model.provider_id,
                provider_label: model.provider_label,
                display_name: model.display_name,
                input_scopes: model.input_scopes,
                output_scopes: model.output_scopes,
                settings: serde_json::Value::Object(model.advanced_model_settings),
            }
        }
        dto::FileKind::ChatJsonl => {
            let chat = lettuce_transfer::parse_chat_jsonl(text(&file)?, now.get())
                .map_err(format_error)?;
            dto::ImportPreviewContent::Chat {
                title: chat
                    .metadata
                    .as_ref()
                    .and_then(|value| value.get("character_name"))
                    .and_then(serde_json::Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| stem(&file.name))
                    .unwrap_or("Imported Chat")
                    .into(),
                speakers: chat.speakers().into_iter().collect(),
                message_count: chat.messages.len() as u64,
                group: chat.is_group(),
            }
        }
        _ => {
            return Err(api_error(
                ApiErrorCode::Unsupported,
                "this file kind cannot be imported as an entity",
            ));
        }
    };
    Ok(dto::ImportPreview {
        kind: file.kind,
        name: file.name,
        digest: file.digest,
        content,
        images,
        warnings,
    })
}

fn cancelled(context: &ApiContext) -> Result<(), ApiError> {
    if context.shutdown_token().is_cancelled() {
        Err(api_error(
            ApiErrorCode::Cancelled,
            "the application is shutting down",
        ))
    } else {
        Ok(())
    }
}

async fn download_avatar(
    context: &ApiContext,
    avatar: &mut Option<String>,
    warnings: &mut Vec<dto::FileWarning>,
) -> Result<(), ApiError> {
    let Some(url) = avatar.as_ref().filter(|value| remote(value)).cloned() else {
        return Ok(());
    };
    let enabled = context
        .blocking(|context| {
            Ok(context
                .backend()
                .database()
                .load()
                .map_err(|e| api_error(ApiErrorCode::Unavailable, e.to_string()))?
                .settings
                .auto_download_character_card_avatars)
        })
        .await?;
    if !enabled {
        *avatar = None;
        warnings.push(warning(
            dto::FileWarningCode::RemoteAvatarNotDownloaded,
            "avatar",
        ));
        return Ok(());
    }
    let tls = context
        .blocking(|context| context.backend().tls_policy().map_err(internal))
        .await?;
    let download = async {
        let client = lettuce_network::ArtifactDownloadClient::with_tls(&tls).map_err(|_| ())?;
        client
            .download_avatar(&url, lettuce_media::MAX_MEDIA_BLOB_BYTES)
            .await
            .map_err(|_| ())
    };
    tokio::select! {
        biased;
        _ = context.shutdown_token().cancelled() => Err(api_error(ApiErrorCode::Cancelled,"avatar download was cancelled")),
        result = download => {
            *avatar = match result { Ok(bytes)=>Some(base64::engine::general_purpose::STANDARD.encode(bytes)),Err(())=>{warnings.push(warning(dto::FileWarningCode::ImageDownloadFailed,"avatar"));None} };
            Ok(())
        }
    }
}

fn stage_image(
    context: &ApiContext,
    data: Option<&str>,
    kind: lettuce_media::AssetKind,
    field: &str,
    uploads: &mut Vec<lettuce_media::IngestedMedia>,
    pins: &mut Vec<lettuce_media::MediaObjectPin>,
    warnings: &mut Vec<dto::FileWarning>,
) -> Option<lettuce_types::AssetId> {
    let data = data?;
    if remote(data) {
        warnings.push(warning(
            dto::FileWarningCode::RemoteAvatarNotDownloaded,
            field,
        ));
        return None;
    }
    let bytes = match base64::engine::general_purpose::STANDARD
        .decode(data.split_once(',').map_or(data, |(_, data)| data))
    {
        Ok(bytes) => bytes,
        Err(_) => {
            warnings.push(warning(dto::FileWarningCode::ImageDecodeFailed, field));
            return None;
        }
    };
    let hash = lettuce_types::ContentHash::parse(blake3::hash(&bytes).to_hex().to_string())
        .expect("BLAKE3 digest");
    let (_, pin) =
        lettuce_media::pin_media_objects(|| Ok::<_, std::convert::Infallible>(((), vec![hash])))
            .expect("pin");
    let Some(media) = context.media() else {
        warnings.push(warning(dto::FileWarningCode::ImageStoreFailed, field));
        return None;
    };
    let upload = media.ingest_with_commit(
        bytes.as_slice(),
        lettuce_media::IngestRequest::new(
            kind,
            lettuce_media::AssetOrigin::Import,
            lettuce_media::RetentionClass::Persistent,
            lettuce_media::AssetProvenanceV1 {
                imported_format: Some("lettuceai-entity-file".into()),
                ..Default::default()
            },
        ),
        |upload| upload,
    );
    match upload {
        Ok(upload) => {
            let id = upload.asset.id;
            uploads.push(upload);
            pins.push(pin);
            Some(id)
        }
        Err(_) => {
            warnings.push(warning(dto::FileWarningCode::ImageStoreFailed, field));
            None
        }
    }
}

pub async fn import_commit(
    context: &ApiContext,
    request: dto::ImportCommitRequest,
) -> Result<dto::ImportCommitResult, ApiError> {
    if request.client_operation_id.trim().is_empty() {
        return Err(invalid_field(
            "client_operation_id",
            "operation ID is required",
        ));
    }
    let digest = super::jobs::local::digest(&request)?;
    let request_copy = request.clone();
    let replay_digest = digest.clone();
    let replay = context
        .blocking(move |context| {
            let receipt = context
                .backend()
                .database()
                .lookup_api_operation("import_commit", &request_copy.client_operation_id)
                .map_err(|error| Failure::from(error).0)?;
            receipt
                .map(|receipt| {
                    if receipt.request_digest != replay_digest {
                        return Err(api_error(
                            ApiErrorCode::Conflict,
                            "operation ID was used with another request",
                        ));
                    }
                    serde_json::from_value(receipt.result).map_err(internal)
                })
                .transpose()
        })
        .await?;
    if let Some(result) = replay {
        return Ok(result);
    }
    let _work = context
        .maintenance()
        .try_work()
        .ok_or_else(|| api_error(ApiErrorCode::Busy, "database maintenance is active"))?;
    cancelled(context)?;
    let source = request.source.clone();
    let file = context
        .blocking(move |context| read(context, &source))
        .await?;
    if file.digest != request.digest {
        return Err(api_error(
            ApiErrorCode::Conflict,
            "the file changed after preview",
        ));
    }
    let mut warnings = Vec::new();
    enum Prepared {
        Character(Box<lettuce_transfer::CharacterPackage>),
        Persona(lettuce_transfer::PersonaPackage),
        Text,
    }
    let prepared = match file.kind {
        dto::FileKind::CharacterCard => {
            let mut package = character(&file, context.now())?;
            download_avatar(context, &mut package.avatar_data, &mut warnings).await?;
            Prepared::Character(Box::new(package))
        }
        dto::FileKind::PersonaFile => {
            let mut package = persona(&file, context.now())?;
            download_avatar(context, &mut package.avatar_data, &mut warnings).await?;
            Prepared::Persona(package)
        }
        _ => Prepared::Text,
    };
    if file.kind == dto::FileKind::ChatJsonl {
        return super::entity_import_jobs::admit(context, request, digest, file).await;
    }
    context
        .blocking(move |context| {
            cancelled(context)?;
            let now = context.now();
            let database = context.backend().database();
            enum WritePlan {
                Character(
                    Box<lettuce_transfer::CharacterFilePlan>,
                    lettuce_transfer::CharacterFileAssets,
                ),
                Persona(lettuce_transfer::PersonaFileImport),
                Lorebook(
                    lettuce_context::LorebookMetadataDraft,
                    Vec<lettuce_context::LorebookEntryDraft>,
                ),
                Prompt(PromptMetadataDraft, Vec<lettuce_context::PromptEntryDraft>),
                Starter(
                    lettuce_types::CharacterId,
                    lettuce_types::Revision,
                    lettuce_characters::ConversationStarter,
                ),
                Model(Box<lettuce_models::ModelProfile>),
            }
            let mut uploads = Vec::new();
            let mut pins = Vec::new();
            let plan = match (file.kind, prepared) {
                (dto::FileKind::CharacterCard, Prepared::Character(package)) => {
                    let media = context.media().ok_or_else(|| {
                        api_error(ApiErrorCode::Unavailable, "no media store is open")
                    })?;
                    let plan = context
                        .backend()
                        .character_files(media)
                        .plan(&package, now)
                        .map_err(IntoApiError::into_api_error)?;
                    let assets = lettuce_transfer::CharacterFileAssets {
                        avatar: stage_image(
                            context,
                            plan.avatar_data.as_deref(),
                            lettuce_media::AssetKind::AvatarOriginal,
                            "avatar",
                            &mut uploads,
                            &mut pins,
                            &mut warnings,
                        ),
                        background: stage_image(
                            context,
                            plan.background_image_data.as_deref(),
                            lettuce_media::AssetKind::BackgroundImage,
                            "background",
                            &mut uploads,
                            &mut pins,
                            &mut warnings,
                        ),
                        scene_backgrounds: plan
                            .scene_backgrounds
                            .iter()
                            .filter_map(|(scene, data)| {
                                stage_image(
                                    context,
                                    Some(data),
                                    lettuce_media::AssetKind::BackgroundImage,
                                    &format!("scenes:{scene}.background"),
                                    &mut uploads,
                                    &mut pins,
                                    &mut warnings,
                                )
                                .map(|id| (*scene, id))
                            })
                            .collect(),
                    };
                    WritePlan::Character(Box::new(plan), assets)
                }
                (dto::FileKind::PersonaFile, Prepared::Persona(package)) => {
                    let media = context.media().ok_or_else(|| {
                        api_error(ApiErrorCode::Unavailable, "no media store is open")
                    })?;
                    let mut without_image = package.clone();
                    without_image.avatar_data = None;
                    let (mut import, skips, images) = context
                        .backend()
                        .persona_files(media)
                        .prepare(&without_image, now)
                        .map_err(IntoApiError::into_api_error)?;
                    warnings.extend(images);
                    warnings.extend(
                        skips.into_iter().map(|skip| {
                            warning(dto::FileWarningCode::FieldSkipped, skip.source_key)
                        }),
                    );
                    if let Some(id) = stage_image(
                        context,
                        package.avatar_data.as_deref(),
                        lettuce_media::AssetKind::AvatarOriginal,
                        "avatar",
                        &mut uploads,
                        &mut pins,
                        &mut warnings,
                    ) {
                        import
                            .persona
                            .media
                            .links
                            .push(lettuce_characters::PersonaMediaLink {
                                asset_id: id,
                                slot: lettuce_characters::PersonaMediaSlot::Avatar,
                                ordinal: 0,
                            });
                    }
                    WritePlan::Persona(import)
                }
                (dto::FileKind::Lorebook, _) => {
                    let (metadata, entries) =
                        crate::files::lorebook_files::prepare_import(text(&file)?)
                            .map_err(format_error)?;
                    WritePlan::Lorebook(metadata, entries)
                }
                (dto::FileKind::PromptPreset, _) => {
                    let imported = lettuce_transfer::parse_prompt_import(
                        text(&file)?,
                        stem(&file.name),
                        "Imported entry",
                        "Imported preset",
                    )
                    .map_err(format_error)?;
                    if imported.purpose_defaulted {
                        warnings.push(warning(dto::FileWarningCode::FieldConverted, "purpose"));
                    }
                    WritePlan::Prompt(
                        PromptMetadataDraft {
                            name: imported.name,
                            purpose: imported.purpose,
                            condense: imported.condense,
                            behavior_version: PromptBehaviorVersion::LegacyV1,
                        },
                        imported.entries,
                    )
                }
                (dto::FileKind::ChatTemplate, _) => {
                    let id = parse_id(
                        request.options.character_id.as_deref().ok_or_else(|| {
                            invalid_field("options.character_id", "target character is required")
                        })?,
                        "options.character_id",
                    )?;
                    let details = lettuce_characters::CharacterRepository::get(database, id)
                        .map_err(IntoApiError::into_api_error)?
                        .ok_or_else(|| {
                            api_error(ApiErrorCode::NotFound, "character was not found")
                        })?;
                    if details.character.status != lettuce_characters::LifecycleStatus::Active {
                        return Err(invalid_field(
                            "options.character_id",
                            "character is archived",
                        ));
                    }
                    let imported = lettuce_transfer::parse_chat_template_import(text(&file)?)
                        .map_err(format_error)?;
                    let (revision, starter) = context
                        .backend()
                        .chat_template_files()
                        .prepare(id, text(&file)?, now)
                        .map_err(IntoApiError::into_api_error)?;
                    if imported.scene_id.is_some() && starter.scene_id.is_none() {
                        warnings.push(warning(dto::FileWarningCode::FieldSkipped, "scene_id"));
                    }
                    if imported.prompt_template_id.is_some() && starter.prompt_id.is_none() {
                        warnings.push(warning(
                            dto::FileWarningCode::FieldSkipped,
                            "prompt_template_id",
                        ));
                    }
                    if let (Some(ids), lettuce_characters::Selection::Explicit(kept)) =
                        (&imported.lorebook_ids_override, &starter.lorebooks)
                    {
                        for id in ids {
                            if !kept.iter().any(|kept| kept.to_string() == *id) {
                                warnings.push(warning(
                                    dto::FileWarningCode::FieldSkipped,
                                    format!("lorebook_ids_override:{id}"),
                                ));
                            }
                        }
                    }
                    WritePlan::Starter(id, revision, starter)
                }
                (dto::FileKind::ModelFile, _) => {
                    let imported = context
                        .backend()
                        .model_files()
                        .prepare(
                            lettuce_transfer::parse_model_import(text(&file)?)
                                .map_err(format_error)?,
                            now,
                        )
                        .map_err(IntoApiError::into_api_error)?;
                    warnings.extend(
                        imported
                            .lossy_fields
                            .into_iter()
                            .map(|field| warning(dto::FileWarningCode::FieldConverted, field)),
                    );
                    warnings.extend(
                        imported
                            .unknown_fields
                            .into_iter()
                            .map(|field| warning(dto::FileWarningCode::FieldSkipped, field)),
                    );
                    WritePlan::Model(Box::new(imported.profile))
                }
                _ => {
                    return Err(api_error(
                        ApiErrorCode::Unsupported,
                        "this file kind cannot be imported as an entity",
                    ));
                }
            };
            database
                .commit_api_operation(
                    "import_commit",
                    &request.client_operation_id,
                    &digest,
                    now,
                    |scope| {
                        cancelled(context).map_err(Failure)?;
                        for upload in uploads {
                            scope
                                .install_library_upload(upload)
                                .map_err(|e| Failure(e.into_api_error()))?;
                        }
                        let id = match plan {
                            WritePlan::Character(plan, assets) => {
                                let references = scope
                                    .character_file_references()
                                    .map_err(|e| Failure(internal(e)))?;
                                let import = plan
                                    .import(&references, &assets, now.get(), uuid::Uuid::new_v4)
                                    .map_err(|e| Failure(format_error(e)))?;
                                warnings.extend(
                                    plan.skipped.into_iter().chain(import.skipped.clone()).map(
                                        |skip| {
                                            warning(
                                                dto::FileWarningCode::FieldSkipped,
                                                skip.source_key,
                                            )
                                        },
                                    ),
                                );
                                warnings.extend(
                                    plan.notices.into_iter().chain(import.notices.clone()).map(
                                        |notice| {
                                            warning(
                                                dto::FileWarningCode::FieldConverted,
                                                notice.field,
                                            )
                                        },
                                    ),
                                );
                                scope
                                    .import_character_file(&import)
                                    .map_err(|e| Failure(internal(e)))?
                                    .character
                                    .id
                                    .to_string()
                            }
                            WritePlan::Persona(import) => scope
                                .import_persona_file(&import)
                                .map_err(|e| Failure(e.into_api_error()))?
                                .id
                                .to_string(),
                            WritePlan::Lorebook(metadata, entries) => scope
                                .create_lorebook(metadata, entries, now)
                                .map_err(|e| Failure(e.into_api_error()))?
                                .book
                                .id
                                .to_string(),
                            WritePlan::Prompt(metadata, entries) => scope
                                .create_prompt(metadata, entries, now)
                                .map_err(|e| Failure(e.into_api_error()))?
                                .id
                                .to_string(),
                            WritePlan::Starter(id, revision, starter) => scope
                                .import_starter(id, revision, starter, now)
                                .map_err(|e| Failure(e.into_api_error()))?
                                .id
                                .to_string(),
                            WritePlan::Model(profile) => scope
                                .save_model_profile(*profile, None, true)
                                .map_err(|e| Failure(super::model_profiles::model_error(e)))?
                                .id
                                .to_string(),
                        };
                        cancelled(context).map_err(Failure)?;
                        Ok::<_, Failure>(dto::ImportCommitResult::Imported {
                            file_kind: file.kind,
                            id,
                            warnings,
                        })
                    },
                )
                .map_err(|e| e.0)
        })
        .await
}

fn publish(
    context: &ApiContext,
    target: dto::FileTarget,
    name: String,
    content: String,
    warnings: Vec<dto::FileWarning>,
) -> Result<dto::EntityExportResult, ApiError> {
    cancelled(context)?;
    if target.uri.trim().is_empty() {
        return Err(invalid_field("target", "target is empty"));
    }
    let _work = context
        .maintenance()
        .try_work()
        .ok_or_else(|| api_error(ApiErrorCode::Busy, "database maintenance is active"))?;
    let protection = super::files::FileExportProtection::new(context)?;
    let protected = |uri: &str, file: Option<&std::fs::File>| protection.protects(uri, file);
    let files = context.files();
    let temporary_location = files
        .backup_temporary_location(
            Some(&target.uri),
            &format!("entity-{}", uuid::Uuid::new_v4()),
        )
        .map_err(IntoApiError::into_api_error)?;
    let (temporary, mut writer) = files
        .create_backup_temporary(&temporary_location, &protected)
        .map_err(IntoApiError::into_api_error)?;
    let result = (|| {
        writer
            .write_all(content.as_bytes())
            .map_err(|_| super::files::FileAccessError::Io.into_api_error())?;
        writer
            .flush()
            .map_err(|_| super::files::FileAccessError::Io.into_api_error())?;
        drop(writer);
        files
            .sync_backup(&temporary)
            .map_err(IntoApiError::into_api_error)?;
        cancelled(context)?;
        files
            .publish_backup(&temporary, Some(&target.uri), &name, &protected)
            .map_err(IntoApiError::into_api_error)?;
        Ok(dto::EntityExportResult {
            suggested_name: name,
            warnings,
        })
    })();
    if result.is_err() {
        match files.remove(&temporary) {
            Ok(()) | Err(super::files::FileAccessError::NotFound) => {}
            Err(error) => return Err(error.into_api_error()),
        }
    }
    result
}

pub async fn character_export(
    context: &ApiContext,
    request: dto::CharacterExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let id = parse_id(&request.id, "id")?;
    context
        .blocking(move |context| {
            let media = context
                .media()
                .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no media store is open"))?;
            let details =
                lettuce_characters::CharacterRepository::get(context.backend().database(), id)
                    .map_err(IntoApiError::into_api_error)?
                    .ok_or_else(|| api_error(ApiErrorCode::NotFound, "character was not found"))?;
            let (format, extension) = match request.format {
                dto::CharacterExportFormat::Uec => {
                    (lettuce_transfer::CharacterFileFormat::Uec, "uec")
                }
                dto::CharacterExportFormat::V3 => {
                    (lettuce_transfer::CharacterFileFormat::CharaCardV3, "json")
                }
                dto::CharacterExportFormat::V2 => {
                    (lettuce_transfer::CharacterFileFormat::CharaCardV2, "json")
                }
            };
            let now = context.now();
            let (content, warnings) = context
                .backend()
                .character_files(media)
                .export_with_warnings(id, format, now)
                .map_err(IntoApiError::into_api_error)?;
            publish(
                context,
                request.target,
                format!(
                    "character_{}_{}.{extension}",
                    crate::files::prompt_files::export_name(&details.character.profile.name),
                    crate::files::prompt_files::export_date(now)
                ),
                content,
                warnings,
            )
        })
        .await
}
pub async fn persona_export(
    context: &ApiContext,
    request: dto::EntityExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let id = parse_id(&request.id, "id")?;
    context
        .blocking(move |context| {
            let media = context
                .media()
                .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no media store is open"))?;
            let persona =
                lettuce_characters::PersonaRepository::get(context.backend().database(), id)
                    .map_err(IntoApiError::into_api_error)?
                    .ok_or_else(|| api_error(ApiErrorCode::NotFound, "persona was not found"))?;
            let (content, warnings) = context
                .backend()
                .persona_files(media)
                .export_with_warnings(id)
                .map_err(IntoApiError::into_api_error)?;
            publish(
                context,
                request.target,
                format!(
                    "persona_{}_{}.uec",
                    crate::files::prompt_files::export_name(&persona.title),
                    crate::files::prompt_files::export_date(context.now())
                ),
                content,
                warnings,
            )
        })
        .await
}
pub async fn lorebook_export(
    context: &ApiContext,
    request: dto::LorebookExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let id = parse_id(&request.id, "id")?;
    context
        .blocking(move |context| {
            let book = LorebookRepository::get(context.backend().database(), id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "lorebook was not found"))?;
            let files = context.backend().lorebook_files();
            let (content, extension) = match request.format {
                dto::LorebookExportFormat::WorldInfo => (files.export_world_info(id), "json"),
                dto::LorebookExportFormat::Usc => (files.export_usc(id), "usc"),
            };
            publish(
                context,
                request.target,
                format!(
                    "lorebook_{}_{}.{extension}",
                    crate::files::prompt_files::export_name(&book.book.name),
                    crate::files::prompt_files::export_date(context.now())
                ),
                content.map_err(IntoApiError::into_api_error)?,
                Vec::new(),
            )
        })
        .await
}
pub async fn prompt_export(
    context: &ApiContext,
    request: dto::EntityExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let id = parse_id(&request.id, "id")?;
    context
        .blocking(move |context| {
            let file = context
                .backend()
                .prompt_files()
                .export(id, crate::PromptFileFormat::Usc, context.now())
                .map_err(IntoApiError::into_api_error)?;
            publish(
                context,
                request.target,
                file.filename,
                file.content,
                Vec::new(),
            )
        })
        .await
}
pub async fn chat_template_export(
    context: &ApiContext,
    request: dto::ChatTemplateExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let character_id = parse_id(&request.character_id, "character_id")?;
    let starter_id = parse_id(&request.starter_id, "starter_id")?;
    context
        .blocking(move |context| {
            let format = match request.format {
                dto::EntityExportFormat::Json => crate::ChatTemplateFileFormat::Json,
                dto::EntityExportFormat::Usc => crate::ChatTemplateFileFormat::Usc,
            };
            let file = context
                .backend()
                .chat_template_files()
                .export(character_id, starter_id, format, context.now())
                .map_err(IntoApiError::into_api_error)?;
            publish(
                context,
                request.target,
                file.filename,
                file.content,
                Vec::new(),
            )
        })
        .await
}
pub async fn model_export(
    context: &ApiContext,
    request: dto::ModelExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let id = parse_id(&request.id, "id")?;
    context
        .blocking(move |context| {
            let format = match request.format {
                dto::EntityExportFormat::Json => crate::ModelFileFormat::Json,
                dto::EntityExportFormat::Usc => crate::ModelFileFormat::Usc,
            };
            let file = context
                .backend()
                .model_files()
                .export(id, format, context.now())
                .map_err(IntoApiError::into_api_error)?;
            publish(
                context,
                request.target,
                file.filename,
                file.content,
                Vec::new(),
            )
        })
        .await
}
pub async fn chat_export(
    context: &ApiContext,
    request: dto::ChatExportRequest,
) -> Result<dto::EntityExportResult, ApiError> {
    let id = parse_id(&request.conversation_id, "conversation_id")?;
    context
        .blocking(move |context| {
            let file = context
                .backend()
                .chat_files()
                .export(id, context.now())
                .map_err(IntoApiError::into_api_error)?;
            publish(
                context,
                request.target,
                file.filename,
                file.content,
                Vec::new(),
            )
        })
        .await
}
