mod appearance;
pub use appearance::*;
mod character;
pub use character::*;
mod companion;
pub use companion::*;

mod starter;
pub use starter::*;

mod persona;
pub use persona::*;

mod prompt;
pub use prompt::*;

mod model;
mod model_serde;
pub use model::*;

use std::collections::BTreeMap;
use std::io::Write;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid UEC 3 document: {0}")]
    Format(String),
    #[error("unsupported UEC version: {0}")]
    UnsupportedVersion(String),
    #[error("asset hash mismatch: {key}")]
    AssetHash { key: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub exported_at: i64,
    pub app: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "entity", rename_all = "snake_case")]
pub enum Entity {
    Character(Box<Character>),
    Lorebook(Lorebook),
    ChatTemplate(ChatTemplate),
    Persona(Persona),
    Prompt(Prompt),
    ModelProfile(Box<ModelProfile>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dependency {
    pub key: String,
    #[serde(flatten)]
    pub entity: Entity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub mime: String,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub schema: Schema,
    pub meta: Meta,
    #[serde(flatten)]
    pub entity: Entity,
    pub dependencies: Vec<Dependency>,
    pub state: Option<State>,
    #[serde(skip)]
    pub assets: BTreeMap<String, Asset>,
}

#[derive(Debug)]
pub struct ReadResult {
    pub document: Document,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Detection {
    RecentMessageWindow,
    LatestUserMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorVersion {
    LegacyV1,
    DeterministicV2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lorebook {
    pub name: String,
    pub icon: Option<String>,
    pub detection: Detection,
    pub behavior_version: BehaviorVersion,
    pub entries: Vec<LorebookEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LorebookEntry {
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub match_mode: lettuce_context::KeywordMatchMode,
    pub content: String,
    pub priority: i32,
}

fn format(error: impl std::fmt::Display) -> Error {
    Error::Format(error.to_string())
}

fn unknown_fields(input: &Value, known: &Value, path: &str, warnings: &mut Vec<String>) {
    match (input, known) {
        (Value::Object(input), Value::Object(known)) => {
            for (key, value) in input {
                let path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if let Some(known) = known.get(key) {
                    unknown_fields(value, known, &path, warnings);
                } else {
                    warnings.push(path);
                }
            }
        }
        (Value::Array(input), Value::Array(known)) => {
            for (index, (value, known)) in input.iter().zip(known).enumerate() {
                unknown_fields(value, known, &format!("{path}[{index}]"), warnings);
            }
        }
        _ => {}
    }
}

pub fn read(
    json: &str,
    mut stage: impl FnMut(&str, &str, &[u8]) -> Result<(), Error>,
) -> Result<ReadResult, Error> {
    let mut input: Value = serde_json::from_str(json).map_err(format)?;
    let schema: Schema = serde_json::from_value(
        input
            .get("schema")
            .cloned()
            .ok_or_else(|| format("schema is required"))?,
    )
    .map_err(format)?;
    if schema.name != "UEC" {
        return Err(format("schema.name must be UEC"));
    }
    let (major, minor) = schema
        .version
        .split_once('.')
        .ok_or_else(|| format("invalid schema.version"))?;
    if major != "3" {
        return Err(Error::UnsupportedVersion(schema.version));
    }
    minor.parse::<u32>().map_err(format)?;
    let assets = input
        .as_object_mut()
        .ok_or_else(|| format("expected object"))?
        .remove("assets")
        .ok_or_else(|| format("assets is required"))?;
    let mut document: Document = serde_json::from_value(input.clone()).map_err(format)?;
    validate_dependencies(&document)?;
    let mut warnings = Vec::new();
    unknown_fields(
        &input,
        &serde_json::to_value(&document).map_err(format)?,
        "",
        &mut warnings,
    );
    for (key, value) in assets
        .as_object()
        .ok_or_else(|| format("assets must be an object"))?
    {
        let asset = Asset::deserialize(value).map_err(format)?;
        let data = value
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| format("asset.data is required"))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(format)?;
        if blake3::hash(&bytes).to_hex().as_str() != asset.blake3 {
            return Err(Error::AssetHash { key: key.clone() });
        }
        let mut known = serde_json::to_value(&asset).map_err(format)?;
        known["data"] = Value::Null;
        unknown_fields(value, &known, &format!("assets.{key}"), &mut warnings);
        stage(key, &asset.mime, &bytes)?;
        document.assets.insert(key.clone(), asset);
    }
    warnings.sort();
    Ok(ReadResult { document, warnings })
}

pub fn write(
    writer: &mut dyn Write,
    document: &Document,
    mut read_asset: impl FnMut(&str, &mut dyn Write) -> Result<(), Error>,
) -> Result<(), Error> {
    validate_dependencies(document)?;
    let mut value = serde_json::to_value(document).map_err(format)?;
    value
        .as_object_mut()
        .ok_or_else(|| format("expected object"))?
        .remove("assets");
    let header = serde_json::to_vec(&value).map_err(format)?;
    writer.write_all(&header[..header.len() - 1])?;
    writer.write_all(b",\"assets\":{")?;
    for (index, (key, asset)) in document.assets.iter().enumerate() {
        if index > 0 {
            writer.write_all(b",")?;
        }
        serde_json::to_writer(&mut *writer, key).map_err(format)?;
        writer.write_all(b":{\"mime\":")?;
        serde_json::to_writer(&mut *writer, &asset.mime).map_err(format)?;
        writer.write_all(b",\"blake3\":")?;
        serde_json::to_writer(&mut *writer, &asset.blake3).map_err(format)?;
        writer.write_all(b",\"data\":\"")?;
        let mut encoder = base64::write::EncoderWriter::new(
            &mut *writer,
            &base64::engine::general_purpose::STANDARD,
        );
        let mut sink = HashWriter {
            writer: &mut encoder,
            hash: blake3::Hasher::new(),
        };
        read_asset(key, &mut sink)?;
        if sink.hash.finalize().to_hex().as_str() != asset.blake3 {
            return Err(Error::AssetHash { key: key.clone() });
        }
        encoder.finish()?;
        drop(encoder);
        writer.write_all(b"\"}")?;
    }
    writer.write_all(b"}}")?;
    Ok(())
}

struct HashWriter<'a> {
    writer: &'a mut dyn Write,
    hash: blake3::Hasher,
}

impl Write for HashWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let written = self.writer.write(bytes)?;
        self.hash.update(&bytes[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl Lorebook {
    #[must_use]
    pub fn from_details(details: &lettuce_context::LorebookDetails, icon: Option<String>) -> Self {
        let mut entries = details.entries.iter().collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.ordinal);
        Self {
            name: details.book.name.clone(),
            icon,
            detection: match details.book.detection_policy {
                lettuce_context::DetectionPolicy::RecentMessageWindow => {
                    Detection::RecentMessageWindow
                }
                lettuce_context::DetectionPolicy::LatestUserMessage => Detection::LatestUserMessage,
            },
            behavior_version: match details.book.behavior_version {
                lettuce_context::LorebookBehaviorVersion::LegacyV1 => BehaviorVersion::LegacyV1,
                lettuce_context::LorebookBehaviorVersion::DeterministicV2 => {
                    BehaviorVersion::DeterministicV2
                }
            },
            entries: entries
                .into_iter()
                .map(|entry| LorebookEntry {
                    title: entry.title.clone(),
                    enabled: entry.enabled,
                    always_active: entry.always_active,
                    keywords: entry.keywords.clone(),
                    case_sensitive: entry.case_sensitive,
                    match_mode: entry.match_mode,
                    content: entry.content.clone(),
                    priority: entry.priority,
                })
                .collect(),
        }
    }

    pub fn into_details(
        &self,
        icon: Option<lettuce_types::AssetId>,
        now: lettuce_types::TimestampMillis,
    ) -> Result<lettuce_context::LorebookDetails, Error> {
        let (metadata, entries) = self.drafts(icon)?;
        let id = lettuce_types::LorebookId::new();
        let details = lettuce_context::LorebookDetails {
            book: lettuce_context::Lorebook {
                id,
                status: lettuce_context::LifecycleStatus::Active,
                name: metadata.name,
                detection_policy: metadata.detection_policy,
                icon_asset_id: icon,
                behavior_version: metadata.behavior_version,
                revision: lettuce_types::Revision::INITIAL,
                created_at: now,
                updated_at: now,
            },
            entries: entries
                .into_iter()
                .enumerate()
                .map(|(index, entry)| {
                    Ok(lettuce_context::LorebookEntry {
                        id: lettuce_types::LorebookEntryId::new(),
                        lorebook_id: id,
                        title: entry.title,
                        enabled: entry.enabled,
                        always_active: entry.always_active,
                        keywords: entry.keywords,
                        case_sensitive: entry.case_sensitive,
                        match_mode: entry.match_mode,
                        content: entry.content,
                        priority: entry.priority,
                        ordinal: u32::try_from(index).map_err(format)?,
                        revision: lettuce_types::Revision::INITIAL,
                        created_at: now,
                        updated_at: now,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?,
        };
        details.validate().map_err(format)?;
        Ok(details)
    }

    pub fn drafts(
        &self,
        icon: Option<lettuce_types::AssetId>,
    ) -> Result<
        (
            lettuce_context::LorebookMetadataDraft,
            Vec<lettuce_context::LorebookEntryDraft>,
        ),
        Error,
    > {
        let metadata = lettuce_context::LorebookMetadataDraft {
            name: self.name.clone(),
            icon_asset_id: icon,
            detection_policy: match self.detection {
                Detection::RecentMessageWindow => {
                    lettuce_context::DetectionPolicy::RecentMessageWindow
                }
                Detection::LatestUserMessage => lettuce_context::DetectionPolicy::LatestUserMessage,
            },
            behavior_version: match self.behavior_version {
                BehaviorVersion::LegacyV1 => lettuce_context::LorebookBehaviorVersion::LegacyV1,
                BehaviorVersion::DeterministicV2 => {
                    lettuce_context::LorebookBehaviorVersion::DeterministicV2
                }
            },
        };
        metadata.validate().map_err(format)?;
        let entries = self
            .entries
            .iter()
            .map(|entry| {
                let draft = lettuce_context::LorebookEntryDraft {
                    title: entry.title.clone(),
                    enabled: entry.enabled,
                    always_active: entry.always_active,
                    keywords: entry.keywords.clone(),
                    case_sensitive: entry.case_sensitive,
                    match_mode: entry.match_mode,
                    content: entry.content.clone(),
                    priority: entry.priority,
                };
                draft.validate().map_err(format)?;
                Ok(draft)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok((metadata, entries))
    }
}

pub fn is_document(json: &str) -> Result<bool, Error> {
    #[derive(Deserialize)]
    struct Header {
        schema: Option<Schema>,
    }
    let Ok(header) = serde_json::from_str::<Header>(json) else {
        return Ok(false);
    };
    let Some(schema) = header.schema.filter(|schema| schema.name == "UEC") else {
        return Ok(false);
    };
    let major = schema.version.split('.').next().unwrap_or_default();
    match major {
        "1" | "2" => Ok(false),
        "3" => Ok(true),
        _ => Err(Error::UnsupportedVersion(schema.version)),
    }
}

fn validate_dependencies(document: &Document) -> Result<(), Error> {
    if document.state.is_some() && !matches!(document.entity, Entity::Character(_)) {
        return Err(format("learned state is only valid for characters"));
    }
    let mut keys = std::collections::BTreeSet::new();
    for dependency in &document.dependencies {
        if dependency.key.is_empty() || !keys.insert(&dependency.key) {
            return Err(format("dependency keys must be nonempty and unique"));
        }
        if !matches!(dependency.entity, Entity::Lorebook(_) | Entity::Prompt(_)) {
            return Err(format("dependencies may only contain lorebooks and prompts"));
        }
    }
    Ok(())
}
