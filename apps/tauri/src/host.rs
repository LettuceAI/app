#[cfg(desktop)]
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use lettuce_contracts::FileSource;
use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_specta::Event;

#[cfg(desktop)]
const OPENED_EXTENSIONS: [&str; 2] = ["lettuce", "uec"];
const NAVIGATION_PATHS: [&str; 3] = ["/characters/new", "/library", "/"];

/// Something the operating system asked the app to open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OpenedItem {
    File { source: FileSource },
    Navigate { path: String },
}

impl OpenedItem {
    fn is_allowed(&self) -> bool {
        match self {
            Self::File { .. } => true,
            Self::Navigate { path } => navigation_allowed(path),
        }
    }
}

/// Tells the webview that `opened_files_take` has something to return.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct OpenedFiles;

#[derive(Default)]
pub(crate) struct OpenedInbox(Mutex<Vec<OpenedItem>>);

impl OpenedInbox {
    fn push(&self, items: Vec<OpenedItem>) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend(items);
    }

    fn take(&self) -> Vec<OpenedItem> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

/// Returns the items opened since the last call and clears the inbox.
#[tauri::command]
#[specta::specta]
pub fn opened_files_take(inbox: State<'_, OpenedInbox>) -> Vec<OpenedItem> {
    inbox.take()
}

pub(crate) fn navigation_allowed(path: &str) -> bool {
    NAVIGATION_PATHS.contains(&path)
}

#[cfg(desktop)]
fn path_item(path: &Path) -> Option<OpenedItem> {
    let extension = path.extension()?.to_str()?;
    if !OPENED_EXTENSIONS
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(extension))
    {
        return None;
    }
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let Some(uri) = path.to_str() else {
        tracing::warn!("an opened file was skipped because its path is not valid text");
        return None;
    };
    Some(OpenedItem::File {
        source: FileSource {
            uri: uri.to_owned(),
        },
    })
}

#[cfg(desktop)]
pub(crate) fn desktop_files(argv: &[String], cwd: Option<&Path>) -> Vec<OpenedItem> {
    argv.iter()
        .skip(1)
        .filter_map(|argument| {
            let path = PathBuf::from(argument);
            let path = if path.is_absolute() {
                path
            } else {
                cwd?.join(path)
            };
            path_item(&path)
        })
        .collect()
}

#[cfg(target_os = "macos")]
pub(crate) fn opened_paths<R: Runtime>(handle: &AppHandle<R>, paths: Vec<PathBuf>) {
    deliver(
        handle,
        paths.iter().filter_map(|path| path_item(path)).collect(),
    );
}

#[cfg(desktop)]
fn activated<R: Runtime>(
    handle: AppHandle<R>,
    payload: tauri_plugin_single_window::ActivationPayload,
) {
    let cwd = payload.cwd.as_deref().map(Path::new);
    deliver(&handle, desktop_files(&payload.argv, cwd));
}

#[cfg(desktop)]
pub(crate) fn single_window_config() -> tauri_plugin_single_window::Config {
    tauri_plugin_single_window::Config::new().on_activate::<tauri::DynRuntime, _>(activated)
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Deserialize)]
struct Batch {
    items: Vec<OpenedItem>,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn parse_batch(json: &str) -> Vec<OpenedItem> {
    match serde_json::from_str::<Batch>(json) {
        Ok(batch) => batch
            .items
            .into_iter()
            .filter(|item| {
                let allowed = item.is_allowed();
                if !allowed {
                    tracing::warn!(
                        "an Android navigation request outside the allowed paths was dropped"
                    );
                }
                allowed
            })
            .collect(),
        Err(error) => {
            tracing::warn!(%error, "an Android opened-items message could not be read");
            Vec::new()
        }
    }
}

fn deliver<R: Runtime>(handle: &AppHandle<R>, items: Vec<OpenedItem>) {
    if items.is_empty() {
        return;
    }
    let Some(inbox) = handle.try_state::<OpenedInbox>() else {
        tracing::warn!("opened items arrived before the inbox existed");
        return;
    };
    inbox.push(items);
    if let Err(error) = OpenedFiles.emit(handle) {
        tracing::warn!(%error, "the opened-files event could not be emitted");
    }
}

pub(crate) fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("lettuce-host")
        .setup(|app, api| {
            app.manage(OpenedInbox::default());
            #[cfg(desktop)]
            {
                let _ = &api;
                let argv = std::env::args_os()
                    .map(|argument| argument.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                let cwd = std::env::current_dir().ok();
                deliver(app, desktop_files(&argv, cwd.as_deref()));
            }
            #[cfg(target_os = "android")]
            android::register(app, api)?;
            #[cfg(not(any(desktop, target_os = "android")))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}

#[cfg(target_os = "android")]
pub(crate) use android::{finish_startup, follow_active_work};

#[cfg(target_os = "android")]
mod android {
    use std::sync::Arc;

    use serde::de::DeserializeOwned;
    use tauri::ipc::{Channel, InvokeResponseBody};
    use tauri::plugin::{PluginApi, PluginHandle};
    use tauri::{AppHandle, Manager, Runtime};

    use super::{deliver, parse_batch};
    use crate::active_work::{ActiveWorkAggregate, SETTLE_GRACE, drive};

    const PLUGIN_PACKAGE: &str = "com.lettuceai.app";
    const PLUGIN_CLASS: &str = "HostPlugin";

    struct AndroidHost<R: Runtime>(PluginHandle<R>);

    pub(super) fn register<R: Runtime, C: DeserializeOwned>(
        app: &AppHandle<R>,
        api: PluginApi<R, C>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let handle = api.register_android_plugin(PLUGIN_PACKAGE, PLUGIN_CLASS)?;
        let target = app.clone();
        let channel = Channel::<serde_json::Value>::new(move |body| {
            match body {
                InvokeResponseBody::Json(json) => {
                    let pressure = serde_json::from_str::<serde_json::Value>(&json).ok()
                        .is_some_and(|value| value.get("memoryPressure").and_then(serde_json::Value::as_bool) == Some(true));
                    if pressure {
                        if let Some(slot) = target.try_state::<lettuce_app::api::ApiContextSlot>()
                            && let Ok(context) = slot.snapshot()
                        { context.memory_warning(); }
                    } else { deliver(&target, parse_batch(&json)); }
                },
                InvokeResponseBody::Raw(_) => {
                    tracing::warn!("an Android opened-items message was not JSON");
                }
            }
            Ok(())
        });
        handle.run_mobile_plugin::<()>("subscribe", serde_json::json!({ "handler": channel }))?;
        app.manage(AndroidHost(handle));
        let aggregate = Arc::new(ActiveWorkAggregate::default());
        let signal = aggregate.watch();
        app.manage(aggregate);
        let driven = app.clone();
        tauri::async_runtime::spawn(drive(signal, SETTLE_GRACE, move |active| {
            set_background_work(driven.clone(), active)
        }));
        Ok(())
    }

    pub(crate) fn follow_active_work<R: Runtime>(
        handle: &AppHandle<R>,
        signal: tokio::sync::watch::Receiver<bool>,
    ) {
        let Some(aggregate) = handle.try_state::<Arc<ActiveWorkAggregate>>() else {
            tracing::warn!("the background work service is not registered");
            return;
        };
        tauri::async_runtime::spawn(Arc::clone(&aggregate).follow(signal));
    }

    pub(crate) async fn finish_startup<R: Runtime>(app: &AppHandle<R>) {
        let Some(host) = app.try_state::<AndroidHost<R>>() else {
            tracing::warn!("the Android host plugin is not registered at startup");
            return;
        };
        if let Err(error) = host.0.run_mobile_plugin_async::<()>(
            "finishStartup", serde_json::json!({}),
        ).await {
            tracing::warn!(%error, "the Android startup splash could not be dismissed");
        }
    }

    async fn set_background_work<R: Runtime>(app: AppHandle<R>, active: bool) -> bool {
        let Some(host) = app.try_state::<AndroidHost<R>>() else {
            tracing::warn!(active, "the Android host plugin is not registered");
            return false;
        };
        match host
            .0
            .run_mobile_plugin_async::<()>(
                "setBackgroundWork",
                serde_json::json!({ "active": active }),
            )
            .await
        {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(%error, active, "the background work service could not be updated");
                false
            }
        }
    }
}

#[cfg(all(test, desktop))]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("lettuce-host-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).expect("directory");
        directory
    }

    fn file(path: &Path) -> OpenedItem {
        OpenedItem::File {
            source: FileSource {
                uri: path.to_str().expect("utf-8 path").to_owned(),
            },
        }
    }

    fn argv(arguments: &[&Path]) -> Vec<String> {
        std::iter::once("lettuce-tauri".to_owned())
            .chain(
                arguments
                    .iter()
                    .map(|path| path.to_str().expect("utf-8 path").to_owned()),
            )
            .collect()
    }

    #[test]
    fn absolute_paths_with_the_accepted_extensions_are_opened() {
        let directory = temp("absolute");
        let backup = directory.join("chats.lettuce");
        let card = directory.join("hero.uec");
        std::fs::write(&backup, b"x").expect("backup");
        std::fs::write(&card, b"x").expect("card");
        let items = desktop_files(&argv(&[&backup, &card]), None);
        assert_eq!(items, vec![file(&backup), file(&card)]);
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn the_extension_is_matched_without_regard_to_case() {
        let directory = temp("case");
        let upper = directory.join("CHATS.LETTUCE");
        let mixed = directory.join("hero.UeC");
        std::fs::write(&upper, b"x").expect("upper");
        std::fs::write(&mixed, b"x").expect("mixed");
        let items = desktop_files(&argv(&[&upper, &mixed]), None);
        assert_eq!(items, vec![file(&upper), file(&mixed)]);
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn relative_paths_resolve_against_the_given_working_directory() {
        let directory = temp("relative");
        let backup = directory.join("chats.lettuce");
        std::fs::write(&backup, b"x").expect("backup");
        let arguments = vec!["lettuce-tauri".to_owned(), "chats.lettuce".to_owned()];
        assert_eq!(
            desktop_files(&arguments, Some(&directory)),
            vec![file(&backup)]
        );
        assert!(desktop_files(&arguments, None).is_empty());
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn other_extensions_missing_files_and_directories_are_skipped() {
        let directory = temp("skipped");
        let text = directory.join("notes.txt");
        let bare = directory.join("lettuce");
        let folder = directory.join("folder.lettuce");
        let missing = directory.join("gone.uec");
        std::fs::write(&text, b"x").expect("text");
        std::fs::write(&bare, b"x").expect("bare");
        std::fs::create_dir(&folder).expect("folder");
        let items = desktop_files(&argv(&[&text, &bare, &folder, &missing]), None);
        assert!(items.is_empty());
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn the_executable_path_and_flags_are_not_files() {
        let directory = temp("program");
        let program = directory.join("lettuce-tauri.uec");
        std::fs::write(&program, b"x").expect("program");
        let arguments = vec![
            program.to_str().expect("utf-8 path").to_owned(),
            "--flag.lettuce".to_owned(),
        ];
        assert!(desktop_files(&arguments, Some(&directory)).is_empty());
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn take_returns_everything_in_order_and_clears_the_inbox() {
        let inbox = OpenedInbox::default();
        assert!(inbox.take().is_empty());
        let first = OpenedItem::Navigate {
            path: "/library".into(),
        };
        let second = OpenedItem::File {
            source: FileSource {
                uri: "content://a".into(),
            },
        };
        let third = OpenedItem::Navigate { path: "/".into() };
        inbox.push(vec![first.clone(), second.clone()]);
        inbox.push(vec![third.clone()]);
        assert_eq!(inbox.take(), vec![first, second, third]);
        assert!(inbox.take().is_empty());
    }

    #[test]
    fn only_the_three_shortcut_paths_are_navigable() {
        for path in ["/characters/new", "/library", "/"] {
            assert!(navigation_allowed(path), "{path}");
        }
        for path in [
            "",
            "/characters",
            "/characters/new/",
            "/Library",
            "/library?x=1",
            "/settings",
            "library",
            "//library",
            "/library/../settings",
        ] {
            assert!(!navigation_allowed(path), "{path}");
        }
    }

    #[test]
    fn a_batch_keeps_files_and_allowed_navigation_and_drops_the_rest() {
        let json = r#"{"items":[
            {"kind":"file","source":{"uri":"content://media/1"}},
            {"kind":"navigate","path":"/library"},
            {"kind":"navigate","path":"/settings"},
            {"kind":"file","source":{"uri":"file:///sdcard/a.uec"}}
        ]}"#;
        assert_eq!(
            parse_batch(json),
            vec![
                OpenedItem::File {
                    source: FileSource {
                        uri: "content://media/1".into()
                    }
                },
                OpenedItem::Navigate {
                    path: "/library".into()
                },
                OpenedItem::File {
                    source: FileSource {
                        uri: "file:///sdcard/a.uec".into()
                    }
                },
            ]
        );
    }

    #[test]
    fn a_malformed_batch_yields_nothing() {
        assert!(parse_batch("not json").is_empty());
        assert!(parse_batch(r#"{"items":[{"kind":"launch"}]}"#).is_empty());
        assert!(
            parse_batch(r#"{"items":[{"kind":"file","source":{"uri":"a","extra":1}}]}"#).is_empty()
        );
    }

    #[test]
    fn items_serialize_with_a_kind_tag() {
        let value = serde_json::to_value(OpenedItem::Navigate { path: "/".into() }).expect("json");
        assert_eq!(value, serde_json::json!({"kind": "navigate", "path": "/"}));
        let value = serde_json::to_value(file(Path::new("/a.uec"))).expect("json");
        assert_eq!(
            value,
            serde_json::json!({"kind": "file", "source": {"uri": "/a.uec"}})
        );
    }
}
