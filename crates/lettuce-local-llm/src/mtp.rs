//! Multi-token prediction (speculative decoding with a draft head or a
//! separate draft model): detecting a bundled NextN draft and finding an
//! external `mtp-*.gguf` draft beside the model.

use std::path::Path;

use llama_cpp_2::gguf::GgufContext;

pub const MTP_DRAFT_DEFAULT: u32 = 4;
pub const MTP_DRAFT_MAX: u32 = 8;

/// Whether the model carries its own NextN draft layers.
#[must_use]
pub fn model_has_mtp(model_path: &str) -> bool {
    let Some(gguf) = GgufContext::from_file(Path::new(model_path)) else {
        return false;
    };
    let arch_key = gguf.find_key("general.architecture");
    if arch_key < 0 {
        return false;
    }
    let Some(arch) = gguf.val_str(arch_key) else {
        return false;
    };
    let nextn_idx = gguf.find_key(&format!("{arch}.nextn_predict_layers"));
    nextn_idx >= 0 && gguf.val_u32(nextn_idx) > 0
}

/// The first (by name) `mtp-<stem>*.gguf` or `<stem>-mtp.gguf` beside the
/// model whose stem the model's file name contains.
#[must_use]
pub fn discover_external_mtp(model_path: &str) -> Option<String> {
    let path = Path::new(model_path);
    let model_stem = path.file_name()?.to_str()?.to_lowercase();
    let dir = path.parent()?;
    let entries = std::fs::read_dir(dir).ok()?;

    let mut candidates: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_str()?.to_string();
            let lower = name.to_lowercase();
            if !lower.ends_with(".gguf") {
                return None;
            }
            let stem = lower
                .strip_prefix("mtp-")
                .or_else(|| {
                    lower
                        .strip_suffix("-mtp.gguf")
                        .map(|_| &lower[..lower.len() - 9])
                })?
                .trim_end_matches(".gguf")
                .to_string();
            if stem.is_empty() || !model_stem.contains(&stem) {
                return None;
            }
            Some(entry.path().to_string_lossy().to_string())
        })
        .collect();

    candidates.sort();
    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_drafts_match_by_stem_and_the_first_name_wins() {
        let dir = std::env::temp_dir().join(format!("lettuce-mtp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        for name in [
            "Qwen3-8B-Q4.gguf",
            "mtp-qwen3-8b.gguf",
            "qwen3-8b-mtp.gguf",
            "mtp-other.gguf",
            "notes.txt",
        ] {
            std::fs::write(dir.join(name), b"x").expect("file");
        }
        let model = dir.join("Qwen3-8B-Q4.gguf");
        let found = discover_external_mtp(model.to_str().expect("path")).expect("draft");
        assert!(found.ends_with("mtp-qwen3-8b.gguf"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
