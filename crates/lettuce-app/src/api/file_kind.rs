//! What a picked file holds, found without reading a large file into
//! memory: magic numbers from its first bytes, a PNG's text chunks, a JSONL
//! transcript's first lines, and a JSON document streamed into a sample.

use std::io::{BufRead, BufReader, Read, SeekFrom};

use lettuce_contracts as dto;
use lettuce_media::MediaKind;
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

use super::files::{FileAccessError, FileReader};

pub(super) const HEADER_BYTES: usize = 64 * 1024;
/// How much of each string and how many items of each list or object the
/// JSON sample keeps; the detectors read only a document's shape and short
/// fields, so the rest is streamed past.
const SAMPLE_STRING_BYTES: usize = 64 * 1024;
const SAMPLE_ITEMS: usize = 256;
const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";
const GGUF_MAGIC: &[u8] = b"GGUF";
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
const PNG_TEXT_CHUNKS: [&[u8; 4]; 5] = [b"IHDR", b"tEXt", b"iTXt", b"zTXt", b"IEND"];

fn io(_: std::io::Error) -> FileAccessError {
    FileAccessError::Io
}

pub(super) fn read_header(reader: &mut dyn FileReader) -> Result<Vec<u8>, FileAccessError> {
    let mut header = Vec::with_capacity(HEADER_BYTES);
    reader
        .take(HEADER_BYTES as u64)
        .read_to_end(&mut header)
        .map_err(io)?;
    Ok(header)
}

pub(super) fn detect_kind(reader: &mut dyn FileReader) -> Result<dto::FileKind, FileAccessError> {
    let header = read_header(reader)?;
    if header.starts_with(SQLITE_MAGIC) {
        return Ok(dto::FileKind::LegacyDatabase);
    }
    if header.starts_with(GGUF_MAGIC) {
        return Ok(dto::FileKind::GgufModel);
    }
    match lettuce_transfer::detect_backup_format(&header) {
        Ok(lettuce_transfer::BackupFormatVersion::CurrentV2) => return Ok(dto::FileKind::BackupV2),
        Ok(lettuce_transfer::BackupFormatVersion::LegacyV1) => return Ok(dto::FileKind::BackupV1),
        Err(_) => {}
    }
    if header.starts_with(PNG_MAGIC) && png_holds_a_card(reader)? {
        return Ok(dto::FileKind::CharacterCard);
    }
    match lettuce_media::sniff_media_kind(&header) {
        Some(MediaKind::Image) => return Ok(dto::FileKind::Image),
        Some(MediaKind::Audio) => return Ok(dto::FileKind::Audio),
        Some(MediaKind::Video | MediaKind::Document) | None => {}
    }
    reader.seek(SeekFrom::Start(0)).map_err(io)?;
    detect_text_kind(reader)
}

/// Copies only the PNG's header and text chunks, seeking past image data,
/// and asks the card reader whether they hold a character card.
fn png_holds_a_card(reader: &mut dyn FileReader) -> Result<bool, FileAccessError> {
    reader
        .seek(SeekFrom::Start(PNG_MAGIC.len() as u64))
        .map_err(io)?;
    let mut kept = PNG_MAGIC.to_vec();
    loop {
        let mut head = [0_u8; 8];
        if reader.read_exact(&mut head).is_err() {
            break;
        }
        let length = u64::from(u32::from_be_bytes([head[0], head[1], head[2], head[3]]));
        let kind = [head[4], head[5], head[6], head[7]];
        if PNG_TEXT_CHUNKS.contains(&&kind) {
            let mut rest = Vec::new();
            reader.take(length + 4).read_to_end(&mut rest).map_err(io)?;
            kept.extend_from_slice(&head);
            kept.extend_from_slice(&rest);
            if &kind == b"IEND" {
                break;
            }
        } else {
            let skip = i64::try_from(length + 4).map_err(|_| FileAccessError::Io)?;
            reader.seek(SeekFrom::Current(skip)).map_err(io)?;
        }
    }
    Ok(lettuce_transfer::extract_character_json_from_png(&kept)
        .ok()
        .and_then(|json| serde_json::from_str::<Value>(&json).ok())
        .is_some_and(|value| lettuce_transfer::detect_character_format(&value).is_some()))
}

fn detect_text_kind(reader: &mut dyn FileReader) -> Result<dto::FileKind, FileAccessError> {
    let mut lines = BufReader::new(&mut *reader);
    let mut first = Vec::new();
    lines.read_until(b'\n', &mut first).map_err(io)?;
    let first_line = String::from_utf8_lossy(&first);
    let first_line = first_line.trim_start_matches('\u{feff}').trim();
    if first_line.starts_with('{')
        && serde_json::from_str::<Value>(first_line).is_ok_and(|value| value.is_object())
    {
        let mut second = String::new();
        while second.trim().is_empty() {
            second.clear();
            if lines.read_line(&mut second).map_err(io)? == 0 {
                break;
            }
        }
        if !second.trim().is_empty() {
            let sample = format!("{first_line}\n{}", second.trim());
            return Ok(if lettuce_transfer::parse_chat_jsonl(&sample, 0).is_ok() {
                dto::FileKind::ChatJsonl
            } else {
                dto::FileKind::Other
            });
        }
    }
    drop(lines);
    reader.seek(SeekFrom::Start(0)).map_err(io)?;
    let mut document = BufReader::new(&mut *reader);
    let mut bom = [0_u8; 3];
    let mut peeked = 0;
    while peeked < 3 {
        match document.read(&mut bom[peeked..]).map_err(io)? {
            0 => break,
            read => peeked += read,
        }
    }
    let prefix = if bom[..peeked] == [0xef, 0xbb, 0xbf] {
        Vec::new()
    } else {
        bom[..peeked].to_vec()
    };
    let mut deserializer = serde_json::Deserializer::from_reader(prefix.chain(document));
    let Ok(value) = Sample::TOP.deserialize(&mut deserializer) else {
        return Ok(dto::FileKind::Other);
    };
    if deserializer.end().is_err() || !value.is_object() {
        return Ok(dto::FileKind::Other);
    }
    Ok(json_kind(&value))
}

fn json_kind(value: &Value) -> dto::FileKind {
    if lettuce_transfer::looks_like_uec(value) {
        return if lettuce_transfer::parse_persona_import(value, 0).is_ok() {
            dto::FileKind::PersonaFile
        } else {
            dto::FileKind::CharacterCard
        };
    }
    let text = value.to_string();
    if lettuce_transfer::detect_character_format(value).is_some() {
        dto::FileKind::CharacterCard
    } else if lettuce_transfer::parse_persona_import(value, 0).is_ok() {
        dto::FileKind::PersonaFile
    } else if lettuce_transfer::parse_world_info(&text).is_ok() {
        dto::FileKind::Lorebook
    } else if lettuce_transfer::parse_prompt_import(&text, None, "Imported", "Imported").is_ok() {
        dto::FileKind::PromptPreset
    } else {
        dto::FileKind::Other
    }
}

/// Builds a JSON value from a stream, keeping each string's first
/// `SAMPLE_STRING_BYTES` and, below the top level, each list's and object's
/// first `SAMPLE_ITEMS` items; the top level keeps every field.
#[derive(Clone, Copy)]
struct Sample {
    top: bool,
}

impl Sample {
    const TOP: Self = Self { top: true };
    const NESTED: Self = Self { top: false };
}

impl<'de> DeserializeSeed<'de> for Sample {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(SampleVisitor(self))
    }
}

struct SampleVisitor(Sample);

fn prefix(text: &str) -> String {
    if text.len() <= SAMPLE_STRING_BYTES {
        return text.to_owned();
    }
    let mut end = SAMPLE_STRING_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

impl<'de> Visitor<'de> for SampleVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(prefix(value)))
    }

    fn visit_string<E>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(prefix(&value)))
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        self.0.deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(Sample::NESTED)? {
            if self.0.top || items.len() < SAMPLE_ITEMS {
                items.push(item);
            }
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let value = map.next_value_seed(Sample::NESTED)?;
            if self.0.top || object.len() < SAMPLE_ITEMS {
                object.insert(key, value);
            }
        }
        Ok(Value::Object(object))
    }
}
