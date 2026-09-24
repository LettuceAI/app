//! The legacy remote image adapters: where each provider's image request
//! goes, what it carries and how its response lists images.

use base64::Engine as _;
use lettuce_image_generation::{
    ImageInput, ProviderImageRequest, sd_runtime::payload::parse_size_dimensions,
};
use lettuce_network::MultipartField;
use serde_json::{Map, Value, json};

pub(crate) enum ImagePayload {
    Json(Value),
    Multipart(Vec<MultipartField>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageAuth {
    Bearer,
    OptionalBearer,
    GoogleApiKeyHeader,
    QueryKey,
    None,
}

/// One HTTP call: `endpoint` is the base URL, `path` what follows it.
pub(crate) struct ImageCall {
    pub endpoint: String,
    pub path: String,
    pub auth: ImageAuth,
    pub accept_json: bool,
    pub payload: ImagePayload,
    pub binary_response: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ImageResponseData {
    pub url: Option<String>,
    pub b64_json: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Adapter {
    OpenAi,
    OpenRouter,
    OpenRouterChat,
    Pollinations,
    Gemini,
    GeminiExpress,
    Stability,
    Xai,
    NanoGpt,
    LiteRouter,
    Automatic1111,
    Diffusers,
}

/// Legacy `get_adapter`; `custom` and `lettuce-host` speak the OpenAI API.
pub(crate) fn adapter_for(kind: &str) -> Option<Adapter> {
    Some(match kind {
        "automatic1111" => Adapter::Automatic1111,
        "diffusers" => Adapter::Diffusers,
        "openai" | "custom" | "lettuce-host" => Adapter::OpenAi,
        "openrouter" => Adapter::OpenRouter,
        "pollinations" => Adapter::Pollinations,
        "gemini" => Adapter::Gemini,
        "gemini-agent-platform-express" => Adapter::GeminiExpress,
        "stability" => Adapter::Stability,
        "xai" => Adapter::Xai,
        "nanogpt" => Adapter::NanoGpt,
        "literouter" => Adapter::LiteRouter,
        _ => return None,
    })
}

/// Legacy provider catalog base URLs for the kinds that make images.
pub(crate) fn default_base_url(kind: &str) -> &'static str {
    match kind {
        "openai" => "https://api.openai.com",
        "openrouter" => "https://openrouter.ai/api",
        "literouter" => "https://api.literouter.com/v1",
        "pollinations" => "https://gen.pollinations.ai",
        "nanogpt" => "https://nano-gpt.com/api",
        "xai" => "https://api.x.ai",
        "gemini" => "https://generativelanguage.googleapis.com/v1",
        "gemini-agent-platform-express" => "https://aiplatform.googleapis.com",
        "stability" => "https://api.stability.ai",
        "automatic1111" => "http://127.0.0.1:7860",
        "comfyui" => "http://127.0.0.1:8188",
        "diffusers" => "http://127.0.0.1:8000",
        _ => "https://api.openai.com",
    }
}

pub(crate) fn data_url(image: &ImageInput) -> String {
    format!(
        "data:{};base64,{}",
        image.mime_type,
        base64::engine::general_purpose::STANDARD.encode(&image.bytes)
    )
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/jpeg" | "image/jpg" => "jpg",
        _ => "png",
    }
}

fn image_part(name: &str, stem: &str, image: &ImageInput) -> MultipartField {
    MultipartField::File {
        name: name.to_owned(),
        filename: format!("{stem}.{}", extension_for_mime(&image.mime_type)),
        mime_type: image.mime_type.clone(),
        bytes: image.bytes.clone(),
    }
}

fn text(name: &str, value: impl Into<String>) -> MultipartField {
    MultipartField::Text {
        name: name.to_owned(),
        value: value.into(),
    }
}

/// `/v1` is appended unless the base already ends with it.
fn v1_path(base: &str, path: &str) -> String {
    if base.ends_with("/v1") {
        path.to_owned()
    } else {
        format!("/v1{path}")
    }
}

fn parse_error(error: &serde_json::Error) -> String {
    format!("Failed to parse response: {error}")
}

fn size_or_sd_size(request: &ProviderImageRequest) -> Option<&str> {
    request.size.as_deref().or(request.settings.size.as_deref())
}

impl Adapter {
    /// Legacy asked for an API key unless the adapter sends no auth header.
    pub(crate) const fn requires_api_key(self) -> bool {
        !matches!(self, Self::Automatic1111 | Self::Diffusers)
    }

    /// The adapter to retry with when the provider rejects the request with
    /// this status and body: OpenRouter's Image API does not serve every image
    /// model, and chat completions still do for chat-style ones.
    pub(crate) fn fallback(self, status: u16, body: &str) -> Option<Self> {
        (self == Self::OpenRouter && should_fall_back_to_chat(status, body))
            .then_some(Self::OpenRouterChat)
    }

    pub(crate) fn call(
        self,
        base_url: &str,
        request: &ProviderImageRequest,
    ) -> Result<ImageCall, String> {
        let base = base_url.trim_end_matches('/');
        let has_images = !request.input_images.is_empty();
        let json_call = |path: String, auth, payload| ImageCall {
            endpoint: base.to_owned(),
            path,
            auth,
            accept_json: false,
            payload: ImagePayload::Json(payload),
            binary_response: false,
        };
        Ok(match self {
            Self::OpenAi | Self::Pollinations => {
                let path = v1_path(
                    base,
                    if has_images {
                        "/images/edits"
                    } else {
                        "/images/generations"
                    },
                );
                if has_images {
                    let mut fields = vec![
                        text("model", request.external_model_id.clone()),
                        text("prompt", request.prompt.clone()),
                        text("n", request.count.to_string()),
                    ];
                    if let Some(size) = &request.size {
                        fields.push(text("size", size.clone()));
                    }
                    if let Some(quality) = &request.quality {
                        fields.push(text("quality", quality.clone()));
                    }
                    fields.push(text("response_format", "b64_json"));
                    fields.extend(
                        request
                            .input_images
                            .iter()
                            .map(|image| image_part("image[]", "input", image)),
                    );
                    ImageCall {
                        endpoint: base.to_owned(),
                        path,
                        auth: ImageAuth::Bearer,
                        accept_json: false,
                        payload: ImagePayload::Multipart(fields),
                        binary_response: false,
                    }
                } else {
                    let mut body = Map::new();
                    body.insert("model".into(), json!(request.external_model_id));
                    body.insert("prompt".into(), json!(request.prompt));
                    body.insert("n".into(), json!(request.count));
                    if let Some(size) = &request.size {
                        body.insert("size".into(), json!(size));
                    }
                    if let Some(quality) = &request.quality {
                        body.insert("quality".into(), json!(quality));
                    }
                    if let Some(style) = &request.style {
                        body.insert("style".into(), json!(style));
                    }
                    body.insert("response_format".into(), json!("b64_json"));
                    json_call(path, ImageAuth::Bearer, Value::Object(body))
                }
            }
            Self::OpenRouter => json_call(
                v1_path(base, "/images"),
                ImageAuth::Bearer,
                openrouter_image_payload(request),
            ),
            Self::OpenRouterChat => {
                let content = if has_images {
                    let mut parts = vec![json!({"type": "text", "text": request.prompt})];
                    parts.extend(request.input_images.iter().map(
                        |image| json!({"type": "image_url", "image_url": {"url": data_url(image)}}),
                    ));
                    Value::Array(parts)
                } else {
                    Value::String(request.prompt.clone())
                };
                let mut modalities = vec!["image"];
                if request.text_output {
                    modalities.push("text");
                }
                json_call(
                    v1_path(base, "/chat/completions"),
                    ImageAuth::Bearer,
                    json!({
                        "model": request.external_model_id,
                        "messages": [{"role": "user", "content": content}],
                        "modalities": modalities,
                    }),
                )
            }
            Self::Xai => {
                let (aspect_ratio, resolution) = match request.size.as_deref() {
                    Some("1024x1024") => (Some("1:1"), Some("1k")),
                    Some("1536x1024") => (Some("3:2"), Some("1k")),
                    Some("1024x1536") => (Some("2:3"), Some("1k")),
                    Some("2048x2048") => (Some("1:1"), Some("2k")),
                    Some("2048x1024") => (Some("2:1"), Some("2k")),
                    Some("1024x2048") => (Some("1:2"), Some("2k")),
                    _ => (None, None),
                };
                let path = v1_path(
                    base,
                    if has_images {
                        "/images/edits"
                    } else {
                        "/images/generations"
                    },
                );
                let mut body = Map::new();
                body.insert("model".into(), json!(request.external_model_id));
                body.insert("prompt".into(), json!(request.prompt));
                if has_images {
                    let refs = request
                        .input_images
                        .iter()
                        .map(|image| json!({"type": "image_url", "url": data_url(image)}))
                        .collect::<Vec<_>>();
                    if refs.len() == 1 {
                        body.insert("image".into(), refs[0].clone());
                    } else {
                        body.insert("images".into(), Value::Array(refs));
                    }
                    if let Some(aspect_ratio) = aspect_ratio {
                        body.insert("aspect_ratio".into(), json!(aspect_ratio));
                    }
                } else {
                    body.insert("n".into(), json!(request.count));
                    if let Some(aspect_ratio) = aspect_ratio {
                        body.insert("aspect_ratio".into(), json!(aspect_ratio));
                    }
                    if let Some(resolution) = resolution {
                        body.insert("resolution".into(), json!(resolution));
                    }
                }
                json_call(path, ImageAuth::Bearer, Value::Object(body))
            }
            Self::NanoGpt => {
                let normalized = base.trim_end_matches("/api");
                let mut body = Map::new();
                body.insert("model".into(), json!(request.external_model_id));
                body.insert("prompt".into(), json!(request.prompt));
                body.insert("n".into(), json!(request.count));
                if let Some(size) = &request.size {
                    body.insert("size".into(), json!(size));
                }
                body.insert("responseFormat".into(), json!("b64_json"));
                match request.input_images.as_slice() {
                    [image] => {
                        body.insert("imageDataUrl".into(), json!(data_url(image)));
                    }
                    [] => {}
                    images => {
                        body.insert(
                            "imageDataUrls".into(),
                            Value::Array(
                                images.iter().map(|image| json!(data_url(image))).collect(),
                            ),
                        );
                    }
                }
                ImageCall {
                    endpoint: normalized.to_owned(),
                    path: v1_path(normalized, "/images/generations"),
                    auth: ImageAuth::Bearer,
                    accept_json: false,
                    payload: ImagePayload::Json(Value::Object(body)),
                    binary_response: false,
                }
            }
            Self::Automatic1111 => {
                let normalized = base.trim_end_matches("/sdapi/v1");
                let settings = &request.settings;
                let (width, height) = parse_size_dimensions(size_or_sd_size(request), 1024, 1024);
                let mut body = Map::new();
                body.insert("prompt".into(), json!(request.prompt));
                body.insert("width".into(), json!(width));
                body.insert("height".into(), json!(height));
                body.insert("batch_size".into(), json!(request.count));
                body.insert("n_iter".into(), json!(1));
                body.insert("steps".into(), json!(settings.steps.unwrap_or(28)));
                body.insert("cfg_scale".into(), json!(settings.cfg_scale.unwrap_or(6.5)));
                body.insert(
                    "sampler_index".into(),
                    json!(settings.sampler.as_deref().unwrap_or("DPM++ 2M Karras")),
                );
                body.insert(
                    "override_settings".into(),
                    json!({"sd_model_checkpoint": request.external_model_id}),
                );
                if let Some(seed) = settings.seed {
                    body.insert("seed".into(), json!(seed));
                }
                if let Some(negative) = settings
                    .negative_prompt
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    body.insert("negative_prompt".into(), json!(negative));
                }
                if has_images {
                    body.insert(
                        "init_images".into(),
                        Value::Array(
                            request
                                .input_images
                                .iter()
                                .map(|image| json!(data_url(image)))
                                .collect(),
                        ),
                    );
                    body.insert(
                        "denoising_strength".into(),
                        json!(settings.denoising_strength.unwrap_or(0.75)),
                    );
                }
                ImageCall {
                    endpoint: normalized.to_owned(),
                    path: if has_images {
                        "/sdapi/v1/img2img"
                    } else {
                        "/sdapi/v1/txt2img"
                    }
                    .to_owned(),
                    auth: ImageAuth::None,
                    accept_json: false,
                    payload: ImagePayload::Json(Value::Object(body)),
                    binary_response: false,
                }
            }
            Self::Diffusers => {
                let settings = &request.settings;
                let (width, height) = parse_size_dimensions(size_or_sd_size(request), 1024, 1024);
                let mut body = Map::new();
                body.insert("prompt".into(), json!(request.prompt));
                body.insert("model".into(), json!(request.external_model_id));
                body.insert("width".into(), json!(width));
                body.insert("height".into(), json!(height));
                body.insert("steps".into(), json!(settings.steps.unwrap_or(28)));
                body.insert("cfg_scale".into(), json!(settings.cfg_scale.unwrap_or(6.5)));
                body.insert(
                    "sampler".into(),
                    json!(settings.sampler.as_deref().unwrap_or("DPM++ 2M Karras")),
                );
                body.insert("n".into(), json!(request.count));
                if let Some(seed) = settings.seed {
                    body.insert("seed".into(), json!(seed));
                }
                if let Some(negative) = settings
                    .negative_prompt
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    body.insert("negative_prompt".into(), json!(negative));
                }
                if has_images {
                    let encoded = request
                        .input_images
                        .iter()
                        .map(|image| {
                            json!(base64::engine::general_purpose::STANDARD.encode(&image.bytes))
                        })
                        .collect::<Vec<_>>();
                    body.insert("init_image".into(), encoded[0].clone());
                    body.insert("images".into(), Value::Array(encoded));
                    body.insert(
                        "denoising_strength".into(),
                        json!(settings.denoising_strength.unwrap_or(0.75)),
                    );
                }
                json_call(
                    "/generate".to_owned(),
                    ImageAuth::OptionalBearer,
                    Value::Object(body),
                )
            }
            Self::Stability => {
                let (width, height) = parse_size_dimensions(request.size.as_deref(), 1024, 1024);
                let model = &request.external_model_id;
                if let Some(image) = request.input_images.first() {
                    ImageCall {
                        endpoint: base.to_owned(),
                        path: format!("/v1/generation/{model}/image-to-image"),
                        auth: ImageAuth::Bearer,
                        accept_json: true,
                        payload: ImagePayload::Multipart(vec![
                            text("init_image_mode", "IMAGE_STRENGTH"),
                            text("image_strength", "0.35"),
                            text("samples", request.count.to_string()),
                            text("width", width.to_string()),
                            text("height", height.to_string()),
                            text("text_prompts[0][text]", request.prompt.clone()),
                            image_part("init_image", "input", image),
                        ]),
                        binary_response: false,
                    }
                } else {
                    ImageCall {
                        endpoint: base.to_owned(),
                        path: format!("/v1/generation/{model}/text-to-image"),
                        auth: ImageAuth::Bearer,
                        accept_json: true,
                        payload: ImagePayload::Json(json!({
                            "text_prompts": [{"text": request.prompt, "weight": 1}],
                            "width": width,
                            "height": height,
                            "samples": request.count,
                        })),
                        binary_response: false,
                    }
                }
            }
            Self::LiteRouter => {
                const IMAGE_HOST: &str = "https://image.literouter.com";
                let normalized = if base.is_empty()
                    || (base.contains("literouter.com") && !base.contains("image.literouter.com"))
                {
                    IMAGE_HOST
                } else {
                    base
                };
                let (width, height) = parse_size_dimensions(request.size.as_deref(), 1024, 1024);
                let mut body = json!({
                    "prompt": request.prompt,
                    "model": request.external_model_id,
                    "width": width,
                    "height": height,
                });
                if let Some(seed) = request.settings.seed {
                    body["seed"] = json!(seed);
                }
                ImageCall {
                    endpoint: normalized.to_owned(),
                    path: "/generate".to_owned(),
                    auth: ImageAuth::Bearer,
                    accept_json: false,
                    payload: ImagePayload::Json(body),
                    binary_response: true,
                }
            }
            Self::Gemini | Self::GeminiExpress => {
                let mut parts = vec![json!({"text": request.prompt})];
                for image in &request.input_images {
                    if image.mime_type.is_empty() || image.bytes.is_empty() {
                        return Err(
                            "Gemini image editing received an empty image data URL".to_owned()
                        );
                    }
                    parts.push(json!({
                        "inline_data": {
                            "mimeType": image.mime_type,
                            "data": base64::engine::general_purpose::STANDARD.encode(&image.bytes),
                        }
                    }));
                }
                let mut response_modalities = Vec::new();
                if request.text_output {
                    response_modalities.push("TEXT");
                }
                response_modalities.push("IMAGE");
                let mut generation_config = json!({"responseModalities": response_modalities});
                if let Some(aspect_ratio) = gemini_aspect_ratio(request.size.as_deref()) {
                    if self == Self::Gemini {
                        generation_config["responseFormat"] =
                            json!({"image": {"aspectRatio": aspect_ratio}});
                    } else {
                        generation_config["imageConfig"] = json!({"aspectRatio": aspect_ratio});
                    }
                }
                let payload = json!({
                    "contents": [{"role": "user", "parts": parts}],
                    "generationConfig": generation_config,
                });
                if self == Self::Gemini {
                    let root = base
                        .strip_suffix("/v1beta")
                        .or_else(|| base.strip_suffix("/v1"))
                        .unwrap_or(base);
                    json_call(
                        format!(
                            "/v1beta/models/{}:generateContent",
                            request.external_model_id
                        ),
                        ImageAuth::QueryKey,
                        payload,
                    )
                    .with_endpoint(root)
                } else {
                    let express = if base.ends_with("/v1beta1") {
                        base.to_owned()
                    } else if let Some(prefix) = base
                        .strip_suffix("/v1beta")
                        .or_else(|| base.strip_suffix("/v1"))
                    {
                        format!("{prefix}/v1beta1")
                    } else {
                        format!("{base}/v1beta1")
                    };
                    let bare = request
                        .external_model_id
                        .strip_prefix("publishers/google/models/")
                        .unwrap_or(&request.external_model_id);
                    json_call(
                        format!(
                            "/publishers/google/models/{}:generateContent",
                            percent_encode(bare)
                        ),
                        ImageAuth::GoogleApiKeyHeader,
                        payload,
                    )
                    .with_endpoint(&express)
                }
            }
        })
    }

    pub(crate) fn parse(self, response: Value) -> Result<Vec<ImageResponseData>, String> {
        match self {
            Self::OpenAi | Self::Pollinations | Self::Xai | Self::NanoGpt => {
                let data = response
                    .get("data")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "Failed to parse response: missing field `data`".to_owned())?;
                data.iter()
                    .map(|item| {
                        Ok(ImageResponseData {
                            url: optional_string(item, "url")?,
                            b64_json: optional_string(item, "b64_json")?,
                            text: None,
                        })
                    })
                    .collect()
            }
            Self::OpenRouter => parse_openrouter_images(response),
            Self::OpenRouterChat => parse_openrouter(response),
            Self::Automatic1111 => {
                #[derive(serde::Deserialize)]
                struct Response {
                    images: Vec<String>,
                }
                let parsed = serde_json::from_value::<Response>(response)
                    .map_err(|error| parse_error(&error))?;
                Ok(parsed
                    .images
                    .into_iter()
                    .map(|image| ImageResponseData {
                        b64_json: Some(image),
                        ..ImageResponseData::default()
                    })
                    .collect())
            }
            Self::Diffusers => {
                let collect = |value: &Value| {
                    let raw = value.as_str()?;
                    let data = raw.split_once("base64,").map_or(raw, |(_, data)| data);
                    (!data.is_empty()).then(|| ImageResponseData {
                        b64_json: Some(data.to_owned()),
                        ..ImageResponseData::default()
                    })
                };
                let mut images = response
                    .get("images")
                    .and_then(Value::as_array)
                    .map(|array| array.iter().filter_map(collect).collect::<Vec<_>>())
                    .unwrap_or_default();
                if images.is_empty()
                    && let Some(image) = response.get("image").and_then(collect)
                {
                    images.push(image);
                }
                if images.is_empty() {
                    return Err("Diffusers response did not contain any image data".to_owned());
                }
                Ok(images)
            }
            Self::Stability => {
                #[derive(serde::Deserialize)]
                struct Artifact {
                    #[serde(default)]
                    base64: Option<String>,
                }
                #[derive(serde::Deserialize)]
                struct Response {
                    artifacts: Vec<Artifact>,
                }
                let parsed = serde_json::from_value::<Response>(response)
                    .map_err(|error| parse_error(&error))?;
                Ok(parsed
                    .artifacts
                    .into_iter()
                    .filter_map(|artifact| artifact.base64)
                    .map(|image| ImageResponseData {
                        b64_json: Some(image),
                        ..ImageResponseData::default()
                    })
                    .collect())
            }
            Self::LiteRouter => Err("LiteRouter returns binary image data, not JSON".to_owned()),
            Self::Gemini | Self::GeminiExpress => parse_gemini(response),
        }
    }
}

impl ImageCall {
    fn with_endpoint(mut self, endpoint: &str) -> Self {
        endpoint.clone_into(&mut self.endpoint);
        self
    }
}

fn optional_string(item: &Value, key: &str) -> Result<Option<String>, String> {
    match item.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!(
            "Failed to parse response: invalid type for `{key}`, expected a string"
        )),
    }
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

fn gemini_aspect_ratio(size: Option<&str>) -> Option<String> {
    let (width, height) = size?.split_once('x')?;
    let width = width.trim().parse::<u32>().ok()?;
    let height = height.trim().parse::<u32>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let (mut a, mut b) = (width, height);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    let ratio = format!("{}:{}", width / a, height / a);
    matches!(
        ratio.as_str(),
        "1:1" | "2:3" | "3:2" | "3:4" | "4:3" | "4:5" | "5:4" | "9:16" | "16:9" | "21:9"
    )
    .then_some(ratio)
}

const OPENROUTER_ASPECT_RATIOS: [(&str, f64); 11] = [
    ("1:1", 1.0),
    ("4:5", 0.8),
    ("5:4", 1.25),
    ("3:4", 0.75),
    ("4:3", 4.0 / 3.0),
    ("2:3", 2.0 / 3.0),
    ("3:2", 1.5),
    ("9:16", 9.0 / 16.0),
    ("16:9", 16.0 / 9.0),
    ("9:21", 9.0 / 21.0),
    ("21:9", 21.0 / 9.0),
];

const OPENROUTER_QUALITIES: [&str; 4] = ["auto", "low", "medium", "high"];

pub(crate) fn nearest_aspect_ratio(size: Option<&str>) -> Option<&'static str> {
    let (width, height) = parse_size_dimensions(Some(size?), 0, 0);
    if width == 0 || height == 0 {
        return None;
    }
    let ratio = f64::from(width) / f64::from(height);
    OPENROUTER_ASPECT_RATIOS
        .iter()
        .min_by(|(_, a), (_, b)| (a - ratio).abs().total_cmp(&(b - ratio).abs()))
        .map(|(label, _)| *label)
}

fn openrouter_quality(quality: Option<&str>) -> Option<&'static str> {
    let quality = quality?.trim().to_ascii_lowercase();
    OPENROUTER_QUALITIES
        .into_iter()
        .find(|candidate| *candidate == quality)
}

pub(crate) fn openrouter_image_payload(request: &ProviderImageRequest) -> Value {
    let mut body = Map::new();
    body.insert("model".into(), json!(request.external_model_id));
    body.insert("prompt".into(), json!(request.prompt));
    if request.count > 0 {
        body.insert("n".into(), json!(request.count));
    }
    if let Some(aspect_ratio) = nearest_aspect_ratio(request.size.as_deref()) {
        body.insert("aspect_ratio".into(), json!(aspect_ratio));
    }
    if let Some(quality) = openrouter_quality(request.quality.as_deref()) {
        body.insert("quality".into(), json!(quality));
    }
    if !request.input_images.is_empty() {
        body.insert(
            "input_references".into(),
            Value::Array(
                request
                    .input_images
                    .iter()
                    .map(
                        |image| json!({"type": "image_url", "image_url": {"url": data_url(image)}}),
                    )
                    .collect(),
            ),
        );
    }
    Value::Object(body)
}

pub(crate) fn should_fall_back_to_chat(status: u16, body: &str) -> bool {
    match status {
        404 => true,
        400 | 422 => {
            let body = body.to_ascii_lowercase();
            [
                "model",
                "endpoint",
                "not supported",
                "unsupported",
                "not available",
            ]
            .iter()
            .any(|needle| body.contains(needle))
        }
        _ => false,
    }
}

fn parse_openrouter_images(response: Value) -> Result<Vec<ImageResponseData>, String> {
    #[derive(serde::Deserialize)]
    struct Image {
        #[serde(default)]
        b64_json: Option<String>,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        media_type: Option<String>,
        #[serde(default)]
        revised_prompt: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct Response {
        #[serde(default)]
        data: Vec<Image>,
    }
    let parsed =
        serde_json::from_value::<Response>(response).map_err(|error| parse_error(&error))?;
    let mut results = Vec::new();
    for image in parsed.data {
        let text = image
            .revised_prompt
            .filter(|value| !value.trim().is_empty());
        if let Some(encoded) = image.b64_json.filter(|value| !value.is_empty()) {
            let b64_json = if encoded.starts_with("data:") {
                encoded
            } else {
                let media_type = image
                    .media_type
                    .as_deref()
                    .filter(|value| value.starts_with("image/"))
                    .unwrap_or("image/png");
                format!("data:{media_type};base64,{encoded}")
            };
            results.push(ImageResponseData {
                url: None,
                b64_json: Some(b64_json),
                text,
            });
        } else if let Some(url) = image.url.filter(|value| !value.is_empty()) {
            results.push(ImageResponseData {
                url: Some(url),
                b64_json: None,
                text,
            });
        }
    }
    if results.is_empty() {
        return Err("No images generated in response".to_owned());
    }
    Ok(results)
}

fn parse_openrouter(response: Value) -> Result<Vec<ImageResponseData>, String> {
    #[derive(serde::Deserialize)]
    struct ImageUrl {
        url: String,
    }
    #[derive(serde::Deserialize)]
    struct Image {
        image_url: ImageUrl,
    }
    #[derive(serde::Deserialize)]
    struct Message {
        #[serde(default)]
        content: Option<String>,
        #[serde(default)]
        images: Vec<Image>,
    }
    #[derive(serde::Deserialize)]
    struct Choice {
        message: Message,
    }
    #[derive(serde::Deserialize)]
    struct Response {
        choices: Vec<Choice>,
    }
    let parsed =
        serde_json::from_value::<Response>(response).map_err(|error| parse_error(&error))?;
    if parsed.choices.is_empty() {
        return Err("No choices in response".to_owned());
    }
    let mut results = Vec::new();
    for choice in parsed.choices {
        let text = choice.message.content.filter(|value| !value.is_empty());
        if choice.message.images.is_empty() {
            if let Some(text) = text {
                results.push(ImageResponseData {
                    text: Some(text),
                    ..ImageResponseData::default()
                });
            }
            continue;
        }
        for image in choice.message.images {
            let url = image.image_url.url;
            let (url, b64_json) = if url.starts_with("data:") {
                (None, Some(url))
            } else {
                (Some(url), None)
            };
            results.push(ImageResponseData {
                url,
                b64_json,
                text: text.clone(),
            });
        }
    }
    if results.is_empty() {
        return Err(
            "The model finished without returning an image. Try the request again or pick a different model."
                .to_owned(),
        );
    }
    Ok(results)
}

fn parse_gemini(response: Value) -> Result<Vec<ImageResponseData>, String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct InlineData {
        mime_type: String,
        data: String,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Part {
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        inline_data: Option<InlineData>,
    }
    #[derive(serde::Deserialize, Default)]
    struct Content {
        #[serde(default)]
        parts: Vec<Part>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Candidate {
        #[serde(default)]
        content: Content,
        #[serde(default)]
        finish_reason: Option<String>,
        #[serde(default)]
        finish_message: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct Response {
        candidates: Vec<Candidate>,
    }
    let parsed =
        serde_json::from_value::<Response>(response).map_err(|error| parse_error(&error))?;
    if parsed.candidates.is_empty() {
        return Err("No candidates in response".to_owned());
    }
    let mut images = Vec::new();
    let mut block_reason = None;
    for candidate in &parsed.candidates {
        let text = candidate
            .content
            .parts
            .first()
            .and_then(|part| part.text.clone());
        let mut image_data_found = false;
        for part in &candidate.content.parts {
            if let Some(inline) = &part.inline_data {
                images.push(ImageResponseData {
                    url: None,
                    b64_json: Some(format!("data:{};base64,{}", inline.mime_type, inline.data)),
                    text: text.clone(),
                });
                image_data_found = true;
            }
        }
        if !image_data_found {
            if let Some(text) = text {
                images.push(ImageResponseData {
                    text: Some(text),
                    ..ImageResponseData::default()
                });
            } else if candidate
                .finish_reason
                .as_deref()
                .is_some_and(|reason| reason != "STOP")
            {
                block_reason = Some(
                    candidate
                        .finish_message
                        .clone()
                        .or_else(|| candidate.finish_reason.clone())
                        .unwrap_or_default(),
                );
            }
        }
    }
    if images.is_empty() {
        if let Some(reason) = block_reason {
            return Err(format!("Gemini declined to generate the image: {reason}"));
        }
        return Err("No images found in response".to_owned());
    }
    Ok(images)
}
