use std::sync::Arc;

use lettuce_context::PromptPurpose;
use lettuce_contracts::{self as dto};
use lettuce_jobs::JobStore;

use crate::api::jobs::JobHandler;
use crate::api::tests::Harness;
use crate::launch::tests::seed_prompt;

fn world() -> (std::path::PathBuf, Harness) {
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority};
    let root = std::env::temp_dir().join(format!("lettuce-backup-matrix-{}", lettuce_types::OperationId::new()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot")).expect("authority");
    let location = crate::AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority).expect("location");
    let active = location.active_path().expect("active");
    let backend = Arc::new(crate::AppBackend::open(&active, lettuce_types::TimestampMillis::new(10)).expect("backend"));
    let media = Arc::new(crate::api::ApiMediaStore::new(
        authority.managed_files(),
        authority.read_capability(lettuce_platform::ManagedRoot::MediaBlobs).expect("read"),
        authority.write_capability(lettuce_platform::ManagedRoot::MediaBlobs).expect("write"),
        lettuce_database::Database::open(&active).expect("blobs"),
        lettuce_database::Database::open(&active).expect("assets"),
    ));
    let harness = crate::api::tests::harness_over_files(
        backend,
        crate::api::tests::Reply::Text("A reply with naïve text, emoji 🌿, <b>markup</b> and *actions*."),
        Arc::new(lettuce_jobs::SystemClock),
        Some(media),
        Some(root.clone()),
        Arc::new(crate::api::NoModels),
        Arc::new(crate::api::tests::NoImages),
        Some(crate::api::ApiDatabaseFiles { location, active }),
    );
    (root, harness)
}

async fn export(harness: &Harness, root: &std::path::Path, label: &str) -> Result<u64, String> {
    let target = root.join(format!("{label}.lettuce"));
    let accepted = crate::api::backup_export(&harness.context, dto::BackupExportRequest { password: "correct horse".into(), target: Some(dto::FileTarget { uri: target.display().to_string() }), client_operation_id: format!("export-{label}") }).await.map_err(|error| format!("admission: {error:?}"))?;
    let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::export_jobs::ExportHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.map_err(|error| format!("claim: {error:?}"))?.ok_or("claim: no work")?;
    work.run(harness.context.clone(), Arc::new(super::tests::Progress)).await.map_err(|error| format!("run: {error:?}"))?;
    let view = crate::api::job_get(&harness.context, dto::JobGetRequest { job_id: accepted.job_id }).await.map_err(|error| format!("job: {error:?}"))?;
    if view.state != dto::JobStateDto::Succeeded {
        return Err(format!("state {:?}, failure {:?}", view.state, view.failure));
    }
    crate::api::backup_inspect(&harness.context, dto::BackupInspectRequest { source: dto::FileSource { uri: target.display().to_string() } }).await.map_err(|error| format!("inspect: {error:?}"))?;
    std::fs::metadata(&target).map(|metadata| metadata.len()).map_err(|error| format!("target: {error}"))
}

fn run_isolated<F: std::future::Future<Output = ()> + Send + 'static>(work: impl FnOnce() -> F + Send + 'static) {
    std::thread::Builder::new().stack_size(256 * 1024 * 1024).spawn(move || {
        tokio::runtime::Builder::new_multi_thread().thread_stack_size(64 * 1024 * 1024).enable_all().build().expect("runtime").block_on(Box::pin(work()));
    }).expect("thread").join().expect("scenario");
}

async fn audio_voice(harness: &Harness, key: &str) -> (dto::AudioProviderView, dto::UserVoiceView) {
    let provider = crate::api::audio_provider_create(&harness.context, dto::AudioProviderCreateRequest { client_operation_id: format!("{key}-provider"), label: "Hosted voices".into(), draft: dto::AudioProviderDraft { configuration: dto::AudioProviderConfiguration::Elevenlabs, api_key: Some("audio-secret".into()) } }).await.expect("audio provider");
    let voice = crate::api::user_voice_create(&harness.context, dto::UserVoiceCreateRequest { client_operation_id: format!("{key}-voice"), provider_id: provider.id.clone(), name: "Narrator".into(), model_id: "eleven_multilingual_v2".into(), voice_id: "remote-voice".into(), prompt: None }).await.expect("user voice");
    (provider, voice)
}

#[test]
fn backup_survives_deleting_a_user_voice_a_character_speaks_with() {
    run_isolated(|| async {
        let (root, harness) = world();
        let (_provider, voice) = audio_voice(&harness, "voice-used").await;
        let database = harness.context.backend().database();
        let character = lettuce_characters::CharacterRepository::get(database, harness.character_id).expect("character").expect("present").character;
        let mut defaults = character.defaults;
        defaults.voice = Some(lettuce_characters::VoicePreference::VoiceProfile(voice.id.parse().expect("voice id")));
        lettuce_characters::CharacterRepository::update_defaults(database, harness.character_id, character.revision, defaults, harness.context.now()).expect("character voice");
        crate::api::user_voice_delete(&harness.context, dto::UserVoiceRequest { voice_id: voice.id }).await.expect("delete voice");
        export(&harness, &root, "deleted-voice").await.expect("backup after deleting a used voice");
    });
}

#[test]
fn backup_survives_deleting_an_audio_provider_that_owns_user_voices() {
    run_isolated(|| async {
        let (root, harness) = world();
        let (provider, _voice) = audio_voice(&harness, "provider-owner").await;
        crate::api::audio_provider_delete(&harness.context, dto::AudioProviderDeleteRequest { provider_id: provider.id, expected_revision: provider.revision }).await.expect("delete provider");
        export(&harness, &root, "deleted-provider").await.expect("backup after deleting a provider with voices");
    });
}

#[test]
fn backup_survives_deleting_a_prompt_a_starter_uses() {
    run_isolated(|| async {
        let (root, harness) = world();
        let database = harness.context.backend().database();
        let prompt = seed_prompt(database, "Starter prompt", PromptPurpose::DirectChat);
        let mut starter = crate::launch::tests::starter_with(lettuce_types::CharacterId::new(), 0, "Greeting", Vec::new());
        starter.prompt_id = Some(prompt);
        crate::launch::tests::seed_character(database, Vec::new(), Vec::new(), vec![starter], |_| {});
        let view = crate::api::prompt_get(&harness.context, dto::PromptGetRequest { prompt_id: prompt.to_string() }).await.expect("prompt");
        crate::api::prompt_delete(&harness.context, dto::PromptDeleteRequest { client_operation_id: "delete-starter-prompt".into(), prompt_id: prompt.to_string(), expected_revision: view.prompt.revision }).await.expect("delete prompt");
        export(&harness, &root, "deleted-starter-prompt").await.expect("backup after deleting a starter prompt");
    });
}
