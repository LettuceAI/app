#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
#[cfg(not(target_os = "android"))]
use std::{env, process::Command, sync::OnceLock};

use tauri::AppHandle;

use super::model::KokoroError;
use super::phonemizer::EspeakConfig;

pub fn resolve_espeak_config(
    app: &AppHandle,
    requested_bin_path: Option<PathBuf>,
    requested_data_path: Option<PathBuf>,
) -> Result<EspeakConfig, KokoroError> {
    if let Some(config) = explicit_espeak_config(requested_bin_path, requested_data_path)? {
        return Ok(config);
    }

    #[cfg(target_os = "android")]
    {
        return android::resolve_bundled_espeak(app);
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        resolve_desktop_espeak()
    }
}

fn explicit_espeak_config(
    requested_bin_path: Option<PathBuf>,
    requested_data_path: Option<PathBuf>,
) -> Result<Option<EspeakConfig>, KokoroError> {
    let Some(bin_path) = requested_bin_path else {
        return Ok(None);
    };

    if !bin_path.is_file() {
        return Err(KokoroError::Config(format!(
            "Configured eSpeak NG binary does not exist: {}",
            bin_path.display()
        )));
    }

    let data_path = match requested_data_path {
        Some(path) if path.is_dir() => Some(path),
        Some(path) => {
            return Err(KokoroError::Config(format!(
                "Configured eSpeak NG data path does not exist: {}",
                path.display()
            )));
        }
        None => None,
    };

    Ok(Some(EspeakConfig {
        bin_path: Some(bin_path),
        data_path,
    }))
}

#[cfg(not(target_os = "android"))]
fn resolve_desktop_espeak() -> Result<EspeakConfig, KokoroError> {
    static ESPEAK_ON_PATH: OnceLock<()> = OnceLock::new();
    if ESPEAK_ON_PATH.get().is_some() || command_available("espeak-ng") {
        let _ = ESPEAK_ON_PATH.set(());
        return Ok(EspeakConfig::default());
    }

    Err(KokoroError::EspeakUnavailable(espeak_install_guide()))
}

#[cfg(not(target_os = "android"))]
fn command_available(command: &str) -> bool {
    let mut cmd = Command::new(command);
    cmd.arg("--version");

    #[cfg(target_os = "windows")]
    cmd.creation_flags(super::phonemizer::CREATE_NO_WINDOW);

    cmd.output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "android"))]
fn espeak_install_guide() -> String {
    let install = match env::consts::OS {
        "windows" => "Install eSpeak NG, then restart the app:\n\nwinget install eSpeak-NG.eSpeak-NG",
        "macos" => "Install eSpeak NG, then restart the app:\n\nbrew install espeak-ng",
        "linux" => {
            "Install eSpeak NG with your distro package manager, then restart the app:\n\nUbuntu/Debian: sudo apt install espeak-ng\nFedora: sudo dnf install espeak-ng\nArch: sudo pacman -S espeak-ng"
        }
        _ => "Install eSpeak NG, make sure `espeak-ng` is on PATH, then restart the app.",
    };

    format!(
        "Kokoro requires eSpeak NG for phonemization, but `espeak-ng` was not found.\n\n{install}\n\nGuide: https://github.com/espeak-ng/espeak-ng"
    )
}

#[cfg(target_os = "android")]
mod android {
    use std::path::PathBuf;
    use std::sync::mpsc;

    use jni::objects::{JClass, JString, JValue};
    use tauri::{AppHandle, Manager};

    use super::{EspeakConfig, KokoroError};

    pub fn resolve_bundled_espeak(app: &AppHandle) -> Result<EspeakConfig, KokoroError> {
        let window = app.get_webview_window("main").ok_or_else(|| {
            KokoroError::EspeakUnavailable(
                "Kokoro Android phonemizer bridge is unavailable because the main WebView is not initialized."
                    .to_string(),
            )
        })?;

        let (tx, rx) = mpsc::channel();
        window
            .with_webview(move |webview| {
                webview.jni_handle().exec(move |env, activity, _webview| {
                    let result = (|| -> Result<String, jni::errors::Error> {
                        let class_loader = env
                            .call_method(
                                activity,
                                "getClassLoader",
                                "()Ljava/lang/ClassLoader;",
                                &[],
                            )?
                            .l()?;
                        let class_name = env.new_string(env!("KOKORO_ANDROID_BRIDGE_CLASS"))?;
                        let class_obj = env
                            .call_method(
                                &class_loader,
                                "loadClass",
                                "(Ljava/lang/String;)Ljava/lang/Class;",
                                &[JValue::Object(&class_name)],
                            )?
                            .l()?;
                        let class: JClass = class_obj.into();
                        let value = env.call_static_method(
                            &class,
                            "resolve",
                            "(Landroid/content/Context;)Ljava/lang/String;",
                            &[JValue::Object(activity)],
                        )?;
                        let object = value.l()?;
                        let output: String = env.get_string(&JString::from(object))?.into();
                        Ok(output)
                    })();
                    let _ = tx.send(result.map_err(|err: jni::errors::Error| err.to_string()));
                });
            })
            .map_err(|err| KokoroError::EspeakUnavailable(err.to_string()))?;

        let data_path = rx
            .recv()
            .map_err(|err| KokoroError::EspeakUnavailable(err.to_string()))?
            .map_err(KokoroError::EspeakUnavailable)?;
        parse_bridge_payload(&data_path)
    }

    fn parse_bridge_payload(payload: &str) -> Result<EspeakConfig, KokoroError> {
        let data_path = payload.lines().next().map(PathBuf::from).ok_or_else(|| {
            KokoroError::EspeakUnavailable(
                "Android Kokoro phonemizer bridge returned an empty data path.".to_string(),
            )
        })?;

        if !data_path.is_dir() {
            return Err(KokoroError::EspeakUnavailable(format!(
                "Bundled Android eSpeak NG data directory is missing: {}",
                data_path.display()
            )));
        }

        Ok(EspeakConfig {
            bin_path: None,
            data_path: Some(data_path),
        })
    }
}
