use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails};
use lettuce_jobs::{JobState, JobStore, SystemClock};
use lettuce_models::{ModelCatalog, ProviderAccountRepository};
use lettuce_settings::{DeviceSettingsStore, SecretRecord, SecretValue};
use lettuce_types::{JobId, OperationId, ProviderAccountId, Revision, TimestampMillis};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::tests::{Harness, Reply, harness_in};
use super::*;
use crate::{
    ArtifactBody, ArtifactSource, ArtifactSourceClient, ArtifactSourceError, KokoroDownloadSource,
    KokoroVoiceDownloadSource, WhisperDownloadSource,
};

const MODEL_BYTES: &[u8] = b"GGUF model bytes";

type Route = Arc<dyn Fn(&str) -> (u16, String) + Send + Sync>;

/// A local HTTP server answering every request with `route(request line)`;
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
            let route = Arc::clone(&route);
            let captured = Arc::clone(&captured);
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

fn pin_listing() -> String {
    let sha = format!("{:x}", Sha256::digest(MODEL_BYTES));
    format!(
        r#"{{"sha": "{}", "siblings": [
            {{"rfilename": "Q4/m-Q4_K_M.gguf", "size": {len}, "lfs": {{"size": {len}, "sha256": "{sha}"}}}},
            {{"rfilename": "m-Q8_0.gguf", "size": {len}, "lfs": {{"size": {len}, "sha256": "{sha}"}}}}
        ]}}"#,
        "c".repeat(40),
        len = MODEL_BYTES.len()
    )
}

async fn hugging_face(context: &ApiContext) -> Arc<Mutex<Vec<String>>> {
    let (endpoint, seen) = serve(Arc::new(|line: &str| {
        if line.starts_with("GET /api/models/org/m?blobs=true ") {
            (200, pin_listing())
        } else {
            (404, "{}".to_owned())
        }
    }))
    .await;
    context.local_models().use_hugging_face_endpoint(endpoint);
    seen
}

fn scratch(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "lettuce-local-models-{label}-{}",
        OperationId::new()
    ));
    std::fs::create_dir_all(&root).expect("root");
    root
}

fn local_harness(label: &str) -> (Harness, PathBuf) {
    let folder = scratch(label);
    let harness = harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        None,
        Some(folder.clone()),
        Arc::new(NoModels),
    );
    (harness, folder)
}

fn download(operation: &str, file: &str, setup: dto::HfDownloadSetup) -> dto::HfDownloadRequest {
    dto::HfDownloadRequest {
        repo: "org/m".to_owned(),
        revision: None,
        file: file.to_owned(),
        mmproj_file: None,
        mtp_file: None,
        mtp_bundled: false,
        setup,
        client_operation_id: operation.to_owned(),
    }
}

fn setup(context_length: u64) -> dto::HfDownloadSetup {
    dto::HfDownloadSetup {
        context_length: Some(context_length),
        create_model: true,
        ..dto::HfDownloadSetup::default()
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

/// Serves the model bytes, or refuses every file with `refusal`.
struct Files {
    refusal: Option<ArtifactSourceError>,
    opens: Mutex<u32>,
}

struct Bytes(Option<Vec<u8>>, u64);

#[async_trait]
impl ArtifactBody for Bytes {
    fn start(&self) -> u64 {
        self.1
    }

    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ArtifactSourceError> {
        Ok(self.0.take())
    }
}

#[async_trait]
impl ArtifactSourceClient for Arc<Files> {
    async fn open(
        &self,
        _source: &ArtifactSource,
        offset: u64,
        _expected_size: u64,
    ) -> Result<Box<dyn ArtifactBody>, ArtifactSourceError> {
        *self.opens.lock().expect("opens") += 1;
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        let start = usize::try_from(offset).expect("offset");
        Ok(Box::new(Bytes(Some(MODEL_BYTES[start..].to_vec()), offset)))
    }
}

struct Sources(Arc<Files>);

#[async_trait]
impl InstallSources for Sources {
    async fn artifacts(
        &self,
        _context: &ApiContext,
        _finish: &InstallFinish,
    ) -> Result<Box<dyn ArtifactSourceClient>, lettuce_contracts::ApiError> {
        Ok(Box::new(Arc::clone(&self.0)))
    }

    fn whisper(&self) -> Result<Box<dyn WhisperDownloadSource>, lettuce_contracts::ApiError> {
        NetworkInstallSources.whisper()
    }

    fn kokoro_model(&self) -> Result<Box<dyn KokoroDownloadSource>, lettuce_contracts::ApiError> {
        NetworkInstallSources.kokoro_model()
    }

    fn kokoro_voices(
        &self,
    ) -> Result<Box<dyn KokoroVoiceDownloadSource>, lettuce_contracts::ApiError> {
        NetworkInstallSources.kokoro_voices()
    }
}

fn runner(context: &ApiContext, refusal: Option<ArtifactSourceError>) -> JobRunner {
    JobRunner::new(
        context.clone(),
        JobHandlers::new(vec![
            Arc::new(ArtifactInstallHandler::new(Arc::new(Sources(Arc::new(
                Files {
                    refusal,
                    opens: Mutex::new(0),
                },
            ))))),
            Arc::new(ModelPullHandler),
            Arc::new(ModelsFolderMoveHandler),
        ]),
    )
}

async fn run(runner: &JobRunner) {
    runner.run_once().await.expect("run");
    runner.wait_idle().await;
}

fn busy_reason(error: &lettuce_contracts::ApiError) -> Option<&dto::LocalModelsBusyReason> {
    match &error.details {
        Some(ApiErrorDetails::LocalModelsBusy { reason }) if error.code == ApiErrorCode::Busy => {
            Some(reason)
        }
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_request_for_one_install_joins_it_or_conflicts() {
    let (harness, folder) = local_harness("join");
    let context = &harness.context;
    hugging_face(context).await;
    let first = hf_download(context, download("op-a", "Q4/m-Q4_K_M.gguf", setup(8192)))
        .await
        .expect("first");
    let joined = hf_download(context, download("op-b", "Q4/m-Q4_K_M.gguf", setup(8192)))
        .await
        .expect("same setup joins");
    assert_eq!(joined, first);
    let other_setup = hf_download(context, download("op-c", "Q4/m-Q4_K_M.gguf", setup(4096)))
        .await
        .expect_err("legacy silently replaced the queued setup");
    assert_eq!(other_setup.code, ApiErrorCode::Conflict);
    let reused_key = hf_download(context, download("op-a", "m-Q8_0.gguf", setup(8192)))
        .await
        .expect_err("a key names one request");
    assert_eq!(reused_key.code, ApiErrorCode::Conflict);
    let replayed = hf_download(context, download("op-a", "Q4/m-Q4_K_M.gguf", setup(8192)))
        .await
        .expect("replay");
    assert_eq!(replayed, first);
    let listed = view(context, job_id(&first)).await;
    assert_eq!(
        listed.subject_detail,
        Some(dto::JobSubjectDetail::ModelDownload {
            repo: "org/m".to_owned(),
            file: "Q4/m-Q4_K_M.gguf".to_owned(),
            display_name: "m-Q4_K_M".to_owned(),
        })
    );
    assert_eq!(state(context, job_id(&first)), JobState::Queued);
    std::fs::remove_dir_all(folder).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_finished_download_names_its_model_and_a_repeat_reuses_it() {
    let (harness, folder) = local_harness("finish");
    let context = &harness.context;
    hugging_face(context).await;
    let runner = runner(context, None);
    let first = hf_download(context, download("op-1", "Q4/m-Q4_K_M.gguf", setup(8192)))
        .await
        .expect("download");
    run(&runner).await;
    assert_eq!(state(context, job_id(&first)), JobState::Succeeded);
    let finished = view(context, job_id(&first)).await;
    let Some(dto::JobResultDto::ModelInstalled {
        model_path,
        model_profile_id: Some(model_profile_id),
    }) = finished.result
    else {
        panic!("a model install result: {:?}", finished.result);
    };
    assert_eq!(std::fs::read(&model_path).expect("installed"), MODEL_BYTES);
    assert!(model_path.ends_with(&format!(
        "org--m{0}Q4{0}m-Q4_K_M.gguf",
        std::path::MAIN_SEPARATOR
    )));
    let again = hf_download(context, download("op-2", "Q4/m-Q4_K_M.gguf", setup(8192)))
        .await
        .expect("again");
    assert_ne!(again, first);
    run(&runner).await;
    let repeated = view(context, job_id(&again)).await;
    assert_eq!(
        repeated.result,
        Some(dto::JobResultDto::ModelInstalled {
            model_path: model_path.clone(),
            model_profile_id: Some(model_profile_id),
        }),
        "legacy created a new model for every completed download of the same file"
    );
    let llama_models = context
        .backend()
        .database()
        .model_profiles()
        .expect("models")
        .into_iter()
        .filter(|profile| profile.external_model_id == model_path)
        .count();
    assert_eq!(llama_models, 1);
    let listed = local_models_list(context).await.expect("list");
    let file = listed
        .files
        .iter()
        .find(|file| file.path == model_path)
        .expect("the nested file is listed");
    assert_eq!(file.filename, "Q4/m-Q4_K_M.gguf");
    assert_eq!(file.used_by.len(), 1);
    assert_eq!(file.used_by[0].fields, [dto::LocalModelPathField::Model]);
    std::fs::remove_dir_all(folder).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_download_fails_with_the_hugging_face_reason() {
    let (harness, folder) = local_harness("refused");
    let context = &harness.context;
    hugging_face(context).await;
    let runner = runner(
        context,
        Some(ArtifactSourceError::Refused {
            status: 403,
            signed_in: true,
        }),
    );
    let accepted = hf_download(context, download("op-1", "m-Q8_0.gguf", setup(8192)))
        .await
        .expect("download");
    run(&runner).await;
    let failed = view(context, job_id(&accepted)).await;
    assert_eq!(failed.state, dto::JobStateDto::Failed);
    let failure = failed.failure.expect("failure");
    assert_eq!(failure.code, dto::JobFailureCode::Authentication);
    assert_eq!(
        failure.hugging_face,
        Some(dto::HfFailure::GatedAccess {
            model_id: "org/m".to_owned()
        })
    );
    std::fs::remove_dir_all(folder).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_gated_repository_is_a_typed_error() {
    let (harness, folder) = local_harness("gated");
    let context = &harness.context;
    let (endpoint, _) = serve(Arc::new(|_: &str| (403, "{}".to_owned()))).await;
    context.local_models().use_hugging_face_endpoint(endpoint);
    let error = hf_model_files(
        context,
        dto::HfModelRequest {
            model_id: "org/gated".to_owned(),
            mode: dto::HfBrowseMode::Llm,
        },
    )
    .await
    .expect_err("gated");
    assert_eq!(
        error.details,
        Some(ApiErrorDetails::HuggingFace {
            failure: dto::HfFailure::GatedAccess {
                model_id: "org/gated".to_owned()
            }
        })
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let closed = format!("http://{}", listener.local_addr().expect("address"));
    drop(listener);
    context.local_models().use_hugging_face_endpoint(closed);
    let offline = hf_readme(
        context,
        dto::HfReadmeRequest {
            model_id: "org/m".to_owned(),
        },
    )
    .await
    .expect_err("offline");
    assert_eq!(
        offline.details,
        Some(ApiErrorDetails::HuggingFace {
            failure: dto::HfFailure::Offline
        })
    );
    std::fs::remove_dir_all(folder).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_saved_token_reads_as_unknown_while_offline() {
    let (harness, folder) = local_harness("token");
    let context = &harness.context;
    let purpose = lettuce_settings::SecretPurpose::HuggingFaceAccessToken;
    let reference = purpose.app_secret_ref().expect("reference");
    context
        .secret_store()
        .put(
            SecretRecord::new(reference, purpose),
            SecretValue::new("hf_token").expect("token"),
            None,
        )
        .await
        .expect("saved");
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let closed = format!("http://{}", listener.local_addr().expect("address"));
    drop(listener);
    context.local_models().use_hugging_face_endpoint(closed);
    assert_eq!(
        hf_auth_status(context).await.expect("status"),
        dto::HfTokenStatus::Unknown { offline: true },
        "legacy reported every failed check as an invalid token"
    );
    std::fs::remove_dir_all(folder).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_folder_move_waits_for_installs_and_loaded_models() {
    let (harness, folder) = local_harness("move");
    let context = &harness.context;
    hugging_face(context).await;
    let root = crate::llm_models_root(&lettuce_settings::DeviceSettings::default(), &folder);
    let target = folder.join("elsewhere");
    let move_request = |operation: &str| dto::LocalModelsDirSetRequest {
        path: target.to_string_lossy().into_owned(),
        move_existing: true,
        client_operation_id: operation.to_owned(),
    };
    let install = hf_download(context, download("op-install", "m-Q8_0.gguf", setup(8192)))
        .await
        .expect("queued install");
    let refused = local_models_dir_set(context, move_request("move-1"))
        .await
        .expect_err("an install writes into the folder");
    assert_eq!(
        busy_reason(&refused),
        Some(&dto::LocalModelsBusyReason::InstallActive {
            job_id: install.job_id.clone()
        })
    );
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: install.job_id.clone(),
        },
    )
    .await
    .expect("cancel");
    let nested = root.join("org--m").join("Q4");
    std::fs::create_dir_all(&nested).expect("nested");
    let loaded = nested.join("m-Q4_K_M.gguf");
    std::fs::write(&loaded, MODEL_BYTES).expect("model");
    let resident = Arc::new(Mutex::new(vec![loaded.to_string_lossy().into_owned()]));
    let files = Arc::clone(&resident);
    context
        .local_models()
        .use_resident_files(move || files.lock().expect("resident").clone());
    let refused = local_models_dir_set(context, move_request("move-2"))
        .await
        .expect_err("llama.cpp holds a model from the folder");
    assert_eq!(
        busy_reason(&refused),
        Some(&dto::LocalModelsBusyReason::ModelLoaded {
            path: loaded.to_string_lossy().into_owned()
        })
    );
    resident.lock().expect("resident").clear();
    let accepted = local_models_dir_set(context, move_request("move-3"))
        .await
        .expect("move");
    assert_eq!(
        local_models_dir_set(context, move_request("move-3"))
            .await
            .expect("replay"),
        accepted
    );
    let during = hf_download(context, download("op-late", "m-Q8_0.gguf", setup(8192)))
        .await
        .expect_err("no download starts while the folder moves");
    assert_eq!(
        busy_reason(&during),
        Some(&dto::LocalModelsBusyReason::FolderMoveActive {
            job_id: accepted.job_id.clone()
        })
    );
    let runner = runner(context, None);
    run(&runner).await;
    let moved = view(context, job_id(&accepted)).await;
    assert_eq!(moved.state, dto::JobStateDto::Succeeded);
    assert!(matches!(
        moved.result,
        Some(dto::JobResultDto::ModelsFolderMoved {
            moved_entries: 1,
            ..
        })
    ));
    assert!(
        target
            .join("org--m")
            .join("Q4")
            .join("m-Q4_K_M.gguf")
            .exists()
    );
    assert!(!loaded.exists());
    assert_eq!(
        context
            .backend()
            .database()
            .load_device_settings()
            .expect("device")
            .llm_models_dir
            .as_deref(),
        Some(target.to_string_lossy().as_ref())
    );
    std::fs::remove_dir_all(folder).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_a_loaded_file_unloads_it_and_names_its_models() {
    let (harness, folder) = local_harness("delete");
    let context = &harness.context;
    let root = crate::llm_models_root(&lettuce_settings::DeviceSettings::default(), &folder);
    let download = crate::GgufDownload {
        model_id: "org/m".to_owned(),
        model_file: "m.gguf".to_owned(),
        mmproj_file: None,
        mtp_file: None,
    };
    let path = download.installed(&root).model_path;
    std::fs::create_dir_all(root.join("org--m")).expect("folder");
    std::fs::write(&path, MODEL_BYTES).expect("model");
    let model = crate::register_downloaded_gguf(
        context.backend().database(),
        &root,
        &download,
        &crate::GgufModelSetup::default(),
        TimestampMillis::new(2),
    )
    .expect("model");
    let resident = path.clone();
    context
        .local_models()
        .use_resident_files(move || vec![resident.clone()]);
    let outside = local_model_delete(
        context,
        dto::LocalModelDeleteRequest {
            path: folder.join("outside.gguf").to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("a missing file is already gone");
    assert!(!outside.unloaded);
    let deleted = local_model_delete(context, dto::LocalModelDeleteRequest { path: path.clone() })
        .await
        .expect("delete");
    assert!(
        deleted.unloaded,
        "legacy deleted a file llama.cpp still held"
    );
    assert_eq!(
        deleted.referencing_profiles,
        [dto::LocalModelReference {
            model_profile_id: Some(model.id.to_string()),
            display_name: Some(model.display_name),
            fields: vec![dto::LocalModelPathField::Model],
        }]
    );
    assert!(!Path::new(&path).exists());
    std::fs::remove_dir_all(folder).ok();
}

fn ollama_account(context: &ApiContext, endpoint: String) -> ProviderAccountId {
    let account = lettuce_models::ProviderAccount {
        id: ProviderAccountId::new(),
        secret_owner_id: lettuce_settings::SecretOwnerId::new(),
        provider_kind: "ollama".to_owned(),
        protocol: lettuce_models::ProviderProtocol::Ollama,
        label: "Pulls".to_owned(),
        endpoint: Some(endpoint),
        enabled: true,
        streaming_enabled: true,
        allow_invalid_tls: false,
        api_key_ref: None,
        secret_headers: Vec::new(),
        config: lettuce_models::ProviderConfig::Standard,
        revision: Revision::INITIAL,
        created_at: TimestampMillis::new(1),
        updated_at: TimestampMillis::new(1),
    };
    ProviderAccountRepository::upsert(context.backend().database(), account, None)
        .expect("account")
        .id
}

fn pull(account: ProviderAccountId, model: &str, operation: &str) -> dto::OllamaPullRequest {
    dto::OllamaPullRequest {
        provider_account_id: account.to_string(),
        model: model.to_owned(),
        client_operation_id: operation.to_owned(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pull_cancelled_while_queued_never_reaches_ollama() {
    let (harness, folder) = local_harness("pull-cancel");
    let context = &harness.context;
    let (endpoint, seen) = serve(Arc::new(|_: &str| {
        (200, "{\"status\":\"success\"}\n".to_owned())
    }))
    .await;
    let account = ollama_account(context, endpoint);
    let accepted = ollama_pull(context, pull(account, "hf.co/org/m:Q4_K_M", "pull-1"))
        .await
        .expect("pull");
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: accepted.job_id.clone(),
        },
    )
    .await
    .expect("cancel");
    run(&runner(context, None)).await;
    assert_eq!(state(context, job_id(&accepted)), JobState::Cancelled);
    assert!(
        seen.lock().expect("seen").is_empty(),
        "legacy kept pulling a pull cancelled while queued"
    );
    std::fs::remove_dir_all(folder).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn pulls_report_success_and_fail_when_the_stream_stops_short() {
    let (harness, folder) = local_harness("pull-end");
    let context = &harness.context;
    let (endpoint, _) = serve(Arc::new(|line: &str| {
        (
            200,
            if line.starts_with("POST /api/pull ") {
                "{\"status\":\"pulling ab\",\"completed\":4,\"total\":8}\n".to_owned()
            } else {
                "{}".to_owned()
            },
        )
    }))
    .await;
    let short = ollama_account(context, endpoint);
    let (endpoint, _) = serve(Arc::new(|_: &str| {
        (
            200,
            "{\"status\":\"pulling ab\",\"completed\":8,\"total\":8}\n{\"status\":\"success\"}\n"
                .to_owned(),
        )
    }))
    .await;
    let complete = ollama_account(context, endpoint);
    let stopped = ollama_pull(context, pull(short, "hf.co/org/m:Q4_K_M", "pull-short"))
        .await
        .expect("pull");
    let pulled = ollama_pull(context, pull(complete, "llama3:8b", "pull-ok"))
        .await
        .expect("pull");
    assert_eq!(
        ollama_pull(context, pull(complete, "llama3:8b", "pull-ok"))
            .await
            .expect("replay"),
        pulled
    );
    assert_eq!(
        ollama_pull(context, pull(complete, "other", "pull-ok"))
            .await
            .expect_err("reused key")
            .code,
        ApiErrorCode::Conflict
    );
    run(&runner(context, None)).await;
    let failed = view(context, job_id(&stopped)).await;
    assert_eq!(
        failed.state,
        dto::JobStateDto::Failed,
        "legacy reported a pull that ended without success as complete"
    );
    assert_eq!(
        failed.failure.map(|failure| failure.code),
        Some(dto::JobFailureCode::WorkerFailed)
    );
    let succeeded = view(context, job_id(&pulled)).await;
    assert_eq!(succeeded.state, dto::JobStateDto::Succeeded);
    assert_eq!(
        succeeded.result,
        Some(dto::JobResultDto::ModelPulled {
            model: "llama3:8b".to_owned()
        })
    );
    assert_eq!(
        succeeded.subject_detail,
        Some(dto::JobSubjectDetail::ModelPull {
            provider_account_id: complete.to_string(),
            model: "llama3:8b".to_owned()
        })
    );
    std::fs::remove_dir_all(folder).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_download_admitted_before_its_detail_was_kept_is_joined_and_described() {
    let (harness, folder) = local_harness("heal");
    let context = &harness.context;
    hugging_face(context).await;
    let root = crate::llm_models_root(&lettuce_settings::DeviceSettings::default(), &folder);
    let plan = context
        .local_models()
        .browser(context)
        .expect("browser")
        .gguf_install_plan(
            context.secret_store().as_ref(),
            &root,
            &crate::GgufDownload {
                model_id: "org/m".to_owned(),
                model_file: "m-Q8_0.gguf".to_owned(),
                mmproj_file: None,
                mtp_file: None,
            },
            None,
        )
        .await
        .expect("plan");
    let orphan = crate::ArtifactInstallCoordinator::new(context.backend().database())
        .admit(&plan)
        .expect("job without its detail")
        .job
        .id;
    let accepted = hf_download(context, download("op-heal", "m-Q8_0.gguf", setup(8192)))
        .await
        .expect("retry after the crash");
    assert_eq!(job_id(&accepted), orphan);
    assert!(matches!(
        view(context, orphan).await.subject_detail,
        Some(dto::JobSubjectDetail::ModelDownload { .. })
    ));
    std::fs::remove_dir_all(folder).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_pull_stops_when_cancelled() {
    let (harness, folder) = local_harness("pull-running");
    let context = &harness.context;
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let endpoint = format!("http://{}", listener.local_addr().expect("address"));
    let closed = Arc::new(tokio::sync::Notify::new());
    let dropped = Arc::clone(&closed);
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buffer = [0_u8; 4096];
        let _ = stream.read(&mut buffer).await;
        let head =
            "HTTP/1.1 200 OK\r\ncontent-type: application/x-ndjson\r\nconnection: close\r\n\r\n";
        stream.write_all(head.as_bytes()).await.expect("head");
        stream
            .write_all(b"{\"status\":\"pulling ab\",\"completed\":2,\"total\":8}\n")
            .await
            .expect("line");
        let mut rest = [0_u8; 16];
        while stream.read(&mut rest).await.is_ok_and(|read| read > 0) {}
        dropped.notify_one();
    });
    let account = ollama_account(context, endpoint);
    let accepted = ollama_pull(context, pull(account, "llama3:8b", "pull-run"))
        .await
        .expect("pull");
    let job = job_id(&accepted);
    let runner = runner(context, None);
    assert!(runner.run_once().await.expect("run"));
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while JobStore::get(context.backend().database(), job)
            .expect("job")
            .and_then(|job| job.progress.bytes)
            .is_none()
        {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("progress recorded");
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: accepted.job_id.clone(),
        },
    )
    .await
    .expect("cancel");
    runner.wait_idle().await;
    assert_eq!(state(context, job), JobState::Cancelled);
    tokio::time::timeout(std::time::Duration::from_secs(10), closed.notified())
        .await
        .expect("the pull request was dropped");
    std::fs::remove_dir_all(folder).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn any_refused_hugging_face_install_names_its_repository() {
    let (harness, folder) = local_harness("refused-bundle");
    let context = &harness.context;
    let source = ArtifactSource::HuggingFace {
        repository: "org/gated-bundle".to_owned(),
        revision: "a".repeat(40),
        path: "vae.safetensors".to_owned(),
    };
    let plan = crate::ArtifactInstallPlan {
        install_id: "bundle".to_owned(),
        root: folder.join("bundle"),
        artifacts: vec![crate::PlannedArtifact {
            artifact: lettuce_model_hub::PinnedArtifact {
                source_identity: source.identity(),
                local_segments: vec!["vae.safetensors".to_owned()],
                byte_size: MODEL_BYTES.len() as u64,
                sha256: Some(format!("{:x}", Sha256::digest(MODEL_BYTES))),
            },
            source,
        }],
    };
    let accepted = admit_install(
        context,
        InstallWork::Artifact {
            plan,
            finish: Box::new(InstallFinish::Files),
        },
    )
    .await
    .expect("admit");
    run(&runner(
        context,
        Some(ArtifactSourceError::Refused {
            status: 403,
            signed_in: true,
        }),
    ))
    .await;
    assert_eq!(
        view(context, job_id(&accepted))
            .await
            .failure
            .and_then(|failure| failure.hugging_face),
        Some(dto::HfFailure::GatedAccess {
            model_id: "org/gated-bundle".to_owned()
        })
    );
    std::fs::remove_dir_all(folder).ok();
}
