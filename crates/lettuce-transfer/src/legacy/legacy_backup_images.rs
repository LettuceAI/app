//! Legacy image-generation rows that only the live legacy database holds:
//! the local LoRA library (`image_loras`) and the playground history
//! (`playground_generations`). LoRA rows keep their device-local
//! paths; values the new library rejects are replaced and recorded, and
//! keywords are normalized the way legacy read them back (trimmed, unique,
//! at most 32).

use std::collections::{BTreeMap, BTreeSet};

use lettuce_image_generation::sd_runtime::lora_library::normalize_lora_keywords;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    LegacyBackupConversionNotice, LegacyBackupConversionNoticeKind, LegacyBackupDocumentKind,
    LegacyBackupInventory, LegacyImportSkip, LegacyImportSkipReason, legacy_value_skip,
};

const KEYWORD_SOURCES: [&str; 4] = ["none", "metadata", "civitai", "manual"];
const ARCHITECTURE_SOURCES: [&str; 3] = ["none", "metadata", "civitai"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImageLoraRecord {
    pub path: String,
    pub filename: String,
    pub bytes_on_disk: u64,
    pub modified_at: u64,
    pub sha256: Option<String>,
    pub keywords: Vec<String>,
    pub keyword_source: String,
    pub architecture: Option<String>,
    pub architecture_source: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// One legacy playground history entry, kept as legacy stored it; its
/// images are resolved to media by the media plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyPlaygroundGeneration {
    pub source_id: String,
    pub created_at: i64,
    pub provider_id: String,
    pub model_id: String,
    pub model_name: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub seed: Option<i64>,
    pub params_json: String,
    pub status: String,
    pub error: Option<String>,
    pub images: Vec<LegacyPlaygroundImage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyPlaygroundImage {
    pub legacy_asset_id: String,
    pub mime_type: Option<String>,
    pub url: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LegacyBackupImagePlan {
    pub loras: Vec<LegacyImageLoraRecord>,
    pub playground: Vec<LegacyPlaygroundGeneration>,
    pub notices: Vec<LegacyBackupConversionNotice>,
    pub skipped: Vec<LegacyImportSkip>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyBackupImageError {
    #[error("legacy image document is malformed")]
    Malformed { field: String },
    #[error("legacy image document exceeds its record limit")]
    LimitExceeded,
}

#[derive(Deserialize)]
struct PlaygroundRow {
    id: String,
    created_at: i64,
    provider_id: String,
    model_id: String,
    model_name: String,
    prompt: String,
    negative_prompt: Option<String>,
    seed: Option<i64>,
    params_json: String,
    status: String,
    error: Option<String>,
    images_json: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaygroundImageRow {
    asset_id: String,
    #[serde(default)]
    mime_type: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    width: Option<Value>,
    #[serde(default)]
    height: Option<Value>,
}

/// The images of a legacy `images_json` column the way legacy parsed it (an
/// unreadable column is no images); entries without an asset id are dropped.
#[must_use]
pub fn legacy_playground_images(raw: &str) -> Option<Vec<LegacyPlaygroundImage>> {
    let values = serde_json::from_str::<Vec<Value>>(raw).ok()?;
    Some(
        values
            .into_iter()
            .filter_map(|value| serde_json::from_value::<PlaygroundImageRow>(value).ok())
            .filter(|image| !image.asset_id.trim().is_empty())
            .map(|image| LegacyPlaygroundImage {
                legacy_asset_id: image.asset_id,
                mime_type: image.mime_type,
                url: image.url,
                width: image
                    .width
                    .and_then(|value| value.as_u64())
                    .and_then(|value| u32::try_from(value).ok()),
                height: image
                    .height
                    .and_then(|value| value.as_u64())
                    .and_then(|value| u32::try_from(value).ok()),
            })
            .collect(),
    )
}

#[derive(Deserialize)]
struct LoraRow {
    path: String,
    filename: String,
    bytes_on_disk: i64,
    modified_at: i64,
    sha256: Option<String>,
    keywords: Option<String>,
    keyword_source: Option<String>,
    architecture: Option<String>,
    architecture_source: Option<String>,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

/// Legacy `image_loras`, absent from archive backups (legacy never exported
/// it), read from the live legacy database.
pub fn plan_legacy_backup_images(
    inventory: &LegacyBackupInventory,
) -> Result<LegacyBackupImagePlan, LegacyBackupImageError> {
    let mut plan = LegacyBackupImagePlan::default();
    plan_playground(inventory, &mut plan)?;
    plan_loras(inventory, &mut plan)?;
    plan.notices.sort();
    plan.notices.dedup();
    plan.skipped.sort();
    Ok(plan)
}

fn plan_loras(
    inventory: &LegacyBackupInventory,
    plan: &mut LegacyBackupImagePlan,
) -> Result<(), LegacyBackupImageError> {
    let Some(document) = inventory
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::ImageLoras)
    else {
        return Ok(());
    };
    let rows: Vec<LoraRow> = serde_json::from_slice(&document.bytes).map_err(|_| malformed("$"))?;
    let mut paths = BTreeSet::new();
    for (index, row) in rows.into_iter().enumerate() {
        if !row.extra.is_empty() {
            plan.notices.push(LegacyBackupConversionNotice {
                kind: LegacyBackupConversionNoticeKind::Unsupported,
                document: LegacyBackupDocumentKind::ImageLoras,
                field: format!("[{index}]"),
            });
        }
        if row.path.is_empty() || !paths.insert(row.path.clone()) {
            plan.skipped.push(legacy_value_skip(
                "image_loras.path",
                &index.to_string(),
                LegacyImportSkipReason::MalformedLegacyValue,
            ));
            continue;
        }
        plan.loras.push(map_row(row, &mut plan.skipped));
    }
    Ok(())
}

fn plan_playground(
    inventory: &LegacyBackupInventory,
    plan: &mut LegacyBackupImagePlan,
) -> Result<(), LegacyBackupImageError> {
    let Some(document) = inventory
        .documents
        .iter()
        .find(|document| document.kind == LegacyBackupDocumentKind::PlaygroundGenerations)
    else {
        return Ok(());
    };
    let rows: Vec<PlaygroundRow> =
        serde_json::from_slice(&document.bytes).map_err(|_| malformed("$"))?;
    let mut ids = BTreeSet::new();
    for (index, row) in rows.into_iter().enumerate() {
        if !row.extra.is_empty() {
            plan.notices.push(LegacyBackupConversionNotice {
                kind: LegacyBackupConversionNoticeKind::Unsupported,
                document: LegacyBackupDocumentKind::PlaygroundGenerations,
                field: format!("[{index}]"),
            });
        }
        if row.id.trim().is_empty() || !ids.insert(row.id.clone()) {
            plan.skipped.push(legacy_value_skip(
                "playground_generations.id",
                &index.to_string(),
                LegacyImportSkipReason::MalformedLegacyValue,
            ));
            continue;
        }
        let parsed = legacy_playground_images(&row.images_json);
        let lossless = parsed.as_ref().is_some_and(|images| {
            serde_json::from_str::<Vec<Value>>(&row.images_json).is_ok_and(|raw| {
                raw.len() == images.len()
                    && raw.iter().zip(images).all(|(raw, image)| {
                        ["width", "height"]
                            .iter()
                            .zip([image.width, image.height])
                            .all(|(field, parsed)| {
                                raw.get(*field).is_none_or(Value::is_null) || parsed.is_some()
                            })
                    })
            })
        });
        if !lossless {
            plan.skipped.push(legacy_value_skip(
                "playground_generations.images_json",
                &row.id,
                LegacyImportSkipReason::MalformedLegacyValue,
            ));
        }
        let images = parsed.unwrap_or_default();
        plan.playground.push(LegacyPlaygroundGeneration {
            source_id: row.id,
            created_at: row.created_at,
            provider_id: row.provider_id,
            model_id: row.model_id,
            model_name: row.model_name,
            prompt: row.prompt,
            negative_prompt: row.negative_prompt,
            seed: row.seed,
            params_json: row.params_json,
            status: row.status,
            error: row.error,
            images,
        });
    }
    Ok(())
}

fn map_row(row: LoraRow, skipped: &mut Vec<LegacyImportSkip>) -> LegacyImageLoraRecord {
    let path = row.path;
    let mut skip = |field: &str, reason| skipped.push(legacy_value_skip(field, &path, reason));
    let count = |value: i64, field: &str, skip: &mut dyn FnMut(&str, LegacyImportSkipReason)| {
        u64::try_from(value).unwrap_or_else(|_| {
            skip(field, LegacyImportSkipReason::MalformedLegacyValue);
            0
        })
    };
    let bytes_on_disk = count(row.bytes_on_disk, "image_loras.bytes_on_disk", &mut skip);
    let modified_at = count(row.modified_at, "image_loras.modified_at", &mut skip);
    let sha256 = row.sha256.and_then(|value| {
        let value = value.to_ascii_lowercase();
        let valid = value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
        if !valid {
            skip(
                "image_loras.sha256",
                LegacyImportSkipReason::MalformedLegacyValue,
            );
        }
        valid.then_some(value)
    });
    let keywords = match row.keywords.as_deref() {
        None => Vec::new(),
        Some(raw) => match serde_json::from_str::<Vec<String>>(raw) {
            Ok(keywords) => normalize_lora_keywords(keywords),
            Err(_) => {
                skip(
                    "image_loras.keywords",
                    LegacyImportSkipReason::MalformedLegacyValue,
                );
                Vec::new()
            }
        },
    };
    let mut source = |value: Option<String>, allowed: &[&str], field: &str| match value {
        None => "none".to_owned(),
        Some(value) if allowed.contains(&value.as_str()) => value,
        Some(_) => {
            skip(field, LegacyImportSkipReason::UnknownLegacyValue);
            "none".to_owned()
        }
    };
    let keyword_source = source(
        row.keyword_source,
        &KEYWORD_SOURCES,
        "image_loras.keyword_source",
    );
    let architecture_source = source(
        row.architecture_source,
        &ARCHITECTURE_SOURCES,
        "image_loras.architecture_source",
    );
    LegacyImageLoraRecord {
        filename: row.filename,
        bytes_on_disk,
        modified_at,
        sha256,
        keywords,
        keyword_source,
        architecture: row.architecture,
        architecture_source,
        created_at: row.created_at,
        updated_at: row.updated_at,
        path,
    }
}

fn malformed(field: &str) -> LegacyBackupImageError {
    LegacyBackupImageError::Malformed {
        field: field.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use lettuce_types::ContentHash;
    use serde_json::json;

    use super::*;
    use crate::{LegacyBackupDocument, LegacyBackupMedia};

    fn inventory(rows: Value) -> LegacyBackupInventory {
        LegacyBackupInventory {
            version: 1,
            created_at: 1,
            app_version: "legacy".into(),
            source_hash: ContentHash::parse("ef".repeat(32)).expect("hash"),
            documents: vec![LegacyBackupDocument {
                kind: LegacyBackupDocumentKind::ImageLoras,
                bytes: zeroize::Zeroizing::new(serde_json::to_vec(&rows).expect("rows")),
            }],
            media: Vec::<LegacyBackupMedia>::new(),
        }
    }

    fn row(path: &str) -> Value {
        json!({
            "path": path,
            "filename": "style.safetensors",
            "bytes_on_disk": 10,
            "modified_at": 20,
            "sha256": "ab".repeat(32),
            "keywords": "[\"ink\",\"wash\"]",
            "keyword_source": "civitai",
            "architecture": "sdxl",
            "architecture_source": "civitai",
            "created_at": 3,
            "updated_at": 4
        })
    }

    #[test]
    fn lora_rows_keep_their_values_and_record_what_the_library_rejects() {
        let mut odd = row("/loras/odd.safetensors");
        odd["sha256"] = json!("é".repeat(32));
        odd["keywords"] = json!("not json");
        odd["keyword_source"] = json!("guessed");
        odd["bytes_on_disk"] = json!(-1);
        odd["legacy_column"] = json!(true);
        let mut style = row("/loras/style.safetensors");
        style["sha256"] = json!("AB".repeat(32));
        style["keywords"] = json!(
            serde_json::to_string(
                &std::iter::once(" ink ".to_owned())
                    .chain(["INK".to_owned(), "wash".to_owned()])
                    .chain((0..300).map(|index| format!("k{index}")))
                    .collect::<Vec<_>>()
            )
            .expect("keywords")
        );
        let plan = plan_legacy_backup_images(&inventory(json!([
            style,
            odd,
            row("/loras/style.safetensors"),
            row(""),
        ])))
        .expect("plan");
        assert_eq!(plan.loras.len(), 2);
        let style = &plan.loras[0];
        assert_eq!(style.keywords.len(), 32);
        assert_eq!(style.keywords[..2], ["ink".to_owned(), "wash".to_owned()]);
        assert_eq!(
            (
                style.keyword_source.as_str(),
                style.architecture_source.as_str()
            ),
            ("civitai", "civitai")
        );
        assert_eq!((style.bytes_on_disk, style.modified_at), (10, 20));
        assert_eq!(style.sha256, Some("ab".repeat(32)));
        let odd = &plan.loras[1];
        assert_eq!(odd.sha256, None);
        assert!(odd.keywords.is_empty());
        assert_eq!(odd.keyword_source, "none");
        assert_eq!(odd.bytes_on_disk, 0);
        let skip = |field: &str, row: &str, reason| legacy_value_skip(field, row, reason);
        let mut expected = vec![
            skip(
                "image_loras.sha256",
                "/loras/odd.safetensors",
                LegacyImportSkipReason::MalformedLegacyValue,
            ),
            skip(
                "image_loras.keywords",
                "/loras/odd.safetensors",
                LegacyImportSkipReason::MalformedLegacyValue,
            ),
            skip(
                "image_loras.keyword_source",
                "/loras/odd.safetensors",
                LegacyImportSkipReason::UnknownLegacyValue,
            ),
            skip(
                "image_loras.bytes_on_disk",
                "/loras/odd.safetensors",
                LegacyImportSkipReason::MalformedLegacyValue,
            ),
            skip(
                "image_loras.path",
                "2",
                LegacyImportSkipReason::MalformedLegacyValue,
            ),
            skip(
                "image_loras.path",
                "3",
                LegacyImportSkipReason::MalformedLegacyValue,
            ),
        ];
        expected.sort();
        assert_eq!(plan.skipped, expected);
        assert_eq!(plan.notices.len(), 1);
    }

    #[test]
    fn playground_rows_keep_their_values_and_record_unreadable_images() {
        let entry = |id: &str, images_json: &str| {
            json!({
                "id": id,
                "created_at": 5,
                "provider_id": "sdcpp",
                "model_id": "model",
                "model_name": "Flux",
                "prompt": "harbor",
                "negative_prompt": null,
                "seed": 7,
                "params_json": "not json",
                "status": "complete",
                "error": null,
                "images_json": images_json
            })
        };
        let mut inventory = inventory(json!([]));
        inventory.documents = vec![LegacyBackupDocument {
            kind: LegacyBackupDocumentKind::PlaygroundGenerations,
            bytes: zeroize::Zeroizing::new(
                serde_json::to_vec(&json!([
                    entry(
                        "a",
                        "[{\"assetId\":\"img-1\",\"width\":512,\"height\":-1},{\"filePath\":\"x\"}]"
                    ),
                    entry("b", "broken"),
                    entry("a", "[]"),
                ]))
                .expect("rows"),
            ),
        }];
        let plan = plan_legacy_backup_images(&inventory).expect("plan");
        assert_eq!(plan.playground.len(), 2);
        let first = &plan.playground[0];
        assert_eq!(first.params_json, "not json");
        assert_eq!(first.seed, Some(7));
        assert_eq!(
            first.images,
            vec![LegacyPlaygroundImage {
                legacy_asset_id: "img-1".to_owned(),
                mime_type: None,
                url: None,
                width: Some(512),
                height: None,
            }]
        );
        assert!(plan.playground[1].images.is_empty());
        let mut expected = vec![
            legacy_value_skip(
                "playground_generations.images_json",
                "a",
                LegacyImportSkipReason::MalformedLegacyValue,
            ),
            legacy_value_skip(
                "playground_generations.images_json",
                "b",
                LegacyImportSkipReason::MalformedLegacyValue,
            ),
            legacy_value_skip(
                "playground_generations.id",
                "2",
                LegacyImportSkipReason::MalformedLegacyValue,
            ),
        ];
        expected.sort();
        assert_eq!(plan.skipped, expected);
    }

    #[test]
    fn a_source_without_the_table_imports_no_loras() {
        let mut empty = inventory(json!([]));
        empty.documents.clear();
        assert_eq!(
            plan_legacy_backup_images(&empty).expect("plan"),
            LegacyBackupImagePlan::default()
        );
    }
}
