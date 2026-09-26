//! The sd-server native `img_gen` request, built from fixed defaults.

use serde_json::{Value, json};

use lettuce_models::{StableDiffusionCacheMode, StableDiffusionSettings};

use crate::DiffusionProfile;

/// One LoRA as sd-server loads it: a library-relative path.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EngineLora {
    pub path: String,
    pub multiplier: f64,
    pub is_high_noise: bool,
}

/// The model facts a local generation checks references against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineModelLimits<'a> {
    pub display_name: &'a str,
    pub max_reference_images: Option<u32>,
    pub requires_reference_image: bool,
    pub supports_image_edit: bool,
}

/// A local generation's inputs after prompt composition and LoRA
/// normalization; images are data URLs.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineGenerationInput<'a> {
    pub prompt: &'a str,
    pub settings: &'a StableDiffusionSettings,
    pub size: Option<&'a str>,
    pub count: u32,
    pub references: &'a [String],
    pub mask_image: Option<&'a str>,
    pub loras: &'a [EngineLora],
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineRequestError {
    #[error("{display_name} accepts at most {maximum} reference images.")]
    TooManyReferences { display_name: String, maximum: u32 },
    #[error("{0} requires at least one reference image.")]
    ReferenceRequired(String),
    #[error("Inpainting requires a source image alongside the mask.")]
    MaskWithoutSource,
}

/// A positive `WxH`, else the defaults.
#[must_use]
pub fn parse_size_dimensions(
    size: Option<&str>,
    default_width: u32,
    default_height: u32,
) -> (u32, u32) {
    let Some((width, height)) = size.and_then(|size| size.split_once('x')) else {
        return (default_width, default_height);
    };
    let width = width.parse::<u32>().ok().filter(|value| *value > 0);
    let height = height.parse::<u32>().ok().filter(|value| *value > 0);
    match (width, height) {
        (Some(width), Some(height)) => (width, height),
        _ => (default_width, default_height),
    }
}

const fn cache_mode_name(mode: StableDiffusionCacheMode) -> &'static str {
    match mode {
        StableDiffusionCacheMode::Disabled => "disabled",
        StableDiffusionCacheMode::Easycache => "easycache",
        StableDiffusionCacheMode::Ucache => "ucache",
        StableDiffusionCacheMode::Dbcache => "dbcache",
        StableDiffusionCacheMode::Taylorseer => "taylorseer",
        StableDiffusionCacheMode::CacheDit => "cache-dit",
        StableDiffusionCacheMode::Spectrum => "spectrum",
    }
}

fn parse_slg_layers(raw: &str) -> Vec<i64> {
    raw.split(',')
        .filter_map(|part| part.trim().parse::<i64>().ok())
        .collect()
}

/// The `img_gen` payload. Reference rules come first: a mask, or a single
/// reference on a model without editing, becomes the init image.
pub fn build_generation_payload(
    input: &EngineGenerationInput<'_>,
    limits: EngineModelLimits<'_>,
    profile: Option<&DiffusionProfile>,
) -> Result<Value, EngineRequestError> {
    let references = input.references;
    if let Some(maximum) = limits.max_reference_images
        && references.len() > maximum as usize
    {
        return Err(EngineRequestError::TooManyReferences {
            display_name: limits.display_name.to_owned(),
            maximum,
        });
    }
    if limits.requires_reference_image && references.is_empty() {
        return Err(EngineRequestError::ReferenceRequired(
            limits.display_name.to_owned(),
        ));
    }
    let mask_image = input
        .mask_image
        .map(str::trim)
        .filter(|mask| !mask.is_empty());
    if mask_image.is_some() && references.is_empty() {
        return Err(EngineRequestError::MaskWithoutSource);
    }
    let use_init_image =
        mask_image.is_some() || (!limits.supports_image_edit && references.len() == 1);
    let (init_image, payload_references) = if use_init_image {
        (Some(references[0].as_str()), &[][..])
    } else {
        (None, references)
    };
    let settings = input.settings;
    let (width, height) = parse_size_dimensions(
        input.size.or(settings.size.as_deref()),
        profile.map_or(1024, |profile| profile.default_width),
        profile.map_or(1024, |profile| profile.default_height),
    );
    let steps = settings
        .steps
        .unwrap_or(profile.map_or(20, |profile| u32::from(profile.default_steps)));
    let cfg = settings
        .cfg_scale
        .unwrap_or(profile.map_or(7.0, |profile| f64::from(profile.default_cfg)));
    let seed = settings.seed.map_or(-1, i64::from);

    let mut guidance = json!({ "txt_cfg": cfg });
    if let Some(value) = settings.image_cfg_scale {
        guidance["img_cfg"] = json!(value);
    }
    if let Some(value) = settings.distilled_guidance {
        guidance["distilled_guidance"] = json!(value);
    }
    if let Some(scale) = settings.slg_scale.filter(|scale| *scale > 0.0) {
        let mut slg = json!({ "scale": scale });
        if let Some(layers) = settings
            .slg_layers
            .as_deref()
            .map(parse_slg_layers)
            .filter(|layers| !layers.is_empty())
        {
            slg["layers"] = json!(layers);
        }
        if let Some(value) = settings.slg_layer_start {
            slg["layer_start"] = json!(value);
        }
        if let Some(value) = settings.slg_layer_end {
            slg["layer_end"] = json!(value);
        }
        guidance["slg"] = slg;
    }

    let mut sample_params = json!({
        "sample_steps": steps,
        "guidance": guidance,
    });
    if let Some(value) = &settings.sampler {
        sample_params["sample_method"] = json!(value);
    }
    if let Some(value) = &settings.scheduler {
        sample_params["scheduler"] = json!(value);
    }
    if let Some(value) = settings.eta {
        sample_params["eta"] = json!(value);
    }
    if let Some(value) = settings.flow_shift {
        sample_params["flow_shift"] = json!(value);
    }

    let mut vae_tiling = json!({ "enabled": settings.vae_tiling_enabled.unwrap_or(true) });
    if let Some(value) = settings.vae_tile_size_x {
        vae_tiling["tile_size_x"] = json!(value);
    }
    if let Some(value) = settings.vae_tile_size_y {
        vae_tiling["tile_size_y"] = json!(value);
    }
    if let Some(value) = settings.vae_tile_overlap {
        vae_tiling["target_overlap"] = json!(value);
    }

    let mut hires = json!({ "enabled": settings.hires_enabled.unwrap_or(false) });
    if let Some(value) = &settings.hires_upscaler {
        hires["upscaler"] = json!(value);
    }
    if let Some(value) = settings.hires_scale {
        hires["scale"] = json!(value);
    }
    if let Some(value) = settings.hires_width {
        hires["target_width"] = json!(value);
    }
    if let Some(value) = settings.hires_height {
        hires["target_height"] = json!(value);
    }
    if let Some(value) = settings.hires_steps {
        hires["steps"] = json!(value);
    }
    if let Some(value) = settings.hires_denoising_strength {
        hires["denoising_strength"] = json!(value);
    }

    let mut payload = json!({
        "prompt": input.prompt,
        "negative_prompt": settings.negative_prompt.as_deref().unwrap_or_default(),
        "width": width,
        "height": height,
        "seed": seed,
        "batch_count": input.count,
        "auto_resize_ref_image": settings.auto_resize_reference_images.unwrap_or(true),
        "increase_ref_index": settings.increase_reference_index.unwrap_or(false),
        "ref_images": payload_references,
        "sample_params": sample_params,
        "lora": input.loras,
        "vae_tiling_params": vae_tiling,
        "hires": hires,
        "output_format": "png",
        "output_compression": 100
    });
    if let Some(value) = settings.denoising_strength {
        payload["strength"] = json!(value);
    }
    if let Some(value) = init_image {
        payload["init_image"] = json!(value);
    }
    if let Some(value) = mask_image {
        payload["mask_image"] = json!(value);
    }
    if let Some(mode) = settings
        .cache_mode
        .filter(|mode| *mode != StableDiffusionCacheMode::Disabled)
    {
        payload["cache_mode"] = json!(cache_mode_name(mode));
        if let Some(option) = settings
            .cache_option
            .as_deref()
            .map(str::trim)
            .filter(|option| !option.is_empty())
        {
            payload["cache_option"] = json!(option);
        }
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMITS: EngineModelLimits<'static> = EngineModelLimits {
        display_name: "Z-Image Turbo (Q4 K)",
        max_reference_images: None,
        requires_reference_image: false,
        supports_image_edit: true,
    };

    fn input<'a>(
        settings: &'a StableDiffusionSettings,
        references: &'a [String],
        loras: &'a [EngineLora],
    ) -> EngineGenerationInput<'a> {
        EngineGenerationInput {
            prompt: "a detailed prompt",
            settings,
            size: Some("1280x768"),
            count: 2,
            references,
            mask_image: None,
            loras,
        }
    }

    #[test]
    fn generation_payload_preserves_every_memory_relevant_request_field() {
        let references = vec!["data:image/png;base64,reference".to_owned()];
        let loras = vec![EngineLora {
            path: "style.safetensors".to_owned(),
            multiplier: 0.75,
            is_high_noise: false,
        }];
        let settings = StableDiffusionSettings {
            negative_prompt: Some("blur".to_owned()),
            seed: Some(42),
            sampler: Some("dpm++2m".to_owned()),
            scheduler: Some("karras".to_owned()),
            steps: Some(24),
            cfg_scale: Some(3.5),
            image_cfg_scale: Some(1.75),
            distilled_guidance: Some(2.5),
            eta: Some(0.35),
            flow_shift: Some(3.0),
            denoising_strength: Some(0.72),
            auto_resize_reference_images: Some(false),
            increase_reference_index: Some(true),
            vae_tiling_enabled: Some(true),
            vae_tile_size_x: Some(768),
            vae_tile_size_y: Some(512),
            vae_tile_overlap: Some(0.25),
            hires_enabled: Some(true),
            hires_upscaler: Some("Lanczos".to_owned()),
            hires_scale: Some(1.5),
            hires_width: Some(1920),
            hires_height: Some(1080),
            hires_steps: Some(12),
            hires_denoising_strength: Some(0.45),
            ..StableDiffusionSettings::default()
        };
        let payload =
            build_generation_payload(&input(&settings, &references, &loras), LIMITS, None)
                .expect("payload");
        assert_eq!(payload["prompt"], "a detailed prompt");
        assert_eq!(payload["negative_prompt"], "blur");
        assert_eq!(payload["width"], 1280);
        assert_eq!(payload["height"], 768);
        assert_eq!(payload["seed"], 42);
        assert_eq!(payload["batch_count"], 2);
        assert_eq!(payload["ref_images"], json!(references));
        assert_eq!(payload["sample_params"]["sample_method"], "dpm++2m");
        assert_eq!(payload["sample_params"]["scheduler"], "karras");
        assert_eq!(payload["sample_params"]["sample_steps"], 24);
        assert_eq!(payload["sample_params"]["guidance"]["txt_cfg"], 3.5);
        assert_eq!(payload["sample_params"]["guidance"]["img_cfg"], 1.75);
        assert_eq!(
            payload["sample_params"]["guidance"]["distilled_guidance"],
            2.5
        );
        assert_eq!(payload["sample_params"]["eta"], 0.35);
        assert_eq!(payload["sample_params"]["flow_shift"], 3.0);
        assert_eq!(payload["strength"], 0.72);
        assert_eq!(payload["auto_resize_ref_image"], false);
        assert_eq!(payload["increase_ref_index"], true);
        assert_eq!(
            payload["lora"],
            json!([{"path": "style.safetensors", "multiplier": 0.75, "is_high_noise": false}])
        );
        assert_eq!(payload["vae_tiling_params"]["enabled"], true);
        assert_eq!(payload["vae_tiling_params"]["tile_size_x"], 768);
        assert_eq!(payload["vae_tiling_params"]["tile_size_y"], 512);
        assert_eq!(payload["vae_tiling_params"]["target_overlap"], 0.25);
        assert_eq!(payload["hires"]["enabled"], true);
        assert_eq!(payload["hires"]["upscaler"], "Lanczos");
        assert_eq!(payload["hires"]["scale"], 1.5);
        assert_eq!(payload["hires"]["target_width"], 1920);
        assert_eq!(payload["hires"]["target_height"], 1080);
        assert_eq!(payload["hires"]["steps"], 12);
        assert_eq!(payload["hires"]["denoising_strength"], 0.45);
        assert_eq!(payload["output_format"], "png");
        assert_eq!(payload["output_compression"], 100);
    }

    #[test]
    fn generation_payload_leaves_optional_engine_defaults_unset() {
        let settings = StableDiffusionSettings::default();
        let payload =
            build_generation_payload(&input(&settings, &[], &[]), LIMITS, None).expect("payload");
        assert!(payload["sample_params"].get("sample_method").is_none());
        assert!(payload["sample_params"].get("scheduler").is_none());
        assert!(payload["sample_params"].get("eta").is_none());
        assert!(payload["sample_params"].get("flow_shift").is_none());
        assert!(
            payload["sample_params"]["guidance"]
                .get("img_cfg")
                .is_none()
        );
        assert!(
            payload["sample_params"]["guidance"]
                .get("distilled_guidance")
                .is_none()
        );
        assert!(payload["sample_params"]["guidance"].get("slg").is_none());
        assert!(payload.get("strength").is_none());
        assert!(payload.get("init_image").is_none());
        assert!(payload.get("mask_image").is_none());
        assert!(payload.get("cache_mode").is_none());
        assert!(payload.get("cache_option").is_none());
        assert_eq!(payload["hires"], json!({ "enabled": false }));
        assert_eq!(payload["seed"], -1);
        assert_eq!(payload["negative_prompt"], "");
        assert_eq!(payload["sample_params"]["sample_steps"], 20);
        assert_eq!(payload["sample_params"]["guidance"]["txt_cfg"], 7.0);
        assert_eq!(payload["vae_tiling_params"], json!({ "enabled": true }));
        assert_eq!(payload["auto_resize_ref_image"], true);
        assert_eq!(payload["increase_ref_index"], false);
    }

    #[test]
    fn generation_payload_wires_inpainting_caching_and_slg() {
        let references = vec!["data:image/png;base64,source".to_owned()];
        let settings = StableDiffusionSettings {
            slg_scale: Some(2.5),
            slg_layers: Some("7, 8,9,junk".to_owned()),
            slg_layer_start: Some(0.01),
            slg_layer_end: Some(0.2),
            cache_mode: Some(StableDiffusionCacheMode::CacheDit),
            cache_option: Some("threshold=0.2".to_owned()),
            denoising_strength: Some(0.6),
            ..StableDiffusionSettings::default()
        };
        let mut request = input(&settings, &references, &[]);
        request.mask_image = Some("data:image/png;base64,mask");
        let payload = build_generation_payload(&request, LIMITS, None).expect("payload");
        assert_eq!(payload["init_image"], "data:image/png;base64,source");
        assert_eq!(payload["ref_images"], json!([]));
        assert_eq!(payload["mask_image"], "data:image/png;base64,mask");
        assert_eq!(payload["strength"], 0.6);
        assert_eq!(payload["cache_mode"], "cache-dit");
        assert_eq!(payload["cache_option"], "threshold=0.2");
        let slg = &payload["sample_params"]["guidance"]["slg"];
        assert_eq!(slg["scale"], 2.5);
        assert_eq!(slg["layers"], json!([7, 8, 9]));
        assert_eq!(slg["layer_start"], 0.01);
        assert_eq!(slg["layer_end"], 0.2);
    }

    #[test]
    fn disabled_cache_mode_is_not_sent_to_the_engine() {
        let settings = StableDiffusionSettings {
            cache_mode: Some(StableDiffusionCacheMode::Disabled),
            cache_option: Some("threshold=0.2".to_owned()),
            ..StableDiffusionSettings::default()
        };
        let payload =
            build_generation_payload(&input(&settings, &[], &[]), LIMITS, None).expect("payload");
        assert!(payload.get("cache_mode").is_none());
        assert!(payload.get("cache_option").is_none());
    }

    #[test]
    fn reference_rules_and_profile_defaults_follow_legacy() {
        let catalog = crate::diffusion_catalog();
        let turbo = catalog.profile("z-image-turbo").expect("turbo");
        let settings = StableDiffusionSettings::default();
        let mut request = input(&settings, &[], &[]);
        request.size = Some("bad");
        let payload = build_generation_payload(&request, LIMITS, Some(turbo)).expect("payload");
        assert_eq!(payload["width"], 1024);
        assert_eq!(payload["sample_params"]["sample_steps"], 8);
        assert_eq!(payload["sample_params"]["guidance"]["txt_cfg"], 0.0);

        let references = vec!["data:image/png;base64,a".to_owned()];
        let no_edit = EngineModelLimits {
            supports_image_edit: false,
            ..LIMITS
        };
        let single = build_generation_payload(&input(&settings, &references, &[]), no_edit, None)
            .expect("payload");
        assert_eq!(single["init_image"], "data:image/png;base64,a");
        let limited = EngineModelLimits {
            max_reference_images: Some(0),
            ..LIMITS
        };
        assert_eq!(
            build_generation_payload(&input(&settings, &references, &[]), limited, None)
                .map_err(|error| error.to_string()),
            Err("Z-Image Turbo (Q4 K) accepts at most 0 reference images.".to_owned())
        );
        let required = EngineModelLimits {
            requires_reference_image: true,
            ..LIMITS
        };
        assert_eq!(
            build_generation_payload(&input(&settings, &[], &[]), required, None)
                .map_err(|error| error.to_string()),
            Err("Z-Image Turbo (Q4 K) requires at least one reference image.".to_owned())
        );
        let mut mask_only = input(&settings, &[], &[]);
        mask_only.mask_image = Some("data:image/png;base64,mask");
        assert_eq!(
            build_generation_payload(&mask_only, LIMITS, None),
            Err(EngineRequestError::MaskWithoutSource)
        );
    }
}
