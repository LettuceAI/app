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
