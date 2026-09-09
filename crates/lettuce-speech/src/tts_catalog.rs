use std::collections::BTreeMap;

use crate::AudioProviderKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtsCatalogModel {
    pub id: &'static str,
    pub name: &'static str,
    pub provider_kind: AudioProviderKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtsCatalogVoice {
    pub id: &'static str,
    pub name: &'static str,
    pub labels: BTreeMap<&'static str, &'static str>,
}

#[must_use]
pub fn tts_catalog_models(provider_kind: AudioProviderKind) -> Vec<TtsCatalogModel> {
    if provider_kind == AudioProviderKind::Kokoro {
        return lettuce_model_hub::kokoro_supported_model_variants()
            .into_iter()
            .map(|variant| TtsCatalogModel {
                id: variant.id,
                name: variant.label,
                provider_kind,
            })
            .collect();
    }
    let values: &[(&str, &str)] = match provider_kind {
        AudioProviderKind::GeminiTts => &[
            ("gemini-2.5-flash-tts", "Gemini 2.5 Flash TTS"),
            (
                "gemini-2.5-flash-lite-preview-tts",
                "Gemini 2.5 Flash Lite TTS (Preview)",
            ),
            ("gemini-2.5-pro-tts", "Gemini 2.5 Pro TTS"),
        ],
        AudioProviderKind::Elevenlabs => &[
            ("eleven_v3", "Eleven v3 (5K chars)"),
            ("eleven_flash_v2_5", "Flash v2.5 (40K chars)"),
            ("eleven_flash_v2", "Flash v2 (30K chars)"),
            ("eleven_turbo_v2_5", "Turbo v2.5 (40K chars)"),
            ("eleven_turbo_v2", "Turbo v2 (30K chars)"),
            ("eleven_multilingual_v2", "Multilingual v2 (10K chars)"),
            ("eleven_multilingual_v1", "Multilingual v1 (10K chars)"),
            ("eleven_english_sts_v2", "English STS v2 (10K chars)"),
        ],
        AudioProviderKind::FishTts => &[("s2-pro", "S2 Pro"), ("s1", "S1")],
        AudioProviderKind::FishSpeech => {
            &[("server-default", "Server Default (configured at startup)")]
        }
        AudioProviderKind::OpenAiTts => &[
            ("gpt-4o-mini-tts", "gpt-4o-mini-tts"),
            ("tts-1", "tts-1"),
            ("tts-1-hd", "tts-1-hd"),
        ],
        AudioProviderKind::Kokoro => unreachable!(),
    };
    values
        .iter()
        .map(|(id, name)| TtsCatalogModel {
            id,
            name,
            provider_kind,
        })
        .collect()
}

#[must_use]
pub fn tts_voice_design_models(provider_kind: AudioProviderKind) -> Vec<TtsCatalogModel> {
    if provider_kind != AudioProviderKind::Elevenlabs {
        return tts_catalog_models(provider_kind);
    }
    [
        (
            "eleven_multilingual_ttv_v2",
            "Multilingual TTV v2 (Default)",
        ),
        ("eleven_ttv_v3", "TTV v3 (Latest)"),
    ]
    .into_iter()
    .map(|(id, name)| TtsCatalogModel {
        id,
        name,
        provider_kind,
    })
    .collect()
}

#[must_use]
pub fn tts_catalog_voices(provider_kind: AudioProviderKind) -> Vec<TtsCatalogVoice> {
    if provider_kind != AudioProviderKind::GeminiTts {
        return Vec::new();
    }
    [
        ("kore", "Kore", "female", "Warm and friendly"),
        ("aoede", "Aoede", "female", "Bright and articulate"),
        ("algieba", "Algieba", "male", "Professional and clear"),
        (
            "callirrhoe",
            "Callirrhoe",
            "female",
            "Expressive and dynamic",
        ),
        ("leda", "Leda", "female", "Calm and soothing"),
        ("puck", "Puck", "male", "Energetic and youthful"),
        ("charon", "Charon", "male", "Deep and authoritative"),
        ("fenrir", "Fenrir", "male", "Strong and bold"),
        ("orus", "Orus", "male", "Warm and resonant"),
        ("zephyr", "Zephyr", "neutral", "Light and airy"),
    ]
    .into_iter()
    .map(|(id, name, gender, description)| TtsCatalogVoice {
        id,
        name,
        labels: BTreeMap::from([("gender", gender), ("description", description)]),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_exact_model_and_design_catalogs() {
        let kinds = [
            AudioProviderKind::GeminiTts,
            AudioProviderKind::Elevenlabs,
            AudioProviderKind::FishTts,
            AudioProviderKind::FishSpeech,
            AudioProviderKind::OpenAiTts,
            AudioProviderKind::Kokoro,
        ];
        let counts: Vec<_> = kinds
            .into_iter()
            .map(|kind| tts_catalog_models(kind).len())
            .collect();
        let kokoro_count = if cfg!(any(target_os = "android", target_os = "ios")) {
            1
        } else {
            3
        };
        assert_eq!(counts, [3, 8, 2, 1, 3, kokoro_count]);
        assert_eq!(
            tts_voice_design_models(AudioProviderKind::Elevenlabs)
                .iter()
                .map(|model| model.id)
                .collect::<Vec<_>>(),
            ["eleven_multilingual_ttv_v2", "eleven_ttv_v3"]
        );
        assert_eq!(
            tts_catalog_models(AudioProviderKind::FishSpeech)[0].name,
            "Server Default (configured at startup)"
        );
    }

    #[test]
    fn exposes_only_the_exact_built_in_gemini_voices() {
        let voices = tts_catalog_voices(AudioProviderKind::GeminiTts);
        assert_eq!(voices.len(), 10);
        assert_eq!(voices[0].id, "kore");
        assert_eq!(voices[0].labels["gender"], "female");
        assert_eq!(voices.last().expect("last voice").id, "zephyr");
        assert!(tts_catalog_voices(AudioProviderKind::Elevenlabs).is_empty());
    }
}
