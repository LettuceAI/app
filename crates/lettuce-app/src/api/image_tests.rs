use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails};
use lettuce_database::Database;
use lettuce_image_generation::sd_runtime::layout::{
    DiffusionPaths, cli_executable_name, server_executable_name,
};
use lettuce_image_generation::sd_runtime::output::GenerationProgress;
use lettuce_image_generation::{
    ImageError, ImageFailureKind, ImageGenerationRepository, ImageGenerationState,
    ImageProviderError, ImageProviderPort, ProviderImage, ProviderImageOutput,
    ProviderImageRequest,
};
use lettuce_jobs::{JobState, JobStore, SystemClock};
use lettuce_media::LocalMediaBlobStore;
use lettuce_models::{CapabilityStatus, ModelProfileRepository, ProviderProtocol};
use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
use lettuce_settings::DeviceSettings;
use lettuce_types::{AssetId, JobId, ModelProfileId, OperationId, RequestId, TimestampMillis};

use super::tests::{Harness, Reply, harness_over_files};
use super::*;
use crate::{AppBackend, AppDatabaseLocation};

const RELEASE: &str = "master-778-a";
const CPU_BUILD: &str = "sd-master-bin-Linux-Ubuntu-24.04-x86_64.zip";

fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&2_u32.to_be_bytes());
    bytes.extend_from_slice(&3_u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes.extend_from_slice(b"generated image bytes");
    bytes
}

enum Next {
    Fail(ImageProviderError),
}

/// An image provider that holds every generation until released, reports
/// progress for it as the local engine does, and ends it when cancelled.
struct HeldImages {
    started: Mutex<Vec<(JobId, String)>>,
    release: tokio::sync::Semaphore,
    entered: tokio::sync::Notify,
    next: Mutex<Option<Next>>,
}

impl HeldImages {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            started: Mutex::new(Vec::new()),
            release: tokio::sync::Semaphore::new(0),
            entered: tokio::sync::Notify::new(),
            next: Mutex::new(None),
        })
    }

    fn failing(error: ImageProviderError) -> Arc<Self> {
        let images = Self::new();
        *images.next.lock().expect("next") = Some(Next::Fail(error));
        images
    }

    fn calls(&self) -> usize {
        self.started.lock().expect("started").len()
    }

    fn proceed(&self) {
        self.release.add_permits(1);
    }
}

#[async_trait]
impl ImageProviderPort for HeldImages {
    async fn generate(
        &self,
        request: ProviderImageRequest,
    ) -> Result<ProviderImageOutput, ImageProviderError> {
        self.started
            .lock()
            .expect("started")
            .push((request.job_id, request.prompt.clone()));
        let steps = if request.prompt == "second" { 5 } else { 3 };
        if let Some(progress) = &request.progress {
            progress.0.progress(GenerationProgress::Starting);
            progress
                .0
                .progress(GenerationProgress::Sampling { step: 1, steps });
        }
        self.entered.notify_one();
        if let Some(Next::Fail(error)) = self.next.lock().expect("next").take() {
            return Err(error);
        }
        tokio::select! {
            () = request.cancellation.cancelled() => Err(ImageProviderError::Cancelled),
            permit = self.release.acquire() => {
                permit.expect("open semaphore").forget();
                Ok(ProviderImageOutput {
                    images: vec![ProviderImage {
                        bytes: png(),
                        declared_mime_type: Some("image/png".to_owned()),
                        text: None,
                    }],
                    usage: None,
                })
            }
        }
    }
}

struct Desktop {
    harness: Harness,
    root: PathBuf,
}

impl Drop for Desktop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A context over a database file, a media store, the database files media
/// collection reads and, with `engine`, the local engine over the app folder.
fn desktop(images: Arc<dyn ImageProviderPort>, engine: bool) -> Desktop {
    let root = std::env::temp_dir().join(format!("lettuce-image-api-{}", OperationId::new()));
    let private = root.join("private-persistent-v2");
    std::fs::create_dir_all(&private).expect("root");
    let authority = FilesystemAuthority::new(
        &DirectorySnapshot::with_private_persistent(&root, &private).expect("snapshot"),
    )
    .expect("authority");
    let location = AppDatabaseLocation::new(private, &authority).expect("location");
    let path = location.active_path().expect("path");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("database folder");
    let mut backend = AppBackend::open(&path, TimestampMillis::new(1)).expect("backend");
    if engine {
        backend = backend
            .with_local_diffusion(crate::image::local_diffusion::diffusion_paths(
                &DeviceSettings::default(),
                &root,
            ))
            .expect("engine");
    }
    let media = LocalMediaBlobStore::new(
        authority.managed_files(),
        authority
            .read_capability(ManagedRoot::MediaBlobs)
            .expect("read capability"),
        authority
            .write_capability(ManagedRoot::MediaBlobs)
            .expect("write capability"),
        Database::open(&path).expect("blob catalog"),
        Database::open(&path).expect("asset catalog"),
    );
    let harness = harness_over_files(
        Arc::new(backend),
        Reply::Text("unused"),
        Arc::new(SystemClock),
        Some(Arc::new(media)),
        Some(root.clone()),
        Arc::new(NoModels),
        images,
        Some(ApiDatabaseFiles {
            location,
            active: path,
        }),
    );
    Desktop { harness, root }
}

fn image_model(context: &ApiContext, protocol: ProviderProtocol, kind: &str) -> ModelProfileId {
    let database = context.backend().database();
    let id = crate::launch::tests::seed_model(database, protocol, kind);
    let mut model = ModelProfileRepository::get(database, id)
        .expect("model")
        .expect("exists");
    let revision = model.revision;
    model.config.capabilities.output_modalities.image = CapabilityStatus::Supported;
    ModelProfileRepository::upsert(database, model, Some(revision)).expect("image output");
    id
}

fn local_model(context: &ApiContext) -> ModelProfileId {
    image_model(context, ProviderProtocol::StableDiffusion, "sdcpp")
}

fn remote_model(context: &ApiContext) -> ModelProfileId {
    image_model(context, ProviderProtocol::OpenAiCompatible, "openai")
}

fn generate_request(model: ModelProfileId, prompt: &str) -> dto::ImageGenerateRequest {
    dto::ImageGenerateRequest {
        request_id: RequestId::new().to_string(),
        model_id: model.to_string(),
        prompt: prompt.to_owned(),
        settings: dto::ImageSettings::default(),
        input_images: Vec::new(),
        mask_image: None,
        loras: Vec::new(),
        size: Some("512x512".to_owned()),
        quality: None,
        style: None,
        count: Some(1),
        source: dto::ImageRequestSource::Direct,
        attribution: dto::ImageAttribution::default(),
        output: dto::ImageOutput::Retained,
    }
}

fn job_id(accepted: &dto::JobAccepted) -> JobId {
    accepted.job_id.parse().expect("job id")
}

fn state(context: &ApiContext, job_id: JobId) -> JobState {
    JobStore::get(context.backend().database(), job_id)
        .expect("job")
        .expect("job exists")
        .state
}

async fn view(context: &ApiContext, job_id: JobId) -> dto::JobView {
    job_get(
        context,
        dto::JobGetRequest {
            job_id: job_id.to_string(),
        },
    )
    .await
    .expect("job")
}

#[derive(Default)]
struct Watch(Mutex<Vec<dto::JobEvent>>);

impl JobEventSink for Watch {
    fn emit(&self, event: dto::JobEvent) -> bool {
        self.0.lock().expect("events").push(event);
        true
    }
}

impl Watch {
    fn progress(&self) -> Vec<dto::ImageProgress> {
        self.0
            .lock()
            .expect("events")
            .iter()
            .filter_map(|event| match event {
                dto::JobEvent::ImageProgress { progress } => Some(progress.clone()),
                _ => None,
            })
            .collect()
    }
}

async fn watch(context: &ApiContext, job_id: JobId) -> Arc<Watch> {
    let sink = Arc::new(Watch::default());
    job_watch(
        context,
        dto::JobWatchRequest {
            job_id: job_id.to_string(),
        },
        sink.clone(),
    )
    .await
    .expect("watch");
    sink
}

async fn until(condition: impl Fn() -> bool) {
    for _ in 0..400 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the condition was never met");
}

#[tokio::test(flavor = "multi_thread")]
async fn overlapping_local_generations_take_turns_and_each_job_hears_only_its_own_progress() {
    let images = HeldImages::new();
    let desktop = desktop(images.clone(), false);
    let context = &desktop.harness.context;
    let model = local_model(context);
    let first = job_id(
        &image_generate(context, generate_request(model, "first"))
            .await
            .expect("first"),
    );
    let second = job_id(
        &image_generate(context, generate_request(model, "second"))
            .await
            .expect("second"),
    );
    let (first_watch, second_watch) = (watch(context, first).await, watch(context, second).await);
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("first starts"));
    images.entered.notified().await;
    assert!(
        !runner.run_once().await.expect("lane check"),
        "the second local job waits for the lane"
    );
    assert_eq!(images.calls(), 1);
    assert_eq!(state(context, second), JobState::Queued);
    until(|| {
        first_watch
            .progress()
            .iter()
            .any(|progress| progress.total_steps == Some(3))
    })
    .await;
    assert!(second_watch.progress().is_empty());

    images.proceed();
    runner.wait_idle().await;
    assert_eq!(state(context, first), JobState::Succeeded);
    assert!(runner.run_once().await.expect("second starts"));
    images.entered.notified().await;
    until(|| {
        second_watch
            .progress()
            .iter()
            .any(|progress| progress.total_steps == Some(5))
    })
    .await;
    images.proceed();
    runner.wait_idle().await;
    assert_eq!(state(context, second), JobState::Succeeded);
    assert!(
        first_watch
            .progress()
            .iter()
            .filter(|progress| progress.total_steps.is_some())
            .all(|progress| progress.total_steps == Some(3)),
        "the first job never heard the second one's progress"
    );
    let finished = view(context, first).await;
    let Some(dto::JobResultDto::ImageGeneration { images, .. }) = finished.result else {
        panic!("expected the generated image: {:?}", finished.result);
    };
    assert_eq!(images.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_one_local_job_leaves_the_others_alone() {
    let images = HeldImages::new();
    let desktop = desktop(images.clone(), false);
    let context = &desktop.harness.context;
    let model = local_model(context);
    let running = job_id(
        &image_generate(context, generate_request(model, "first"))
            .await
            .expect("running"),
    );
    let queued = job_id(
        &image_generate(context, generate_request(model, "second"))
            .await
            .expect("queued"),
    );
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("first starts"));
    images.entered.notified().await;
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: queued.to_string(),
        },
    )
    .await
    .expect("cancel the queued job");
    assert_eq!(state(context, queued), JobState::Cancelled);
    assert_eq!(
        images.calls(),
        1,
        "a job cancelled in the queue never reaches the engine"
    );
    assert!(matches!(
        state(context, running),
        JobState::Running | JobState::Claimed
    ));
    images.proceed();
    runner.wait_idle().await;
    assert_eq!(state(context, running), JobState::Succeeded);

    let third = job_id(
        &image_generate(context, generate_request(model, "second"))
            .await
            .expect("third"),
    );
    assert!(runner.run_once().await.expect("third starts"));
    images.entered.notified().await;
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: third.to_string(),
        },
    )
    .await
    .expect("cancel the running job");
    runner.wait_idle().await;
    assert_eq!(state(context, third), JobState::Cancelled);
    assert_eq!(
        view(context, third).await.state,
        dto::JobStateDto::Cancelled
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_same_request_returns_its_job_and_another_one_under_the_id_conflicts() {
    let desktop = desktop(HeldImages::new(), false);
    let context = &desktop.harness.context;
    let model = remote_model(context);
    let request = generate_request(model, "a harbor");
    let first = image_generate(context, request.clone())
        .await
        .expect("first");
    let again = image_generate(context, request.clone())
        .await
        .expect("a replay");
    assert_eq!(first, again);
    let other = image_generate(
        context,
        dto::ImageGenerateRequest {
            prompt: "a different harbor".to_owned(),
            ..request
        },
    )
    .await
    .expect_err("another request under the same id");
    assert_eq!(other.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restart_cancels_a_queued_playground_generation_and_interrupts_a_running_one() {
    let desktop = desktop(HeldImages::new(), false);
    let context = &desktop.harness.context;
    let model = remote_model(context);
    let playground = |prompt: &str| {
        let mut request = generate_request(model, prompt);
        request.source = dto::ImageRequestSource::Playground;
        request
    };
    let queued = job_id(
        &image_generate(context, playground("queued"))
            .await
            .expect("queued"),
    );
    let running = job_id(
        &image_generate(context, playground("running"))
            .await
            .expect("running"),
    );
    let database = context.backend().database();
    let work = context
        .backend()
        .image_generations()
        .claim(
            running,
            lettuce_jobs::WorkerId::new(),
            context.now(),
            Duration::from_secs(60),
            &lettuce_jobs::ResourceAvailability::all(),
        )
        .expect("claim")
        .expect("claimed");
    drop(work);

    let restarted = context.restarted();
    restarted.recover_after_restart().expect("recovery");
    assert_eq!(state(&restarted, queued), JobState::Cancelled);
    assert_eq!(state(&restarted, running), JobState::Interrupted);
    let queued_record = ImageGenerationRepository::get(database, queued).expect("record");
    assert!(matches!(
        queued_record.state,
        ImageGenerationState::Cancelled { .. }
    ));
    let running_record = ImageGenerationRepository::get(database, running).expect("record");
    assert!(
        matches!(
            &running_record.state,
            ImageGenerationState::Failed { message, .. }
                if message == "Image generation was interrupted."
        ),
        "{:?}",
        running_record.state
    );
    let page = playground_history_list(
        &restarted,
        dto::PlaygroundHistoryListRequest {
            limit: None,
            before: None,
        },
    )
    .await
    .expect("history");
    let status = |job: JobId| {
        page.entries
            .iter()
            .find(|entry| entry.id == job.to_string())
            .map(|entry| entry.status)
    };
    assert_eq!(status(queued), Some(dto::PlaygroundStatus::Cancelled));
    assert_eq!(status(running), Some(dto::PlaygroundStatus::Failed));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_engine_failure_reaches_the_job_and_the_playground_entry_typed() {
    let error = ImageError::new(
        ImageFailureKind::ModelFileMissing,
        "Local image model file not found: /models/x.gguf",
    );
    let desktop = desktop(
        HeldImages::failing(ImageProviderError::Engine(error.clone())),
        false,
    );
    let context = &desktop.harness.context;
    let model = local_model(context);
    let mut request = generate_request(model, "first");
    request.source = dto::ImageRequestSource::Playground;
    let job = job_id(&image_generate(context, request).await.expect("accepted"));
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    runner.wait_idle().await;
    let failed = view(context, job).await;
    assert_eq!(failed.state, dto::JobStateDto::Failed);
    assert_eq!(
        failed.failure.expect("failure").image,
        Some(dto::ImageFailure {
            kind: dto::ImageFailureKind::ModelFileMissing,
            message: error.message.clone(),
        })
    );
    let page = playground_history_list(
        context,
        dto::PlaygroundHistoryListRequest {
            limit: Some(5),
            before: None,
        },
    )
    .await
    .expect("history");
    assert_eq!(
        page.entries[0].failure,
        Some(dto::ImageFailure {
            kind: dto::ImageFailureKind::ModelFileMissing,
            message: error.message,
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_playground_request_replaces_the_models_base_loras_by_its_own() {
    let desktop = desktop(HeldImages::new(), false);
    let context = &desktop.harness.context;
    let model = local_model(context);
    let mut request = generate_request(model, "first");
    request.source = dto::ImageRequestSource::Playground;
    request.loras = vec![dto::ImageLora {
        path: "kept.safetensors".to_owned(),
        multiplier: 0.8,
        is_high_noise: false,
        keywords: Vec::new(),
    }];
    let job = job_id(&image_generate(context, request).await.expect("accepted"));
    let stored = ImageGenerationRepository::get(context.backend().database(), job)
        .expect("record")
        .request;
    assert_eq!(
        stored.settings.base_loras,
        Some(Vec::new()),
        "a base LoRA the draft dropped is not run"
    );
    assert_eq!(stored.loras.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_a_playground_entry_removes_its_image_files_at_once() {
    let images = HeldImages::new();
    let desktop = desktop(images.clone(), false);
    let context = &desktop.harness.context;
    let model = remote_model(context);
    let mut request = generate_request(model, "first");
    request.source = dto::ImageRequestSource::Playground;
    let job = job_id(&image_generate(context, request).await.expect("accepted"));
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    images.entered.notified().await;
    images.proceed();
    runner.wait_idle().await;
    assert_eq!(state(context, job), JobState::Succeeded);
    let files = |root: &Path| -> usize {
        fn count(path: &Path) -> usize {
            std::fs::read_dir(path)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| {
                    if entry.path().is_dir() {
                        count(&entry.path())
                    } else {
                        1
                    }
                })
                .sum()
        }
        count(root)
    };
    let media_root = desktop.root.clone();
    let before = files(&media_root);
    let deleted = playground_history_delete(
        context,
        dto::PlaygroundHistoryDeleteRequest {
            id: job.to_string(),
            delete_images: true,
        },
    )
    .await
    .expect("delete");
    assert_eq!(deleted.deleted_images, 1);
    let page = playground_history_list(
        context,
        dto::PlaygroundHistoryListRequest {
            limit: None,
            before: None,
        },
    )
    .await
    .expect("history");
    assert!(page.entries.is_empty());
    assert!(
        files(&media_root) < before,
        "the image's file is gone right after the delete: {} before, {} after",
        before,
        files(&media_root)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_playground_form_is_described_by_the_backend() {
    let desktop = desktop(HeldImages::new(), false);
    let context = &desktop.harness.context;
    let local = image_capabilities(
        context,
        dto::ImageCapabilitiesRequest {
            target: dto::ImageCapabilityTarget::Model {
                model_id: local_model(context).to_string(),
            },
        },
    )
    .await
    .expect("local");
    assert!(local.local && local.negative_prompt);
    assert!(local.samplers.contains(&"euler_a".to_owned()));
    assert!(local.quality.is_empty());
    let openai = image_capabilities(
        context,
        dto::ImageCapabilitiesRequest {
            target: dto::ImageCapabilityTarget::Provider {
                provider_kind: "openai".to_owned(),
                model: Some("dall-e-3".to_owned()),
            },
        },
    )
    .await
    .expect("openai");
    assert_eq!(openai.sizes, ["1024x1024", "1024x1792", "1792x1024"]);
    assert_eq!(openai.styles, ["vivid", "natural"]);
    assert!(!openai.negative_prompt && !openai.local);
}

#[test]
fn every_setting_the_contract_names_is_one_the_model_has() {
    let full: serde_json::Value = serde_json::from_str(
        r#"{
        "steps": 20, "cfg_scale": 7.5, "sampler": "euler", "scheduler": "karras", "seed": 3,
        "negative_prompt": "blur", "denoising_strength": 0.5, "image_cfg_scale": 1.5,
        "distilled_guidance": 3.5, "eta": 0.1, "flow_shift": 2.0, "size": "512x512",
        "vae_tiling_enabled": true, "vae_tile_size_x": 64, "vae_tile_size_y": 64,
        "vae_tile_overlap": 0.25, "auto_resize_reference_images": true,
        "increase_reference_index": false, "hires_enabled": true, "hires_upscaler": "up",
        "hires_scale": 2.0, "hires_width": 1024, "hires_height": 1024, "hires_steps": 8,
        "hires_denoising_strength": 0.4, "slg_scale": 1.0, "slg_layers": "7,8",
        "slg_layer_start": 0.01, "slg_layer_end": 0.2, "cache_mode": "cache_dit",
        "cache_option": "x", "offload_mode": "mixed", "extra_prompt": "extra",
        "prompt_writer_instructions": "write",
        "base_loras": [{"path": "a.safetensors", "multiplier": 1.0, "is_high_noise": true, "keywords": ["k"]}]
    }"#,
    )
    .expect("settings json");
    let dto: dto::ImageSettings = serde_json::from_value(full.clone()).expect("contract");
    let model = super::image::settings_for_test(&dto).expect("model settings");
    let mut back = serde_json::to_value(&model).expect("json");
    back.as_object_mut().expect("object").remove("cpp");
    assert_eq!(back, full);
}

fn script(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("script");
    let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("mode");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn an_upscale_from_the_playground_is_recorded_as_an_entry_of_its_own() {
    let images = HeldImages::new();
    let desktop = desktop(images.clone(), true);
    let context = &desktop.harness.context;
    let engine = context.backend().local_diffusion().expect("engine").clone();
    let paths = engine.paths();
    let build = paths.runtime_root(RELEASE, CPU_BUILD);
    std::fs::create_dir_all(&build).expect("build");
    script(&build.join(server_executable_name()), "exit 1");
    script(&build.join(cli_executable_name()), "cp \"$6\" \"$8\"");
    std::fs::create_dir_all(&paths.upscalers).expect("upscalers");
    std::fs::write(paths.upscalers.join("upscaler.pth"), b"model").expect("upscaler");

    let model = remote_model(context);
    let mut request = generate_request(model, "first");
    request.source = dto::ImageRequestSource::Playground;
    let generated = job_id(&image_generate(context, request).await.expect("accepted"));
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    images.entered.notified().await;
    images.proceed();
    runner.wait_idle().await;
    let Some(dto::JobResultDto::ImageGeneration { images, .. }) =
        view(context, generated).await.result
    else {
        panic!("expected the generated image");
    };
    let asset = images[0].asset.asset_id.clone();

    let upscale = job_id(
        &image_upscale(
            context,
            dto::ImageUpscaleRequest {
                asset_id: asset,
                origin: Some(dto::ImageUpscaleOrigin::Playground {
                    entry_id: generated.to_string(),
                }),
                client_operation_id: "upscale-1".to_owned(),
            },
        )
        .await
        .expect("upscale accepted"),
    );
    assert!(runner.run_once().await.expect("upscale starts"));
    runner.wait_idle().await;
    let done = view(context, upscale).await;
    let Some(dto::JobResultDto::ImageUpscaled { upscaled }) = done.result else {
        panic!("expected the upscaled image: {done:?}");
    };
    assert_eq!(upscaled.history_id, Some(upscale.to_string()));
    let page = playground_history_list(
        context,
        dto::PlaygroundHistoryListRequest {
            limit: None,
            before: None,
        },
    )
    .await
    .expect("history");
    let entry = page
        .entries
        .iter()
        .find(|entry| entry.id == upscale.to_string())
        .expect("the upscale entry");
    assert_eq!(entry.upscale_of, Some(generated.to_string()));
    assert_eq!(entry.status, dto::PlaygroundStatus::Complete);
    assert_eq!(entry.images.len(), 1);
    assert_eq!(entry.model_id.as_deref(), Some(model.to_string().as_str()));
    assert_eq!(entry.prompt, "first");
    assert!(
        std::fs::read_dir(&paths.upscale_scratch)
            .expect("scratch")
            .next()
            .is_none()
    );

    let replay = image_upscale(
        context,
        dto::ImageUpscaleRequest {
            asset_id: upscaled.asset.asset_id.clone(),
            origin: None,
            client_operation_id: "upscale-1".to_owned(),
        },
    )
    .await
    .expect_err("the key names another request");
    assert_eq!(replay.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_upscale_without_an_upscaler_or_engine_is_refused_typed() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let error = image_upscale(
        context,
        dto::ImageUpscaleRequest {
            asset_id: AssetId::new().to_string(),
            origin: None,
            client_operation_id: "upscale".to_owned(),
        },
    )
    .await
    .expect_err("nothing installed");
    assert_eq!(
        error.details,
        Some(ApiErrorDetails::Image {
            failure: dto::ImageFailureKind::UpscalerMissing
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_engine_is_built_from_the_models_folder_and_follows_a_move() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let engine = context.backend().local_diffusion().expect("engine").clone();
    let default_root = desktop.root.join("models").join("image");
    assert_eq!(engine.paths().image_root, default_root);
    let target = desktop.root.with_file_name(format!(
        "{}-elsewhere",
        desktop.root.file_name().expect("name").to_string_lossy()
    ));
    let accepted = local_models_dir_set(
        context,
        dto::LocalModelsDirSetRequest {
            path: target.display().to_string(),
            move_existing: true,
            client_operation_id: "move-1".to_owned(),
        },
    )
    .await
    .expect("move accepted");
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("move starts"));
    runner.wait_idle().await;
    assert_eq!(
        view(context, job_id(&accepted)).await.state,
        dto::JobStateDto::Succeeded
    );
    let paths = engine.paths();
    assert_eq!(paths.image_root, target.join("image"));
    assert_eq!(paths.default_image_root, Some(default_root));
    let first = &lettuce_image_generation::diffusion_catalog().profiles[0];
    let variant = &first.variants[0];
    let component = first.components(variant)[0];
    assert!(
        paths
            .component_path(component)
            .starts_with(target.join("image")),
        "a model installed after the move lands in the new folder"
    );
    assert_eq!(
        DiffusionPaths::legacy_layout(&desktop.root, target.join("image")).image_root,
        paths.image_root
    );
}

fn bundle_asset(path: &str, sha: &str) -> lettuce_image_generation::BundleAsset {
    lettuce_image_generation::BundleAsset {
        selection_id: "selection".to_owned(),
        profile_id: "z-image-turbo".to_owned(),
        role: lettuce_image_generation::DiffusionComponentRole::DiffusionModel,
        model_id: "org/repo".to_owned(),
        revision: "c".repeat(40),
        relative_path: path.to_owned(),
        format: "gguf".to_owned(),
        quantization: None,
        size: 4,
        sha256: sha.to_owned(),
        architecture: None,
        gated: false,
    }
}

fn bundle_request(files: &[(&str, &str)]) -> dto::HfImageBundleInstallRequest {
    dto::HfImageBundleInstallRequest {
        profile_id: "z-image-turbo".to_owned(),
        display_name: "Bundle".to_owned(),
        runtime_release: RELEASE.to_owned(),
        runtime_asset: CPU_BUILD.to_owned(),
        assets: files
            .iter()
            .map(|(path, sha)| {
                let asset = bundle_asset(path, sha);
                dto::ImageBundleAsset {
                    selection_id: asset.selection_id,
                    profile_id: asset.profile_id,
                    role: dto::ImageComponentRole::DiffusionModel,
                    model_id: asset.model_id,
                    revision: asset.revision,
                    relative_path: asset.relative_path,
                    format: asset.format,
                    quantization: None,
                    size: asset.size,
                    sha256: asset.sha256,
                    architecture: None,
                    gated: false,
                }
            })
            .collect(),
    }
}

/// Puts a running bundle install of `files` into the runner's memory.
fn running_bundle_install(context: &ApiContext, files: &[(&str, &str)]) -> JobId {
    let paths = context.backend().local_diffusion().expect("engine").paths();
    let root = lettuce_image_generation::bundle_root(&paths.image_root);
    let artifacts = files
        .iter()
        .map(|(path, sha)| {
            let asset = bundle_asset(path, sha);
            let source = crate::ArtifactSource::HuggingFace {
                repository: asset.model_id.clone(),
                revision: asset.revision.clone(),
                path: asset.relative_path.clone(),
            };
            crate::PlannedArtifact {
                artifact: lettuce_model_hub::PinnedArtifact {
                    source_identity: source.identity(),
                    local_segments: asset.local_segments().expect("segments"),
                    byte_size: 4,
                    sha256: Some((*sha).to_owned()),
                },
                source,
            }
        })
        .collect();
    let existing = JobId::new();
    context.jobs().put_install(
        existing,
        crate::api::InstallWork::Artifact {
            plan: crate::ArtifactInstallPlan {
                install_id: "hf-bundle:running".to_owned(),
                root,
                artifacts,
            },
            finish: Box::new(crate::api::InstallFinish::HuggingFaceBundle {
                paths: (*paths).clone(),
                bundle_id: "running".to_owned(),
            }),
        },
    );
    existing
}

fn nothing_written(context: &ApiContext) -> bool {
    let paths = context.backend().local_diffusion().expect("engine").paths();
    !lettuce_image_generation::bundle_root(&paths.image_root).exists()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bundle_with_exactly_the_files_of_a_running_install_joins_its_job() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let (a, b) = ("a".repeat(64), "b".repeat(64));
    let existing = running_bundle_install(context, &[("model.gguf", &a), ("other.gguf", &b)]);
    let joined = hf_image_bundle_install(
        context,
        bundle_request(&[("other.gguf", &b), ("model.gguf", &a)]),
    )
    .await
    .expect("joins the running install");
    assert_eq!(joined.job_id, existing.to_string());
    assert_eq!(joined.bundle_id, "running");
    assert!(nothing_written(context));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bundle_sharing_only_some_files_with_a_running_install_is_busy() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let (a, b, c) = ("a".repeat(64), "b".repeat(64), "c".repeat(64));
    running_bundle_install(context, &[("model.gguf", &a), ("other.gguf", &b)]);
    let subset = hf_image_bundle_install(context, bundle_request(&[("model.gguf", &a)]))
        .await
        .expect_err("a subset is not the running bundle");
    assert_eq!(subset.code, ApiErrorCode::Busy);
    let superset = hf_image_bundle_install(
        context,
        bundle_request(&[("model.gguf", &a), ("other.gguf", &b), ("third.gguf", &c)]),
    )
    .await
    .expect_err("a superset is not the running bundle");
    assert_eq!(superset.code, ApiErrorCode::Busy);
    let overlap = hf_image_bundle_install(
        context,
        bundle_request(&[("model.gguf", &a), ("third.gguf", &c)]),
    )
    .await
    .expect_err("an overlap is not the running bundle");
    assert_eq!(overlap.code, ApiErrorCode::Busy);
    assert!(nothing_written(context), "no manifest was written");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bundle_install_waits_for_the_admission_in_progress_and_then_sees_its_job() {
    let desktop = desktop(HeldImages::new(), true);
    let context = desktop.harness.context.clone();
    let (a, b) = ("a".repeat(64), "b".repeat(64));
    let admitting = context.image_state().bundle_admission().lock().await;
    let identical = {
        let (context, a) = (context.clone(), a.clone());
        tokio::spawn(async move {
            hf_image_bundle_install(&context, bundle_request(&[("model.gguf", &a)])).await
        })
    };
    let overlapping = {
        let (context, a, b) = (context.clone(), a.clone(), b.clone());
        tokio::spawn(async move {
            hf_image_bundle_install(
                &context,
                bundle_request(&[("model.gguf", &a), ("x.gguf", &b)]),
            )
            .await
        })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!identical.is_finished() && !overlapping.is_finished());
    let existing = running_bundle_install(&context, &[("model.gguf", &a)]);
    drop(admitting);
    let joined = identical.await.expect("task").expect("joins the first");
    assert_eq!(joined.job_id, existing.to_string());
    let busy = overlapping
        .await
        .expect("task")
        .expect_err("overlaps the first");
    assert_eq!(busy.code, ApiErrorCode::Busy);
    assert!(nothing_written(&context));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restart_cancels_queued_tool_jobs_and_clears_upscale_scratch() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let queued = job_id(
        &lora_keywords_discover(
            context,
            dto::LoraKeywordsDiscoverRequest {
                path: "style.safetensors".to_owned(),
                profile_id: None,
                client_operation_id: "discover-1".to_owned(),
            },
        )
        .await
        .expect("accepted"),
    );
    let replayed = job_id(
        &lora_keywords_discover(
            context,
            dto::LoraKeywordsDiscoverRequest {
                path: "style.safetensors".to_owned(),
                profile_id: None,
                client_operation_id: "discover-1".to_owned(),
            },
        )
        .await
        .expect("replayed"),
    );
    assert_eq!(queued, replayed);
    let scratch = context
        .backend()
        .local_diffusion()
        .expect("engine")
        .paths()
        .upscale_scratch
        .clone();
    std::fs::create_dir_all(&scratch).expect("scratch");
    std::fs::write(scratch.join("left-in.png"), b"cut short").expect("leftover");

    let restarted = context.restarted();
    restarted.recover_after_restart().expect("recovery");
    assert_eq!(state(&restarted, queued), JobState::Cancelled);
    assert!(
        std::fs::read_dir(&scratch)
            .expect("scratch")
            .next()
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_avatar_prompt_follows_the_models_provider() {
    let desktop = desktop(HeldImages::new(), false);
    let context = &desktop.harness.context;
    let kind = || dto::AvatarPromptKind::Generation {
        subject_name: "Ada".to_owned(),
        subject_description: "A meticulous engineer".to_owned(),
        avatar_request: "  in a workshop  ".to_owned(),
    };
    let local = avatar_prompt(
        context,
        dto::AvatarPromptRequest {
            model_id: local_model(context).to_string(),
            kind: kind(),
        },
    )
    .await
    .expect("local");
    assert_eq!(
        local.prompt, "in a workshop",
        "a local model gets the request as typed"
    );
    let remote = avatar_prompt(
        context,
        dto::AvatarPromptRequest {
            model_id: remote_model(context).to_string(),
            kind: kind(),
        },
    )
    .await
    .expect("remote");
    assert!(remote.prompt.contains("Ada") && remote.prompt.contains("in a workshop"));
    let missing = avatar_prompt(
        context,
        dto::AvatarPromptRequest {
            model_id: ModelProfileId::new().to_string(),
            kind: kind(),
        },
    )
    .await
    .expect_err("unknown model");
    assert_eq!(missing.code, ApiErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_gradient_of_a_missing_or_undecodable_avatar_fails_typed() {
    let desktop = desktop(HeldImages::new(), false);
    let context = &desktop.harness.context;
    let missing = avatar_gradient(
        context,
        dto::AvatarGradientRequest {
            asset_id: AssetId::new().to_string(),
            force: false,
        },
    )
    .await
    .expect_err("no such asset");
    assert_eq!(missing.code, ApiErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_design_reference_needs_an_image_and_a_repeated_request_returns_its_job() {
    let desktop = desktop(HeldImages::new(), false);
    let context = &desktop.harness.context;
    let request = |avatar: Option<String>| dto::ImageDesignReferenceRequest {
        subject_name: Some("Ada".to_owned()),
        subject_description: None,
        current_description: None,
        avatar,
        references: Vec::new(),
        client_operation_id: "design-1".to_owned(),
    };
    let none = image_design_reference(context, request(None))
        .await
        .expect_err("no image");
    assert_eq!(none.code, ApiErrorCode::InvalidInput);
    let avatar = AssetId::new().to_string();
    let first = image_design_reference(context, request(Some(avatar.clone())))
        .await
        .expect("accepted");
    let again = image_design_reference(context, request(Some(avatar)))
        .await
        .expect("replayed");
    assert_eq!(first, again);
    let other = image_design_reference(context, request(Some(AssetId::new().to_string())))
        .await
        .expect_err("another request under the key");
    assert_eq!(other.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn engine_settings_fail_typed_when_nothing_is_installed() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let switched = sd_runtime_switch(
        context,
        dto::SdRuntimeRef {
            release: RELEASE.to_owned(),
            asset: CPU_BUILD.to_owned(),
        },
    )
    .await
    .expect_err("no such build");
    assert_eq!(
        switched.details,
        Some(ApiErrorDetails::Image {
            failure: dto::ImageFailureKind::RuntimeNotInstalled
        })
    );
    let inventory = sd_runtime_inventory(context).await.expect("inventory");
    assert!(inventory.installed.is_empty() && inventory.active.is_none());
    assert!(!sd_disk_usage(context).await.expect("usage").has_engine);
    let repaired = sd_model_repair(
        context,
        dto::SdModelRepairRequest {
            profile_id: "z-image-turbo".to_owned(),
            variant_id: "q4-k".to_owned(),
        },
    )
    .await
    .expect_err("no engine build");
    assert_eq!(repaired.code, ApiErrorCode::Unavailable);
    let unknown = sd_model_uninstall(
        context,
        dto::SdModelUninstallRequest {
            profile_id: "nope".to_owned(),
            variant_id: "nope".to_owned(),
            also_remove_engine_if_unused: false,
        },
    )
    .await
    .expect_err("unknown variant");
    assert_eq!(unknown.code, ApiErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bundle_estimate_runs_as_a_job_and_ends_with_its_verdict() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let accepted = sd_bundle_runnability(
        context,
        dto::SdBundleRunnabilityRequest {
            profile_id: "z-image-turbo".to_owned(),
            runtime_release: RELEASE.to_owned(),
            runtime_asset: CPU_BUILD.to_owned(),
            diffusion_bytes: 4,
            text_encoder_bytes: 2,
            vae_bytes: 1,
            vision_encoder_bytes: 0,
            client_operation_id: "estimate-1".to_owned(),
        },
    )
    .await
    .expect("accepted");
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    runner.wait_idle().await;
    let done = view(context, job_id(&accepted)).await;
    assert_eq!(done.state, dto::JobStateDto::Succeeded);
    let Some(dto::JobResultDto::Runnability { verdict }) = done.result else {
        panic!("expected a verdict: {done:?}");
    };
    assert_eq!(verdict.status, dto::RunnabilityStatus::NotInstalled);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_runnability_probe_stops_the_engine_it_started() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let engine = context.backend().local_diffusion().expect("engine").clone();
    let paths = engine.paths();
    let pid_file = desktop.root.join("probe.pid");
    let build = paths.runtime_root(RELEASE, CPU_BUILD);
    std::fs::create_dir_all(&build).expect("build");
    script(
        &build.join(server_executable_name()),
        &format!("echo $$ > '{}'\nexec sleep 60", pid_file.display()),
    );
    let catalog = lettuce_image_generation::diffusion_catalog();
    let (profile, variant) = catalog
        .find_variant("z-image-turbo", "q4-k")
        .expect("variant");
    for component in profile.components(variant) {
        let path = paths.component_path(component);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::File::create(&path)
            .and_then(|file| file.set_len(component.bytes))
            .expect("sparse component");
    }
    let accepted = sd_runnability(
        context,
        dto::SdRunnabilityRequest {
            profile_id: "z-image-turbo".to_owned(),
            variant_id: "q4-k".to_owned(),
            runtime_release: RELEASE.to_owned(),
            runtime_asset: CPU_BUILD.to_owned(),
            client_operation_id: "probe-1".to_owned(),
            ..dto::SdRunnabilityRequest::default()
        },
    )
    .await
    .expect("accepted");
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    until(|| std::fs::read_to_string(&pid_file).is_ok_and(|text| !text.trim().is_empty())).await;
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: accepted.job_id.clone(),
        },
    )
    .await
    .expect("cancel");
    tokio::time::timeout(Duration::from_secs(10), runner.wait_idle())
        .await
        .expect("the probe ends promptly");
    assert_eq!(
        view(context, job_id(&accepted)).await.state,
        dto::JobStateDto::Cancelled
    );
    let pid = std::fs::read_to_string(&pid_file)
        .expect("pid")
        .trim()
        .to_owned();
    until(|| {
        !std::process::Command::new("kill")
            .args(["-0", &pid])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lora_discovery_job_reads_the_files_metadata() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let loras = context
        .backend()
        .local_diffusion()
        .expect("engine")
        .paths()
        .loras
        .clone();
    std::fs::create_dir_all(&loras).expect("library");
    let header = serde_json::to_vec(&serde_json::json!({
        "__metadata__": {
            "ss_activation_tags": "ArsMovieStill",
            "ss_base_model_version": "z_image_turbo",
        }
    }))
    .expect("header");
    let padded = header.len().div_ceil(8) * 8;
    let mut file = (padded as u64).to_le_bytes().to_vec();
    file.extend_from_slice(&header);
    file.resize(8 + padded, b' ');
    file.extend_from_slice(&[0; 16]);
    std::fs::write(loras.join("style.safetensors"), &file).expect("lora");
    let accepted = lora_keywords_discover(
        context,
        dto::LoraKeywordsDiscoverRequest {
            path: "style.safetensors".to_owned(),
            profile_id: Some("z-image-turbo".to_owned()),
            client_operation_id: "discover-1".to_owned(),
        },
    )
    .await
    .expect("accepted");
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    runner.wait_idle().await;
    let done = view(context, job_id(&accepted)).await;
    let Some(dto::JobResultDto::LoraDiscovered { discovered }) = done.result else {
        panic!("expected the discovery: {done:?}");
    };
    assert_eq!(discovered.discovery.keywords, ["ArsMovieStill"]);
    assert_eq!(
        discovered.discovery.compatibility,
        dto::LoraCompatibility::Compatible
    );
    let listed = loras_list(
        context,
        dto::LorasListRequest {
            profile_id: Some("z-image-turbo".to_owned()),
        },
    )
    .await
    .expect("list");
    assert_eq!(listed.loras[0].keywords, ["ArsMovieStill"]);
    let deleted = loras_delete(
        context,
        dto::LorasDeleteRequest {
            path: "style.safetensors".to_owned(),
        },
    )
    .await
    .expect("delete");
    assert!(deleted.left_behind.is_empty());
}

type Route = Arc<dyn Fn(&str) -> (u16, String) + Send + Sync>;

/// A local HTTP server answering each request with `route(request line)`;
/// returns its address and the request lines it saw.
async fn serve(route: Route) -> (String, Arc<Mutex<Vec<String>>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&seen);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let (route, captured) = (Arc::clone(&route), Arc::clone(&captured));
            tokio::spawn(async move {
                let mut bytes = Vec::new();
                let mut buffer = [0_u8; 4096];
                while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => bytes.extend_from_slice(&buffer[..read]),
                    }
                }
                let head = String::from_utf8_lossy(&bytes).into_owned();
                let line = head.lines().next().unwrap_or_default().to_owned();
                captured.lock().expect("seen").push(line.clone());
                let (status, body) = route(&line);
                let response = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    (format!("http://{address}"), seen)
}

fn set_pure_mode(context: &ApiContext, mode: lettuce_settings::PureMode) {
    use lettuce_settings::GlobalSettingsStore;
    let database = context.backend().database();
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.pure_mode = mode;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("saved settings");
}

#[tokio::test(flavor = "multi_thread")]
async fn civitai_follows_the_pure_mode_level_and_reports_a_missing_model_as_not_found() {
    let desktop = desktop(HeldImages::new(), false);
    let context = &desktop.harness.context;
    let (endpoint, seen) = serve(Arc::new(|line: &str| {
        if line.starts_with("GET /api/v1/models?") {
            (
                200,
                r#"{"items": [{"id": 7, "name": "Style", "type": "LORA", "modelVersions": [{"id": 70, "baseModel": "ZImageTurbo"}]}], "metadata": {}}"#.to_owned(),
            )
        } else {
            (404, "{}".to_owned())
        }
    }))
    .await;
    context.image_state().use_civitai_endpoint(endpoint);
    let search = || dto::CivitaiSearchRequest::default();
    let standard = civitai_search(context, search()).await.expect("standard");
    assert_eq!(standard.items.len(), 1);
    set_pure_mode(context, lettuce_settings::PureMode::Off);
    civitai_search(context, search()).await.expect("off");
    let requests = seen.lock().expect("seen").clone();
    assert!(requests[0].contains("nsfw=false"), "{}", requests[0]);
    assert!(requests[1].contains("nsfw=true"), "{}", requests[1]);
    let missing = civitai_model(context, dto::CivitaiModelRequest { model_id: 9 })
        .await
        .expect_err("no such model");
    assert_eq!(missing.code, ApiErrorCode::NotFound);
    let status = civitai_auth_status(context).await.expect("status");
    assert!(!status.saved);
    assert_eq!(
        status.error_kind,
        Some(dto::CivitaiAuthErrorKind::MissingToken)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_models_folder_does_not_move_while_local_image_work_runs() {
    let images = HeldImages::new();
    let desktop = desktop(images.clone(), true);
    let context = &desktop.harness.context;
    let model = local_model(context);
    let job = job_id(
        &image_generate(context, generate_request(model, "first"))
            .await
            .expect("accepted"),
    );
    let target = desktop.root.with_file_name(format!(
        "{}-elsewhere",
        desktop.root.file_name().expect("name").to_string_lossy()
    ));
    let request = |operation: &str| dto::LocalModelsDirSetRequest {
        path: target.display().to_string(),
        move_existing: true,
        client_operation_id: operation.to_owned(),
    };
    let queued = local_models_dir_set(context, request("move-queued"))
        .await
        .expect_err("a queued local job counts");
    assert_eq!(
        queued.details,
        Some(ApiErrorDetails::LocalModelsBusy {
            reason: dto::LocalModelsBusyReason::ImageWorkActive {
                job_id: Some(job.to_string())
            }
        })
    );
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    images.entered.notified().await;
    let running = local_models_dir_set(context, request("move-running"))
        .await
        .expect_err("a running local job counts");
    assert_eq!(running.code, ApiErrorCode::Busy);
    images.proceed();
    runner.wait_idle().await;
    let accepted = local_models_dir_set(context, request("move-after"))
        .await
        .expect("allowed once the work settled");
    assert!(runner.run_once().await.expect("move starts"));
    runner.wait_idle().await;
    assert_eq!(
        view(context, job_id(&accepted)).await.state,
        dto::JobStateDto::Succeeded
    );
}

const FAKE_SERVER: &str = r#"
import sys, json
from http.server import BaseHTTPRequestHandler, HTTPServer
port = int(sys.argv[sys.argv.index('--listen-port') + 1])
class H(BaseHTTPRequestHandler):
    def reply(self, body):
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header('content-length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def do_GET(self):
        if self.path.endswith('/p'):
            self.reply({"status": "completed", "result": {"images": [{"b64_json": "AAAA"}]}})
        else:
            self.reply({})
    def do_POST(self):
        self.rfile.read(int(self.headers.get('content-length', 0)))
        self.reply({"id": "j", "poll_url": "/p"})
    def log_message(self, *args):
        pass
HTTPServer(('127.0.0.1', port), H).serve_forever()
"#;

fn engine_request(root: &Path) -> lettuce_image_generation::ProviderImageRequest {
    use lettuce_models::{ProviderAccount, ProviderConfig, StableDiffusionSettings};
    let file = |name: &str| {
        let path = root.join(name);
        std::fs::write(&path, b"weights").expect("component");
        path.display().to_string()
    };
    let binding = lettuce_models::StableDiffusionCppBinding {
        text_encoder_path: Some(file("encoder.gguf")),
        vae_path: Some(file("vae.safetensors")),
        runtime_release: Some(RELEASE.to_owned()),
        runtime_asset: Some(CPU_BUILD.to_owned()),
        ..Default::default()
    };
    lettuce_image_generation::ProviderImageRequest {
        job_id: JobId::new(),
        model_profile_id: ModelProfileId::new(),
        account: ProviderAccount {
            id: lettuce_types::ProviderAccountId::new(),
            secret_owner_id: lettuce_settings::SecretOwnerId::new(),
            provider_kind: "sdcpp".to_owned(),
            protocol: ProviderProtocol::StableDiffusion,
            label: "stable-diffusion.cpp".to_owned(),
            endpoint: None,
            enabled: true,
            streaming_enabled: false,
            allow_invalid_tls: false,
            api_key_ref: None,
            secret_headers: Vec::new(),
            config: ProviderConfig::Standard,
            revision: lettuce_types::Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        },
        external_model_id: file("diffusion.gguf"),
        model_display_name: "Local model".to_owned(),
        prompt: "a lighthouse".to_owned(),
        settings: StableDiffusionSettings {
            cpp: binding,
            ..StableDiffusionSettings::default()
        },
        loras: Vec::new(),
        input_images: Vec::new(),
        mask_image: None,
        size: Some("512x512".to_owned()),
        quality: None,
        style: None,
        count: 1,
        text_output: false,
        cancellation: lettuce_jobs::handle::CancellationToken::new(),
        progress: None,
    }
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn an_idle_cached_server_does_not_block_the_move_and_is_stopped_before_it() {
    use lettuce_image_generation::ImageProviderPort;
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let engine = context.backend().local_diffusion().expect("engine").clone();
    let build = engine.paths().runtime_root(RELEASE, CPU_BUILD);
    std::fs::create_dir_all(&build).expect("build");
    let fake = desktop.root.join("fake_server.py");
    std::fs::write(&fake, FAKE_SERVER).expect("fake server");
    let pid_file = desktop.root.join("server.pid");
    script(
        &build.join(server_executable_name()),
        &format!(
            "echo $$ > '{}'\nexec python3 '{}' \"$@\"",
            pid_file.display(),
            fake.display()
        ),
    );
    script(&build.join(cli_executable_name()), "exit 1");
    engine
        .generate(engine_request(&desktop.root))
        .await
        .expect("the first generation");
    assert!(engine.server_running().await, "the server stays cached");
    assert!(!engine.call_active());
    let pid = std::fs::read_to_string(&pid_file)
        .expect("pid")
        .trim()
        .to_owned();

    let target = desktop.root.with_file_name(format!(
        "{}-elsewhere",
        desktop.root.file_name().expect("name").to_string_lossy()
    ));
    let accepted = local_models_dir_set(
        context,
        dto::LocalModelsDirSetRequest {
            path: target.display().to_string(),
            move_existing: true,
            client_operation_id: "move-idle".to_owned(),
        },
    )
    .await
    .expect("an idle server does not block the move");
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("move starts"));
    runner.wait_idle().await;
    assert_eq!(
        view(context, job_id(&accepted)).await.state,
        dto::JobStateDto::Succeeded
    );
    assert!(!engine.server_running().await, "stopped before the move");
    until(|| {
        !std::process::Command::new("kill")
            .args(["-0", &pid])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    })
    .await;
    assert_eq!(engine.paths().image_root, target.join("image"));
    engine
        .generate(engine_request(&desktop.root))
        .await
        .expect("a generation after the move");
    assert!(engine.server_running().await, "a new server starts");
    engine.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_models_folder_does_not_move_while_a_lora_discovery_is_queued() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let discovery = job_id(
        &lora_keywords_discover(
            context,
            dto::LoraKeywordsDiscoverRequest {
                path: "style.safetensors".to_owned(),
                profile_id: None,
                client_operation_id: "discover-move".to_owned(),
            },
        )
        .await
        .expect("accepted"),
    );
    let refused = local_models_dir_set(
        context,
        dto::LocalModelsDirSetRequest {
            path: desktop
                .root
                .with_file_name(format!(
                    "{}-elsewhere",
                    desktop.root.file_name().expect("name").to_string_lossy()
                ))
                .display()
                .to_string(),
            move_existing: true,
            client_operation_id: "move-discovery".to_owned(),
        },
    )
    .await
    .expect_err("a queued discovery counts");
    assert_eq!(
        refused.details,
        Some(ApiErrorDetails::LocalModelsBusy {
            reason: dto::LocalModelsBusyReason::ImageWorkActive {
                job_id: Some(discovery.to_string())
            }
        })
    );
}

async fn queue_move(context: &ApiContext, root: &Path, operation: &str) -> dto::JobAccepted {
    local_models_dir_set(
        context,
        dto::LocalModelsDirSetRequest {
            path: root
                .with_file_name(format!(
                    "{}-elsewhere",
                    root.file_name().expect("name").to_string_lossy()
                ))
                .display()
                .to_string(),
            move_existing: false,
            client_operation_id: operation.to_owned(),
        },
    )
    .await
    .expect("move queued")
}

fn is_folder_move(error: &dto::ApiError, accepted: &dto::JobAccepted) -> bool {
    error.code == ApiErrorCode::Busy
        && error.details
            == Some(ApiErrorDetails::LocalModelsBusy {
                reason: dto::LocalModelsBusyReason::FolderMoveActive {
                    job_id: accepted.job_id.clone(),
                },
            })
}

#[tokio::test(flavor = "multi_thread")]
async fn local_image_work_is_refused_while_the_models_folder_moves_but_remote_work_is_not() {
    let images = HeldImages::new();
    let desktop = desktop(images.clone(), true);
    let context = &desktop.harness.context;
    let local = local_model(context);
    let remote = remote_model(context);
    let moving = queue_move(context, &desktop.root, "move-admission").await;

    let generation = image_generate(context, generate_request(local, "first"))
        .await
        .expect_err("a local generation");
    assert!(is_folder_move(&generation, &moving), "{generation:?}");
    let upscale = image_upscale(
        context,
        dto::ImageUpscaleRequest {
            asset_id: AssetId::new().to_string(),
            origin: None,
            client_operation_id: "upscale-move".to_owned(),
        },
    )
    .await
    .expect_err("an upscale");
    assert!(is_folder_move(&upscale, &moving), "{upscale:?}");
    let probe = sd_bundle_runnability(
        context,
        dto::SdBundleRunnabilityRequest {
            profile_id: "z-image-turbo".to_owned(),
            runtime_release: RELEASE.to_owned(),
            runtime_asset: CPU_BUILD.to_owned(),
            diffusion_bytes: 4,
            text_encoder_bytes: 2,
            vae_bytes: 1,
            vision_encoder_bytes: 0,
            client_operation_id: "estimate-move".to_owned(),
        },
    )
    .await
    .expect_err("a probe");
    assert!(is_folder_move(&probe, &moving), "{probe:?}");
    let discovery = lora_keywords_discover(
        context,
        dto::LoraKeywordsDiscoverRequest {
            path: "style.safetensors".to_owned(),
            profile_id: None,
            client_operation_id: "discover-admission".to_owned(),
        },
    )
    .await
    .expect_err("a discovery");
    assert!(is_folder_move(&discovery, &moving), "{discovery:?}");
    let import = loras_import(
        context,
        dto::LorasImportRequest {
            source: dto::FileSource {
                uri: desktop.root.join("style.safetensors").display().to_string(),
            },
        },
    )
    .await
    .expect_err("a LoRA import");
    assert!(is_folder_move(&import, &moving), "{import:?}");
    let delete = loras_delete(
        context,
        dto::LorasDeleteRequest {
            path: "style.safetensors".to_owned(),
        },
    )
    .await
    .expect_err("a LoRA delete");
    assert!(is_folder_move(&delete, &moving), "{delete:?}");

    image_generate(context, generate_request(remote, "first"))
        .await
        .expect("a remote generation is not blocked");
    assert_eq!(images.calls(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn image_installs_are_refused_while_the_models_folder_moves_and_write_nothing() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let moving = queue_move(context, &desktop.root, "move-installs").await;
    let a = "a".repeat(64);

    let bundle = hf_image_bundle_install(context, bundle_request(&[("model.gguf", &a)]))
        .await
        .expect_err("a bundle install");
    assert!(is_folder_move(&bundle, &moving), "{bundle:?}");
    let retry = hf_image_bundle_retry(
        context,
        dto::HfImageBundleRetryRequest {
            bundle_id: "any".to_owned(),
        },
    )
    .await
    .expect_err("a bundle retry");
    assert!(is_folder_move(&retry, &moving), "{retry:?}");
    let model = sd_model_install(
        context,
        dto::SdModelInstallRequest {
            profile_id: "z-image-turbo".to_owned(),
            variant_id: "any".to_owned(),
            release: RELEASE.to_owned(),
            asset: CPU_BUILD.to_owned(),
        },
    )
    .await
    .expect_err("a model install");
    assert!(is_folder_move(&model, &moving), "{model:?}");
    let runtime = sd_runtime_install(
        context,
        dto::SdRuntimeInstallRequest {
            release: RELEASE.to_owned(),
            asset: CPU_BUILD.to_owned(),
            then_register: None,
        },
    )
    .await
    .expect_err("a runtime install");
    assert!(is_folder_move(&runtime, &moving), "{runtime:?}");
    let upscalers = sd_upscalers_install(context)
        .await
        .expect_err("an upscaler install");
    assert!(is_folder_move(&upscalers, &moving), "{upscalers:?}");
    assert!(nothing_written(context));
    assert!(context.jobs().installs().is_empty());
}

fn image_only_runner(context: &ApiContext) -> JobRunner {
    JobRunner::new(
        context.clone(),
        JobHandlers::new(vec![
            Arc::new(ImageGenerateHandler),
            Arc::new(ImageToolHandler),
        ]),
    )
}

async fn deferred_jobs(desktop: &Desktop, images: &Arc<HeldImages>) -> (JobId, JobId, JobId) {
    let _ = images;
    let context = &desktop.harness.context;
    let mut request = generate_request(local_model(context), "first");
    request.source = dto::ImageRequestSource::Playground;
    let generation = job_id(&image_generate(context, request).await.expect("accepted"));
    let discovery = job_id(
        &lora_keywords_discover(
            context,
            dto::LoraKeywordsDiscoverRequest {
                path: "style.safetensors".to_owned(),
                profile_id: None,
                client_operation_id: "discover-race".to_owned(),
            },
        )
        .await
        .expect("accepted"),
    );
    let moving = job_id(&queue_move(context, &desktop.root, "move-race").await);
    (generation, discovery, moving)
}

#[tokio::test(flavor = "multi_thread")]
async fn local_jobs_found_behind_a_move_stay_queued_and_run_after_it_without_polling() {
    let images = HeldImages::new();
    let desktop = desktop(images.clone(), true);
    let context = &desktop.harness.context;
    let (generation, discovery, _moving) = deferred_jobs(&desktop, &images).await;
    let before = view(context, generation).await;

    let runner = image_only_runner(context);
    assert!(!runner.run_once().await.expect("claim attempt"));
    runner.wait_idle().await;
    assert_eq!(state(context, generation), JobState::Queued);
    assert_eq!(state(context, discovery), JobState::Queued);
    assert_eq!(view(context, generation).await, before);
    assert_eq!(images.calls(), 0);
    let record =
        ImageGenerationRepository::get(context.backend().database(), generation).expect("record");
    assert!(!matches!(
        record.state,
        ImageGenerationState::Cancelled { .. }
    ));

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let driver = {
        let runner = JobRunner::new(context.clone(), JobHandlers::standard());
        tokio::spawn(async move {
            runner
                .run(async {
                    let _ = stopped.await;
                })
                .await;
        })
    };
    images.entered.notified().await;
    images.proceed();
    until(|| state(context, generation) == JobState::Succeeded).await;
    until(|| state(context, discovery) != JobState::Queued).await;
    let _ = stop.send(());
    driver.await.expect("runner");
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_deferred_job_settles_it_cancelled_by_the_user() {
    let images = HeldImages::new();
    let desktop = desktop(images.clone(), true);
    let context = &desktop.harness.context;
    let (generation, _discovery, _moving) = deferred_jobs(&desktop, &images).await;
    let runner = image_only_runner(context);
    assert!(!runner.run_once().await.expect("claim attempt"));
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: generation.to_string(),
        },
    )
    .await
    .expect("cancel");
    let cancelled = JobStore::get(context.backend().database(), generation)
        .expect("job")
        .expect("exists");
    assert_eq!(cancelled.state, JobState::Cancelled);
    assert_eq!(
        cancelled.cancellation.reason,
        Some(lettuce_jobs::CancellationReason::User)
    );
    assert_eq!(images.calls(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_move_releases_the_deferred_jobs() {
    let images = HeldImages::new();
    let desktop = desktop(images.clone(), true);
    let context = &desktop.harness.context;
    let (generation, _discovery, moving) = deferred_jobs(&desktop, &images).await;
    let runner = image_only_runner(context);
    assert!(!runner.run_once().await.expect("claim attempt"));
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: moving.to_string(),
        },
    )
    .await
    .expect("cancel the move");
    assert!(runner.run_once().await.expect("released"));
    images.entered.notified().await;
    images.proceed();
    runner.wait_idle().await;
    assert_eq!(state(context, generation), JobState::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restart_during_a_move_cancels_the_deferred_jobs() {
    let images = HeldImages::new();
    let desktop = desktop(images.clone(), true);
    let context = &desktop.harness.context;
    let (generation, discovery, _moving) = deferred_jobs(&desktop, &images).await;
    let runner = image_only_runner(context);
    assert!(!runner.run_once().await.expect("claim attempt"));
    let restarted = context.restarted();
    restarted.recover_after_restart().expect("recovery");
    assert_eq!(state(&restarted, generation), JobState::Cancelled);
    assert_eq!(state(&restarted, discovery), JobState::Cancelled);
    assert_eq!(images.calls(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn duplicate_assets_in_one_bundle_request_are_rejected() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let (a, b) = ("a".repeat(64), "b".repeat(64));
    running_bundle_install(context, &[("model.gguf", &a), ("other.gguf", &b)]);
    let error = hf_image_bundle_install(
        context,
        bundle_request(&[("model.gguf", &a), ("model.gguf", &a)]),
    )
    .await
    .expect_err("[A, A] is not [A, B]");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert!(nothing_written(context));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_same_paths_under_another_hash_are_not_the_running_bundle() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let (a, b, c) = ("a".repeat(64), "b".repeat(64), "c".repeat(64));
    running_bundle_install(context, &[("model.gguf", &a), ("other.gguf", &b)]);
    let error = hf_image_bundle_install(
        context,
        bundle_request(&[("model.gguf", &a), ("other.gguf", &c)]),
    )
    .await
    .expect_err("another hash is another file");
    assert_eq!(error.code, ApiErrorCode::Busy);
    assert!(nothing_written(context));
}

fn unverified_bundle(context: &ApiContext, bundle_id: &str, files: &[(&str, &str)]) {
    let paths = context.backend().local_diffusion().expect("engine").paths();
    let manifest = lettuce_image_generation::BundleManifest {
        bundle_id: bundle_id.to_owned(),
        profile_id: "z-image-turbo".to_owned(),
        display_name: "Bundle".to_owned(),
        runtime_release: RELEASE.to_owned(),
        runtime_asset: CPU_BUILD.to_owned(),
        runnability: None,
        assets: files
            .iter()
            .map(|(path, sha)| lettuce_image_generation::ManifestAsset {
                asset: bundle_asset(path, sha),
                local_path: String::new(),
                verified: false,
            })
            .collect(),
        registration_state: lettuce_image_generation::BundleRegistrationState::SetupFailed,
        model_id: None,
        setup_error: Some("interrupted".to_owned()),
    };
    lettuce_image_generation::write_bundle_manifest(&paths.image_root, &manifest)
        .expect("manifest");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bundle_retry_follows_the_running_install_rule() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let (a, b) = ("a".repeat(64), "b".repeat(64));
    unverified_bundle(context, "retry-me", &[("model.gguf", &a)]);
    let retry = || {
        hf_image_bundle_retry(
            context,
            dto::HfImageBundleRetryRequest {
                bundle_id: "retry-me".to_owned(),
            },
        )
    };
    running_bundle_install(context, &[("model.gguf", &a), ("other.gguf", &b)]);
    let overlapping = retry().await.expect_err("an overlapping install runs");
    assert_eq!(overlapping.code, ApiErrorCode::Busy);

    let desktop = self::desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    unverified_bundle(context, "retry-me", &[("model.gguf", &a)]);
    let existing = running_bundle_install(context, &[("model.gguf", &a)]);
    let joined = hf_image_bundle_retry(
        context,
        dto::HfImageBundleRetryRequest {
            bundle_id: "retry-me".to_owned(),
        },
    )
    .await
    .expect("the identical install is joined");
    assert_eq!(joined.job_id, Some(existing.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bundle_retry_waits_for_the_admission_in_progress() {
    let desktop = desktop(HeldImages::new(), true);
    let context = desktop.harness.context.clone();
    let a = "a".repeat(64);
    unverified_bundle(&context, "retry-me", &[("model.gguf", &a)]);
    let admitting = context.image_state().bundle_admission().lock().await;
    let retry = {
        let context = context.clone();
        tokio::spawn(async move {
            hf_image_bundle_retry(
                &context,
                dto::HfImageBundleRetryRequest {
                    bundle_id: "retry-me".to_owned(),
                },
            )
            .await
        })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!retry.is_finished());
    let existing = running_bundle_install(&context, &[("model.gguf", &a)]);
    drop(admitting);
    let joined = retry.await.expect("task").expect("joins the install");
    assert_eq!(joined.job_id, Some(existing.to_string()));
}
