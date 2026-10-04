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
    let target = desktop.root.join("elsewhere");
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

#[tokio::test(flavor = "multi_thread")]
async fn a_bundle_that_a_running_install_already_fetches_joins_its_job() {
    let desktop = desktop(HeldImages::new(), true);
    let context = &desktop.harness.context;
    let engine = context.backend().local_diffusion().expect("engine").clone();
    let paths = engine.paths();
    let asset = |path: &str, sha: &str| lettuce_image_generation::BundleAsset {
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
    };
    let running = asset("model.gguf", &"a".repeat(64));
    let root = lettuce_image_generation::bundle_root(&paths.image_root);
    let source = crate::ArtifactSource::HuggingFace {
        repository: running.model_id.clone(),
        revision: running.revision.clone(),
        path: running.relative_path.clone(),
    };
    let existing = JobId::new();
    context.jobs().put_install(
        existing,
        crate::api::InstallWork::Artifact {
            plan: crate::ArtifactInstallPlan {
                install_id: "hf-bundle:running".to_owned(),
                root,
                artifacts: vec![crate::PlannedArtifact {
                    artifact: lettuce_model_hub::PinnedArtifact {
                        source_identity: source.identity(),
                        local_segments: running.local_segments().expect("segments"),
                        byte_size: 4,
                        sha256: Some("a".repeat(64)),
                    },
                    source,
                }],
            },
            finish: Box::new(crate::api::InstallFinish::HuggingFaceBundle {
                paths: (*paths).clone(),
                bundle_id: "running".to_owned(),
            }),
        },
    );
    let dto_asset = |asset: &lettuce_image_generation::BundleAsset| dto::ImageBundleAsset {
        selection_id: asset.selection_id.clone(),
        profile_id: asset.profile_id.clone(),
        role: dto::ImageComponentRole::DiffusionModel,
        model_id: asset.model_id.clone(),
        revision: asset.revision.clone(),
        relative_path: asset.relative_path.clone(),
        format: asset.format.clone(),
        quantization: None,
        size: asset.size,
        sha256: asset.sha256.clone(),
        architecture: None,
        gated: false,
    };
    let install = |assets: Vec<dto::ImageBundleAsset>| dto::HfImageBundleInstallRequest {
        profile_id: "z-image-turbo".to_owned(),
        display_name: "Bundle".to_owned(),
        runtime_release: RELEASE.to_owned(),
        runtime_asset: CPU_BUILD.to_owned(),
        assets,
    };
    let joined = hf_image_bundle_install(context, install(vec![dto_asset(&running)]))
        .await
        .expect("joins the running install");
    assert_eq!(joined.job_id, existing.to_string());
    assert_eq!(joined.bundle_id, "running");

    let other = asset("other.gguf", &"b".repeat(64));
    let partial = hf_image_bundle_install(
        context,
        install(vec![dto_asset(&running), dto_asset(&other)]),
    )
    .await
    .expect_err("only some of the files are being fetched");
    assert_eq!(partial.code, ApiErrorCode::Busy);
    assert!(
        !lettuce_image_generation::bundle_root(&paths.image_root).exists(),
        "nothing was written for the refused bundle"
    );
}
