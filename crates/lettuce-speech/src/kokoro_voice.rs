pub const KOKORO_STYLE_DIMENSIONS: usize = 256;
const MAX_BLEND_ENTRIES: usize = 64;
const STYLE_ROW_BYTES: usize = KOKORO_STYLE_DIMENSIONS * size_of::<f32>();

#[derive(Debug, Clone, PartialEq)]
pub struct KokoroVoiceBlendSpec {
    pub voice_id: String,
    pub weight: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct KokoroVoiceMaterial<'a> {
    pub voice_id: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq)]
pub struct KokoroVoiceBlend {
    normalized_specs: Vec<KokoroVoiceBlendSpec>,
    styles: Vec<[f32; KOKORO_STYLE_DIMENSIONS]>,
}

impl KokoroVoiceBlend {
    #[must_use]
    pub fn normalized_specs(&self) -> &[KokoroVoiceBlendSpec] {
        &self.normalized_specs
    }

    #[must_use]
    pub fn row_count(&self) -> usize {
        self.styles.len()
    }

    #[must_use]
    pub fn style_for_token_count(
        &self,
        token_count: usize,
    ) -> [f32; KOKORO_STYLE_DIMENSIONS] {
        self.styles[token_count.min(self.styles.len().saturating_sub(1))]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KokoroVoiceError {
    #[error("Kokoro voice blend is invalid")]
    InvalidBlend,
    #[error("Kokoro voice data is invalid")]
    InvalidVoiceData,
    #[error("Kokoro voice material is missing")]
    MissingVoice,
}

pub fn normalize_kokoro_voice_blend(
    specs: &[KokoroVoiceBlendSpec],
) -> Result<Vec<KokoroVoiceBlendSpec>, KokoroVoiceError> {
    if specs.is_empty() || specs.len() > MAX_BLEND_ENTRIES {
        return Err(KokoroVoiceError::InvalidBlend);
    }
    let mut merged: Vec<KokoroVoiceBlendSpec> = Vec::new();
    for spec in specs {
        let voice_id = spec.voice_id.trim();
        if !spec.weight.is_finite() {
            return Err(KokoroVoiceError::InvalidBlend);
        }
        if voice_id.is_empty() || spec.weight <= 0.0 {
            continue;
        }
        if !lettuce_model_hub::is_valid_kokoro_voice_id(voice_id) {
            return Err(KokoroVoiceError::InvalidBlend);
        }
        if let Some(existing) = merged.iter_mut().find(|entry| entry.voice_id == voice_id) {
            existing.weight += spec.weight;
            if !existing.weight.is_finite() {
                return Err(KokoroVoiceError::InvalidBlend);
            }
        } else {
            merged.push(KokoroVoiceBlendSpec {
                voice_id: voice_id.to_owned(),
                weight: spec.weight,
            });
        }
    }
    if merged.is_empty() {
        return Err(KokoroVoiceError::InvalidBlend);
    }
    let total = merged.iter().try_fold(0.0_f32, |total, spec| {
        let total = total + spec.weight;
        total.is_finite().then_some(total)
    });
    let Some(total) = total.filter(|total| *total > 0.0) else {
        return Err(KokoroVoiceError::InvalidBlend);
    };
    for spec in &mut merged {
        spec.weight /= total;
    }
    Ok(merged)
}

pub fn blend_kokoro_voices(
    specs: &[KokoroVoiceBlendSpec],
    materials: &[KokoroVoiceMaterial<'_>],
) -> Result<KokoroVoiceBlend, KokoroVoiceError> {
    let normalized_specs = normalize_kokoro_voice_blend(specs)?;
    let mut loaded = Vec::with_capacity(normalized_specs.len());
    for spec in &normalized_specs {
        let mut matches = materials
            .iter()
            .filter(|material| material.voice_id == spec.voice_id);
        let material = matches.next().ok_or(KokoroVoiceError::MissingVoice)?;
        if matches.next().is_some() {
            return Err(KokoroVoiceError::InvalidVoiceData);
        }
        loaded.push(load_voice(material.bytes)?);
    }
    if loaded.len() == 1 {
        return Ok(KokoroVoiceBlend {
            normalized_specs,
            styles: loaded.pop().expect("one loaded voice"),
        });
    }
    let max_rows = loaded
        .iter()
        .map(Vec::len)
        .max()
        .ok_or(KokoroVoiceError::MissingVoice)?;
    let mut styles = Vec::with_capacity(max_rows);
    for row_index in 0..max_rows {
        let mut blended = [0.0_f32; KOKORO_STYLE_DIMENSIONS];
        for (spec, voice) in normalized_specs.iter().zip(&loaded) {
            let source = &voice[row_index.min(voice.len().saturating_sub(1))];
            for (target, value) in blended.iter_mut().zip(source) {
                *target += value * spec.weight;
                if !target.is_finite() {
                    return Err(KokoroVoiceError::InvalidVoiceData);
                }
            }
        }
        styles.push(blended);
    }
    Ok(KokoroVoiceBlend {
        normalized_specs,
        styles,
    })
}

fn load_voice(
    bytes: &[u8],
) -> Result<Vec<[f32; KOKORO_STYLE_DIMENSIONS]>, KokoroVoiceError> {
    if bytes.is_empty() || bytes.len() % STYLE_ROW_BYTES != 0 {
        return Err(KokoroVoiceError::InvalidVoiceData);
    }
    let mut styles = Vec::with_capacity(bytes.len() / STYLE_ROW_BYTES);
    for row in bytes.chunks_exact(STYLE_ROW_BYTES) {
        let mut style = [0.0_f32; KOKORO_STYLE_DIMENSIONS];
        for (slot, encoded) in style.iter_mut().zip(row.chunks_exact(size_of::<f32>())) {
            *slot = f32::from_le_bytes(encoded.try_into().expect("four-byte float"));
            if !slot.is_finite() {
                return Err(KokoroVoiceError::InvalidVoiceData);
            }
        }
        styles.push(style);
    }
    Ok(styles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice_rows(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| {
                std::iter::repeat_n(*value, KOKORO_STYLE_DIMENSIONS)
                    .flat_map(f32::to_le_bytes)
            })
            .collect()
    }

    #[test]
    fn merges_normalizes_extends_and_clamps_like_legacy_blends() {
        let first = voice_rows(&[2.0]);
        let second = voice_rows(&[4.0, 8.0]);
        let blend = blend_kokoro_voices(
            &[
                KokoroVoiceBlendSpec {
                    voice_id: " af_heart ".to_owned(),
                    weight: 0.25,
                },
                KokoroVoiceBlendSpec {
                    voice_id: "bf_emma".to_owned(),
                    weight: 0.5,
                },
                KokoroVoiceBlendSpec {
                    voice_id: "af_heart".to_owned(),
                    weight: 0.25,
                },
                KokoroVoiceBlendSpec {
                    voice_id: "ignored".to_owned(),
                    weight: 0.0,
                },
            ],
            &[
                KokoroVoiceMaterial {
                    voice_id: "af_heart",
                    bytes: &first,
                },
                KokoroVoiceMaterial {
                    voice_id: "bf_emma",
                    bytes: &second,
                },
            ],
        )
        .expect("voice blend");

        assert_eq!(blend.row_count(), 2);
        assert_eq!(blend.normalized_specs()[0].voice_id, "af_heart");
        assert_eq!(blend.normalized_specs()[0].weight, 0.5);
        assert_eq!(blend.style_for_token_count(0), [3.0; KOKORO_STYLE_DIMENSIONS]);
        assert_eq!(blend.style_for_token_count(999), [5.0; KOKORO_STYLE_DIMENSIONS]);
    }

    #[test]
    fn rejects_nonfinite_weights_samples_and_overflow() {
        assert_eq!(
            normalize_kokoro_voice_blend(&[KokoroVoiceBlendSpec {
                voice_id: "af_heart".to_owned(),
                weight: f32::NAN,
            }]),
            Err(KokoroVoiceError::InvalidBlend)
        );
        assert_eq!(
            normalize_kokoro_voice_blend(&[
                KokoroVoiceBlendSpec {
                    voice_id: "af_heart".to_owned(),
                    weight: f32::MAX,
                },
                KokoroVoiceBlendSpec {
                    voice_id: "af_heart".to_owned(),
                    weight: f32::MAX,
                },
            ]),
            Err(KokoroVoiceError::InvalidBlend)
        );
        let invalid = voice_rows(&[f32::INFINITY]);
        assert_eq!(
            blend_kokoro_voices(
                &[KokoroVoiceBlendSpec {
                    voice_id: "af_heart".to_owned(),
                    weight: 1.0,
                }],
                &[KokoroVoiceMaterial {
                    voice_id: "af_heart",
                    bytes: &invalid,
                }],
            ),
            Err(KokoroVoiceError::InvalidVoiceData)
        );
    }
}
