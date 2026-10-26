use crate::gguf_runnability::GgufReader;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GgufPreviewTensor {
    pub name: String,
    pub dimensions: Vec<u64>,
    pub bytes: u64,
    pub offset: u64,
    pub kind: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GgufPreviewMetadata {
    pub architecture: String,
    pub values: BTreeMap<String, Vec<u64>>,
    pub tensors: Vec<GgufPreviewTensor>,
    pub tensor_data_offset: u64,
    pub split_count: u64,
}

impl GgufPreviewMetadata {
    pub fn scalar(&self, suffix: &str) -> Option<u64> {
        let values = self
            .values
            .get(&format!("{}.{suffix}", self.architecture))?;
        (values.len() == 1).then(|| values[0])
    }

    pub fn layer_value(&self, suffix: &str, layer: usize, layers: usize) -> Option<u64> {
        let values = self
            .values
            .get(&format!("{}.{suffix}", self.architecture))?;
        match values.as_slice() {
            [value] => Some(*value),
            values if values.len() == layers => values.get(layer).copied(),
            _ => None,
        }
    }

    pub fn model_bytes(&self) -> Option<u64> {
        self.tensors
            .iter()
            .try_fold(0u64, |sum, tensor| sum.checked_add(tensor.bytes))
    }
}

fn numeric_values(reader: &mut GgufReader<'_>, kind: u32) -> Option<Vec<u64>> {
    if kind == 9 {
        let inner = reader.read_u32()?;
        if inner == 9 || inner == 8 || inner == 6 || inner == 12 {
            return None;
        }
        let length = usize::try_from(reader.read_u64()?).ok()?;
        if length > reader.remaining() {
            return None;
        }
        (0..length)
            .map(|_| reader.read_value_as_u64(inner))
            .collect()
    } else {
        reader.read_value_as_u64(kind).map(|value| vec![value])
    }
}

pub fn gguf_tensor_storage(kind: u32) -> Option<(u64, u64)> {
    Some(match kind {
        0 => (1, 4),
        1 | 25 | 30 => (1, 2),
        2 | 20 => (32, 18),
        3 => (32, 20),
        6 => (32, 22),
        7 => (32, 24),
        8 => (32, 34),
        9 => (32, 36),
        10 => (256, 84),
        11 => (256, 110),
        12 => (256, 144),
        13 => (256, 176),
        14 => (256, 210),
        15 => (256, 292),
        16 => (256, 66),
        17 => (256, 74),
        18 => (256, 98),
        19 => (256, 50),
        21 => (256, 110),
        22 => (256, 82),
        23 => (256, 136),
        24 => (1, 1),
        26 => (1, 4),
        27 | 28 => (1, 8),
        29 => (256, 56),
        34 => (256, 54),
        35 => (256, 66),
        39 => (32, 17),
        40 => (64, 36),
        41 => (128, 18),
        42 => (64, 18),
        _ => return None,
    })
}

pub fn parse_gguf_preview(data: &[u8]) -> Option<GgufPreviewMetadata> {
    let mut reader = GgufReader::new(data);
    if reader.read_bytes(4)? != b"GGUF" || !(2..=3).contains(&reader.read_u32()?) {
        return None;
    }
    let tensors = reader.read_u64()?;
    let entries = reader.read_u64()?;
    let mut architecture = None;
    let mut values = BTreeMap::new();
    let mut alignment = 32;
    let mut split_count = 1;
    for _ in 0..entries {
        let key = reader.read_string()?;
        let kind = reader.read_u32()?;
        if key == "general.architecture" && kind == 8 {
            architecture = Some(reader.read_string()?);
        } else if key == "general.alignment" {
            alignment = reader.read_value_as_u64(kind)?;
        } else if key == "split.count" {
            split_count = reader.read_value_as_u64(kind)?;
        } else if key.contains(".attention.")
            || key.ends_with(".block_count")
            || key.ends_with(".embedding_length")
            || key.ends_with(".context_length")
            || key.ends_with(".nextn_predict_layers")
        {
            let start = reader.pos;
            if let Some(value) = numeric_values(&mut reader, kind) {
                values.insert(key, value);
            } else {
                reader.pos = start;
                reader.skip_value(kind)?;
            }
        } else {
            reader.skip_value(kind)?;
        }
    }
    if alignment == 0 || !alignment.is_power_of_two() {
        return None;
    }
    let mut names = HashSet::new();
    let mut index = Vec::new();
    for _ in 0..tensors {
        let name = reader.read_string()?;
        if !names.insert(name.clone()) {
            return None;
        }
        let count = reader.read_u32()?;
        if !(1..=4).contains(&count) {
            return None;
        }
        let dimensions = (0..count)
            .map(|_| reader.read_u64())
            .collect::<Option<Vec<_>>>()?;
        if dimensions
            .iter()
            .any(|dimension| *dimension == 0 || *dimension > i64::MAX as u64)
        {
            return None;
        }
        let kind = reader.read_u32()?;
        let offset = reader.read_u64()?;
        let (block, storage) = gguf_tensor_storage(kind)?;
        if dimensions[0] % block != 0 || offset % alignment != 0 {
            return None;
        }
        let cells = dimensions
            .iter()
            .try_fold(1u64, |total, value| total.checked_mul(*value))?;
        if cells > i64::MAX as u64 {
            return None;
        }
        let bytes = (cells / block).checked_mul(storage)?;
        index.push(GgufPreviewTensor {
            name,
            dimensions,
            bytes,
            offset,
            kind,
        });
    }
    let offset =
        u64::try_from(reader.pos).ok()?.checked_add(alignment - 1)? / alignment * alignment;
    Some(GgufPreviewMetadata {
        architecture: architecture?,
        values,
        tensors: index,
        tensor_data_offset: offset,
        split_count,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GgufHeaderBoundary {
    Incomplete { required_prefix_bytes: usize },
    Complete { header_index_bytes: usize },
    Invalid,
}

pub fn gguf_preview_boundary(data: &[u8]) -> GgufHeaderBoundary {
    let mut reader = GgufReader::new(data);
    let scan = |reader: &mut GgufReader<'_>| -> Option<()> {
        if reader.read_bytes(4)? != b"GGUF" || !(2..=3).contains(&reader.read_u32()?) {
            return None;
        }
        let tensors = reader.read_u64()?;
        let entries = reader.read_u64()?;
        for _ in 0..entries {
            reader.read_string()?;
            let kind = reader.read_u32()?;
            reader.skip_value(kind)?;
        }
        for _ in 0..tensors {
            reader.read_string()?;
            let dimensions = reader.read_u32()?;
            if !(1..=4).contains(&dimensions) {
                return None;
            }
            for _ in 0..dimensions {
                reader.read_u64()?;
            }
            reader.read_u32()?;
            reader.read_u64()?;
        }
        Some(())
    };
    if scan(&mut reader).is_some() {
        GgufHeaderBoundary::Complete {
            header_index_bytes: reader.pos,
        }
    } else if reader.needed > data.len() {
        GgufHeaderBoundary::Incomplete {
            required_prefix_bytes: reader.needed,
        }
    } else {
        GgufHeaderBoundary::Invalid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/preview-llama.gguf");

    #[test]
    fn requires_the_entire_tensor_index_and_never_mistakes_essentials_for_complete_metadata() {
        assert!(parse_gguf_preview(&FIXTURE[..512]).is_none());
        assert!(matches!(
            gguf_preview_boundary(&FIXTURE[..512]),
            GgufHeaderBoundary::Incomplete { .. }
        ));
        let complete = parse_gguf_preview(FIXTURE).expect("complete GGUF");
        assert_eq!(complete.architecture, "llama");
        assert_eq!(complete.tensors.len(), 21);
        assert_eq!(complete.scalar("context_length"), Some(32768));
        let prefix = &FIXTURE[..complete.tensor_data_offset as usize];
        assert_eq!(parse_gguf_preview(prefix), Some(complete));
        assert!(matches!(
            gguf_preview_boundary(prefix),
            GgufHeaderBoundary::Complete { .. }
        ));
    }

    #[test]
    fn unknown_tensor_type_is_not_a_request_for_model_weight_bytes() {
        let mut data = FIXTURE.to_vec();
        let marker = data
            .windows("token_embd.weight".len())
            .position(|window| window == b"token_embd.weight")
            .expect("tensor name");
        let kind = marker + "token_embd.weight".len() + 4 + 16;
        data[kind..kind + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_gguf_preview(&data).is_none());
        assert!(matches!(
            gguf_preview_boundary(&data),
            GgufHeaderBoundary::Complete { .. }
        ));
    }

    #[test]
    fn truncated_headers_and_overflowing_dimensions_are_unknown() {
        assert_eq!(gguf_preview_boundary(b"nope"), GgufHeaderBoundary::Invalid);
        assert!(parse_gguf_preview(b"GGUF").is_none());
        let mut data = FIXTURE.to_vec();
        let marker = data
            .windows("token_embd.weight".len())
            .position(|window| window == b"token_embd.weight")
            .expect("tensor name");
        let dimension = marker + "token_embd.weight".len() + 4;
        data[dimension..dimension + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(parse_gguf_preview(&data).is_none());
    }
}
