use lettuce_contracts::{ApiError, ApiErrorCode};
use tokio::sync::watch;

/// Registered before the webview is created. Its first status request may
/// arrive while setup is still opening the database and recovering jobs.
pub(crate) struct BackendStartup(watch::Sender<Option<Result<(), ApiError>>>);

impl Default for BackendStartup {
    fn default() -> Self {
        Self(watch::channel(None).0)
    }
}

impl BackendStartup {
    pub(crate) fn finish(&self, result: Result<(), ApiError>) {
        self.0.send_replace(Some(result));
    }

    pub(crate) async fn ready(&self) -> Result<(), ApiError> {
        let mut state = self.0.subscribe();
        loop {
            if let Some(result) = state.borrow_and_update().clone() {
                return result;
            }
            state.changed().await.map_err(|error| ApiError {
                code: ApiErrorCode::Internal,
                message: error.to_string(),
                details: None,
            })?;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use lettuce_app::api::{ApiContext, ApiContextSlot};
    use tauri::Manager;

    use super::*;

    fn status_request() -> tauri::webview::InvokeRequest {
        tauri::webview::InvokeRequest {
            cmd: "app_status".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().expect("mock origin"),
            body: tauri::ipc::InvokeBody::default(),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn first_status_request_waits_for_backend_startup() {
        let root = std::env::temp_dir().join(format!("lettuce-startup-{}", uuid::Uuid::new_v4()));
        let app = tauri::test::mock_builder()
            .manage(BackendStartup::default())
            .invoke_handler(tauri::generate_handler![crate::commands::app::app_status])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock shell");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build().expect("webview before backend setup");
        let first_webview = webview.clone();
        let mut first = tokio::task::spawn_blocking(move || {
            tauri::test::get_ipc_response(&first_webview, status_request())
        });
        let early = tokio::time::timeout(Duration::from_millis(100), &mut first).await;
        let context = ApiContext::open_desktop(
            &root, None, Arc::new(lettuce_settings::InMemorySecretStore::new()),
            Arc::new(crate::events::TauriEventSink(app.handle().clone())),
            crate::files::file_access(app.handle()),
            crate::asset_protocol::ASSET_URL_BASE.into(), crate::microphone::capture(),
        ).expect("open backend");
        app.manage(ApiContextSlot::new(Arc::new(context)));
        app.state::<BackendStartup>().finish(Ok(()));
        assert!(early.is_err(), "status must wait for setup instead of rejecting: {early:?}");
        let status = tokio::time::timeout(Duration::from_secs(5), first).await
            .expect("status wakes after startup").expect("IPC task").expect("status succeeds");
        let status = status.deserialize::<lettuce_contracts::AppStatus>().expect("app status");
        assert!(!status.version.is_empty());
        let later = tokio::task::spawn_blocking(move || {
            tauri::test::get_ipc_response(&webview, status_request())
        });
        assert!(tokio::time::timeout(Duration::from_secs(5), later).await
            .expect("later status does not wait").expect("IPC task").is_ok());
        drop(app);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn first_status_request_preserves_backend_startup_failure() {
        let app = tauri::test::mock_builder()
            .manage(BackendStartup::default())
            .invoke_handler(tauri::generate_handler![crate::commands::app::app_status])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock shell");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build().expect("webview");
        let failure = ApiError {
            code: ApiErrorCode::Internal,
            message: "startup database recovery failed".into(),
            details: None,
        };
        let request = tokio::task::spawn_blocking(move || {
            tauri::test::get_ipc_response(&webview, status_request())
        });
        app.state::<BackendStartup>().finish(Err(failure.clone()));
        let error = tokio::time::timeout(Duration::from_secs(5), request).await
            .expect("startup failure wakes status").expect("IPC task").expect_err("startup failed");
        assert_eq!(error, serde_json::to_value(failure).expect("API error"));
    }
}
