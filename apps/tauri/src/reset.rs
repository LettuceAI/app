use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lettuce_app::api::AppResetHost;
use lettuce_contracts::{ApiError, ApiErrorCode, ApiErrorDetails, AppDataResetStage};
use serde::Deserialize;
use tauri::{AppHandle, Listener, Manager, Runtime};

pub(crate) struct ResetHost<R: Runtime>(pub AppHandle<R>);

fn failure(stage: AppDataResetStage) -> ApiError {
    ApiError {
        code: ApiErrorCode::Unavailable,
        message: "the reset shell operation is unavailable".into(),
        details: Some(ApiErrorDetails::AppDataReset {
            stage,
            kept_file: None,
        }),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cleared {
    nonce: String,
    ok: bool,
}

struct ResetListener<R: Runtime>(AppHandle<R>, tauri::EventId);

impl<R: Runtime> Drop for ResetListener<R> {
    fn drop(&mut self) {
        self.0.unlisten(self.1);
    }
}

#[async_trait]
impl<R: Runtime> AppResetHost for ResetHost<R> {
    async fn preflight(&self) -> Result<(), ApiError> {
        if self.0.get_webview_window("main").is_none() {
            return Err(failure(AppDataResetStage::Preflight));
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let binary = tauri::process::current_binary(&self.0.env())
                .map_err(|_| failure(AppDataResetStage::Preflight))?;
            if !binary.is_file() {
                return Err(failure(AppDataResetStage::Preflight));
            }
        }
        Ok(())
    }

    async fn stop_workers(&self) -> Result<(), ApiError> {
        let workers = self
            .0
            .try_state::<crate::Workers>()
            .ok_or_else(|| failure(AppDataResetStage::Workers))?
            .0
            .lock()
            .map_err(|_| failure(AppDataResetStage::Workers))?
            .take()
            .ok_or_else(|| failure(AppDataResetStage::Workers))?;
        workers.stop_for_reset().await
    }

    async fn reload_database(&self, job: &str) -> Result<Arc<lettuce_app::api::ApiContext>, ApiError> {
        let slot = self.0.try_state::<lettuce_app::api::ApiContextSlot>().ok_or_else(crate::reload_failure)?;
        let original = slot.snapshot()?;
        let mut pending = None;
        let result = async {
            let context = Arc::new(crate::open_context(&self.0, original.secret_store().clone())?);
            let (old, replacement) = slot.start_reloaded(context.clone(), job).await?;
            pending = Some(replacement);
            let workers = self.0.try_state::<crate::Workers>().ok_or_else(crate::reload_failure)?.0.lock().map_err(|_| crate::reload_failure())?.take();
            lettuce_app::api::ApiContextSlot::retire(&old, workers).await?;
            let state = self.0.try_state::<crate::Workers>().ok_or_else(crate::reload_failure)?;
            let installed = match state.0.lock() {
                Ok(mut installed) => { *installed = pending.take(); true }
                Err(_) => false,
            };
            if !installed { return Err(crate::reload_failure()); }
            Ok(context)
        }.await;
        if result.is_err() {
            let active = slot.snapshot().unwrap_or_else(|_| original.clone());
            slot.require_recovery();
            let mut workers = self.0.try_state::<crate::Workers>().and_then(|workers| workers.0.lock().ok()?.take());
            let active_workers = pending.or_else(|| if Arc::ptr_eq(&active, &original) { workers.take() } else { None });
            let _ = lettuce_app::api::ApiContextSlot::retire(&active, active_workers).await;
            if !Arc::ptr_eq(&active, &original) || workers.is_some() {
                let _ = lettuce_app::api::ApiContextSlot::retire(&original, workers).await;
            }
        }
        result.map_err(|_| crate::reload_failure())
    }

    async fn clear_webview_storage(&self) -> Result<(), ApiError> {
        let window = self
            .0
            .get_webview_window("main")
            .ok_or_else(|| failure(AppDataResetStage::WebviewStorage))?;
        let nonce = uuid::Uuid::new_v4().to_string();
        let (sent, received) = tokio::sync::oneshot::channel();
        let sender = Arc::new(Mutex::new(Some(sent)));
        let deliver = sender.clone();
        let expected = nonce.clone();
        let id = self.0.listen("lettuce-reset-storage", move |event| {
            if let Ok(reply) = serde_json::from_str::<Cleared>(event.payload())
                && reply.nonce == expected
                && let Ok(mut sender) = deliver.lock()
                && let Some(sender) = sender.take()
            {
                let _ = sender.send(reply.ok);
            }
        });
        let _listener = ResetListener(self.0.clone(), id);
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed)
                && let Ok(mut sender) = sender.lock()
                && let Some(sender) = sender.take()
            {
                let _ = sender.send(false);
            }
        });
        window
            .eval(format!(
                "(() => {{ const resetNonce = {};\n{}\n }})();",
                serde_json::to_string(&nonce)
                    .map_err(|_| failure(AppDataResetStage::WebviewStorage))?,
                include_str!("reset_storage.js")
            ))
            .map_err(|_| failure(AppDataResetStage::WebviewStorage))?;
        match received.await {
            Ok(true) => Ok(()),
            _ => Err(failure(AppDataResetStage::WebviewStorage)),
        }
    }

    async fn prepare_restart(&self) -> Result<(), ApiError> {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            Ok(())
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let env = self.0.env();
            let binary = tauri::process::current_binary(&env)
                .map_err(|_| failure(AppDataResetStage::Restart))?;
            std::process::Command::new(binary)
                .args(env.args_os.iter().skip(1))
                .spawn()
                .map_err(|_| failure(AppDataResetStage::Restart))?;
            Ok(())
        }
    }

    fn exit_for_restart(&self) {
        self.0.exit(0);
    }
}

#[cfg(all(test, desktop))]
mod tests {
    use super::*;
    use lettuce_app::api::{ApiContextSlot, JobEventSink};
    use lettuce_contracts::{self as dto, JobEvent};
    use tauri_specta::Event;

    struct JobEvents(tokio::sync::mpsc::UnboundedSender<JobEvent>);
    impl JobEventSink for JobEvents {
        fn emit(&self, event: JobEvent) -> bool { self.0.send(event).is_ok() }
    }

    async fn completed(context: &lettuce_app::api::ApiContext, id: String) {
        let (sent, mut received) = tokio::sync::mpsc::unbounded_channel();
        lettuce_app::api::job_watch(context, dto::JobWatchRequest { job_id: id }, Arc::new(JobEvents(sent))).await.expect("watch");
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            while let Some(event) = received.recv().await {
                match event {
                    JobEvent::Completed { .. } => return,
                    JobEvent::Failed { job } | JobEvent::Cancelled { job } => panic!("backup failed: {:?}", job.failure),
                    _ => {}
                }
            }
            panic!("job stream closed before completion");
        }).await.expect("completion");
    }

    #[tokio::test]
    async fn fix11a_real_reset_host_restores_rebinds_and_stops_old_workers() {
        let root = std::env::temp_dir().join(format!("lettuce-real-reload-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("logs")).expect("logs");
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().app.app_directories_override = Some(tauri::utils::config::AppDirectoriesOverride::Root(root.clone()));
        let builder = crate::specta_builder::<tauri::test::MockRuntime>();
        let app = tauri::test::mock_builder().build(context).expect("mock shell");
        builder.mount_events(&app);
        tauri::WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default()).build().expect("main webview");
        let output = lettuce_observability::install(lettuce_observability::ObservabilityConfig::default().with_local_output(lettuce_observability::LocalOutputConfig::new(root.join("logs")))).expect("logging").expect("local output");
        app.manage(output);
        let original = Arc::new(crate::open_context(app.handle(), Arc::new(lettuce_settings::InMemorySecretStore::new())).expect("ordinary context construction"));
        let old_file = lettuce_app::api::storage_database_files_list(&original).await.expect("original inventory").into_iter().find(|file| file.active).expect("active file").file;
        let workers = lettuce_app::api::startup(&original).await.expect("startup");
        workers.started_result().await.expect("old workers running");
        app.manage(ApiContextSlot::new(original.clone()));
        app.manage(crate::Workers(Mutex::new(Some(workers))));
        let target = root.join("backup.lettuce");
        let exported = lettuce_app::api::backup_export(&original, dto::BackupExportRequest { target: Some(dto::FileTarget { uri: target.display().to_string() }), password: "password".into(), client_operation_id: "native-export".into() }).await.expect("export");
        completed(&original, exported.job_id).await;
        let (sent, mut received) = tokio::sync::mpsc::unbounded_channel();
        let listener = app.listen(crate::events::AppEvent::NAME, move |event| {
            if let Ok(crate::events::AppEvent(dto::ApiEvent::DatabaseReloaded)) = serde_json::from_str(event.payload()) { let _ = sent.send(()); }
        });
        let restored = lettuce_app::api::backup_restore(&original, dto::BackupRestoreRequest { source: dto::FileSource { uri: target.display().to_string() }, password: "password".into(), client_operation_id: "native-restore".into() }).await.expect("restore");
        completed(&original, restored.job_id).await;
        tokio::time::timeout(std::time::Duration::from_secs(30), received.recv()).await.expect("reload notification").expect("event");
        assert!(received.try_recv().is_err(), "one reload event");
        let fresh = app.state::<ApiContextSlot>().snapshot().expect("fresh context");
        assert!(!Arc::ptr_eq(&original, &fresh));
        assert_ne!(lettuce_app::api::storage_database_files_list(&fresh).await.expect("restored inventory").into_iter().find(|file| file.active).expect("active file").file, old_file);
        assert!(original.backend().database().is_file_write_fenced().expect("old context fenced"));
        assert!(app.state::<crate::Workers>().0.lock().expect("worker state").as_ref().expect("replacement workers").steps().contains(&lettuce_app::api::StartupStep::StartWorkers));
        app.unlisten(listener);
        let workers = app.state::<crate::Workers>().0.lock().expect("workers").take().expect("new workers");
        workers.stop_for_restore().await.expect("stop replacement workers");
        drop(fresh);
        drop(original);
        drop(app);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
