use crate::HfModelFile;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfFilePart {
    pub filename: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GgufShardName {
    pub stem: String,
    pub index: u32,
    pub count: u32,
}

impl GgufShardName {
    pub fn parse(filename: &str) -> Option<Self> {
        let base = filename.strip_suffix(".gguf")?;
        let (prefix, total) = base.rsplit_once("-of-")?;
        let (stem, index) = prefix.rsplit_once('-')?;
        if index.len() != 5
            || total.len() != 5
            || !index
                .bytes()
                .chain(total.bytes())
                .all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let index = index.parse().ok()?;
        let count = total.parse().ok()?;
        (index > 0 && count > 1 && index <= count).then(|| Self {
            stem: stem.to_owned(),
            index,
            count,
        })
    }

    pub fn filename(&self, index: u32) -> String {
        format!("{}-{index:05}-of-{:05}.gguf", self.stem, self.count)
    }

    pub fn display_name(&self) -> String {
        format!("{}.gguf", self.stem)
    }
}

pub fn group_gguf_files(files: Vec<HfModelFile>) -> Vec<HfModelFile> {
    let mut groups: BTreeMap<(String, u32), Vec<(u32, HfModelFile)>> = BTreeMap::new();
    let mut singles = Vec::new();
    for file in files {
        if let Some(shard) = GgufShardName::parse(&file.filename) {
            groups
                .entry((shard.stem, shard.count))
                .or_default()
                .push((shard.index, file));
        } else {
            singles.push(file);
        }
    }
    for ((stem, _), mut group) in groups {
        group.sort_by_key(|(index, _)| *index);
        let mut file = group[0].1.clone();
        file.display_name = Some(format!("{stem}.gguf"));
        file.size = group
            .iter()
            .fold(0u64, |sum, (_, part)| sum.saturating_add(part.size));
        file.parts = group
            .into_iter()
            .map(|(_, part)| HfFilePart {
                filename: part.filename,
                size: part.size,
            })
            .collect();
        singles.push(file);
    }
    singles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_nested_blob_listing_groups_bf16_and_preserves_true_sizes() {
        let detail = include_bytes!("../tests/fixtures/qwen38-file-list.json");
        let info = crate::model_info(
            "unsloth/Qwen3.8-27B-GGUF",
            detail,
            b"[]",
            crate::HfBrowseMode::Llm,
        )
        .expect("listing");
        assert_eq!(info.files.len(), 28);
        assert_eq!(
            info.files
                .iter()
                .filter(|file| !file.is_mmproj && !file.is_mtp)
                .count(),
            25
        );
        let bf16 = info
            .files
            .iter()
            .find(|file| file.parts.len() == 2)
            .expect("logical BF16");
        assert_eq!(
            bf16.display_name.as_deref(),
            Some("BF16/Qwen3.8-27B-BF16.gguf")
        );
        assert_eq!(bf16.size, 54_657_735_616);
        assert_eq!(
            bf16.parts.iter().map(|part| part.size).collect::<Vec<_>>(),
            [49_986_159_616, 4_671_576_000]
        );
    }

    #[test]
    fn shard_names_retain_directory_total_and_exact_numbering() {
        let shard = GgufShardName::parse("nested/model-00001-of-00002.gguf").expect("split");
        assert_eq!(shard.display_name(), "nested/model.gguf");
        assert_eq!(shard.filename(2), "nested/model-00002-of-00002.gguf");
        for name in [
            "model-1-of-2.gguf",
            "model-00000-of-00002.gguf",
            "model-00003-of-00002.gguf",
            "model-00001-of-00001.gguf",
        ] {
            assert!(GgufShardName::parse(name).is_none());
        }
    }

    #[test]
    fn split_pin_expands_all_real_parts_and_rejects_missing_or_nonfirst_selection() {
        let body = serde_json::json!({"sha": "a".repeat(40), "siblings": [
            {"rfilename":"a/model-00001-of-00002.gguf", "size":100, "blobId":"b".repeat(40)},
            {"rfilename":"a/model-00002-of-00002.gguf", "size":50, "blobId":"c".repeat(40)},
            {"rfilename":"b/model-00001-of-00002.gguf", "size":100, "blobId":"d".repeat(40)}
        ]});
        let bytes = serde_json::to_vec(&body).expect("json");
        let pinned =
            crate::pinned_gguf_files("test/model", &bytes, &["a/model-00001-of-00002.gguf"])
                .expect("complete");
        assert_eq!(pinned.files.len(), 2);
        assert_eq!(pinned.files.iter().map(|file| file.size).sum::<u64>(), 150);
        assert!(pinned.files.iter().all(|file| file.git_blob_id.is_some()));
        assert!(
            crate::pinned_gguf_files("test/model", &bytes, &["b/model-00001-of-00002.gguf"])
                .is_err()
        );
        assert!(
            crate::pinned_gguf_files("test/model", &bytes, &["a/model-00002-of-00002.gguf"])
                .is_err()
        );
    }
}
