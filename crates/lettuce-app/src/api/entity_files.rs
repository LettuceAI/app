pub(super) mod character3;

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

pub(super) fn read_for_commit(
    context: &ApiContext,
    source: &dto::FileSource,
) -> Result<ImportFile, ApiError> {
    read_file(context, source, &mut |_| Ok(()), false)
}

pub(super) fn read(context: &ApiContext, source: &dto::FileSource) -> Result<ImportFile, ApiError> {
    read_with_progress(context, source, &mut |_| Ok(()))
}

pub(super) fn read_with_progress(
    context: &ApiContext,
    source: &dto::FileSource,
    progress: &mut dyn FnMut(u64) -> Result<(), ApiError>,
) -> Result<ImportFile, ApiError> {
    read_file(context, source, progress, true)
}

fn read_file(
    context: &ApiContext,
    source: &dto::FileSource,
    progress: &mut dyn FnMut(u64) -> Result<(), ApiError>,
    retain_chat: bool,
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
    if !matches!(
        kind,
        dto::FileKind::CharacterCard
            | dto::FileKind::PersonaFile
            | dto::FileKind::Lorebook
            | dto::FileKind::PromptPreset
            | dto::FileKind::ChatTemplate
            | dto::FileKind::ModelFile
            | dto::FileKind::ChatJsonl
    ) {
        return Err(api_error(
            ApiErrorCode::Unsupported,
            "this file kind cannot be imported as an entity",
        ));
    }
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|_| super::files::FileAccessError::Io.into_api_error())?;
    let mut bytes = Vec::new();
    let mut buffer = vec![0; if retain_chat { 1024 * 1024 } else { 64 * 1024 }];
    let mut hasher = blake3::Hasher::new();
    let mut completed = 0;
    loop {
        cancelled(context)?;
        progress(completed)?;
        let length = reader
            .read(&mut buffer)
            .map_err(|_| super::files::FileAccessError::Io.into_api_error())?;
        if length == 0 {
            break;
        }
        hasher.update(&buffer[..length]);
        completed += length as u64;
        if retain_chat || kind != dto::FileKind::ChatJsonl {
            bytes.extend_from_slice(&buffer[..length]);
        }
    }
    progress(completed)?;
    let digest = hasher.finalize().to_hex().to_string();
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

fn uec3_error(error: lettuce_transfer::uec3::Error) -> ApiError {
    match error {
        lettuce_transfer::uec3::Error::Io(_) => {
            api_error(ApiErrorCode::Unavailable, error.to_string())
        }
        lettuce_transfer::uec3::Error::UnsupportedVersion(_) => {
            api_error(ApiErrorCode::Unsupported, error.to_string())
        }
        _ => format_error(error),
    }
}

fn is_uec3(file: &ImportFile) -> Result<bool, ApiError> {
    if file.bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(false);
    }
    lettuce_transfer::uec3::is_document(text(file)?).map_err(uec3_error)
}

fn uec3_document(
    file: &ImportFile,
    on_asset: impl FnMut(&str, &str, &[u8]) -> Result<(), lettuce_transfer::uec3::Error>,
) -> Result<Option<lettuce_transfer::uec3::ReadResult>, ApiError> {
    if !is_uec3(file)? {
        return Ok(None);
    }
    let card = lettuce_transfer::uec3::read(text(file)?, on_asset).map_err(uec3_error)?;
    Ok(Some(card))
}

fn prepare_uec3_model(
    context: &ApiContext,
    model: &lettuce_transfer::uec3::ModelProfile,
) -> Result<lettuce_models::ModelProfile, ApiError> {
    let account = context
        .backend()
        .model_files()
        .account(&model.provider.kind, &model.provider.label)
        .map_err(IntoApiError::into_api_error)?;
    let mut model = model.clone();
    if matches!(
        account.protocol,
        lettuce_models::ProviderProtocol::LlamaCpp
            | lettuce_models::ProviderProtocol::StableDiffusion
    ) {
        model.external_id = lettuce_models::UNPICKED_LOCAL_MODEL_FILE.into();
    }
    model
        .into_stored(account.id, context.now())
        .map_err(uec3_error)
}

fn prepare_uec3_prompt(
    context: &ApiContext,
    prompt: &lettuce_transfer::uec3::Prompt,
    warnings: &mut Vec<dto::FileWarning>,
) -> Result<lettuce_context::PromptDocument, ApiError> {
    use crate::generation::built_in_prompts::BuiltInPromptId;
    let source = match prompt.built_in.as_deref() {
        Some(key) => match BuiltInPromptId::from_key_or_alias(key) {
            Some(id) => Some(
                lettuce_context::PromptRepository::get(
                    context.backend().database(),
                    context.backend().built_in_prompt_ids().get(id),
                )
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| {
                    api_error(
                        ApiErrorCode::NotFound,
                        "source built-in prompt was not found",
                    )
                })?,
            ),
            None => {
                warnings.push(warning(
                    dto::FileWarningCode::FieldConverted,
                    format!("builtIn:{key}"),
                ));
                None
            }
        },
        None => None,
    };
    prompt
        .clone()
        .into_document(source.as_ref(), context.now())
        .map_err(uec3_error)
}

fn export_uec3_entity(
    context: &ApiContext,
    entity: lettuce_transfer::uec3::Entity,
    created_at: TimestampMillis,
    updated_at: TimestampMillis,
) -> Uec3Export {
    Uec3Export::new(
        context,
        entity,
        (created_at, updated_at),
        Vec::new(),
        Default::default(),
        Default::default(),
    )
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
    let uec3 = if file.kind == dto::FileKind::ChatJsonl {
        None
    } else {
        uec3_document(&file, |_, _, _| Ok(()))?
    };
    if let Some(card) = &uec3 {
        warnings.extend(
            card.warnings
                .iter()
                .map(|field| warning(dto::FileWarningCode::FieldSkipped, field)),
        );
    }
    let uec3_parts = if let Some(card) = &uec3 {
        let assets = card
            .document
            .assets
            .keys()
            .map(|key| (key.clone(), lettuce_types::AssetId::new()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let books = uec3_books(context, &card.document, &assets)?;
        let prompts = uec3_prompts(context, &card.document, &mut warnings)?;
        Some((assets, books, prompts))
    } else {
        None
    };
    let mut uec3_preview = if let Some(card) = &uec3 {
        let mut dependencies = Vec::new();
        for dependency in &card.document.dependencies {
            let (kind, name) = match &dependency.entity {
                lettuce_transfer::uec3::Entity::Lorebook(book) => {
                    (dto::FileKind::Lorebook, book.name.clone())
                }
                lettuce_transfer::uec3::Entity::Prompt(prompt) => {
                    (dto::FileKind::PromptPreset, prompt.name.clone())
                }
                _ => return Err(format_error("invalid dependency kind")),
            };
            dependencies.push(dto::ImportUec3Dependency {
                key: dependency.key.clone(),
                kind,
                name,
            });
        }
        Some(dto::ImportUec3Preview {
            dependencies,
            include_learned: card.document.state.is_some(),
            character: None,
            persona: None,
            prompt: None,
            lorebook_bindings: Vec::new(),
        })
    } else {
        None
    };
    let content = match file.kind {
        dto::FileKind::CharacterCard if uec3.is_some() => {
            let card = uec3
                .as_ref()
                .ok_or_else(|| internal("missing UEC document"))?;
            let (assets, books, prompts) = uec3_parts
                .as_ref()
                .ok_or_else(|| internal("missing UEC parts"))?;
            let mut import = character3::prepare(
                context,
                &card.document,
                character3::ImportAssets {
                    ids: assets,
                    uploads: &mut Vec::new(),
                    preview: true,
                },
                &mut warnings,
                now,
                books,
                prompts,
            )?;
            import.lorebooks = books.values().cloned().collect();
            let details = lettuce_characters::CharacterDetails {
                character: import.character.character,
                scenes: import.character.scenes,
                variants: import.character.variants,
                starters: import.character.starters,
            };
            if let Some(preview) = &mut uec3_preview {
                preview.character = Some(Box::new(
                    super::library_characters::view(context, details.clone())?.draft,
                ));
                let lettuce_transfer::uec3::Entity::Character(character) = &card.document.entity
                else {
                    return Err(format_error("expected character"));
                };
                preview.lorebook_bindings = character
                    .behavior
                    .lorebooks
                    .iter()
                    .map(|binding| dto::ImportUec3LorebookBinding {
                        book: binding.book.clone(),
                        enabled: binding.enabled,
                    })
                    .collect();
            }
            let source = lettuce_transfer::CharacterExportSource {
                details,
                lorebooks: import.lorebooks,
                scheduled_notes: import.scheduled_notes,
                companion_memory: import.companion_memory,
                avatar_data: None,
                background_image_data: None,
                scene_backgrounds: Default::default(),
            };
            let package = lettuce_transfer::character_package(&source, now.get());
            dto::ImportPreviewContent::Character {
                content: Box::new(convert(&package.character)?),
            }
        }
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
        dto::FileKind::PersonaFile if uec3.is_some() => {
            let card = uec3
                .as_ref()
                .ok_or_else(|| internal("missing UEC document"))?;
            let (assets, books, _) = uec3_parts
                .as_ref()
                .ok_or_else(|| internal("missing UEC parts"))?;
            let import = prepare_uec3_persona(
                context,
                &card.document,
                assets,
                &mut Vec::new(),
                &mut warnings,
                true,
                books,
            )?;
            if let Some(preview) = &mut uec3_preview {
                preview.persona =
                    Some(super::library_personas::view(context, import.persona.clone()).draft);
            }
            let lettuce_transfer::uec3::Entity::Persona(persona) = &card.document.entity else {
                return Err(format_error("expected persona entity"));
            };
            for key in card.document.assets.keys() {
                images.push(dto::ImportImagePreview {
                    field: format!("asset:{key}"),
                    remote_url: None,
                });
            }
            if let Some(preview) = &mut uec3_preview {
                preview.lorebook_bindings = persona
                    .behavior
                    .lorebooks
                    .iter()
                    .map(|binding| dto::ImportUec3LorebookBinding {
                        book: binding.book.clone(),
                        enabled: binding.enabled,
                    })
                    .collect();
            }
            dto::ImportPreviewContent::Persona {
                content: dto::ImportPersonaContent {
                    title: import.persona.title,
                    description: import.persona.description,
                    nickname: import.persona.nickname,
                    is_default: None,
                    avatar_crop: convert(&import.persona.avatar_crop)?,
                    active_lorebook_ids: persona
                        .behavior
                        .lorebooks
                        .iter()
                        .map(|binding| binding.book.clone())
                        .collect(),
                },
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
        dto::FileKind::Lorebook if uec3.is_some() => {
            let card = uec3
                .as_ref()
                .ok_or_else(|| internal("missing UEC document"))?;
            let lettuce_transfer::uec3::Entity::Lorebook(book) = &card.document.entity else {
                return Err(format_error("expected lorebook entity"));
            };
            book.drafts(None).map_err(uec3_error)?;
            if let Some(icon) = &book.icon {
                let key = icon
                    .strip_prefix("asset:")
                    .ok_or_else(|| format_error("invalid icon reference"))?;
                if !card.document.assets.contains_key(key) {
                    return Err(format_error("missing icon asset"));
                }
                images.push(dto::ImportImagePreview {
                    field: "icon".into(),
                    remote_url: None,
                });
            }
            dto::ImportPreviewContent::Lorebook {
                name: book.name.clone(),
                keyword_detection_mode: match book.detection {
                    lettuce_transfer::uec3::Detection::RecentMessageWindow => {
                        dto::ImportKeywordDetection::RecentMessageWindow
                    }
                    lettuce_transfer::uec3::Detection::LatestUserMessage => {
                        dto::ImportKeywordDetection::LatestUserMessage
                    }
                },
                entries: book
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        Ok(dto::ImportLorebookEntry {
                            title: entry.title.clone(),
                            enabled: entry.enabled,
                            always_active: entry.always_active,
                            keywords: entry.keywords.clone(),
                            case_sensitive: entry.case_sensitive,
                            keyword_match_mode: match entry.match_mode {
                                lettuce_context::KeywordMatchMode::Literal => {
                                    dto::LorebookKeywordMode::Literal
                                }
                                lettuce_context::KeywordMatchMode::Regex => {
                                    dto::LorebookKeywordMode::Regex
                                }
                            },
                            content: entry.content.clone(),
                            priority: entry.priority,
                            display_order: i32::try_from(index).map_err(internal)?,
                        })
                    })
                    .collect::<Result<Vec<_>, ApiError>>()?,
            }
        }
        dto::FileKind::Lorebook => {
            let book =
                lettuce_transfer::parse_lorebook_import(text(&file)?).map_err(format_error)?;
            let entries = book
                .entries
                .into_iter()
                .map(|entry| dto::ImportLorebookEntry {
                    title: entry.title,
                    enabled: entry.enabled,
                    always_active: entry.always_active,
                    keywords: entry.keywords,
                    case_sensitive: entry.case_sensitive,
                    keyword_match_mode: dto::LorebookKeywordMode::Literal,
                    content: entry.content,
                    priority: entry.priority,
                    display_order: entry.display_order,
                })
                .collect();
            dto::ImportPreviewContent::Lorebook {
                name: book.name,
                keyword_detection_mode: convert(&book.keyword_detection_mode)?,
                entries,
            }
        }
        dto::FileKind::PromptPreset if uec3.is_some() => {
            let card = uec3
                .as_ref()
                .ok_or_else(|| internal("missing UEC document"))?;
            let lettuce_transfer::uec3::Entity::Prompt(prompt) = &card.document.entity else {
                return Err(format_error("expected prompt entity"));
            };
            let document = prepare_uec3_prompt(context, prompt, &mut warnings)?;
            if let Some(preview) = &mut uec3_preview {
                preview.prompt = Some(super::prompts::view(context, &document)?);
            }
            let entries = document
                .entries
                .into_iter()
                .map(lettuce_context::PromptEntryDraft::from)
                .collect::<Vec<_>>();
            dto::ImportPreviewContent::Prompt {
                name: prompt.name.clone(),
                purpose: super::prompts::kind(document.purpose),
                entries: convert(&entries)?,
                condense: prompt.condense,
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
        dto::FileKind::ChatTemplate if uec3.is_some() => {
            let card = uec3
                .as_ref()
                .ok_or_else(|| internal("missing UEC document"))?;
            let lettuce_transfer::uec3::Entity::ChatTemplate(template) = &card.document.entity
            else {
                return Err(format_error("expected chat template entity"));
            };
            let (_, _, prompts) = uec3_parts
                .as_ref()
                .ok_or_else(|| internal("missing UEC parts"))?;
            if let Some(reference) = &template.prompt {
                uec3_prompt_reference(context, reference, prompts)?;
            }
            dto::ImportPreviewContent::ChatTemplate {
                name: template.name.clone(),
                messages: template
                    .messages
                    .iter()
                    .map(|message| dto::ImportTemplateMessage {
                        role: match message.role {
                            lettuce_characters::StarterRole::User => "user",
                            lettuce_characters::StarterRole::Assistant => "assistant",
                        }
                        .into(),
                        content: message.text.clone(),
                    })
                    .collect(),
                scene_id: None,
                prompt_template_id: template.prompt.clone(),
                lorebook_ids_override: match &template.lorebooks {
                    lettuce_transfer::uec3::LorebookSelection::Keyword(
                        lettuce_transfer::uec3::SelectionKeyword::Inherit,
                    ) => None,
                    lettuce_transfer::uec3::LorebookSelection::Keyword(
                        lettuce_transfer::uec3::SelectionKeyword::Disabled,
                    ) => Some(Vec::new()),
                    lettuce_transfer::uec3::LorebookSelection::Explicit(books) => {
                        Some(books.clone())
                    }
                },
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
        dto::FileKind::ModelFile if uec3.is_some() => {
            let card = uec3
                .as_ref()
                .ok_or_else(|| internal("missing UEC document"))?;
            let lettuce_transfer::uec3::Entity::ModelProfile(model) = &card.document.entity else {
                return Err(format_error("expected model profile entity"));
            };
            let profile = prepare_uec3_model(context, model)?;
            let scopes = |modalities: &lettuce_models::ModalityCapabilities| {
                [
                    ("text", modalities.text),
                    ("image", modalities.image),
                    ("audio", modalities.audio),
                ]
                .into_iter()
                .filter(|(_, status)| *status == lettuce_models::CapabilityStatus::Supported)
                .map(|(name, _)| name.to_owned())
                .collect()
            };
            dto::ImportPreviewContent::Model {
                model_kind: convert(&model.kind)?,
                model_file_required: profile.external_model_id
                    == lettuce_models::UNPICKED_LOCAL_MODEL_FILE,
                name: model.external_id.clone(),
                provider_id: model.provider.kind.clone(),
                provider_label: model.provider.label.clone(),
                display_name: model.display_name.clone(),
                input_scopes: scopes(&model.capabilities.input_modalities),
                output_scopes: scopes(&model.capabilities.output_modalities),
                settings: serde_json::to_value(profile.config).map_err(internal)?,
            }
        }
        dto::FileKind::ModelFile => {
            let model = lettuce_transfer::parse_model_import(text(&file)?).map_err(format_error)?;
            dto::ImportPreviewContent::Model {
                model_kind: dto::ModelKindContract::Chat,
                model_file_required: false,
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
        uec3: uec3_preview,
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
        lettuce_network::ArtifactDownloadClient::download_avatar(
            &tls,
            &url,
            lettuce_media::MAX_MEDIA_BLOB_BYTES,
            std::time::Duration::from_secs(180),
        )
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
    stage_bytes(context, &bytes, kind, field, uploads, pins, warnings)
}

fn stage_bytes(
    context: &ApiContext,
    bytes: &[u8],
    kind: lettuce_media::AssetKind,
    field: &str,
    uploads: &mut Vec<lettuce_media::IngestedMedia>,
    pins: &mut Vec<lettuce_media::MediaObjectPin>,
    warnings: &mut Vec<dto::FileWarning>,
) -> Option<lettuce_types::AssetId> {
    let hash = lettuce_types::ContentHash::parse(blake3::hash(bytes).to_hex().to_string())
        .expect("BLAKE3 digest");
    let (_, pin) =
        lettuce_media::pin_media_objects(|| Ok::<_, std::convert::Infallible>(((), vec![hash])))
            .expect("pin");
    let Some(media) = context.media() else {
        warnings.push(warning(dto::FileWarningCode::ImageStoreFailed, field));
        return None;
    };
    let upload = media.ingest_with_commit(
        bytes,
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
        .blocking(move |context| read_for_commit(context, &source))
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
        dto::FileKind::CharacterCard if !is_uec3(&file)? => {
            let mut package = character(&file, context.now())?;
            download_avatar(context, &mut package.avatar_data, &mut warnings).await?;
            Prepared::Character(Box::new(package))
        }
        dto::FileKind::PersonaFile if !is_uec3(&file)? => {
            let mut package = persona(&file, context.now())?;
            download_avatar(context, &mut package.avatar_data, &mut warnings).await?;
            Prepared::Persona(package)
        }
        _ => Prepared::Text,
    };
    if file.kind == dto::FileKind::ChatJsonl {
        return super::entity_import_jobs::admit(context, request, digest).await;
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
                Character3(Box<lettuce_transfer::CharacterFileImport>),
                Persona(lettuce_transfer::PersonaFileImport),
                Lorebook(
                    lettuce_context::LorebookMetadataDraft,
                    Vec<lettuce_context::LorebookEntryDraft>,
                ),
                Prompt(PromptMetadataDraft, Vec<lettuce_context::PromptEntryDraft>),
                PromptFile(Box<lettuce_context::PromptDocument>),
                Starter(
                    lettuce_types::CharacterId,
                    lettuce_types::Revision,
                    lettuce_characters::ConversationStarter,
                ),
                Model(Box<lettuce_models::ModelProfile>),
            }
            let mut uploads = Vec::new();
            let mut pins = Vec::new();
            let (uec3, assets) =
                read_uec3_staged(context, &file, &mut uploads, &mut pins, &mut warnings)?;
            if let Some(card) = &uec3 {
                warnings.extend(
                    card.warnings
                        .iter()
                        .map(|field| warning(dto::FileWarningCode::FieldSkipped, field)),
                );
            }
            let books = if let Some(card) = &uec3 {
                uec3_books(context, &card.document, &assets)?
            } else {
                Default::default()
            };
            let prompts = if let Some(card) = &uec3 {
                uec3_prompts(context, &card.document, &mut warnings)?
            } else {
                Default::default()
            };
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
                (dto::FileKind::CharacterCard, _) if uec3.is_some() => {
                    let card = uec3
                        .as_ref()
                        .ok_or_else(|| internal("missing UEC document"))?;
                    let import = character3::prepare(
                        context,
                        &card.document,
                        character3::ImportAssets {
                            ids: &assets,
                            uploads: &mut uploads,
                            preview: false,
                        },
                        &mut warnings,
                        now,
                        &books,
                        &prompts,
                    )?;
                    WritePlan::Character3(Box::new(import))
                }
                (dto::FileKind::PersonaFile, _) if uec3.is_some() => {
                    let card = uec3
                        .as_ref()
                        .ok_or_else(|| internal("missing UEC document"))?;
                    WritePlan::Persona(prepare_uec3_persona(
                        context,
                        &card.document,
                        &assets,
                        &mut uploads,
                        &mut warnings,
                        false,
                        &books,
                    )?)
                }
                (dto::FileKind::PersonaFile, Prepared::Persona(package)) => {
                    let media = context.media().ok_or_else(|| {
                        api_error(ApiErrorCode::Unavailable, "no media store is open")
                    })?;
                    let (mut import, skips) = context
                        .backend()
                        .persona_files(media)
                        .prepare(&package, now)
                        .map_err(IntoApiError::into_api_error)?;
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
                (dto::FileKind::Lorebook, _) if uec3.is_some() => {
                    let card = uec3
                        .as_ref()
                        .ok_or_else(|| internal("missing UEC document"))?;
                    let lettuce_transfer::uec3::Entity::Lorebook(book) = &card.document.entity
                    else {
                        return Err(format_error("expected lorebook entity"));
                    };
                    let icon = book
                        .icon
                        .as_deref()
                        .map(|value| uec3_asset(&assets, value))
                        .transpose()?;
                    let (metadata, entries) = book.drafts(icon).map_err(uec3_error)?;
                    WritePlan::Lorebook(metadata, entries)
                }
                (dto::FileKind::Lorebook, _) => {
                    let (metadata, entries) =
                        crate::files::lorebook_files::prepare_import(text(&file)?)
                            .map_err(format_error)?;
                    WritePlan::Lorebook(metadata, entries)
                }
                (dto::FileKind::PromptPreset, _) if uec3.is_some() => {
                    let card = uec3
                        .as_ref()
                        .ok_or_else(|| internal("missing UEC document"))?;
                    let lettuce_transfer::uec3::Entity::Prompt(prompt) = &card.document.entity
                    else {
                        return Err(format_error("expected prompt entity"));
                    };
                    WritePlan::PromptFile(Box::new(prepare_uec3_prompt(
                        context,
                        prompt,
                        &mut warnings,
                    )?))
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
                (dto::FileKind::ChatTemplate, _) if uec3.is_some() => {
                    let id = parse_id(
                        request.options.character_id.as_deref().ok_or_else(|| {
                            invalid_field("options.character_id", "target character is required")
                        })?,
                        "options.character_id",
                    )?;
                    let character = lettuce_characters::CharacterRepository::get(database, id)
                        .map_err(IntoApiError::into_api_error)?
                        .ok_or_else(|| {
                            api_error(ApiErrorCode::NotFound, "character was not found")
                        })?;
                    if character.character.status != lettuce_characters::LifecycleStatus::Active {
                        return Err(invalid_field(
                            "options.character_id",
                            "character is archived",
                        ));
                    }
                    let card = uec3
                        .as_ref()
                        .ok_or_else(|| internal("missing UEC document"))?;
                    let lettuce_transfer::uec3::Entity::ChatTemplate(template) =
                        &card.document.entity
                    else {
                        return Err(format_error("expected chat template entity"));
                    };
                    let starter = prepare_uec3_template(
                        context, template, &character, &books, &prompts, now,
                    )?;
                    WritePlan::Starter(id, character.character.revision, starter)
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
                (dto::FileKind::ModelFile, _) if uec3.is_some() => {
                    let card = uec3
                        .as_ref()
                        .ok_or_else(|| internal("missing UEC document"))?;
                    let lettuce_transfer::uec3::Entity::ModelProfile(model) = &card.document.entity
                    else {
                        return Err(format_error("expected model profile entity"));
                    };
                    WritePlan::Model(Box::new(prepare_uec3_model(context, model)?))
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
                        for book in books.values() {
                            scope
                                .import_lorebook_file(book)
                                .map_err(|e| Failure(e.into_api_error()))?;
                        }
                        for prompt in prompts.values() {
                            scope
                                .import_prompt_file(prompt)
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
                            WritePlan::Character3(import) => scope
                                .import_character_file(&import)
                                .map_err(|e| Failure(internal(e)))?
                                .character
                                .id
                                .to_string(),
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
                            WritePlan::PromptFile(document) => scope
                                .import_prompt_file(&document)
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

fn text_body(content: String) -> impl FnOnce(&mut dyn Write) -> Result<(), ApiError> {
    move |writer| {
        writer
            .write_all(content.as_bytes())
            .map_err(|_| super::files::FileAccessError::Io.into_api_error())
    }
}

fn publish(
    context: &ApiContext,
    target: Option<dto::FileTarget>,
    name: String,
    body: impl FnOnce(&mut dyn Write) -> Result<(), ApiError>,
    warnings: Vec<dto::FileWarning>,
) -> Result<dto::EntityExportResult, ApiError> {
    cancelled(context)?;
    if target
        .as_ref()
        .is_some_and(|target| target.uri.trim().is_empty())
    {
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
            target.as_ref().map(|target| target.uri.as_str()),
            &format!("lettuce-backup-{}.partial", uuid::Uuid::new_v4()),
        )
        .map_err(IntoApiError::into_api_error)?;
    let (temporary, writer) = files
        .create_backup_temporary(&temporary_location, &protected)
        .map_err(IntoApiError::into_api_error)?;
    let result = (|| {
        let mut writer = std::io::BufWriter::new(writer);
        body(&mut writer)?;
        writer
            .flush()
            .map_err(|_| super::files::FileAccessError::Io.into_api_error())?;
        drop(writer);
        files
            .sync_backup(&temporary)
            .map_err(IntoApiError::into_api_error)?;
        cancelled(context)?;
        let uri = files
            .publish_backup(
                &temporary,
                target.as_ref().map(|target| target.uri.as_str()),
                &name,
                &protected,
            )
            .map_err(IntoApiError::into_api_error)?;
        Ok(dto::EntityExportResult {
            uri,
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
            let name = format!(
                "character_{}_{}.{extension}",
                crate::files::prompt_files::export_name(&details.character.profile.name),
                crate::files::prompt_files::export_date(now)
            );
            if request.format == dto::CharacterExportFormat::Uec {
                let mut warnings = Vec::new();
                let export =
                    character3::export(context, id, request.include_learned, &mut warnings)?;
                publish(
                    context,
                    request.target,
                    name,
                    |writer| export.write(context, writer),
                    warnings,
                )
            } else {
                let (content, warnings) = context
                    .backend()
                    .character_files(media)
                    .export_with_warnings(id, format, now)
                    .map_err(IntoApiError::into_api_error)?;
                publish(context, request.target, name, text_body(content), warnings)
            }
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
            let persona =
                lettuce_characters::PersonaRepository::get(context.backend().database(), id)
                    .map_err(IntoApiError::into_api_error)?
                    .ok_or_else(|| api_error(ApiErrorCode::NotFound, "persona was not found"))?;
            let mut warnings = Vec::new();
            let export = export_uec3_persona(context, &persona, &mut warnings)?;
            publish(
                context,
                request.target,
                format!(
                    "persona_{}_{}.uec",
                    crate::files::prompt_files::export_name(&persona.title),
                    crate::files::prompt_files::export_date(context.now())
                ),
                |writer| export.write(context, writer),
                warnings,
            )
        })
        .await
}
fn export_uec3_lorebook(
    context: &ApiContext,
    book: &lettuce_context::LorebookDetails,
) -> Result<Uec3Export, ApiError> {
    use lettuce_transfer::uec3;
    let mut assets = std::collections::BTreeMap::new();
    let mut ids = std::collections::BTreeMap::new();
    let icon = book
        .book
        .icon_asset_id
        .map(|id| add_uec3_asset(context, id, &mut assets, &mut ids))
        .transpose()?;
    Ok(Uec3Export::new(
        context,
        uec3::Entity::Lorebook(uec3::Lorebook::from_details(book, icon)),
        (book.book.created_at, book.book.updated_at),
        Vec::new(),
        assets,
        ids,
    ))
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
            let extension = match request.format {
                dto::LorebookExportFormat::WorldInfo => "json",
                dto::LorebookExportFormat::Uec => "uec",
            };
            let name = format!(
                "lorebook_{}_{}.{extension}",
                crate::files::prompt_files::export_name(&book.book.name),
                crate::files::prompt_files::export_date(context.now())
            );
            match request.format {
                dto::LorebookExportFormat::WorldInfo => {
                    let content = context
                        .backend()
                        .lorebook_files()
                        .export_world_info(id)
                        .map_err(IntoApiError::into_api_error)?;
                    publish(
                        context,
                        request.target,
                        name,
                        text_body(content),
                        Vec::new(),
                    )
                }
                dto::LorebookExportFormat::Uec => {
                    let export = export_uec3_lorebook(context, &book)?;
                    publish(
                        context,
                        request.target,
                        name,
                        |writer| export.write(context, writer),
                        Vec::new(),
                    )
                }
            }
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
            let database = context.backend().database();
            let document = lettuce_context::PromptRepository::get(database, id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "prompt was not found"))?;
            let built_in = match &document.provenance {
                lettuce_context::PromptProvenance::BuiltIn { key, .. } => Some(key.clone()),
                lettuce_context::PromptProvenance::Derived { source, .. } => {
                    lettuce_context::PromptRepository::get(database, *source)
                        .map_err(IntoApiError::into_api_error)?
                        .and_then(|source| match source.provenance {
                            lettuce_context::PromptProvenance::BuiltIn { key, .. } => Some(key),
                            _ => None,
                        })
                }
                _ => None,
            };
            let entity = lettuce_transfer::uec3::Prompt::from_document(&document, built_in)
                .map_err(format_error)?;
            let export = export_uec3_entity(
                context,
                lettuce_transfer::uec3::Entity::Prompt(entity),
                document.created_at,
                document.updated_at,
            );
            publish(
                context,
                request.target,
                format!(
                    "system_prompts_{}_{}.uec",
                    crate::files::prompt_files::export_name(&document.name),
                    crate::files::prompt_files::export_date(context.now())
                ),
                |writer| export.write(context, writer),
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
            let details = lettuce_characters::CharacterRepository::get(
                context.backend().database(),
                character_id,
            )
            .map_err(IntoApiError::into_api_error)?
            .ok_or_else(|| api_error(ApiErrorCode::NotFound, "character was not found"))?;
            let starter = details
                .starters
                .iter()
                .find(|starter| starter.id == starter_id)
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "starter was not found"))?;
            let export = export_uec3_template(context, starter)?;
            publish(
                context,
                request.target,
                format!(
                    "chat_template_{}_{}.uec",
                    crate::files::prompt_files::export_name(&starter.name),
                    crate::files::prompt_files::export_date(context.now())
                ),
                |writer| export.write(context, writer),
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
            let database = context.backend().database();
            let profile = lettuce_models::ModelProfileRepository::get(database, id)
                .map_err(super::model_profiles::model_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "model profile was not found"))?;
            let account = lettuce_models::ProviderAccountRepository::get(
                database,
                profile.provider_account_id,
            )
            .map_err(super::model_profiles::model_error)?
            .ok_or_else(|| api_error(ApiErrorCode::NotFound, "provider account was not found"))?;
            let model = lettuce_transfer::uec3::ModelProfile::from_stored(&profile, &account);
            let export = export_uec3_entity(
                context,
                lettuce_transfer::uec3::Entity::ModelProfile(Box::new(model)),
                profile.created_at,
                profile.updated_at,
            );
            publish(
                context,
                request.target,
                format!(
                    "model_{}_{}.uec",
                    crate::files::prompt_files::export_name(&profile.display_name),
                    crate::files::prompt_files::export_date(context.now())
                ),
                |writer| export.write(context, writer),
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
                text_body(file.content),
                Vec::new(),
            )
        })
        .await
}

fn import_uec3_image(
    context: &ApiContext,
    image: Option<&lettuce_transfer::uec3::ImageRecommendation>,
    warnings: &mut Vec<dto::FileWarning>,
) -> Result<Option<lettuce_characters::ImageRecommendation>, ApiError> {
    use lettuce_image_generation::sd_runtime::lora_library::LoraLibraryRepository;
    let Some(image) = image else {
        return Ok(None);
    };
    let database = context.backend().database();
    let by_hash = image
        .sha256
        .as_deref()
        .map(|hash| database.lora_file_by_sha256(hash))
        .transpose()
        .map_err(internal)?
        .flatten();
    let found = match by_hash {
        Some(record) => Some(record),
        None => database
            .lora_by_filename(&image.file_name)
            .map_err(internal)?,
    };
    Ok(match found {
        Some(record) => Some(lettuce_characters::ImageRecommendation {
            artifact_id: None,
            unresolved_legacy_name: Some(record.path),
            strength: image.strength,
        }),
        None => {
            warnings.push(warning(
                dto::FileWarningCode::FieldSkipped,
                format!("image.lora:{}", image.file_name),
            ));
            None
        }
    })
}

fn export_uec3_image(
    context: &ApiContext,
    image: Option<&lettuce_characters::ImageRecommendation>,
    warnings: &mut Vec<dto::FileWarning>,
) -> Result<Option<lettuce_transfer::uec3::ImageRecommendation>, ApiError> {
    use lettuce_image_generation::sd_runtime::lora_library::LoraLibraryRepository;
    let Some(image) = image else {
        return Ok(None);
    };
    let Some(path) = image.unresolved_legacy_name.as_deref() else {
        warnings.push(warning(
            dto::FileWarningCode::FieldSkipped,
            "image.lora.artifact",
        ));
        return Ok(None);
    };
    let record = context.backend().database().lora(path).map_err(internal)?;
    let Some(file_name) = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
    else {
        warnings.push(warning(
            dto::FileWarningCode::FieldSkipped,
            "image.lora.filename",
        ));
        return Ok(None);
    };
    Ok(Some(lettuce_transfer::uec3::ImageRecommendation {
        file_name: file_name.to_owned(),
        sha256: record.and_then(|record| record.sha256),
        strength: image.strength,
    }))
}

fn uec3_asset(
    assets: &std::collections::BTreeMap<String, lettuce_types::AssetId>,
    reference: &str,
) -> Result<lettuce_types::AssetId, ApiError> {
    let key = reference
        .strip_prefix("asset:")
        .ok_or_else(|| format_error("invalid asset reference"))?;
    assets
        .get(key)
        .copied()
        .ok_or_else(|| format_error(format!("missing asset:{key}")))
}

pub(super) fn uec3_books(
    context: &ApiContext,
    document: &lettuce_transfer::uec3::Document,
    assets: &std::collections::BTreeMap<String, lettuce_types::AssetId>,
) -> Result<std::collections::BTreeMap<String, lettuce_context::LorebookDetails>, ApiError> {
    document
        .dependencies
        .iter()
        .filter_map(|dependency| match &dependency.entity {
            lettuce_transfer::uec3::Entity::Lorebook(book) => Some((dependency.key.clone(), book)),
            _ => None,
        })
        .map(|(key, book)| {
            let icon = book
                .icon
                .as_deref()
                .map(|reference| uec3_asset(assets, reference))
                .transpose()?;
            Ok((
                format!("dep:{key}"),
                book.into_details(icon, context.now()).map_err(uec3_error)?,
            ))
        })
        .collect()
}

fn prepare_uec3_persona(
    context: &ApiContext,
    document: &lettuce_transfer::uec3::Document,
    assets: &std::collections::BTreeMap<String, lettuce_types::AssetId>,
    uploads: &mut Vec<lettuce_media::IngestedMedia>,
    warnings: &mut Vec<dto::FileWarning>,
    preview: bool,
    books: &std::collections::BTreeMap<String, lettuce_context::LorebookDetails>,
) -> Result<lettuce_transfer::PersonaFileImport, ApiError> {
    use lettuce_characters::{PersonaMediaLink, PersonaMediaSlot};
    let lettuce_transfer::uec3::Entity::Persona(persona) = &document.entity else {
        return Err(format_error("expected persona entity"));
    };
    let now = context.now();
    let mut lorebook_ids = Vec::new();
    let mut disabled_lorebook_ids = Vec::new();
    for binding in &persona.behavior.lorebooks {
        let book = books
            .get(&binding.book)
            .ok_or_else(|| format_error(format!("missing {}", binding.book)))?;
        lorebook_ids.push(book.book.id);
        if !binding.enabled {
            disabled_lorebook_ids.push(book.book.id);
        }
    }
    let mut links = Vec::new();
    if let Some(reference) = &persona.media.avatar {
        links.push(PersonaMediaLink {
            asset_id: character3::logical_asset(assets, uploads, reference, preview)?,
            slot: PersonaMediaSlot::Avatar,
            ordinal: 0,
        });
    }
    for (index, reference) in persona.design.references.iter().enumerate() {
        links.push(PersonaMediaLink {
            asset_id: character3::logical_asset(assets, uploads, reference, preview)?,
            slot: PersonaMediaSlot::DesignReference,
            ordinal: u32::try_from(index).map_err(internal)?,
        });
    }
    let persona = lettuce_characters::Persona {
        id: lettuce_types::PersonaId::new(),
        status: lettuce_characters::LifecycleStatus::Active,
        title: persona.name.clone(),
        description: persona.description.clone(),
        nickname: persona.nickname.clone(),
        design_description: persona.design.description.clone(),
        avatar_crop: persona.media.avatar_crop.map(Into::into),
        image_recommendation: import_uec3_image(
            context,
            persona.behavior.image.as_ref(),
            warnings,
        )?,
        media: lettuce_characters::PersonaMedia { links },
        revision: lettuce_types::Revision::INITIAL,
        created_at: now,
        updated_at: now,
    };
    persona.validate().map_err(format_error)?;
    Ok(lettuce_transfer::PersonaFileImport {
        persona,
        lorebook_ids,
        disabled_lorebook_ids,
        make_default: false,
    })
}

pub(super) fn read_uec3_staged(
    context: &ApiContext,
    file: &ImportFile,
    uploads: &mut Vec<lettuce_media::IngestedMedia>,
    pins: &mut Vec<lettuce_media::MediaObjectPin>,
    warnings: &mut Vec<dto::FileWarning>,
) -> Result<
    (
        Option<lettuce_transfer::uec3::ReadResult>,
        std::collections::BTreeMap<String, lettuce_types::AssetId>,
    ),
    ApiError,
> {
    let mut assets = std::collections::BTreeMap::new();
    let card = uec3_document(file, |key, _mime, bytes| {
        let id = stage_bytes(
            context,
            bytes,
            lettuce_media::AssetKind::OtherImage,
            key,
            uploads,
            pins,
            warnings,
        )
        .ok_or_else(|| {
            lettuce_transfer::uec3::Error::Format(format!("asset staging failed: {key}"))
        })?;
        assets.insert(key.to_owned(), id);
        Ok(())
    })?;
    Ok((card, assets))
}

fn add_uec3_asset(
    context: &ApiContext,
    id: lettuce_types::AssetId,
    assets: &mut std::collections::BTreeMap<String, lettuce_transfer::uec3::Asset>,
    ids: &mut std::collections::BTreeMap<String, lettuce_types::AssetId>,
) -> Result<String, ApiError> {
    let media = context
        .media()
        .ok_or_else(|| api_error(ApiErrorCode::Unavailable, "no media store is open"))?;
    let opened = media
        .open_ready(id)
        .map_err(|error| api_error(ApiErrorCode::Unavailable, error.to_string()))?;
    let hash = opened.blob.content_hash.as_str();
    if let Some((key, _)) = assets.iter().find(|(_, asset)| asset.blake3 == hash) {
        return Ok(format!("asset:{key}"));
    }
    let key = format!("a{}", assets.len() + 1);
    assets.insert(
        key.clone(),
        lettuce_transfer::uec3::Asset {
            mime: opened.blob.mime_type,
            blake3: hash.to_owned(),
        },
    );
    ids.insert(key.clone(), id);
    Ok(format!("asset:{key}"))
}

pub(super) struct Uec3Export {
    document: lettuce_transfer::uec3::Document,
    ids: std::collections::BTreeMap<String, lettuce_types::AssetId>,
}

impl Uec3Export {
    fn new(
        context: &ApiContext,
        entity: lettuce_transfer::uec3::Entity,
        (created_at, updated_at): (TimestampMillis, TimestampMillis),
        dependencies: Vec<lettuce_transfer::uec3::Dependency>,
        assets: std::collections::BTreeMap<String, lettuce_transfer::uec3::Asset>,
        ids: std::collections::BTreeMap<String, lettuce_types::AssetId>,
    ) -> Self {
        use lettuce_transfer::uec3;
        Self {
            document: uec3::Document {
                schema: uec3::Schema {
                    name: "UEC".into(),
                    version: "3.0".into(),
                },
                meta: uec3::Meta {
                    exported_at: context.now().get(),
                    app: "LettuceAI 3.x".into(),
                    created_at: created_at.get(),
                    updated_at: updated_at.get(),
                    authors: Vec::new(),
                    license: None,
                    sources: Vec::new(),
                },
                entity,
                dependencies,
                state: None,
                assets,
            },
            ids,
        }
    }

    pub(super) fn write(
        &self,
        context: &ApiContext,
        writer: &mut dyn Write,
    ) -> Result<(), ApiError> {
        let hashes = self
            .document
            .assets
            .values()
            .map(|asset| {
                lettuce_types::ContentHash::parse(asset.blake3.clone()).map_err(format_error)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (_, _pin) =
            lettuce_media::pin_media_objects(|| Ok::<_, std::convert::Infallible>(((), hashes)))
                .expect("pin");
        lettuce_transfer::uec3::write(writer, &self.document, |key, writer| {
            let id = self.ids.get(key).ok_or_else(|| {
                lettuce_transfer::uec3::Error::Format("missing export asset".into())
            })?;
            let media = context.media().ok_or_else(|| {
                lettuce_transfer::uec3::Error::Format("no media store is open".into())
            })?;
            let mut opened = media.open_ready(*id).map_err(|error| {
                lettuce_transfer::uec3::Error::Io(std::io::Error::other(error.to_string()))
            })?;
            std::io::copy(&mut opened.reader, writer)?;
            Ok(())
        })
        .map_err(uec3_error)
    }
}

pub(super) fn export_uec3_persona(
    context: &ApiContext,
    persona: &lettuce_characters::Persona,
    warnings: &mut Vec<dto::FileWarning>,
) -> Result<Uec3Export, ApiError> {
    use lettuce_context::PersonaLorebookBindingRepository;
    use lettuce_transfer::uec3;
    let mut assets = std::collections::BTreeMap::new();
    let mut ids = std::collections::BTreeMap::new();
    let mut dependencies = Vec::new();
    let mut bindings = context
        .backend()
        .database()
        .list_persona_bindings(persona.id)
        .map_err(internal)?;
    bindings.sort_by_key(|binding| binding.ordinal);
    let mut lorebooks = Vec::new();
    for binding in bindings {
        let book = LorebookRepository::get(context.backend().database(), binding.lorebook_id)
            .map_err(IntoApiError::into_api_error)?
            .ok_or_else(|| api_error(ApiErrorCode::NotFound, "bound lorebook was not found"))?;
        let icon = book
            .book
            .icon_asset_id
            .map(|id| add_uec3_asset(context, id, &mut assets, &mut ids))
            .transpose()?;
        let key = format!("l{}", dependencies.len() + 1);
        lorebooks.push(uec3::LorebookBinding {
            book: format!("dep:{key}"),
            enabled: binding.enabled,
        });
        dependencies.push(uec3::Dependency {
            key,
            entity: uec3::Entity::Lorebook(uec3::Lorebook::from_details(&book, icon)),
        });
    }
    let avatar = persona
        .media
        .links
        .iter()
        .find(|link| link.slot == lettuce_characters::PersonaMediaSlot::Avatar)
        .map(|link| add_uec3_asset(context, link.asset_id, &mut assets, &mut ids))
        .transpose()?;
    let mut references = persona
        .media
        .links
        .iter()
        .filter(|link| link.slot == lettuce_characters::PersonaMediaSlot::DesignReference)
        .collect::<Vec<_>>();
    references.sort_by_key(|link| link.ordinal);
    let references = references
        .into_iter()
        .map(|link| add_uec3_asset(context, link.asset_id, &mut assets, &mut ids))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Uec3Export::new(
        context,
        uec3::Entity::Persona(uec3::Persona {
            name: persona.title.clone(),
            nickname: persona.nickname.clone(),
            description: persona.description.clone(),
            design: uec3::Design {
                description: persona.design_description.clone(),
                references,
            },
            media: uec3::PersonaMedia {
                avatar,
                avatar_crop: persona.avatar_crop.map(Into::into),
            },
            behavior: uec3::PersonaBehavior {
                image: export_uec3_image(context, persona.image_recommendation.as_ref(), warnings)?,
                lorebooks,
            },
        }),
        (persona.created_at, persona.updated_at),
        dependencies,
        assets,
        ids,
    ))
}

pub(super) fn uec3_prompts(
    context: &ApiContext,
    document: &lettuce_transfer::uec3::Document,
    warnings: &mut Vec<dto::FileWarning>,
) -> Result<std::collections::BTreeMap<String, lettuce_context::PromptDocument>, ApiError> {
    document
        .dependencies
        .iter()
        .filter_map(|dependency| match &dependency.entity {
            lettuce_transfer::uec3::Entity::Prompt(prompt) => {
                Some((dependency.key.clone(), prompt))
            }
            _ => None,
        })
        .map(|(key, prompt)| {
            Ok((
                format!("dep:{key}"),
                prepare_uec3_prompt(context, prompt, warnings)?,
            ))
        })
        .collect()
}

fn uec3_prompt_reference(
    context: &ApiContext,
    reference: &str,
    prompts: &std::collections::BTreeMap<String, lettuce_context::PromptDocument>,
) -> Result<lettuce_context::PromptDocument, ApiError> {
    if let Some(key) = reference.strip_prefix("builtin:") {
        let id = crate::generation::built_in_prompts::BuiltInPromptId::from_key_or_alias(key)
            .ok_or_else(|| {
                api_error(
                    ApiErrorCode::NotFound,
                    format!("built-in prompt not found: {key}"),
                )
            })?;
        lettuce_context::PromptRepository::get(
            context.backend().database(),
            context.backend().built_in_prompt_ids().get(id),
        )
        .map_err(IntoApiError::into_api_error)?
        .ok_or_else(|| {
            api_error(
                ApiErrorCode::NotFound,
                format!("built-in prompt not found: {key}"),
            )
        })
    } else {
        prompts
            .get(reference)
            .cloned()
            .ok_or_else(|| format_error(format!("missing prompt: {reference}")))
    }
}

fn prepare_uec3_template(
    context: &ApiContext,
    template: &lettuce_transfer::uec3::ChatTemplate,
    character: &lettuce_characters::CharacterDetails,
    books: &std::collections::BTreeMap<String, lettuce_context::LorebookDetails>,
    prompts: &std::collections::BTreeMap<String, lettuce_context::PromptDocument>,
    now: TimestampMillis,
) -> Result<lettuce_characters::ConversationStarter, ApiError> {
    use lettuce_characters::{ConversationStarter, Selection, StarterMessage};
    use lettuce_transfer::uec3::{LorebookSelection, SelectionKeyword};
    let prompt_id = template
        .prompt
        .as_deref()
        .map(|reference| Ok(uec3_prompt_reference(context, reference, prompts)?.id))
        .transpose()?;
    let lorebooks = match &template.lorebooks {
        LorebookSelection::Keyword(SelectionKeyword::Inherit) => Selection::Inherit,
        LorebookSelection::Keyword(SelectionKeyword::Disabled) => Selection::Disabled,
        LorebookSelection::Explicit(references) => Selection::Explicit(
            references
                .iter()
                .map(|reference| {
                    books
                        .get(reference)
                        .map(|book| book.book.id)
                        .ok_or_else(|| format_error(format!("missing lorebook: {reference}")))
                })
                .collect::<Result<Vec<_>, ApiError>>()?,
        ),
    };
    let starter = ConversationStarter {
        id: lettuce_types::ConversationStarterId::new(),
        character_id: character.character.id,
        name: template.name.clone(),
        ordinal: u32::try_from(character.starters.len()).map_err(internal)?,
        messages: template
            .messages
            .iter()
            .map(|message| StarterMessage {
                id: lettuce_types::StarterMessageId::new(),
                role: message.role,
                content: message.text.clone(),
            })
            .collect(),
        scene_id: None,
        prompt_id,
        lorebooks,
        revision: lettuce_types::Revision::INITIAL,
        created_at: now,
        updated_at: now,
    };
    starter.validate().map_err(format_error)?;
    Ok(starter)
}

fn bundle_uec3_prompt(
    context: &ApiContext,
    id: lettuce_types::PromptDocumentId,
    dependencies: &mut Vec<lettuce_transfer::uec3::Dependency>,
    references: &mut std::collections::BTreeMap<lettuce_types::PromptDocumentId, String>,
) -> Result<String, ApiError> {
    use lettuce_transfer::uec3;
    if let Some(reference) = references.get(&id) {
        return Ok(reference.clone());
    }
    let document = lettuce_context::PromptRepository::get(context.backend().database(), id)
        .map_err(IntoApiError::into_api_error)?
        .ok_or_else(|| api_error(ApiErrorCode::NotFound, "referenced prompt was not found"))?;
    if let lettuce_context::PromptProvenance::BuiltIn {
        key,
        seed_digest,
        authored_digest,
        ..
    } = &document.provenance
    {
        if seed_digest == authored_digest {
            let reference = format!("builtin:{key}");
            references.insert(id, reference.clone());
            return Ok(reference);
        }
    }
    let built_in = match &document.provenance {
        lettuce_context::PromptProvenance::BuiltIn { key, .. } => Some(key.clone()),
        lettuce_context::PromptProvenance::Derived { source, .. } => {
            lettuce_context::PromptRepository::get(context.backend().database(), *source)
                .map_err(IntoApiError::into_api_error)?
                .and_then(|source| match source.provenance {
                    lettuce_context::PromptProvenance::BuiltIn { key, .. } => Some(key),
                    _ => None,
                })
        }
        _ => None,
    };
    let key = format!(
        "p{}",
        dependencies
            .iter()
            .filter(|dependency| matches!(dependency.entity, uec3::Entity::Prompt(_)))
            .count()
            + 1
    );
    let entity = uec3::Prompt::from_document(&document, built_in).map_err(uec3_error)?;
    let reference = format!("dep:{key}");
    dependencies.push(uec3::Dependency {
        key,
        entity: uec3::Entity::Prompt(entity),
    });
    references.insert(id, reference.clone());
    Ok(reference)
}

fn export_uec3_template(
    context: &ApiContext,
    starter: &lettuce_characters::ConversationStarter,
) -> Result<Uec3Export, ApiError> {
    use lettuce_transfer::uec3;
    let mut dependencies = Vec::new();
    let mut assets = std::collections::BTreeMap::new();
    let mut ids = std::collections::BTreeMap::new();
    let prompt = starter
        .prompt_id
        .map(|id| bundle_uec3_prompt(context, id, &mut dependencies, &mut Default::default()))
        .transpose()?;
    let lorebooks = match &starter.lorebooks {
        lettuce_characters::Selection::Inherit => {
            uec3::LorebookSelection::Keyword(uec3::SelectionKeyword::Inherit)
        }
        lettuce_characters::Selection::Disabled => {
            uec3::LorebookSelection::Keyword(uec3::SelectionKeyword::Disabled)
        }
        lettuce_characters::Selection::Explicit(book_ids) => {
            let mut references = Vec::new();
            for (index, id) in book_ids.iter().enumerate() {
                let book = LorebookRepository::get(context.backend().database(), *id)
                    .map_err(IntoApiError::into_api_error)?
                    .ok_or_else(|| {
                        api_error(ApiErrorCode::NotFound, "starter lorebook was not found")
                    })?;
                let icon = book
                    .book
                    .icon_asset_id
                    .map(|id| add_uec3_asset(context, id, &mut assets, &mut ids))
                    .transpose()?;
                let key = format!("l{}", index + 1);
                references.push(format!("dep:{key}"));
                dependencies.push(uec3::Dependency {
                    key,
                    entity: uec3::Entity::Lorebook(uec3::Lorebook::from_details(&book, icon)),
                });
            }
            uec3::LorebookSelection::Explicit(references)
        }
    };
    Ok(Uec3Export::new(
        context,
        uec3::Entity::ChatTemplate(uec3::ChatTemplate {
            key: "t1".into(),
            name: starter.name.clone(),
            messages: starter
                .messages
                .iter()
                .map(|message| uec3::StarterMessage {
                    role: message.role,
                    text: message.content.clone(),
                })
                .collect(),
            prompt,
            lorebooks,
        }),
        (starter.created_at, starter.updated_at),
        dependencies,
        assets,
        ids,
    ))
}
