//! The llama.cpp settings one request runs with, resolved like legacy
//! `resolve_llama_*`: each field from the session layer, then the model,
//! then the app layer. A single-GPU pin is dropped when multi-GPU is enabled
//! at the same or a more specific layer. Dynamic memory requests may replace
//! the sampler with the legacy fixed memory sampler.

use serde::{Deserialize, Serialize};

use crate::{LlamaCppSettings, LlamaSamplerSettings, LlamaSamplerStage};

/// Layers besides the model's own settings. For app features the session
/// layer is the feature slot's sampler, as legacy's synthetic session was.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlamaResolutionInput {
    #[serde(default)]
    pub global: LlamaCppSettings,
    #[serde(default)]
    pub session: LlamaCppSettings,
    #[serde(default)]
    pub memory_sampler: Option<LlamaMemorySampler>,
}

/// Legacy `sanitize_dynamic_memory_extra_body_fields`; direct and group
/// memory differed in their DRY settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlamaMemorySampler {
    Direct,
    Group,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedLlamaSettings {
    pub settings: LlamaCppSettings,
    #[serde(default)]
    pub disable_sampler_profile_defaults: bool,
}

#[must_use]
pub fn resolve_llama_settings(
    model: &LlamaCppSettings,
    input: &LlamaResolutionInput,
) -> ResolvedLlamaSettings {
    let layers = [&input.session, model, &input.global];
    macro_rules! pick {
        ($($field:ident).+) => {
            layers.iter().find_map(|layer| layer.$($field).+.clone())
        };
    }
    let multi_gpu_leveled = layers
        .iter()
        .zip([2u8, 1, 0])
        .find_map(|(layer, level)| layer.multi_gpu_enabled.map(|value| (value, level)));
    let pin_leveled = layers
        .iter()
        .zip([2u8, 1, 0])
        .find_map(|(layer, level)| layer.single_gpu_device_id.map(|value| (value, level)));
    let pin_overridden = matches!(
        (multi_gpu_leveled, pin_leveled),
        (Some((true, multi_level)), Some((_, pin_level))) if multi_level >= pin_level
    );
    let kv_layer = layers.iter().find(|layer| {
        layer.kv_type.is_some() || layer.kv_type_k.is_some() || layer.kv_type_v.is_some()
    });
    let mut settings = LlamaCppSettings {
        gpu_layers: pick!(gpu_layers),
        multi_gpu_enabled: multi_gpu_leveled.map(|(value, _)| value),
        gpu_device_ids: pick!(gpu_device_ids),
        gpu_distribution_mode: pick!(gpu_distribution_mode),
        gpu_manual_layers: pick!(gpu_manual_layers),
        cpu_layers: pick!(cpu_layers),
        kv_placement: pick!(kv_placement),
        main_gpu: pick!(main_gpu),
        single_gpu_device_id: pin_leveled
            .filter(|_| !pin_overridden)
            .map(|(value, _)| value),
        priority_vram_limit_bytes: pick!(priority_vram_limit_bytes),
        threads: pick!(threads),
        threads_batch: pick!(threads_batch),
        rope_freq_base: pick!(rope_freq_base),
        rope_freq_scale: pick!(rope_freq_scale),
        offload_kqv: pick!(offload_kqv),
        batch_size: pick!(batch_size),
        ubatch_size: pick!(ubatch_size),
        kv_type: kv_layer.and_then(|layer| layer.kv_type),
        kv_type_k: kv_layer.and_then(|layer| layer.kv_type_k),
        kv_type_v: kv_layer.and_then(|layer| layer.kv_type_v),
        flash_attention: pick!(flash_attention),
        swa_full: pick!(swa_full),
        chat_template_override: pick!(chat_template_override),
        chat_template_preset: pick!(chat_template_preset),
        mmproj_path: pick!(mmproj_path),
        raw_completion_fallback: pick!(raw_completion_fallback),
        strict_mode: pick!(strict_mode),
        mtp_enabled: pick!(mtp_enabled),
        mtp_placement: pick!(mtp_placement),
        mtp_draft_tokens: pick!(mtp_draft_tokens),
        mtp_model_path: pick!(mtp_model_path),
        streaming_enabled: pick!(streaming_enabled),
        sampler: LlamaSamplerSettings {
            profile: pick!(sampler.profile),
            order: pick!(sampler.order),
            min_p: pick!(sampler.min_p),
            typical_p: pick!(sampler.typical_p),
            repeat_penalty: pick!(sampler.repeat_penalty),
            n_pen_range: pick!(sampler.n_pen_range),
            dry_multiplier: pick!(sampler.dry_multiplier),
            dry_base: pick!(sampler.dry_base),
            dry_allowed_length: pick!(sampler.dry_allowed_length),
            dry_penalty_last_n: pick!(sampler.dry_penalty_last_n),
            dry_sequence_breakers: pick!(sampler.dry_sequence_breakers),
            xtc_probability: pick!(sampler.xtc_probability),
            xtc_threshold: pick!(sampler.xtc_threshold),
            seed: pick!(sampler.seed),
        },
    };
    let Some(memory) = input.memory_sampler else {
        return ResolvedLlamaSettings {
            settings,
            disable_sampler_profile_defaults: false,
        };
    };
    let seed = settings.sampler.seed;
    let xtc_probability = settings.sampler.xtc_probability;
    let xtc_threshold = settings.sampler.xtc_threshold;
    settings.sampler = LlamaSamplerSettings {
        profile: None,
        order: Some(vec![
            LlamaSamplerStage::Penalties,
            LlamaSamplerStage::Grammar,
            LlamaSamplerStage::TopK,
            LlamaSamplerStage::TopP,
            LlamaSamplerStage::Temp,
            LlamaSamplerStage::Dry,
            LlamaSamplerStage::MinP,
            LlamaSamplerStage::Typical,
        ]),
        min_p: Some(0.0),
        typical_p: Some(0.0),
        repeat_penalty: Some(1.0),
        n_pen_range: Some(-1),
        dry_multiplier: Some(match memory {
            LlamaMemorySampler::Direct => 0.8,
            LlamaMemorySampler::Group => 0.0,
        }),
        dry_base: (memory == LlamaMemorySampler::Direct).then_some(1.75),
        dry_allowed_length: (memory == LlamaMemorySampler::Direct).then_some(2),
        dry_penalty_last_n: (memory == LlamaMemorySampler::Direct).then_some(-1),
        dry_sequence_breakers: None,
        xtc_probability,
        xtc_threshold,
        seed,
    };
    ResolvedLlamaSettings {
        settings,
        disable_sampler_profile_defaults: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LlamaKvType, LlamaSamplerProfile};

    #[test]
    fn fields_resolve_session_then_model_then_app() {
        let model = LlamaCppSettings {
            threads: Some(8),
            batch_size: Some(1024),
            gpu_device_ids: Some(vec![]),
            ..LlamaCppSettings::default()
        };
        let input = LlamaResolutionInput {
            global: LlamaCppSettings {
                threads: Some(4),
                ubatch_size: Some(256),
                gpu_device_ids: Some(vec![0, 1]),
                ..LlamaCppSettings::default()
            },
            session: LlamaCppSettings {
                batch_size: Some(2048),
                ..LlamaCppSettings::default()
            },
            memory_sampler: None,
        };
        let resolved = resolve_llama_settings(&model, &input).settings;
        assert_eq!(resolved.batch_size, Some(2048));
        assert_eq!(resolved.threads, Some(8));
        assert_eq!(resolved.ubatch_size, Some(256));
        assert_eq!(resolved.gpu_device_ids, Some(vec![]));
    }

    #[test]
    fn a_pin_yields_to_multi_gpu_at_the_same_or_a_more_specific_layer() {
        let pinned_model = LlamaCppSettings {
            single_gpu_device_id: Some(1),
            ..LlamaCppSettings::default()
        };
        let multi = |session: bool| LlamaResolutionInput {
            global: LlamaCppSettings {
                multi_gpu_enabled: Some(!session),
                ..LlamaCppSettings::default()
            },
            session: LlamaCppSettings {
                multi_gpu_enabled: session.then_some(true),
                ..LlamaCppSettings::default()
            },
            memory_sampler: None,
        };
        let overridden = resolve_llama_settings(&pinned_model, &multi(true)).settings;
        assert_eq!(overridden.single_gpu_device_id, None);
        let kept = resolve_llama_settings(&pinned_model, &multi(false)).settings;
        assert_eq!(kept.single_gpu_device_id, Some(1));
        assert_eq!(kept.multi_gpu_enabled, Some(true));
    }

    #[test]
    fn the_kv_cache_types_resolve_as_one_unit() {
        let model = LlamaCppSettings {
            kv_type_k: Some(LlamaKvType::Q80),
            kv_type_v: Some(LlamaKvType::Q40),
            ..LlamaCppSettings::default()
        };
        let input = LlamaResolutionInput {
            global: LlamaCppSettings {
                kv_type: Some(LlamaKvType::F16),
                ..LlamaCppSettings::default()
            },
            ..LlamaResolutionInput::default()
        };
        let resolved = resolve_llama_settings(&model, &input).settings;
        assert_eq!(resolved.kv_type, None);
        assert_eq!(resolved.kv_type_k, Some(LlamaKvType::Q80));
        assert_eq!(resolved.kv_type_v, Some(LlamaKvType::Q40));
    }

    #[test]
    fn memory_requests_use_the_legacy_fixed_sampler() {
        let model = LlamaCppSettings {
            sampler: LlamaSamplerSettings {
                profile: Some(LlamaSamplerProfile::Creative),
                min_p: Some(0.1),
                dry_sequence_breakers: Some(vec![":".into()]),
                seed: Some(7),
                ..LlamaSamplerSettings::default()
            },
            ..LlamaCppSettings::default()
        };
        let direct = resolve_llama_settings(
            &model,
            &LlamaResolutionInput {
                memory_sampler: Some(LlamaMemorySampler::Direct),
                ..LlamaResolutionInput::default()
            },
        );
        assert!(direct.disable_sampler_profile_defaults);
        let sampler = &direct.settings.sampler;
        assert_eq!(sampler.profile, None);
        assert_eq!(sampler.min_p, Some(0.0));
        assert_eq!(sampler.typical_p, Some(0.0));
        assert_eq!(sampler.repeat_penalty, Some(1.0));
        assert_eq!(sampler.n_pen_range, Some(-1));
        assert_eq!(sampler.dry_multiplier, Some(0.8));
        assert_eq!(sampler.dry_base, Some(1.75));
        assert_eq!(sampler.dry_allowed_length, Some(2));
        assert_eq!(sampler.dry_penalty_last_n, Some(-1));
        assert_eq!(sampler.dry_sequence_breakers, None);
        assert_eq!(sampler.seed, Some(7));
        assert_eq!(sampler.order.as_ref().map(Vec::len), Some(8));
        let group = resolve_llama_settings(
            &model,
            &LlamaResolutionInput {
                memory_sampler: Some(LlamaMemorySampler::Group),
                ..LlamaResolutionInput::default()
            },
        );
        let sampler = &group.settings.sampler;
        assert_eq!(sampler.dry_multiplier, Some(0.0));
        assert_eq!(sampler.dry_base, None);
        assert_eq!(sampler.dry_allowed_length, None);
    }
}
