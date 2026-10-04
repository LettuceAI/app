use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use lettuce_jobs::handle::CancellationToken;
use lettuce_models::{
    ProviderAccount, ProviderConfig, ProviderProtocol, StableDiffusionCppBinding,
    StableDiffusionSettings,
};
use lettuce_network::BulkHttpClient;
use lettuce_settings::SecretOwnerId;
use lettuce_types::{
    JobId, ModelProfileId, OperationId, ProviderAccountId, Revision, TimestampMillis,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::layout::{DiffusionPaths, cli_executable_name, server_executable_name};
use super::output::{GenerationProgress, GenerationProgressSink};
use super::policy::HardwareGpu;
use super::server::{EngineHost, EngineModel, LocalDiffusionEngine};
use crate::{ImageFailureKind, ProgressHandle, ProviderImageRequest};

const CPU_BUILD: &str = "sd-master-bin-Linux-Ubuntu-24.04-x86_64.zip";
const RELEASE: &str = "master-778-a";

struct Host;

#[async_trait]
impl EngineHost for Host {
    async fn gpu_devices(&self) -> Result<Vec<HardwareGpu>, String> {
        Ok(Vec::new())
    }

    fn available_memory_bytes(&self) -> Option<u64> {
        None
    }

    async fn unload_local_llm(&self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Default)]
struct Events(Mutex<Vec<GenerationProgress>>);

impl GenerationProgressSink for Events {
    fn progress(&self, progress: GenerationProgress) {
        self.0.lock().expect("events").push(progress);
    }
}

struct Fixture {
    root: PathBuf,
    engine: Arc<LocalDiffusionEngine>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!("sd-local-work-{}", OperationId::new()));
    std::fs::create_dir_all(&root).expect("root");
    let paths = DiffusionPaths::legacy_layout(&root, root.join("models").join("image"));
    let engine = Arc::new(LocalDiffusionEngine::new(
        paths,
        BulkHttpClient::new().expect("client"),
        Arc::new(Host),
    ));
    Fixture { root, engine }
}

#[cfg(unix)]
fn script(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("script");
    let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("mode");
}

#[cfg(unix)]
fn install_build(fixture: &Fixture, server: &str, cli: &str) {
    let build = fixture.engine.paths().runtime_root(RELEASE, CPU_BUILD);
    std::fs::create_dir_all(&build).expect("build");
    script(&build.join(server_executable_name()), server);
    script(&build.join(cli_executable_name()), cli);
}

async fn until(condition: impl Fn() -> bool) {
    for _ in 0..200 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the condition was never met");
}

#[cfg(unix)]
fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn model(root: &Path) -> EngineModel {
    let file = |name: &str| {
        let path = root.join(name);
        std::fs::write(&path, b"weights").expect("component");
        path.display().to_string()
    };
    EngineModel {
        display_name: "Local model".to_owned(),
        diffusion_model_path: file("diffusion.gguf"),
        binding: StableDiffusionCppBinding {
            text_encoder_path: Some(file("encoder.gguf")),
            vae_path: Some(file("vae.safetensors")),
            runtime_release: Some(RELEASE.to_owned()),
            runtime_asset: Some(CPU_BUILD.to_owned()),
            ..StableDiffusionCppBinding::default()
        },
    }
}

fn request(
    model: &EngineModel,
    cancellation: CancellationToken,
    progress: Option<Arc<Events>>,
) -> ProviderImageRequest {
    ProviderImageRequest {
        job_id: JobId::new(),
        model_profile_id: ModelProfileId::new(),
        account: ProviderAccount {
            id: ProviderAccountId::new(),
            secret_owner_id: SecretOwnerId::new(),
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
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        },
        external_model_id: model.diffusion_model_path.clone(),
        model_display_name: model.display_name.clone(),
        prompt: "a lighthouse".to_owned(),
        settings: StableDiffusionSettings {
            cpp: model.binding.clone(),
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
        cancellation,
        progress: progress.map(|events| ProgressHandle(events as Arc<dyn GenerationProgressSink>)),
    }
}

#[tokio::test]
async fn local_calls_take_turns_and_each_hears_only_its_own_progress() {
    let fixture = fixture();
    let engine = Arc::clone(&fixture.engine);
    let (first, second) = (Arc::new(Events::default()), Arc::new(Events::default()));
    let (first_token, second_token) = (CancellationToken::new(), CancellationToken::new());
    let held = engine
        .begin_call(
            &first_token,
            Some(first.clone() as Arc<dyn GenerationProgressSink>),
        )
        .await
        .expect("the first call starts");
    engine.report(GenerationProgress::Starting);

    let waiting = engine.begin_call(
        &second_token,
        Some(second.clone() as Arc<dyn GenerationProgressSink>),
    );
    tokio::pin!(waiting);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut waiting)
            .await
            .is_err(),
        "the second call waits for the first"
    );
    engine.report(GenerationProgress::Generating);
    drop(held);
    let _second_call = waiting
        .await
        .expect("the second call starts after the first");
    engine.report(GenerationProgress::Retrying);

    assert_eq!(
        *first.0.lock().expect("first"),
        [GenerationProgress::Starting, GenerationProgress::Generating]
    );
    assert_eq!(
        *second.0.lock().expect("second"),
        [GenerationProgress::Retrying]
    );
    assert!(!first_token.is_cancelled() && !second_token.is_cancelled());
}

#[tokio::test]
async fn cancelling_a_waiting_call_leaves_the_running_one_alone() {
    let fixture = fixture();
    let engine = Arc::clone(&fixture.engine);
    let running = CancellationToken::new();
    let events = Arc::new(Events::default());
    let _held = engine
        .begin_call(
            &running,
            Some(events.clone() as Arc<dyn GenerationProgressSink>),
        )
        .await
        .expect("the first call starts");
    let queued = CancellationToken::new();
    let waiting = engine.begin_call(&queued, None);
    tokio::pin!(waiting);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut waiting)
            .await
            .is_err()
    );
    queued.cancel();
    assert!(
        tokio::time::timeout(Duration::from_secs(2), waiting)
            .await
            .expect("the waiting call ends promptly")
            .is_none()
    );
    assert!(!running.is_cancelled());
    engine.report(GenerationProgress::Generating);
    assert_eq!(
        *events.0.lock().expect("events"),
        [GenerationProgress::Generating]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn cancelling_while_the_server_loads_returns_promptly_and_stops_it() {
    let fixture = fixture();
    let pid_file = fixture.root.join("server.pid");
    install_build(
        &fixture,
        &format!("echo $$ > '{}'\nexec sleep 60", pid_file.display()),
        "exit 1",
    );
    let model = model(&fixture.root);
    let cancellation = CancellationToken::new();
    let request = request(&model, cancellation.clone(), None);
    let engine = Arc::clone(&fixture.engine);
    let running = {
        let engine = Arc::clone(&engine);
        let model = model.clone();
        tokio::spawn(async move { engine.generate_images(&model, &request).await })
    };
    until(|| {
        pid_file.is_file()
            && !std::fs::read_to_string(&pid_file)
                .unwrap_or_default()
                .is_empty()
    })
    .await;
    let pid = std::fs::read_to_string(&pid_file)
        .expect("pid")
        .trim()
        .to_owned();
    assert!(alive(&pid), "the server is loading");
    cancellation.cancel();
    let outcome = tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .expect("cancelling does not wait for the server to load")
        .expect("task");
    assert_eq!(
        outcome.expect_err("cancelled").kind,
        ImageFailureKind::Cancelled
    );
    until(|| !alive(&pid)).await;
}

#[cfg(unix)]
#[tokio::test]
async fn cancelling_an_upscale_stops_the_tool_and_leaves_no_scratch_file() {
    let fixture = fixture();
    let pid_file = fixture.root.join("cli.pid");
    install_build(
        &fixture,
        "exit 1",
        &format!("echo $$ > '{}'\nexec sleep 60", pid_file.display()),
    );
    let paths = fixture.engine.paths();
    std::fs::create_dir_all(&paths.upscalers).expect("upscalers");
    std::fs::write(paths.upscalers.join("upscaler.pth"), b"model").expect("model");
    let cancellation = CancellationToken::new();
    let engine = Arc::clone(&fixture.engine);
    let running = {
        let (engine, cancellation) = (Arc::clone(&engine), cancellation.clone());
        tokio::spawn(async move { engine.upscale(b"image bytes", &cancellation).await })
    };
    until(|| {
        pid_file.is_file()
            && !std::fs::read_to_string(&pid_file)
                .unwrap_or_default()
                .is_empty()
    })
    .await;
    let pid = std::fs::read_to_string(&pid_file)
        .expect("pid")
        .trim()
        .to_owned();
    assert!(alive(&pid));
    assert_eq!(
        std::fs::read_dir(&paths.upscale_scratch)
            .expect("scratch")
            .count(),
        1,
        "the input is staged while the tool runs"
    );
    cancellation.cancel();
    let outcome = tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .expect("cancelling ends the upscale promptly")
        .expect("task");
    assert_eq!(
        outcome.expect_err("cancelled").kind,
        ImageFailureKind::Cancelled
    );
    until(|| !alive(&pid)).await;
    assert_eq!(
        std::fs::read_dir(&paths.upscale_scratch)
            .expect("scratch")
            .count(),
        0,
        "no scratch file is left"
    );
}

type Route = Arc<dyn Fn(&str) -> (u16, String) + Send + Sync>;

/// A local HTTP server answering each request with `route(request line)`;
/// returns its address and the request lines it saw.
async fn serve(route: Route) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
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

#[tokio::test]
async fn cancelling_a_running_generation_asks_the_engine_to_cancel_and_waits_for_its_end() {
    let fixture = fixture();
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = Arc::clone(&cancelled);
    let (endpoint, seen) = serve(Arc::new(move |line: &str| {
        if line.starts_with("POST /sdcpp/v1/img_gen ") {
            (
                200,
                r#"{"id": "job1", "poll_url": "/sdcpp/v1/jobs/job1"}"#.to_owned(),
            )
        } else if line.starts_with("POST /sdcpp/v1/jobs/job1/cancel ") {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            (200, "{}".to_owned())
        } else if line.starts_with("GET /sdcpp/v1/jobs/job1 ") {
            let status = if flag.load(std::sync::atomic::Ordering::SeqCst) {
                "cancelled"
            } else {
                "generating"
            };
            (200, format!(r#"{{"status": "{status}"}}"#))
        } else {
            (404, "{}".to_owned())
        }
    }))
    .await;
    let engine = Arc::clone(&fixture.engine);
    let events = Arc::new(Events::default());
    let cancellation = CancellationToken::new();
    let call = engine
        .begin_call(
            &cancellation,
            Some(events.clone() as Arc<dyn GenerationProgressSink>),
        )
        .await
        .expect("call");
    let running = {
        let (engine, cancellation) = (Arc::clone(&engine), cancellation.clone());
        tokio::spawn(async move {
            engine
                .run_generation_job(&endpoint, b"{}", &cancellation)
                .await
                .map(|_| ())
                .map_err(|error| matches!(error, super::server::GenerationJobError::Cancelled))
        })
    };
    until(|| {
        events
            .0
            .lock()
            .expect("events")
            .contains(&GenerationProgress::Generating)
    })
    .await;
    cancellation.cancel();
    let outcome = tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .expect("the generation ends after the cancel")
        .expect("task");
    assert_eq!(outcome, Err(true), "the generation ends as cancelled");
    drop(call);
    let seen = seen.lock().expect("seen");
    assert!(
        seen.iter()
            .any(|line| line.starts_with("POST /sdcpp/v1/jobs/job1/cancel "))
    );
    assert!(
        events
            .0
            .lock()
            .expect("events")
            .contains(&GenerationProgress::Cancelled)
    );
}
