use std::sync::Arc;

use base64::Engine as _;
use lettuce_image_generation::{
    ImageInput, ImageProviderError, ImageProviderPort, ProviderImageRequest,
};
use lettuce_jobs::handle::CancellationToken;
use lettuce_models::{
    ComfyUiConfig, ProviderAccount, ProviderConfig, ProviderProtocol, StableDiffusionSettings,
};
use lettuce_network::BulkHttpClient;
use lettuce_settings::{
    InMemorySecretStore, SecretOwnerId, SecretPurpose, SecretRecord, SecretRef, SecretStore,
    SecretValue,
};
use lettuce_types::{JobId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

use super::RemoteImageProviders;
use super::comfyui::substitute_tokens;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake";

struct Recorded {
    head: String,
    body: Vec<u8>,
}

impl Recorded {
    fn request_line(&self) -> &str {
        self.head.lines().next().unwrap_or_default()
    }

    fn header(&self, name: &str) -> Option<String> {
        self.head.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_owned())
        })
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("json body")
    }

    fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

async fn read_request(stream: &mut TcpStream) -> Recorded {
    let mut raw = Vec::new();
    let mut buffer = [0_u8; 8192];
    let body_start = loop {
        let read = stream.read(&mut buffer).await.expect("read request");
        assert!(read > 0, "connection closed before headers");
        raw.extend_from_slice(&buffer[..read]);
        if let Some(position) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let head = String::from_utf8_lossy(&raw[..body_start]).into_owned();
    let length = head
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    while raw.len() < body_start + length {
        let read = stream.read(&mut buffer).await.expect("read body");
        assert!(read > 0, "connection closed before body");
        raw.extend_from_slice(&buffer[..read]);
    }
    Recorded {
        head,
        body: raw[body_start..].to_vec(),
    }
}

fn response(status: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let mut bytes = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

fn json_response(body: &Value) -> Vec<u8> {
    response("200 OK", "application/json", body.to_string().as_bytes())
}

async fn server(responses: Vec<Vec<u8>>) -> (String, oneshot::Receiver<Vec<Recorded>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    let (sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let mut recorded = Vec::with_capacity(responses.len());
        for reply in responses {
            let (mut stream, _) = listener.accept().await.expect("accept");
            recorded.push(read_request(&mut stream).await);
            stream.write_all(&reply).await.expect("write");
            let _ = stream.shutdown().await;
        }
        let _ = sender.send(recorded);
    });
    (format!("http://{address}"), receiver)
}

fn account(
    kind: &str,
    endpoint: &str,
    key: Option<SecretRef>,
    owner: SecretOwnerId,
) -> ProviderAccount {
    ProviderAccount {
        id: ProviderAccountId::new(),
        secret_owner_id: owner,
        provider_kind: kind.to_owned(),
        protocol: ProviderProtocol::OpenAiCompatible,
        label: "Images".to_owned(),
        endpoint: Some(endpoint.to_owned()),
        enabled: true,
        streaming_enabled: true,
        allow_invalid_tls: false,
        api_key_ref: key,
        secret_headers: Vec::new(),
        config: ProviderConfig::Standard,
        revision: Revision::INITIAL,
        created_at: TimestampMillis::new(1),
        updated_at: TimestampMillis::new(1),
    }
}

fn request(account: ProviderAccount) -> ProviderImageRequest {
    ProviderImageRequest {
        job_id: JobId::new(),
        model_profile_id: ModelProfileId::new(),
        account,
        external_model_id: "image-model".to_owned(),
        model_display_name: "Image Model".to_owned(),
        prompt: "a \"quiet\" harbor".to_owned(),
        settings: StableDiffusionSettings::default(),
        loras: Vec::new(),
        input_images: Vec::new(),
        mask_image: None,
        size: Some("1024x1024".to_owned()),
        quality: None,
        style: None,
        count: 1,
        text_output: false,
        cancellation: CancellationToken::new(),
    }
}

async fn keyed() -> (Arc<InMemorySecretStore>, SecretOwnerId, SecretRef) {
    let store = Arc::new(InMemorySecretStore::new());
    let owner = SecretOwnerId::new();
    let key = SecretRef::new();
    store
        .put(
            SecretRecord::new(key, SecretPurpose::ProviderApiKey { owner }),
            SecretValue::new("key-canary").expect("secret"),
            None,
        )
        .await
        .expect("store key");
    (store, owner, key)
}

fn providers(store: Arc<InMemorySecretStore>) -> RemoteImageProviders<InMemorySecretStore> {
    RemoteImageProviders::new(store, BulkHttpClient::new().expect("client"))
        .with_retry_delay(std::time::Duration::ZERO)
}

fn encoded(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn failure(
    result: Result<lettuce_image_generation::ProviderImageOutput, ImageProviderError>,
) -> String {
    match result {
        Err(ImageProviderError::Failed(message)) => message,
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[tokio::test]
async fn openai_generation_posts_json_with_bearer_and_reads_images_and_usage() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![json_response(&json!({
        "data": [{"b64_json": encoded(PNG)}],
        "usage": {"input_tokens": 12, "output_tokens": 30}
    }))])
    .await;
    let output = providers(store)
        .generate(request(account("openai", &base, Some(key), owner)))
        .await
        .expect("generate");
    assert_eq!(output.images.len(), 1);
    assert_eq!(output.images[0].bytes, PNG);
    assert!(output.usage.is_some());
    let recorded = recorded.await.expect("requests");
    assert_eq!(
        recorded[0].request_line(),
        "POST /v1/images/generations HTTP/1.1"
    );
    assert_eq!(
        recorded[0].header("authorization").as_deref(),
        Some("Bearer key-canary")
    );
    assert_eq!(
        recorded[0].json(),
        json!({
            "model": "image-model",
            "prompt": "a \"quiet\" harbor",
            "n": 1,
            "size": "1024x1024",
            "response_format": "b64_json"
        })
    );
}

#[tokio::test]
async fn openai_edits_send_multipart_image_parts() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![json_response(&json!({
        "data": [{"b64_json": encoded(PNG)}]
    }))])
    .await;
    let mut edit = request(account("openai", &format!("{base}/v1/"), Some(key), owner));
    edit.input_images = vec![ImageInput {
        mime_type: "image/webp".to_owned(),
        bytes: b"reference".to_vec(),
    }];
    providers(store).generate(edit).await.expect("generate");
    let recorded = recorded.await.expect("requests");
    assert_eq!(recorded[0].request_line(), "POST /v1/images/edits HTTP/1.1");
    assert!(
        recorded[0]
            .header("content-type")
            .is_some_and(|value| value.starts_with("multipart/form-data"))
    );
    let body = recorded[0].body_text();
    assert!(body.contains("name=\"image[]\"; filename=\"input.webp\""));
    assert!(body.contains("reference"));
    assert!(body.contains("name=\"response_format\"\r\n\r\nb64_json"));
}

#[tokio::test]
async fn a_key_requiring_adapter_without_a_key_fails_like_legacy() {
    let store = Arc::new(InMemorySecretStore::new());
    let message = failure(
        providers(store)
            .generate(request(account(
                "openai",
                "http://127.0.0.1:9",
                None,
                SecretOwnerId::new(),
            )))
            .await,
    );
    assert_eq!(message, "API key not found for provider");
}

#[tokio::test]
async fn keyless_local_image_servers_ignore_an_unreadable_key_like_legacy() {
    let store = Arc::new(InMemorySecretStore::new());
    let (base, recorded) = server(vec![json_response(&json!({
        "images": [encoded(PNG)]
    }))])
    .await;
    let output = providers(store)
        .generate(request(account(
            "automatic1111",
            &format!("{base}/sdapi/v1"),
            Some(SecretRef::new()),
            SecretOwnerId::new(),
        )))
        .await
        .expect("legacy image_generator commands.rs:231 used unwrap_or_default");
    assert_eq!(output.images[0].bytes, PNG);
    assert_eq!(
        recorded.await.expect("requests")[0].header("authorization"),
        None
    );
}

#[tokio::test]
async fn automatic1111_needs_no_key_and_reads_the_images_array() {
    let store = Arc::new(InMemorySecretStore::new());
    let (base, recorded) = server(vec![json_response(&json!({
        "images": [encoded(PNG)]
    }))])
    .await;
    let output = providers(store)
        .generate(request(account(
            "automatic1111",
            &format!("{base}/sdapi/v1"),
            None,
            SecretOwnerId::new(),
        )))
        .await
        .expect("generate");
    assert_eq!(output.images[0].bytes, PNG);
    let recorded = recorded.await.expect("requests");
    assert_eq!(
        recorded[0].request_line(),
        "POST /sdapi/v1/txt2img HTTP/1.1"
    );
    assert_eq!(recorded[0].header("authorization"), None);
    let body = recorded[0].json();
    assert_eq!(body["steps"], json!(28));
    assert_eq!(body["sampler_index"], json!("DPM++ 2M Karras"));
    assert_eq!(
        body["override_settings"],
        json!({"sd_model_checkpoint": "image-model"})
    );
}

#[tokio::test]
async fn literouter_binary_responses_become_one_image() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![response("200 OK", "image/png", PNG)]).await;
    let output = providers(store)
        .generate(request(account("literouter", &base, Some(key), owner)))
        .await
        .expect("generate");
    assert_eq!(output.images.len(), 1);
    assert_eq!(output.images[0].bytes, PNG);
    assert!(output.usage.is_none());
    let recorded = recorded.await.expect("requests");
    assert_eq!(recorded[0].request_line(), "POST /generate HTTP/1.1");
}

#[tokio::test]
async fn empty_binary_responses_fail_like_legacy() {
    let (store, owner, key) = keyed().await;
    let (base, _recorded) = server(vec![response("200 OK", "image/png", b"")]).await;
    let message = failure(
        providers(store)
            .generate(request(account("literouter", &base, Some(key), owner)))
            .await,
    );
    assert_eq!(message, "Provider returned an empty image response");
}

#[tokio::test]
async fn error_statuses_report_the_status_line_and_body() {
    let (store, owner, key) = keyed().await;
    let (base, _recorded) = server(vec![response(
        "400 Bad Request",
        "application/json",
        b"{\"error\":\"bad prompt\"}",
    )])
    .await;
    let message = failure(
        providers(store)
            .generate(request(account("openai", &base, Some(key), owner)))
            .await,
    );
    assert_eq!(
        message,
        "API error 400 Bad Request: {\"error\":\"bad prompt\"}"
    );
}

#[tokio::test]
async fn linked_images_are_downloaded() {
    let (store, owner, key) = keyed().await;
    let (image_base, image_requests) = server(vec![response("200 OK", "image/png", PNG)]).await;
    let (base, _recorded) = server(vec![json_response(&json!({
        "data": [{"url": format!("{image_base}/out.png")}]
    }))])
    .await;
    let output = providers(store)
        .generate(request(account("openai", &base, Some(key), owner)))
        .await
        .expect("generate");
    assert_eq!(output.images[0].bytes, PNG);
    let image_requests = image_requests.await.expect("requests");
    assert_eq!(image_requests[0].request_line(), "GET /out.png HTTP/1.1");
}

#[tokio::test]
async fn text_only_answers_explain_what_the_provider_said() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![
        response("404 Not Found", "application/json", b"{}"),
        json_response(&json!({
            "choices": [{"message": {"content": "I cannot draw that."}}]
        })),
    ])
    .await;
    let message = failure(
        providers(store)
            .generate(request(account("openrouter", &base, Some(key), owner)))
            .await,
    );
    assert_eq!(
        message,
        "No image URL or data in response. Provider returned text instead: I cannot draw that."
    );
    let recorded = recorded.await.expect("requests");
    assert_eq!(recorded[0].request_line(), "POST /v1/images HTTP/1.1");
    assert_eq!(
        recorded[1].request_line(),
        "POST /v1/chat/completions HTTP/1.1"
    );
    assert_eq!(recorded[1].json()["modalities"], json!(["image"]));
}

#[tokio::test]
async fn gemini_sends_the_key_as_a_query_parameter_and_decodes_inline_data() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![json_response(&json!({
        "candidates": [{"content": {"parts": [
            {"inlineData": {"mimeType": "image/png", "data": encoded(PNG)}}
        ]}}]
    }))])
    .await;
    let output = providers(store)
        .generate(request(account(
            "gemini",
            &format!("{base}/v1"),
            Some(key),
            owner,
        )))
        .await
        .expect("generate");
    assert_eq!(output.images[0].bytes, PNG);
    assert_eq!(
        output.images[0].declared_mime_type.as_deref(),
        Some("image/png")
    );
    let recorded = recorded.await.expect("requests");
    assert_eq!(
        recorded[0].request_line(),
        "POST /v1beta/models/image-model:generateContent?key=key-canary HTTP/1.1"
    );
    assert_eq!(recorded[0].header("authorization"), None);
}

#[tokio::test]
async fn a_cancelled_request_never_reaches_the_provider() {
    let (store, owner, key) = keyed().await;
    let cancelled = request(account("openai", "http://127.0.0.1:9", Some(key), owner));
    cancelled.cancellation.cancel();
    assert!(matches!(
        providers(store).generate(cancelled).await,
        Err(ImageProviderError::Cancelled)
    ));
}

#[tokio::test]
async fn cancelling_an_in_flight_request_stops_waiting_for_the_provider() {
    let (store, owner, key) = keyed().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    let (accepted_sender, accepted) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let _ = read_request(&mut stream).await;
        let _ = accepted_sender.send(());
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        drop(stream);
    });
    let in_flight = request(account(
        "openai",
        &format!("http://{address}"),
        Some(key),
        owner,
    ));
    let cancellation = in_flight.cancellation.clone();
    let providers = providers(store);
    let generation = tokio::spawn(async move { providers.generate(in_flight).await });
    accepted.await.expect("request reached the server");
    cancellation.cancel();
    assert!(matches!(
        generation.await.expect("join"),
        Err(ImageProviderError::Cancelled)
    ));
}

fn comfy_account(base: &str, config: ComfyUiConfig) -> ProviderAccount {
    ProviderAccount {
        config: ProviderConfig::ComfyUi(config),
        ..account("comfyui", base, None, SecretOwnerId::new())
    }
}

#[tokio::test]
async fn comfyui_uploads_references_queues_the_workflow_and_fetches_outputs() {
    let store = Arc::new(InMemorySecretStore::new());
    let (base, recorded) = server(vec![
        json_response(&json!({"name": "ref_0.png", "subfolder": "refs"})),
        json_response(&json!({"prompt_id": "prompt-1"})),
        json_response(&json!({"prompt-1": {
            "status": {"status_str": "success"},
            "outputs": {"9": {"images": [
                {"filename": "out 1.png", "subfolder": "", "type": "output"}
            ]}}
        }})),
        response("200 OK", "image/png", PNG),
    ])
    .await;
    let mut edit = request(comfy_account(
        &format!("{base}/"),
        ComfyUiConfig {
            txt2img_workflow: Some("{\"unused\": true}".to_owned()),
            img2img_workflow: Some(
                "{\"3\": {\"inputs\": {\"text\": \"%PROMPT%\", \"image\": \"%IMAGE%\", \"seed\": %SEED%, \"steps\": %STEPS%}}}"
                    .to_owned(),
            ),
        },
    ));
    edit.settings.seed = Some(42);
    edit.input_images = vec![ImageInput {
        mime_type: "image/png".to_owned(),
        bytes: b"reference".to_vec(),
    }];
    let output = providers(store).generate(edit).await.expect("generate");
    assert_eq!(output.images.len(), 1);
    assert_eq!(output.images[0].bytes, PNG);
    assert!(output.usage.is_none());
    let recorded = recorded.await.expect("requests");
    assert_eq!(recorded[0].request_line(), "POST /upload/image HTTP/1.1");
    assert!(recorded[0].body_text().contains("filename=\"ref_0.png\""));
    assert!(
        recorded[0]
            .body_text()
            .contains("name=\"overwrite\"\r\n\r\ntrue")
    );
    assert_eq!(recorded[0].header("authorization"), None);
    assert_eq!(recorded[1].request_line(), "POST /prompt HTTP/1.1");
    let queued = recorded[1].json();
    assert_eq!(
        queued["prompt"],
        json!({"3": {"inputs": {
            "text": "a \"quiet\" harbor",
            "image": "refs/ref_0.png",
            "seed": 42,
            "steps": 28
        }}})
    );
    assert!(
        queued["client_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    assert_eq!(recorded[2].request_line(), "GET /history/prompt-1 HTTP/1.1");
    assert_eq!(
        recorded[3].request_line(),
        "GET /view?filename=out+1.png&subfolder=&type=output HTTP/1.1"
    );
}

#[tokio::test]
async fn comfyui_sends_bearer_auth_only_with_a_key() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![
        json_response(&json!({"prompt_id": "p"})),
        json_response(&json!({"p": {"outputs": {"1": {"images": [{"filename": "a.png"}]}}}})),
        response("200 OK", "image/png", PNG),
    ])
    .await;
    let mut keyed_account = comfy_account(
        &base,
        ComfyUiConfig {
            txt2img_workflow: Some("{\"seed\": %SEED%}".to_owned()),
            img2img_workflow: None,
        },
    );
    keyed_account.secret_owner_id = owner;
    keyed_account.api_key_ref = Some(key);
    providers(store)
        .generate(request(keyed_account))
        .await
        .expect("generate");
    let recorded = recorded.await.expect("requests");
    for call in &recorded {
        assert_eq!(
            call.header("authorization").as_deref(),
            Some("Bearer key-canary")
        );
    }
    let seed = recorded[0].json()["prompt"]["seed"].as_u64().expect("seed");
    assert!(seed < u64::from(u32::MAX));
}

#[tokio::test]
async fn comfyui_without_a_matching_workflow_fails_like_legacy() {
    let store = Arc::new(InMemorySecretStore::new());
    let mut edit = request(comfy_account(
        "http://127.0.0.1:9",
        ComfyUiConfig {
            txt2img_workflow: Some("   ".to_owned()),
            img2img_workflow: None,
        },
    ));
    let message = failure(providers(Arc::clone(&store)).generate(edit.clone()).await);
    assert_eq!(
        message,
        "ComfyUI credential is missing a workflow. Paste an API-format workflow JSON in the provider settings."
    );
    edit.account.config = ProviderConfig::ComfyUi(ComfyUiConfig {
        txt2img_workflow: None,
        img2img_workflow: None,
    });
    edit.input_images = vec![ImageInput {
        mime_type: "image/png".to_owned(),
        bytes: b"x".to_vec(),
    }];
    let message = failure(providers(store).generate(edit).await);
    assert!(message.starts_with("ComfyUI credential is missing a workflow."));
}

#[tokio::test]
async fn comfyui_reports_workflow_errors_from_history() {
    let store = Arc::new(InMemorySecretStore::new());
    let (base, _recorded) = server(vec![
        json_response(&json!({"prompt_id": "p"})),
        json_response(&json!({"p": {"status": {"status_str": "error"}, "outputs": {}}})),
    ])
    .await;
    let message = failure(
        providers(store)
            .generate(request(comfy_account(
                &base,
                ComfyUiConfig {
                    txt2img_workflow: Some("{}".to_owned()),
                    img2img_workflow: None,
                },
            )))
            .await,
    );
    assert_eq!(message, "ComfyUI reported a workflow execution error");
}

#[test]
fn comfyui_tokens_use_legacy_defaults_and_json_escaping() {
    let mut tokens = request(account("comfyui", "http://x", None, SecretOwnerId::new()));
    tokens.size = None;
    tokens.settings.size = Some("832x1216".to_owned());
    tokens.settings.negative_prompt = Some("  blurry \\ noisy  ".to_owned());
    let template = "%PROMPT%|%NEGATIVE%|%SAMPLER%|%CKPT%|%WIDTH%x%HEIGHT%|%STEPS%|%CFG%|%SEED%|%DENOISE%|%IMAGE%|%IMAGE0%|%IMAGE1%|%IMAGE_COUNT%";
    assert_eq!(
        substitute_tokens(template, &tokens, &["a/b.png".to_owned()], || 7),
        "a \\\"quiet\\\" harbor|blurry \\\\ noisy|euler|image-model|832x1216|28|6.5|7|0.75|a/b.png|a/b.png||1"
    );
    tokens.settings.seed = Some(99);
    assert_eq!(
        substitute_tokens("%SEED%|%IMAGE%|%IMAGE15%|%IMAGE16%", &tokens, &[], || 7),
        "99|||%IMAGE16%"
    );
}

#[tokio::test]
async fn data_urls_without_a_payload_are_rejected_like_legacy() {
    let (store, owner, key) = keyed().await;
    let (base, _recorded) = server(vec![json_response(&json!({
        "data": [{"b64_json": "data:image/png;base64"}]
    }))])
    .await;
    let message = failure(
        providers(store)
            .generate(request(account("openai", &base, Some(key), owner)))
            .await,
    );
    assert_eq!(message, "Invalid data URL format");
}

#[tokio::test]
async fn failed_downloads_and_bad_base64_use_legacy_texts() {
    let (store, owner, key) = keyed().await;
    let (image_base, _image_requests) =
        server(vec![response("404 Not Found", "text/plain", b"")]).await;
    let (base, _recorded) = server(vec![
        json_response(&json!({"data": [{"url": format!("{image_base}/gone.png")}]})),
        json_response(&json!({"data": [{"b64_json": "%%%"}]})),
    ])
    .await;
    let providers = providers(store);
    let message = failure(
        providers
            .generate(request(account("openai", &base, Some(key), owner)))
            .await,
    );
    assert_eq!(message, "Failed to download image: HTTP 404 Not Found");
    let message = failure(
        providers
            .generate(request(account("openai", &base, Some(key), owner)))
            .await,
    );
    assert!(message.starts_with("Failed to decode base64: "));
}

#[tokio::test]
async fn custom_and_host_accounts_without_an_endpoint_never_fall_back_to_openai() {
    let (store, owner, key) = keyed().await;
    let providers = providers(store);
    for kind in ["custom", "lettuce-host"] {
        let mut missing = request(account(kind, "", Some(key), owner));
        missing.account.endpoint = None;
        assert_eq!(
            failure(providers.generate(missing).await),
            "Request failed: the provider has no base URL"
        );
    }
}

#[test]
fn automatic1111_keeps_the_legacy_invalid_tls_opt_in() {
    let mut opted_in = request(account(
        "automatic1111",
        "https://sd.lan",
        None,
        SecretOwnerId::new(),
    ));
    opted_in.account.allow_invalid_tls = true;
    assert!(super::allow_invalid_tls(&opted_in, "automatic1111"));
    assert!(super::allow_invalid_tls(&opted_in, "custom"));
    assert!(!super::allow_invalid_tls(&opted_in, "comfyui"));
    assert!(!super::allow_invalid_tls(&opted_in, "diffusers"));
    opted_in.account.allow_invalid_tls = false;
    assert!(!super::allow_invalid_tls(&opted_in, "automatic1111"));
}

fn png_input(image: &image::DynamicImage) -> ImageInput {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode png");
    ImageInput {
        mime_type: "image/png".to_owned(),
        bytes: bytes.into_inner(),
    }
}

#[tokio::test]
async fn openrouter_uses_the_image_api_and_reads_data_urls() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![json_response(&json!({
        "data": [
            {"b64_json": encoded(PNG), "media_type": "image/webp", "revised_prompt": "a harbor"},
            {"url": "data:image/png;base64,QUJD"},
            {"b64_json": ""}
        ],
        "usage": {"total_tokens": 12}
    }))])
    .await;
    let mut generation = request(account(
        "openrouter",
        &format!("{base}/api/v1/"),
        Some(key),
        owner,
    ));
    generation.size = Some("832x1216".to_owned());
    generation.quality = Some(" High ".to_owned());
    generation.input_images = vec![ImageInput {
        mime_type: "image/png".to_owned(),
        bytes: PNG.to_vec(),
    }];
    let output = providers(store)
        .generate(generation)
        .await
        .expect("generate");
    assert_eq!(output.images.len(), 2);
    assert_eq!(output.images[0].bytes, PNG);
    assert_eq!(
        output.images[0].declared_mime_type.as_deref(),
        Some("image/webp")
    );
    assert_eq!(output.images[0].text.as_deref(), Some("a harbor"));
    assert_eq!(output.images[1].bytes, b"ABC");
    assert!(output.usage.is_some());
    let recorded = recorded.await.expect("requests");
    assert_eq!(recorded[0].request_line(), "POST /api/v1/images HTTP/1.1");
    assert_eq!(
        recorded[0].header("authorization").as_deref(),
        Some("Bearer key-canary")
    );
    assert_eq!(
        recorded[0].json(),
        json!({
            "model": "image-model",
            "prompt": "a \"quiet\" harbor",
            "n": 1,
            "aspect_ratio": "2:3",
            "quality": "high",
            "input_references": [{
                "type": "image_url",
                "image_url": {"url": format!("data:image/png;base64,{}", encoded(PNG))}
            }]
        })
    );
}

#[test]
fn openrouter_image_payload_leaves_out_unknown_and_absent_fields() {
    let mut generation = request(account(
        "openrouter",
        "http://x",
        None,
        SecretOwnerId::new(),
    ));
    generation.size = Some("auto".to_owned());
    generation.quality = Some("hd".to_owned());
    generation.count = 0;
    assert_eq!(
        super::adapters::openrouter_image_payload(&generation),
        json!({"model": "image-model", "prompt": "a \"quiet\" harbor"})
    );
}

#[test]
fn openrouter_sizes_map_to_the_nearest_supported_aspect_ratio() {
    use super::adapters::nearest_aspect_ratio;
    assert_eq!(nearest_aspect_ratio(Some("1024x1024")), Some("1:1"));
    assert_eq!(nearest_aspect_ratio(Some("832x1216")), Some("2:3"));
    assert_eq!(nearest_aspect_ratio(Some("1920x1080")), Some("16:9"));
    assert_eq!(nearest_aspect_ratio(Some("2560x1080")), Some("21:9"));
    assert_eq!(nearest_aspect_ratio(Some("1080x2560")), Some("9:21"));
    assert_eq!(nearest_aspect_ratio(Some("1000x1100")), Some("1:1"));
    assert_eq!(nearest_aspect_ratio(Some("1180x1000")), Some("5:4"));
    assert_eq!(nearest_aspect_ratio(Some("0x1024")), None);
    assert_eq!(nearest_aspect_ratio(Some("wide")), None);
    assert_eq!(nearest_aspect_ratio(None), None);
}

#[test]
fn only_model_or_endpoint_rejections_fall_back_to_chat() {
    use super::adapters::{Adapter, should_fall_back_to_chat};
    assert!(should_fall_back_to_chat(404, ""));
    assert!(should_fall_back_to_chat(400, "Model is not served here"));
    assert!(should_fall_back_to_chat(422, "Unsupported ENDPOINT"));
    assert!(should_fall_back_to_chat(400, "image output not available"));
    assert!(!should_fall_back_to_chat(400, "prompt too long"));
    assert!(!should_fall_back_to_chat(401, "unauthorized"));
    assert!(!should_fall_back_to_chat(500, "model failed"));
    assert_eq!(
        Adapter::OpenRouter.fallback(404, ""),
        Some(Adapter::OpenRouterChat)
    );
    assert_eq!(Adapter::OpenRouterChat.fallback(404, ""), None);
    assert_eq!(Adapter::OpenAi.fallback(404, ""), None);
}

#[tokio::test]
async fn an_unsupported_model_is_retried_through_chat_completions() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![
        response(
            "400 Bad Request",
            "application/json",
            b"{\"error\":{\"message\":\"model does not support the images endpoint\"}}",
        ),
        json_response(&json!({
            "choices": [{"message": {"content": "done", "images": [
                {"image_url": {"url": format!("data:image/png;base64,{}", encoded(PNG))}}
            ]}}]
        })),
    ])
    .await;
    let output = providers(store)
        .generate(request(account(
            "openrouter",
            &format!("{base}/v1"),
            Some(key),
            owner,
        )))
        .await
        .expect("generate");
    assert_eq!(output.images[0].bytes, PNG);
    assert_eq!(output.images[0].text.as_deref(), Some("done"));
    let recorded = recorded.await.expect("requests");
    assert_eq!(recorded[0].request_line(), "POST /v1/images HTTP/1.1");
    assert_eq!(
        recorded[1].request_line(),
        "POST /v1/chat/completions HTTP/1.1"
    );
    assert_eq!(
        recorded[1].header("authorization").as_deref(),
        Some("Bearer key-canary")
    );
}

#[tokio::test]
async fn other_rejections_do_not_fall_back() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![response(
        "400 Bad Request",
        "application/json",
        b"{\"error\":\"prompt rejected\"}",
    )])
    .await;
    let message = failure(
        providers(store)
            .generate(request(account("openrouter", &base, Some(key), owner)))
            .await,
    );
    assert_eq!(
        message,
        "API error 400 Bad Request: {\"error\":\"prompt rejected\"}"
    );
    assert_eq!(recorded.await.expect("requests").len(), 1);
}

#[test]
fn chat_answers_without_images_or_text_report_a_readable_error() {
    let error = super::adapters::Adapter::OpenRouterChat
        .parse(json!({"choices": [{"message": {"content": null}}]}))
        .expect_err("no image");
    assert_eq!(
        error,
        "The model finished without returning an image. Try the request again or pick a different model."
    );
    let error = super::adapters::Adapter::OpenRouter
        .parse(json!({"data": [{"revised_prompt": "only text"}]}))
        .expect_err("no image");
    assert_eq!(error, "No images generated in response");
}

#[test]
fn body_errors_are_read_from_strings_and_objects() {
    use super::body_error::extract_body_error;
    let error = extract_body_error(&json!({
        "error": {"code": 504, "message": " The operation was aborted "},
        "id": "gen-1"
    }))
    .expect("error");
    assert_eq!(error.code, Some(504));
    assert!(error.is_transient());
    assert_eq!(
        error.describe(),
        "Provider error 504: The operation was aborted"
    );
    let error = extract_body_error(&json!({"error": "quota exceeded"})).expect("error");
    assert_eq!(error.code, None);
    assert!(!error.is_transient());
    assert_eq!(error.describe(), "Provider error: quota exceeded");
    let error =
        extract_body_error(&json!({"error": {"code": "529", "msg": "overloaded"}})).expect("error");
    assert_eq!(error.code, Some(529));
    assert!(error.is_transient());
    assert_eq!(error.message, "overloaded");
    let error = extract_body_error(&json!({"error": {"code": 70000, "type": "x"}})).expect("error");
    assert_eq!(error.code, None);
    assert_eq!(error.message, r#"{"code":70000,"type":"x"}"#);
    for code in [500, 502, 503] {
        assert!(
            extract_body_error(&json!({"error": {"code": code}}))
                .expect("error")
                .is_transient()
        );
    }
    assert!(
        !extract_body_error(&json!({"error": {"code": 501}}))
            .expect("error")
            .is_transient()
    );
    assert!(extract_body_error(&json!({"choices": []})).is_none());
    assert!(extract_body_error(&json!({"error": null})).is_none());
    assert!(extract_body_error(&json!({"error": false})).is_none());
    assert!(extract_body_error(&json!({"error": "  "})).is_none());
}

#[tokio::test]
async fn transient_body_errors_are_retried_once() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![
        json_response(&json!({"error": {"code": 504, "message": "aborted"}})),
        json_response(&json!({"data": [{"b64_json": encoded(PNG)}]})),
    ])
    .await;
    let output = providers(store)
        .generate(request(account("openrouter", &base, Some(key), owner)))
        .await
        .expect("generate");
    assert_eq!(output.images[0].bytes, PNG);
    assert_eq!(recorded.await.expect("requests").len(), 2);
}

#[tokio::test]
async fn a_second_transient_failure_is_reported() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![
        response("502 Bad Gateway", "text/plain", b"upstream failed"),
        json_response(&json!({"error": {"code": 503, "message": "still down"}})),
    ])
    .await;
    let message = failure(
        providers(store)
            .generate(request(account("openai", &base, Some(key), owner)))
            .await,
    );
    assert_eq!(message, "Provider error 503: still down");
    assert_eq!(recorded.await.expect("requests").len(), 2);
}

#[tokio::test]
async fn server_errors_are_retried_once_then_reported() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![
        response("500 Internal Server Error", "text/plain", b"first"),
        response("502 Bad Gateway", "text/plain", b"upstream failed"),
    ])
    .await;
    let message = failure(
        providers(store)
            .generate(request(account("openrouter", &base, Some(key), owner)))
            .await,
    );
    assert_eq!(message, "API error 502 Bad Gateway: upstream failed");
    assert_eq!(recorded.await.expect("requests").len(), 2);
}

#[tokio::test]
async fn permanent_body_errors_are_not_retried() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![json_response(
        &json!({"error": {"code": "429", "message": "slow down"}}),
    )])
    .await;
    let message = failure(
        providers(store)
            .generate(request(account("openai", &base, Some(key), owner)))
            .await,
    );
    assert_eq!(message, "Provider error 429: slow down");
    assert_eq!(recorded.await.expect("requests").len(), 1);
}

#[tokio::test]
async fn payload_too_large_gets_a_readable_message() {
    let (store, owner, key) = keyed().await;
    let (base, _recorded) = server(vec![response(
        "413 Payload Too Large",
        "text/html",
        b"<html>413</html>",
    )])
    .await;
    let message = failure(
        providers(store)
            .generate(request(account("openai", &base, Some(key), owner)))
            .await,
    );
    assert_eq!(
        message,
        "API error 413 Payload Too Large: the provider rejected the request because it was too large. Use a smaller reference image or fewer reference images."
    );
}

#[tokio::test]
async fn oversized_reference_images_are_shrunk_before_upload() {
    let (store, owner, key) = keyed().await;
    let (base, recorded) = server(vec![json_response(&json!({
        "data": [{"b64_json": encoded(PNG)}]
    }))])
    .await;
    let mut generation = request(account("openrouter", &base, Some(key), owner));
    generation.input_images = vec![png_input(&image::DynamicImage::ImageRgb8(
        image::RgbImage::from_fn(2100, 60, |x, y| {
            image::Rgb([(x % 251) as u8, (y * 3) as u8, ((x + y) % 7) as u8])
        }),
    ))];
    providers(store)
        .generate(generation)
        .await
        .expect("generate");
    let body = recorded.await.expect("requests")[0].json();
    let url = body["input_references"][0]["image_url"]["url"]
        .as_str()
        .expect("reference url")
        .to_owned();
    let encoded_image = url
        .strip_prefix("data:image/jpeg;base64,")
        .expect("jpeg data url");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded_image)
        .expect("base64");
    let decoded = image::load_from_memory(&bytes).expect("decode");
    assert_eq!((decoded.width(), decoded.height()), (2048, 59));
}
