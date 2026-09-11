use std::collections::{BTreeMap, BTreeSet};

use base64::{Engine as _, engine::general_purpose};
use lettuce_types::ContentHash;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind, LegacyBackupDocumentKind,
    LegacyBackupMediaRoot, LegacyBackupMemoryEmbeddingPlan,
};

const SESSION_LIMIT: usize = 10_000;
const MESSAGE_LIMIT: usize = 200_000;
const IMAGE_LIMIT: usize = 100_000;
const SCENE_LIMIT: usize = 100_000;
const JSON_LIMIT: usize = 256 * 1024 * 1024;
const TEXT_LIMIT: usize = 1_000_000;
const INLINE_IMAGE_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Debug)]
pub struct LegacyBackupCreationHelperPlan {
    pub sessions: Vec<LegacyBackupCreationHelperSession>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub source: LegacyBackupMemoryEmbeddingPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupCreationHelperSession {
    pub ordinal: u64,
    pub source_id: String,
    pub creation_goal: LegacyBackupCreationGoal,
    pub status: LegacyBackupCreationStatus,
    pub session_json: String,
    pub uploaded_images_json: String,
    pub session: LegacyBackupCreationSessionState,
    pub uploaded_images: Vec<LegacyBackupCreationImage>,
    pub created_at: u64,
    pub updated_at: u64,
    pub materialization: LegacyBackupCreationMaterialization,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupCreationSessionState {
    pub messages: Vec<LegacyBackupCreationMessage>,
    pub draft: LegacyBackupCreationDraft,
    pub draft_history: Vec<LegacyBackupCreationDraft>,
    pub creation_mode: LegacyBackupCreationMode,
    pub target_type: Option<LegacyBackupCreationGoal>,
    pub target_source_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupCreationMessage {
    pub ordinal: u64,
    pub source_id: String,
    pub role: LegacyBackupCreationMessageRole,
    pub content: String,
    pub tool_calls_json: String,
    pub tool_results_json: String,
    pub blocks_json: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyBackupCreationDraft {
    pub name: Option<String>,
    pub definition: Option<String>,
    pub description: Option<String>,
    pub scenes: Vec<LegacyBackupCreationScene>,
    pub default_scene_source_id: Option<String>,
    pub avatar_locator: Option<String>,
    pub background_locator: Option<String>,
    pub disable_avatar_gradient: bool,
    pub default_model_source_id: Option<String>,
    pub prompt_source_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupCreationScene {
    pub source_id: String,
    pub content: String,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBackupCreationImage {
    pub source_id: String,
    pub data: String,
    pub mime_type: String,
    pub asset_source_id: Option<String>,
    pub archive_locator: Option<String>,
    pub inline_content_hash: Option<ContentHash>,
    pub archive_content_hash: Option<ContentHash>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyBackupCreationGoal {
    Character,
    Persona,
    Lorebook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyBackupCreationStatus {
    Active,
    PreviewShown,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyBackupCreationMode {
    Create,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyBackupCreationMessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyBackupCreationMaterialization {
    InitialDraftSeed,
    RetainedEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupCreationHelperError {
    #[error("legacy backup creation-helper document is malformed")]
    Malformed { field: String },
    #[error("legacy backup creation-helper document exceeds its record limit")]
    LimitExceeded,
    #[error("legacy backup creation-helper graph contains an orphaned link")]
    Orphan { field: String },
    #[error("legacy backup creation-helper row disagrees with its nested state")]
    Conflict { field: String },
}

#[derive(Deserialize)]
struct SessionRow {
    id: String,
    creation_goal: String,
    status: String,
    session_json: String,
    uploaded_images_json: String,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionStateRow {
    id: String,
    messages: Vec<MessageRow>,
    draft: DraftRow,
    #[serde(default)]
    draft_history: Vec<DraftRow>,
    creation_goal: String,
    #[serde(default = "create_mode")]
    creation_mode: String,
    #[serde(default)]
    target_type: Option<String>,
    #[serde(default)]
    target_id: Option<String>,
    status: String,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageRow {
    id: String,
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Vec<Value>,
    #[serde(default)]
    tool_results: Vec<Value>,
    #[serde(default)]
    blocks: Vec<Value>,
    created_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DraftRow {
    name: Option<String>,
    #[serde(default)]
    definition: Option<String>,
    description: Option<String>,
    scenes: Vec<SceneRow>,
    default_scene_id: Option<String>,
    avatar_path: Option<String>,
    background_image_path: Option<String>,
    disable_avatar_gradient: bool,
    default_model_id: Option<String>,
    prompt_template_id: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SceneRow {
    id: String,
    content: String,
    direction: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageRow {
    id: String,
    #[serde(default)]
    data: String,
    mime_type: String,
    #[serde(default)]
    asset_id: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

pub fn plan_legacy_backup_creation_helpers(
    source: LegacyBackupMemoryEmbeddingPlan,
) -> Result<LegacyBackupCreationHelperPlan, LegacyBackupCreationHelperError> {
    let document = source
        .source
        .source
        .source
        .source
        .source
        .source
        .source
        .source
        .authored
        .configuration
        .source
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::CreationHelperSessions);
    let mut notices = source.notices.clone();
    let sessions = match document {
        Some(document) => {
            let rows: Vec<SessionRow> =
                serde_json::from_slice(&document.bytes).map_err(|_| malformed("$"))?;
            map_sessions(rows, &source, &mut notices)?
        }
        None => {
            notices.push(notice(LegacyBackupConversionNoticeKind::Absent, "$"));
            Vec::new()
        }
    };
    notices.sort();
    notices.dedup();
    Ok(LegacyBackupCreationHelperPlan {
        sessions,
        notices,
        source,
    })
}

fn map_sessions(
    rows: Vec<SessionRow>,
    source: &LegacyBackupMemoryEmbeddingPlan,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupCreationHelperSession>, LegacyBackupCreationHelperError> {
    if rows.len() > SESSION_LIMIT {
        return Err(LegacyBackupCreationHelperError::LimitExceeded);
    }
    let mut ids = BTreeSet::new();
    let mut updated_at_values = BTreeSet::new();
    let mut message_count = 0usize;
    let mut image_count = 0usize;
    let mut sessions = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("[{index}]");
        report_extra(&path, &row.extra, notices);
        validate_uuid(&row.id, &format!("{path}.id"))?;
        if !ids.insert(row.id.clone()) {
            return Err(malformed(format!("{path}.id")));
        }
        let creation_goal = parse_goal(&row.creation_goal, &format!("{path}.creation_goal"))?;
        let status = parse_status(&row.status, &format!("{path}.status"))?;
        let nested: SessionStateRow =
            parse_json(&row.session_json, &format!("{path}.session_json"))?;
        if nested.id != row.id
            || parse_goal(
                &nested.creation_goal,
                &format!("{path}.session_json.creationGoal"),
            )? != creation_goal
            || parse_status(&nested.status, &format!("{path}.session_json.status"))? != status
            || nested.created_at != row.created_at
            || nested.updated_at != row.updated_at
        {
            return Err(conflict(format!("{path}.session_json")));
        }
        report_extra(&format!("{path}.session_json"), &nested.extra, notices);
        let created_at = timestamp(row.created_at, &format!("{path}.created_at"))?;
        let updated_at = timestamp(row.updated_at, &format!("{path}.updated_at"))?;
        if created_at > updated_at {
            return Err(malformed(format!("{path}.updated_at")));
        }
        if !updated_at_values.insert(updated_at) {
            notices.push(notice(
                LegacyBackupConversionNoticeKind::Lossy,
                "$[].updated_at_order",
            ));
        }
        let creation_mode = parse_mode(
            &nested.creation_mode,
            &format!("{path}.session_json.creationMode"),
        )?;
        let target_type = nested
            .target_type
            .as_deref()
            .map(|value| parse_goal(value, &format!("{path}.session_json.targetType")))
            .transpose()?;
        validate_target(
            creation_goal,
            creation_mode,
            target_type,
            nested.target_id.as_deref(),
            source,
            &path,
        )?;
        let messages = map_messages(nested.messages, &path, notices)?;
        if messages
            .iter()
            .any(|message| message.created_at < created_at || message.created_at > updated_at)
        {
            return Err(malformed(format!(
                "{path}.session_json.messages[].createdAt"
            )));
        }
        message_count = message_count
            .checked_add(messages.len())
            .ok_or(LegacyBackupCreationHelperError::LimitExceeded)?;
        if message_count > MESSAGE_LIMIT {
            return Err(LegacyBackupCreationHelperError::LimitExceeded);
        }
        let draft = map_draft(nested.draft, &format!("{path}.session_json.draft"), notices)?;
        let draft_history = nested
            .draft_history
            .into_iter()
            .enumerate()
            .map(|(draft_index, draft)| {
                map_draft(
                    draft,
                    &format!("{path}.session_json.draftHistory[{draft_index}]"),
                    notices,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let images: BTreeMap<String, ImageRow> = parse_json(
            &row.uploaded_images_json,
            &format!("{path}.uploaded_images_json"),
        )?;
        image_count = image_count
            .checked_add(images.len())
            .ok_or(LegacyBackupCreationHelperError::LimitExceeded)?;
        if image_count > IMAGE_LIMIT {
            return Err(LegacyBackupCreationHelperError::LimitExceeded);
        }
        let uploaded_images = map_images(images, source, &path, notices)?;
        let materialization = if creation_mode == LegacyBackupCreationMode::Create
            && status == LegacyBackupCreationStatus::Active
            && messages.is_empty()
            && draft_history.is_empty()
            && uploaded_images.is_empty()
            && current_draft_compatible(creation_goal, &draft)
        {
            LegacyBackupCreationMaterialization::InitialDraftSeed
        } else {
            LegacyBackupCreationMaterialization::RetainedEvidence
        };
        sessions.push(LegacyBackupCreationHelperSession {
            ordinal: u64::try_from(index)
                .map_err(|_| LegacyBackupCreationHelperError::LimitExceeded)?,
            source_id: row.id,
            creation_goal,
            status,
            session_json: row.session_json,
            uploaded_images_json: row.uploaded_images_json,
            session: LegacyBackupCreationSessionState {
                messages,
                draft,
                draft_history,
                creation_mode,
                target_type,
                target_source_id: nested.target_id,
            },
            uploaded_images,
            created_at,
            updated_at,
            materialization,
        });
    }
    Ok(sessions)
}

fn map_messages(
    rows: Vec<MessageRow>,
    parent_path: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupCreationMessage>, LegacyBackupCreationHelperError> {
    let mut ids = BTreeSet::new();
    let mut messages = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let path = format!("{parent_path}.session_json.messages[{index}]");
        report_extra(&path, &row.extra, notices);
        validate_uuid(&row.id, &format!("{path}.id"))?;
        if !ids.insert(row.id.clone()) {
            return Err(malformed(format!("{path}.id")));
        }
        let role = match row.role.as_str() {
            "user" => LegacyBackupCreationMessageRole::User,
            "assistant" => LegacyBackupCreationMessageRole::Assistant,
            "system" => LegacyBackupCreationMessageRole::System,
            _ => return Err(malformed(format!("{path}.role"))),
        };
        validate_text(&row.content, &format!("{path}.content"))?;
        validate_tool_state(&row.tool_calls, &row.tool_results, &row.blocks, &path)?;
        messages.push(LegacyBackupCreationMessage {
            ordinal: u64::try_from(index)
                .map_err(|_| LegacyBackupCreationHelperError::LimitExceeded)?,
            source_id: row.id,
            role,
            content: row.content,
            tool_calls_json: serde_json::to_string(&row.tool_calls)
                .map_err(|_| malformed(format!("{path}.toolCalls")))?,
            tool_results_json: serde_json::to_string(&row.tool_results)
                .map_err(|_| malformed(format!("{path}.toolResults")))?,
            blocks_json: serde_json::to_string(&row.blocks)
                .map_err(|_| malformed(format!("{path}.blocks")))?,
            created_at: timestamp(row.created_at, &format!("{path}.createdAt"))?,
        });
    }
    Ok(messages)
}

fn validate_tool_state(
    calls: &[Value],
    results: &[Value],
    blocks: &[Value],
    path: &str,
) -> Result<(), LegacyBackupCreationHelperError> {
    let mut call_ids = BTreeSet::new();
    for (index, call) in calls.iter().enumerate() {
        let object = call
            .as_object()
            .ok_or_else(|| malformed(format!("{path}.toolCalls[{index}]")))?;
        let id = required_string(object.get("id"), &format!("{path}.toolCalls[{index}].id"))?;
        required_string(
            object.get("name"),
            &format!("{path}.toolCalls[{index}].name"),
        )?;
        if !call_ids.insert(id) {
            return Err(malformed(format!("{path}.toolCalls[{index}].id")));
        }
    }
    let mut result_ids = BTreeSet::new();
    for (index, result) in results.iter().enumerate() {
        let object = result
            .as_object()
            .ok_or_else(|| malformed(format!("{path}.toolResults[{index}]")))?;
        let id = required_string(
            object.get("toolCallId"),
            &format!("{path}.toolResults[{index}].toolCallId"),
        )?;
        if !call_ids.contains(id)
            || !result_ids.insert(id)
            || !object.contains_key("result")
            || object.get("success").and_then(Value::as_bool).is_none()
        {
            return Err(malformed(format!("{path}.toolResults[{index}]")));
        }
    }
    for (index, block) in blocks.iter().enumerate() {
        let object = block
            .as_object()
            .ok_or_else(|| malformed(format!("{path}.blocks[{index}]")))?;
        match object.get("kind").and_then(Value::as_str) {
            Some("text") if object.get("content").and_then(Value::as_str).is_some() => {}
            Some("tool")
                if object
                    .get("toolCallId")
                    .and_then(Value::as_str)
                    .is_some_and(|id| call_ids.contains(id)) => {}
            _ => return Err(malformed(format!("{path}.blocks[{index}]"))),
        }
    }
    Ok(())
}

fn map_draft(
    row: DraftRow,
    path: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<LegacyBackupCreationDraft, LegacyBackupCreationHelperError> {
    report_extra(path, &row.extra, notices);
    for (field, value) in [
        ("name", row.name.as_deref()),
        ("definition", row.definition.as_deref()),
        ("description", row.description.as_deref()),
        ("avatarPath", row.avatar_path.as_deref()),
        ("backgroundImagePath", row.background_image_path.as_deref()),
    ] {
        if let Some(value) = value {
            validate_text(value, &format!("{path}.{field}"))?;
        }
    }
    if row.scenes.len() > SCENE_LIMIT {
        return Err(LegacyBackupCreationHelperError::LimitExceeded);
    }
    let mut scene_ids = BTreeSet::new();
    let scenes = row
        .scenes
        .into_iter()
        .enumerate()
        .map(|(index, scene)| {
            let scene_path = format!("{path}.scenes[{index}]");
            report_extra(&scene_path, &scene.extra, notices);
            validate_uuid(&scene.id, &format!("{scene_path}.id"))?;
            if !scene_ids.insert(scene.id.clone()) {
                return Err(malformed(format!("{scene_path}.id")));
            }
            validate_text(&scene.content, &format!("{scene_path}.content"))?;
            if let Some(direction) = &scene.direction {
                validate_text(direction, &format!("{scene_path}.direction"))?;
            }
            Ok(LegacyBackupCreationScene {
                source_id: scene.id,
                content: scene.content,
                direction: scene.direction,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if row
        .default_scene_id
        .as_ref()
        .is_some_and(|id| !scene_ids.contains(id))
    {
        return Err(orphan(format!("{path}.defaultSceneId")));
    }
    Ok(LegacyBackupCreationDraft {
        name: row.name,
        definition: row.definition,
        description: row.description,
        scenes,
        default_scene_source_id: row.default_scene_id,
        avatar_locator: row.avatar_path,
        background_locator: row.background_image_path,
        disable_avatar_gradient: row.disable_avatar_gradient,
        default_model_source_id: row.default_model_id,
        prompt_source_id: row.prompt_template_id,
    })
}

fn map_images(
    rows: BTreeMap<String, ImageRow>,
    source: &LegacyBackupMemoryEmbeddingPlan,
    parent_path: &str,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) -> Result<Vec<LegacyBackupCreationImage>, LegacyBackupCreationHelperError> {
    let media = raw_media(source);
    let mut images = Vec::with_capacity(rows.len());
    for (key, row) in rows {
        let path = format!("{parent_path}.uploaded_images_json.{key}");
        report_extra(&path, &row.extra, notices);
        validate_simple_id(&key, &path)?;
        if row.id != key {
            return Err(conflict(format!("{path}.id")));
        }
        if !row.mime_type.starts_with("image/") || row.mime_type.len() > 128 {
            return Err(malformed(format!("{path}.mimeType")));
        }
        let (archive_locator, archive_hash) = row
            .asset_id
            .as_deref()
            .map(|asset_id| resolve_image_asset(asset_id, media, &format!("{path}.assetId")))
            .transpose()?
            .map_or((None, None), |(locator, hash)| (Some(locator), Some(hash)));
        let inline_hash = if row.data.is_empty() {
            None
        } else {
            Some(validate_inline_image(&row.data, &format!("{path}.data"))?)
        };
        if row.data.is_empty() && row.asset_id.is_none() {
            return Err(orphan(format!("{path}.assetId")));
        }
        images.push(LegacyBackupCreationImage {
            source_id: row.id,
            data: row.data,
            mime_type: row.mime_type,
            asset_source_id: row.asset_id,
            archive_locator,
            inline_content_hash: inline_hash,
            archive_content_hash: archive_hash,
        });
    }
    Ok(images)
}

fn validate_target(
    goal: LegacyBackupCreationGoal,
    mode: LegacyBackupCreationMode,
    target_type: Option<LegacyBackupCreationGoal>,
    target_id: Option<&str>,
    source: &LegacyBackupMemoryEmbeddingPlan,
    path: &str,
) -> Result<(), LegacyBackupCreationHelperError> {
    if mode == LegacyBackupCreationMode::Create {
        if target_id.is_some() {
            return Err(malformed(format!("{path}.session_json.targetId")));
        }
        return Ok(());
    }
    let target_type = target_type.unwrap_or(goal);
    let target_id = target_id.ok_or_else(|| orphan(format!("{path}.session_json.targetId")))?;
    validate_uuid(target_id, &format!("{path}.session_json.targetId"))?;
    let authored = raw_authored(source);
    let found = match target_type {
        LegacyBackupCreationGoal::Character => authored
            .characters
            .iter()
            .any(|candidate| candidate.id.to_string() == target_id),
        LegacyBackupCreationGoal::Persona => authored
            .personas
            .personas
            .iter()
            .any(|candidate| candidate.id.to_string() == target_id),
        LegacyBackupCreationGoal::Lorebook => authored
            .lorebooks
            .lorebooks
            .iter()
            .any(|candidate| candidate.id.to_string() == target_id),
    };
    if found {
        Ok(())
    } else {
        Err(orphan(format!("{path}.session_json.targetId")))
    }
}

fn current_draft_compatible(
    goal: LegacyBackupCreationGoal,
    draft: &LegacyBackupCreationDraft,
) -> bool {
    let common_unused = draft.default_scene_source_id.is_none()
        && draft.avatar_locator.is_none()
        && draft.background_locator.is_none()
        && !draft.disable_avatar_gradient
        && draft.default_model_source_id.is_none()
        && draft.prompt_source_id.is_none();
    common_unused
        && [
            draft.name.as_deref(),
            draft.definition.as_deref(),
            draft.description.as_deref(),
        ]
        .into_iter()
        .flatten()
        .all(|value| !value.trim().is_empty())
        && draft
            .scenes
            .iter()
            .all(|scene| !scene.content.trim().is_empty())
        && match goal {
            LegacyBackupCreationGoal::Character => draft.description.is_none(),
            LegacyBackupCreationGoal::Persona => {
                draft.definition.is_none() && draft.scenes.is_empty()
            }
            LegacyBackupCreationGoal::Lorebook => {
                draft.definition.is_none() && draft.scenes.is_empty()
            }
        }
}

fn resolve_image_asset(
    asset_id: &str,
    media: &[crate::LegacyBackupMedia],
    field: &str,
) -> Result<(String, ContentHash), LegacyBackupCreationHelperError> {
    validate_simple_id(asset_id, field)?;
    let matches = media
        .iter()
        .filter(|item| item.root == LegacyBackupMediaRoot::Images)
        .filter(|item| {
            matches!(item.relative_segments.as_slice(), [filename] if filename.rsplit_once('.').map(|(stem, _)| stem) == Some(asset_id))
        })
        .collect::<Vec<_>>();
    let [item] = matches.as_slice() else {
        return Err(orphan(field));
    };
    let locator = item.relative_segments.join("/");
    let hash = ContentHash::parse(blake3::hash(&item.bytes).to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash");
    Ok((locator, hash))
}

fn validate_inline_image(
    data: &str,
    field: &str,
) -> Result<ContentHash, LegacyBackupCreationHelperError> {
    let encoded = if let Some((_, encoded)) = data.split_once(',') {
        encoded
    } else {
        data
    };
    let estimated = encoded.len().saturating_mul(3) / 4;
    if estimated > INLINE_IMAGE_LIMIT {
        return Err(LegacyBackupCreationHelperError::LimitExceeded);
    }
    let bytes = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| malformed(field))?;
    if bytes.is_empty() || bytes.len() > INLINE_IMAGE_LIMIT {
        return Err(malformed(field));
    }
    Ok(
        ContentHash::parse(blake3::hash(&bytes).to_hex().to_string())
            .expect("BLAKE3 produces a valid content hash"),
    )
}

fn raw_media(source: &LegacyBackupMemoryEmbeddingPlan) -> &[crate::LegacyBackupMedia] {
    &source
        .source
        .source
        .source
        .source
        .source
        .source
        .source
        .source
        .authored
        .configuration
        .source
        .media
}

fn raw_authored(source: &LegacyBackupMemoryEmbeddingPlan) -> &crate::LegacyBackupAuthoredPlan {
    &source
        .source
        .source
        .source
        .source
        .source
        .source
        .source
        .source
        .authored
}

fn parse_goal(
    value: &str,
    field: &str,
) -> Result<LegacyBackupCreationGoal, LegacyBackupCreationHelperError> {
    match value {
        "character" => Ok(LegacyBackupCreationGoal::Character),
        "persona" => Ok(LegacyBackupCreationGoal::Persona),
        "lorebook" => Ok(LegacyBackupCreationGoal::Lorebook),
        _ => Err(malformed(field)),
    }
}

fn parse_status(
    value: &str,
    field: &str,
) -> Result<LegacyBackupCreationStatus, LegacyBackupCreationHelperError> {
    match value {
        "active" => Ok(LegacyBackupCreationStatus::Active),
        "previewShown" => Ok(LegacyBackupCreationStatus::PreviewShown),
        "completed" => Ok(LegacyBackupCreationStatus::Completed),
        "cancelled" => Ok(LegacyBackupCreationStatus::Cancelled),
        _ => Err(malformed(field)),
    }
}

fn parse_mode(
    value: &str,
    field: &str,
) -> Result<LegacyBackupCreationMode, LegacyBackupCreationHelperError> {
    match value {
        "create" => Ok(LegacyBackupCreationMode::Create),
        "edit" => Ok(LegacyBackupCreationMode::Edit),
        _ => Err(malformed(field)),
    }
}

fn parse_json<T: for<'de> Deserialize<'de>>(
    raw: &str,
    field: &str,
) -> Result<T, LegacyBackupCreationHelperError> {
    if raw.len() > JSON_LIMIT {
        return Err(LegacyBackupCreationHelperError::LimitExceeded);
    }
    serde_json::from_str(raw).map_err(|_| malformed(field))
}

fn required_string<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<&'a str, LegacyBackupCreationHelperError> {
    let value = value
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(field))?;
    if value.trim().is_empty() || value.len() > TEXT_LIMIT || value.contains('\0') {
        Err(malformed(field))
    } else {
        Ok(value)
    }
}

fn validate_uuid(value: &str, field: &str) -> Result<(), LegacyBackupCreationHelperError> {
    uuid::Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| malformed(field))
}

fn validate_simple_id(value: &str, field: &str) -> Result<(), LegacyBackupCreationHelperError> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.len() > 1_024
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), LegacyBackupCreationHelperError> {
    if value.len() > TEXT_LIMIT || value.contains('\0') {
        Err(malformed(field))
    } else {
        Ok(())
    }
}

fn timestamp(value: i64, field: &str) -> Result<u64, LegacyBackupCreationHelperError> {
    u64::try_from(value).map_err(|_| malformed(field))
}

fn report_extra(
    path: &str,
    extra: &BTreeMap<String, Value>,
    notices: &mut Vec<LegacyBackupConversionNotice>,
) {
    for field in extra.keys() {
        notices.push(notice(
            LegacyBackupConversionNoticeKind::Unsupported,
            &format!("{path}.{field}"),
        ));
    }
}

fn notice(kind: LegacyBackupConversionNoticeKind, field: &str) -> LegacyBackupConversionNotice {
    LegacyBackupConversionNotice {
        kind,
        document: LegacyBackupDocumentKind::CreationHelperSessions,
        field: field.to_owned(),
    }
}

fn malformed(field: impl Into<String>) -> LegacyBackupCreationHelperError {
    LegacyBackupCreationHelperError::Malformed {
        field: field.into(),
    }
}

fn orphan(field: impl Into<String>) -> LegacyBackupCreationHelperError {
    LegacyBackupCreationHelperError::Orphan {
        field: field.into(),
    }
}

fn conflict(field: impl Into<String>) -> LegacyBackupCreationHelperError {
    LegacyBackupCreationHelperError::Conflict {
        field: field.into(),
    }
}

fn create_mode() -> String {
    "create".to_owned()
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use serde_json::{Value, json};
    use uuid::Uuid;
    use zeroize::Zeroizing;

    use super::*;
    use crate::{
        LegacyBackupDocument, LegacyBackupInventory, LegacyBackupMedia, plan_legacy_backup_asr,
        plan_legacy_backup_authored, plan_legacy_backup_authored_media,
        plan_legacy_backup_companion_shared_memory, plan_legacy_backup_configuration,
        plan_legacy_backup_direct_sessions, plan_legacy_backup_group_sessions,
        plan_legacy_backup_memory_embeddings, plan_legacy_backup_pricing,
        plan_legacy_backup_scheduled_notes, plan_legacy_backup_usage,
    };

    fn id(value: u128) -> String {
        Uuid::from_u128(value).to_string()
    }

    fn document(kind: LegacyBackupDocumentKind, value: Value) -> LegacyBackupDocument {
        LegacyBackupDocument {
            kind,
            bytes: Zeroizing::new(serde_json::to_vec(&value).expect("fixture document")),
        }
    }

    fn draft() -> Value {
        json!({
            "name": "Mira",
            "definition": "A patient guide",
            "description": null,
            "scenes": [],
            "defaultSceneId": null,
            "avatarPath": null,
            "backgroundImagePath": null,
            "disableAvatarGradient": false,
            "defaultModelId": null,
            "promptTemplateId": null
        })
    }

    fn nested_session(session_id: &str, messages: Value, status: &str) -> Value {
        json!({
            "id": session_id,
            "messages": messages,
            "draft": draft(),
            "draftHistory": [],
            "creationGoal": "character",
            "creationMode": "create",
            "targetType": null,
            "targetId": null,
            "status": status,
            "createdAt": 10,
            "updatedAt": 20
        })
    }

    fn row(session_id: &str, nested: Value, images: Value, status: &str) -> Value {
        json!({
            "id": session_id,
            "creation_goal": "character",
            "status": status,
            "session_json": serde_json::to_string(&nested).expect("session JSON"),
            "uploaded_images_json": serde_json::to_string(&images).expect("image JSON"),
            "created_at": 10,
            "updated_at": 20
        })
    }

    fn source(
        rows: Option<Value>,
        media: Vec<LegacyBackupMedia>,
    ) -> LegacyBackupMemoryEmbeddingPlan {
        let mut documents = Vec::new();
        if let Some(rows) = rows {
            documents.push(document(
                LegacyBackupDocumentKind::CreationHelperSessions,
                rows,
            ));
        }
        let inventory = LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("cc".repeat(32)).expect("source hash"),
            documents,
            media,
        };
        let configuration =
            plan_legacy_backup_configuration(inventory).expect("configuration plan");
        let authored = plan_legacy_backup_authored(configuration).expect("authored plan");
        let media = plan_legacy_backup_authored_media(authored).expect("media plan");
        let asr = plan_legacy_backup_asr(media).expect("ASR plan");
        let usage = plan_legacy_backup_usage(asr).expect("usage plan");
        let pricing = plan_legacy_backup_pricing(usage).expect("pricing plan");
        let direct = plan_legacy_backup_direct_sessions(pricing).expect("direct session plan");
        let group = plan_legacy_backup_group_sessions(direct).expect("group session plan");
        let notes = plan_legacy_backup_scheduled_notes(group).expect("scheduled-note plan");
        let shared = plan_legacy_backup_companion_shared_memory(notes).expect("shared-memory plan");
        plan_legacy_backup_memory_embeddings(shared).expect("memory-embedding plan")
    }

    #[test]
    fn empty_active_creation_session_is_an_initial_draft_seed() {
        let session_id = id(1);
        let nested = nested_session(&session_id, json!([]), "active");
        let rows = json!([row(&session_id, nested, json!({}), "active")]);
        let plan = plan_legacy_backup_creation_helpers(source(Some(rows), Vec::new()))
            .expect("creation-helper plan");
        let session = &plan.sessions[0];
        assert_eq!(session.ordinal, 0);
        assert_eq!(session.session.draft.name.as_deref(), Some("Mira"));
        assert_eq!(session.creation_goal, LegacyBackupCreationGoal::Character);
        assert_eq!(
            session.materialization,
            LegacyBackupCreationMaterialization::InitialDraftSeed
        );
    }

    #[test]
    fn missing_creation_helper_document_is_explicit() {
        let plan = plan_legacy_backup_creation_helpers(source(None, Vec::new()))
            .expect("empty creation-helper plan");
        assert!(plan.sessions.is_empty());
        assert!(plan.notices.iter().any(|notice| {
            notice.document == LegacyBackupDocumentKind::CreationHelperSessions
                && notice.kind == LegacyBackupConversionNoticeKind::Absent
        }));
    }

    #[test]
    fn helper_history_and_archived_upload_remain_exact_evidence() {
        let session_id = id(10);
        let message_id = id(11);
        let messages = json!([{
            "id": message_id,
            "role": "assistant",
            "content": "Draft ready",
            "toolCalls": [{
                "id": "call-1",
                "name": "set_character_name",
                "arguments": {"name": "Mira"}
            }],
            "toolResults": [{
                "toolCallId": "call-1",
                "result": {"success": true},
                "success": true
            }],
            "blocks": [
                {"kind": "text", "content": "Draft ready"},
                {"kind": "tool", "toolCallId": "call-1"}
            ],
            "createdAt": 15
        }]);
        let nested = nested_session(&session_id, messages, "previewShown");
        let images = json!({
            "upload-1": {
                "id": "upload-1",
                "data": "",
                "mimeType": "image/png",
                "assetId": "upload-1"
            }
        });
        let rows = json!([row(&session_id, nested, images, "previewShown")]);
        let bytes = vec![0x89, b'P', b'N', b'G'];
        let expected_hash =
            ContentHash::parse(blake3::hash(&bytes).to_hex().to_string()).expect("content hash");
        let media = vec![LegacyBackupMedia {
            root: LegacyBackupMediaRoot::Images,
            relative_segments: vec!["upload-1.png".into()],
            bytes: Zeroizing::new(bytes),
        }];
        let plan = plan_legacy_backup_creation_helpers(source(Some(rows), media))
            .expect("creation-helper plan");
        let session = &plan.sessions[0];
        assert_eq!(session.session.messages[0].content, "Draft ready");
        assert_eq!(
            session.uploaded_images[0].archive_locator.as_deref(),
            Some("upload-1.png")
        );
        assert_eq!(
            session.uploaded_images[0].archive_content_hash,
            Some(expected_hash)
        );
        assert_eq!(session.uploaded_images[0].inline_content_hash, None);
        assert_eq!(
            session.materialization,
            LegacyBackupCreationMaterialization::RetainedEvidence
        );
    }

    #[test]
    fn helper_plan_rejects_outer_mismatch_duplicates_and_missing_uploads() {
        let session_id = id(20);
        let nested = nested_session(&session_id, json!([]), "active");
        let mismatched = json!([row(&session_id, nested.clone(), json!({}), "completed")]);
        assert!(matches!(
            plan_legacy_backup_creation_helpers(source(Some(mismatched), Vec::new())),
            Err(LegacyBackupCreationHelperError::Conflict { ref field })
                if field == "[0].session_json"
        ));

        let duplicate_row = row(&session_id, nested.clone(), json!({}), "active");
        assert!(matches!(
            plan_legacy_backup_creation_helpers(source(
                Some(json!([duplicate_row.clone(), duplicate_row])),
                Vec::new(),
            )),
            Err(LegacyBackupCreationHelperError::Malformed { ref field })
                if field == "[1].id"
        ));

        let images = json!({
            "upload-2": {
                "id": "upload-2",
                "data": "",
                "mimeType": "image/webp",
                "assetId": "upload-2"
            }
        });
        let missing = json!([row(&session_id, nested, images, "active")]);
        assert!(matches!(
            plan_legacy_backup_creation_helpers(source(Some(missing), Vec::new())),
            Err(LegacyBackupCreationHelperError::Orphan { ref field })
                if field == "[0].uploaded_images_json.upload-2.assetId"
        ));
    }
}
