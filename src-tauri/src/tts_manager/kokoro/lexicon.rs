use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::model::KokoroError;

#[derive(Debug, Clone)]
pub struct LexiconOverrides {
    pub path: PathBuf,
    pub entries: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LexiconFile {
    Flat(HashMap<String, String>),
    Scoped {
        #[serde(default)]
        global: HashMap<String, String>,
        #[serde(flatten)]
        by_lang: HashMap<String, HashMap<String, String>>,
    },
}

pub fn resolve_lexicon_path(asset_root: &Path) -> PathBuf {
    asset_root.join("lexicon.json")
}

pub fn load_lexicon_overrides(
    asset_root: &Path,
    lang: &str,
) -> Result<LexiconOverrides, KokoroError> {
    let path = resolve_lexicon_path(asset_root);
    if !path.is_file() {
        return Ok(LexiconOverrides {
            path,
            entries: HashMap::new(),
        });
    }

    let content = std::fs::read_to_string(&path)?;
    let parsed: LexiconFile = serde_json::from_str(&content)
        .map_err(|err| KokoroError::Config(format!("Failed to parse lexicon.json: {err}")))?;

    let mut entries = match parsed {
        LexiconFile::Flat(entries) => entries,
        LexiconFile::Scoped {
            mut global,
            by_lang,
        } => {
            if let Some(lang_entries) = by_lang.get(lang) {
                for (key, value) in lang_entries {
                    global.insert(key.clone(), value.clone());
                }
            }
            global
        }
    };

    entries.retain(|key, value| !key.trim().is_empty() && !value.trim().is_empty());

    Ok(LexiconOverrides { path, entries })
}
