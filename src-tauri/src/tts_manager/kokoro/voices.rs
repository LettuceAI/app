use std::fs;
use std::path::{Path, PathBuf};

use super::model::KokoroError;

pub const STYLE_DIM: usize = 256;

#[derive(Debug, Clone)]
pub struct KokoroVoiceBlendSpec {
    pub voice_id: String,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct KokoroVoice {
    styles: Vec<[f32; STYLE_DIM]>,
}

impl KokoroVoice {
    pub fn load(path: &Path) -> Result<Self, KokoroError> {
        let bytes = fs::read(path)?;
        if bytes.is_empty() {
            return Err(KokoroError::VoiceParse(format!(
                "Voice file is empty: {}",
                path.display()
            )));
        }
        if bytes.len() % 4 != 0 {
            return Err(KokoroError::VoiceParse(format!(
                "Voice file byte length is not aligned to f32 samples: {}",
                path.display()
            )));
        }

        let float_count = bytes.len() / 4;
        if float_count % STYLE_DIM != 0 {
            return Err(KokoroError::VoiceParse(format!(
                "Voice file float count is not a multiple of {}: {}",
                STYLE_DIM,
                path.display()
            )));
        }

        let mut styles = Vec::with_capacity(float_count / STYLE_DIM);
        let mut offset = 0usize;
        while offset < bytes.len() {
            let mut style = [0f32; STYLE_DIM];
            for slot in &mut style {
                *slot = f32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]);
                offset += 4;
            }
            styles.push(style);
        }

        if styles.is_empty() {
            return Err(KokoroError::VoiceParse(format!(
                "Voice file contained no style rows: {}",
                path.display()
            )));
        }

        Ok(Self { styles })
    }

    pub fn style_for_token_count(&self, token_count: usize) -> [f32; STYLE_DIM] {
        let clamped = token_count.min(self.styles.len().saturating_sub(1));
        self.styles[clamped]
    }

    pub fn blend(
        asset_root: &Path,
        specs: &[KokoroVoiceBlendSpec],
    ) -> Result<(Self, Vec<KokoroVoiceBlendSpec>), KokoroError> {
        let normalized = normalize_blend_specs(specs)?;
        if normalized.len() == 1 {
            let only = &normalized[0];
            let path = resolve_voice_path(asset_root, &only.voice_id);
            return Ok((Self::load(&path)?, normalized));
        }

        let mut loaded = Vec::with_capacity(normalized.len());
        let mut max_rows = 0usize;
        for spec in &normalized {
            let path = resolve_voice_path(asset_root, &spec.voice_id);
            let voice = Self::load(&path)?;
            max_rows = max_rows.max(voice.styles.len());
            loaded.push((spec, voice));
        }

        let mut styles = Vec::with_capacity(max_rows);
        for row_index in 0..max_rows {
            let mut blended = [0f32; STYLE_DIM];
            for (spec, voice) in &loaded {
                let source_index = row_index.min(voice.styles.len().saturating_sub(1));
                let source = &voice.styles[source_index];
                for dim in 0..STYLE_DIM {
                    blended[dim] += source[dim] * spec.weight;
                }
            }
            styles.push(blended);
        }

        Ok((Self { styles }, normalized))
    }
}

pub fn resolve_voices_dir(asset_root: &Path) -> PathBuf {
    asset_root.join("voices")
}

pub fn resolve_voice_path(asset_root: &Path, voice_id: &str) -> PathBuf {
    resolve_voices_dir(asset_root).join(format!("{voice_id}.bin"))
}

pub fn list_installed_voice_ids(asset_root: &Path) -> Result<Vec<String>, KokoroError> {
    let voices_dir = resolve_voices_dir(asset_root);
    if !voices_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut voices = Vec::new();
    for entry in fs::read_dir(&voices_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("bin") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if stem.trim().is_empty() {
            continue;
        }
        voices.push(stem.to_string());
    }

    voices.sort_unstable();
    Ok(voices)
}

fn normalize_blend_specs(
    specs: &[KokoroVoiceBlendSpec],
) -> Result<Vec<KokoroVoiceBlendSpec>, KokoroError> {
    if specs.is_empty() {
        return Err(KokoroError::VoiceParse(
            "At least one Kokoro voice must be selected".to_string(),
        ));
    }

    let mut merged: Vec<KokoroVoiceBlendSpec> = Vec::new();
    for spec in specs {
        let voice_id = spec.voice_id.trim();
        if voice_id.is_empty() {
            continue;
        }
        let weight = if spec.weight.is_finite() {
            spec.weight
        } else {
            0.0
        };
        if weight <= 0.0 {
            continue;
        }
        if let Some(existing) = merged.iter_mut().find(|entry| entry.voice_id == voice_id) {
            existing.weight += weight;
        } else {
            merged.push(KokoroVoiceBlendSpec {
                voice_id: voice_id.to_string(),
                weight,
            });
        }
    }

    if merged.is_empty() {
        return Err(KokoroError::VoiceParse(
            "At least one Kokoro voice with positive weight is required".to_string(),
        ));
    }

    let total_weight: f32 = merged.iter().map(|spec| spec.weight).sum();
    if total_weight <= 0.0 {
        return Err(KokoroError::VoiceParse(
            "Kokoro voice blend weights must sum above zero".to_string(),
        ));
    }

    for spec in &mut merged {
        spec.weight /= total_weight;
    }

    Ok(merged)
}
