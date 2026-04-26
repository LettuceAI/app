use super::*;
use crate::utils::log_info;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::collections::HashMap;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::fs;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::io::Cursor;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::atomic::Ordering;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri::path::BaseDirectory;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri::Manager;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn macos_primary_dylib_name() -> String {
    format!("libonnxruntime.{}.dylib", ORT_VERSION)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn leaked_macos_primary_dylib_name() -> &'static str {
    Box::leak(macos_primary_dylib_name().into_boxed_str())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn macos_archive_name(target_arch: &str) -> Option<&'static str> {
    match target_arch {
        "aarch64" => Some("arm64"),
        "x86_64" => Some("x86_64"),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn current_macos_arch() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "aarch64" => Some("arm64"),
        "x86_64" => Some("x86_64"),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn macos_dylib_has_arch(path: &Path, expected_arch: &str) -> bool {
    let Ok(output) = Command::new("lipo").arg("-archs").arg(path).output() else {
        return true;
    };
    if !output.status.success() {
        return true;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.split_whitespace().any(|arch| arch == expected_arch)
}

#[cfg(target_os = "macos")]
fn macos_dylib_is_loadable(path: &Path) -> bool {
    let Ok(output) = Command::new("otool").arg("-hV").arg(path).output() else {
        return true;
    };
    if !output.status.success() {
        return true;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains("MH_DYLIB")
}

pub(super) async fn ensure_ort_init(app: &AppHandle) -> Result<(), String> {
    if ORT_INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    #[cfg(target_os = "android")]
    {
        if let Err(err) = ort::util::preload_dylib("libonnxruntime.so") {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Failed to preload Android ONNX Runtime (libonnxruntime.so): {}",
                    err
                ),
            ));
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let dylib_path = resolve_or_download_onnxruntime(app).await?;
        std::env::set_var("ORT_DYLIB_PATH", &dylib_path);
        #[cfg(target_os = "macos")]
        {
            if let Some(ort_dir) = dylib_path.parent() {
                preload_macos_provider_dylibs(ort_dir);
            }
        }
        if let Err(err) = ort::util::preload_dylib(&dylib_path) {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to preload ONNX Runtime library: {}", err),
            ));
        }
    }

    let init_result = std::panic::catch_unwind(|| {
        #[cfg(target_os = "android")]
        {
            ort::init_from("libonnxruntime.so")
                .with_name("lettuce-embedding")
                .commit()
        }
        #[cfg(not(target_os = "android"))]
        {
            ort::init().with_name("lettuce-embedding").commit()
        }
    })
    .map_err(|panic_payload| {
        let panic_msg = panic_payload_to_string(&*panic_payload);
        crate::utils::log_error(
            app,
            "embedding_debug",
            format!("ONNX Runtime init panic detail: {}", panic_msg),
        );
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("ONNX Runtime initialization panicked: {}", panic_msg),
        )
    })?;

    let init_ok = init_result.into_init_result().map_err(|err| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to initialize ONNX Runtime: {}", err),
        )
    })?;

    if !init_ok {
        log_info(
            app,
            "embedding_debug",
            "ONNX Runtime already initialized; continuing",
        );
    }

    ORT_INITIALIZED.store(true, Ordering::Release);
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn resolve_or_download_onnxruntime(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(value) = std::env::var("ORT_DYLIB_PATH") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            let path = Path::new(trimmed);
            if is_nonempty_file(path) {
                if cfg!(target_os = "windows") {
                    let shared = path
                        .parent()
                        .map(|dir| dir.join("onnxruntime_providers_shared.dll"));
                    if let Some(shared_path) = shared {
                        if shared_path.exists() {
                            return Ok(path.to_path_buf());
                        }
                    }
                } else if cfg!(target_os = "macos") {
                    if is_valid_macos_ort_dylib(path) {
                        if let Some(ort_dir) = path.parent() {
                            log_missing_macos_provider_dylibs(app, ort_dir, path);
                            return Ok(path.to_path_buf());
                        } else {
                            crate::utils::log_warn(
                                app,
                                "embedding_debug",
                                format!(
                                    "ORT_DYLIB_PATH is set to {} but parent directory is unavailable; attempting runtime download fallback.",
                                    path.display()
                                ),
                            );
                        }
                    } else {
                        crate::utils::log_warn(
                            app,
                            "embedding_debug",
                            format!(
                                "ORT_DYLIB_PATH points to {} but it is not a usable macOS MH_DYLIB for this arch; ignoring it.",
                                path.display()
                            ),
                        );
                    }
                } else {
                    return Ok(path.to_path_buf());
                }
            } else if path.exists() {
                crate::utils::log_warn(
                    app,
                    "embedding_debug",
                    format!(
                        "ORT_DYLIB_PATH points to an empty or invalid file at {}; ignoring it.",
                        path.display()
                    ),
                );
            }
        }
    }

    if let Some(path) = resolve_bundled_onnxruntime(app) {
        std::env::set_var("ORT_DYLIB_PATH", &path);
        if cfg!(target_os = "macos") {
            if let Some(ort_dir) = path.parent() {
                log_missing_macos_provider_dylibs(app, ort_dir, &path);
            }
        }
        log_info(
            app,
            "embedding_debug",
            format!("Using bundled ONNX Runtime from {}", path.display()),
        );
        return Ok(path);
    }

    let lettuce_dir = crate::utils::ensure_lettuce_dir(app)?;
    let ort_dir = lettuce_dir.join("onnxruntime");
    fs::create_dir_all(&ort_dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let download_infos = ort_download_info_candidates()?;
    let dest_path = ort_dir.join(download_infos[0].lib_name);
    if is_nonempty_file(&dest_path) {
        if cfg!(target_os = "windows") {
            let shared = ort_dir.join("onnxruntime_providers_shared.dll");
            if shared.exists() {
                return Ok(dest_path);
            }
        } else if cfg!(target_os = "macos") {
            if is_valid_macos_ort_dylib(&dest_path) {
                log_missing_macos_provider_dylibs(app, &ort_dir, &dest_path);
                return Ok(dest_path);
            }
            let _ = fs::remove_file(&dest_path);
        } else {
            return Ok(dest_path);
        }
    } else if dest_path.exists() {
        let _ = fs::remove_file(&dest_path);
    }

    let client = reqwest::Client::new();
    let mut last_error = None;

    for download_info in &download_infos {
        let response = match client.get(&download_info.archive_url).send().await {
            Ok(response) => response,
            Err(e) => {
                let err = crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!(
                        "Failed to download ONNX Runtime archive {}: {}",
                        download_info.archive_url, e
                    ),
                );
                crate::utils::log_warn(app, "embedding_debug", err.clone());
                last_error = Some(err);
                continue;
            }
        };
        if !response.status().is_success() {
            let err = crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Failed to download ONNX Runtime archive {}: HTTP {}",
                    download_info.archive_url,
                    response.status()
                ),
            );
            crate::utils::log_warn(app, "embedding_debug", err.clone());
            last_error = Some(err);
            continue;
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        match extract_onnxruntime_archive(download_info, &bytes, &dest_path, &ort_dir) {
            Ok(()) => {
                last_error = None;
                break;
            }
            Err(err) => {
                crate::utils::log_warn(
                    app,
                    "embedding_debug",
                    format!(
                        "Failed to extract ONNX Runtime archive {}: {}",
                        download_info.archive_url, err
                    ),
                );
                last_error = Some(err);
            }
        }
    }

    if let Some(err) = last_error {
        return Err(err);
    }

    if !is_nonempty_file(&dest_path) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "ONNX Runtime library not found after download: {}",
                dest_path.display()
            ),
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let shared = ort_dir.join("libonnxruntime_providers_shared.dylib");
        let coreml = ort_dir.join("libonnxruntime_providers_coreml.dylib");
        if !shared.exists() {
            crate::utils::log_warn(
                app,
                "embedding_debug",
                format!(
                    "Runtime-downloaded ONNX Runtime at {} is missing provider shared dylib; CoreML acceleration may be unavailable.",
                    dest_path.display()
                ),
            );
        }
        if !coreml.exists() {
            crate::utils::log_warn(
                app,
                "embedding_debug",
                format!(
                    "Runtime-downloaded ONNX Runtime at {} is missing CoreML provider dylib; embeddings will fall back to CPU.",
                    dest_path.display()
                ),
            );
        }
    }

    Ok(dest_path)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
struct OrtDownloadInfo {
    archive_url: String,
    lib_path_in_archive: String,
    lib_name: &'static str,
    lib_dir_in_archive: Option<String>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn ort_download_info_candidates() -> Result<Vec<OrtDownloadInfo>, String> {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    match (os, arch) {
        ("windows", "x86_64") => Ok(vec![OrtDownloadInfo {
            archive_url: format!(
                "https://github.com/microsoft/onnxruntime/releases/download/v{0}/onnxruntime-win-x64-{0}.zip",
                ORT_VERSION
            ),
            lib_path_in_archive: format!("onnxruntime-win-x64-{}/lib/onnxruntime.dll", ORT_VERSION),
            lib_name: "onnxruntime.dll",
            lib_dir_in_archive: Some(format!("onnxruntime-win-x64-{}/lib/", ORT_VERSION)),
        }]),
        ("linux", "x86_64") => Ok(vec![OrtDownloadInfo {
            archive_url: format!(
                "https://github.com/microsoft/onnxruntime/releases/download/v{0}/onnxruntime-linux-x64-{0}.tgz",
                ORT_VERSION
            ),
            lib_path_in_archive: format!(
                "onnxruntime-linux-x64-{}/lib/libonnxruntime.so.{}",
                ORT_VERSION, ORT_VERSION
            ),
            lib_name: "libonnxruntime.so",
            lib_dir_in_archive: None,
        }]),
        ("macos", "aarch64") | ("macos", "x86_64") => {
            let mut candidates = Vec::new();

            if let Some(archive_name) = macos_archive_name(arch) {
                candidates.push(OrtDownloadInfo {
                    archive_url: format!(
                        "https://github.com/microsoft/onnxruntime/releases/download/v{0}/onnxruntime-osx-{1}-{0}.tgz",
                        ORT_VERSION, archive_name
                    ),
                    lib_path_in_archive: format!(
                        "onnxruntime-osx-{}-{}/lib/{}",
                        archive_name,
                        ORT_VERSION,
                        macos_primary_dylib_name()
                    ),
                    lib_name: leaked_macos_primary_dylib_name(),
                    lib_dir_in_archive: Some(format!(
                        "onnxruntime-osx-{}-{}/lib/",
                        archive_name, ORT_VERSION
                    )),
                });
            }

            candidates.push(OrtDownloadInfo {
                archive_url: format!(
                    "https://github.com/microsoft/onnxruntime/releases/download/v{0}/onnxruntime-osx-universal2-{0}.tgz",
                    ORT_VERSION
                ),
                lib_path_in_archive: format!(
                    "onnxruntime-osx-universal2-{}/lib/{}",
                    ORT_VERSION,
                    macos_primary_dylib_name()
                ),
                lib_name: leaked_macos_primary_dylib_name(),
                lib_dir_in_archive: Some(format!("onnxruntime-osx-universal2-{}/lib/", ORT_VERSION)),
            });

            Ok(candidates)
        }
        _ => Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unsupported platform for ONNX Runtime: {} {}", os, arch),
        )),
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn extract_onnxruntime_archive(
    download_info: &OrtDownloadInfo,
    bytes: &[u8],
    dest_path: &Path,
    ort_dir: &Path,
) -> Result<(), String> {
    let reader = Cursor::new(bytes);
    if download_info.archive_url.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(reader)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if let Some(prefix) = download_info.lib_dir_in_archive.as_deref() {
            for index in 0..zip.len() {
                let mut file = zip
                    .by_index(index)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                let name = file.name().to_string();
                if !name.starts_with(prefix) || !name.ends_with(".dll") {
                    continue;
                }
                let filename = Path::new(&name)
                    .file_name()
                    .ok_or_else(|| {
                        crate::utils::err_msg(
                            module_path!(),
                            line!(),
                            format!("Invalid ONNX Runtime entry: {}", name),
                        )
                    })?
                    .to_string_lossy()
                    .to_string();
                let out_path = ort_dir.join(filename);
                let mut outfile = fs::File::create(&out_path)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                std::io::copy(&mut file, &mut outfile)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }
        } else {
            let mut file = zip
                .by_name(&download_info.lib_path_in_archive)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let mut outfile = fs::File::create(dest_path)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
        return Ok(());
    }

    if download_info.archive_url.ends_with(".tgz") {
        let tar = flate2::read::GzDecoder::new(reader);
        let mut archive = tar::Archive::new(tar);
        let mut extracted_files: HashMap<String, Vec<u8>> = HashMap::new();
        let mut linked_files: Vec<(String, String)> = Vec::new();
        for entry in archive
            .entries()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        {
            let mut entry =
                entry.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let path = entry
                .path()
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
                .to_string_lossy()
                .into_owned();
            if let Some(prefix) = download_info.lib_dir_in_archive.as_deref() {
                if let Some(relative_path) = path.strip_prefix(prefix) {
                    if relative_path.contains('/') || !relative_path.ends_with(".dylib") {
                        continue;
                    }
                    let filename = Path::new(&path)
                        .file_name()
                        .ok_or_else(|| {
                            crate::utils::err_msg(
                                module_path!(),
                                line!(),
                                format!("Invalid ONNX Runtime entry: {}", path),
                            )
                        })?
                        .to_string_lossy()
                        .to_string();
                    let entry_type = entry.header().entry_type();
                    if entry_type.is_symlink() || entry_type.is_hard_link() {
                        let target_name = entry
                            .link_name()
                            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
                            .and_then(|target| {
                                target
                                    .file_name()
                                    .map(|name| name.to_string_lossy().to_string())
                            })
                            .ok_or_else(|| {
                                crate::utils::err_msg(
                                    module_path!(),
                                    line!(),
                                    format!("Invalid ONNX Runtime linked entry: {}", path),
                                )
                            })?;
                        linked_files.push((filename, target_name));
                        continue;
                    }

                    let mut contents = Vec::new();
                    std::io::copy(&mut entry, &mut contents)
                        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                    extracted_files.insert(filename, contents);
                }
            } else if path == download_info.lib_path_in_archive {
                let mut outfile = fs::File::create(dest_path)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                return Ok(());
            }
        }
        if download_info.lib_dir_in_archive.is_some() {
            for (filename, contents) in &extracted_files {
                fs::write(ort_dir.join(filename), contents)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }

            for (filename, target_name) in linked_files {
                let target_contents = extracted_files.get(&target_name).ok_or_else(|| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!(
                            "Linked ONNX Runtime dylib '{}' points to missing target '{}'",
                            filename, target_name
                        ),
                    )
                })?;
                fs::write(ort_dir.join(filename), target_contents)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }
        }
        if download_info.lib_dir_in_archive.is_some() && is_nonempty_file(dest_path) {
            return Ok(());
        }
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Could not find {} in archive",
                download_info.lib_path_in_archive
            ),
        ));
    }

    Err(crate::utils::err_msg(
        module_path!(),
        line!(),
        format!("Unsupported archive type: {}", download_info.archive_url),
    ))
}

#[cfg(target_os = "macos")]
fn preload_macos_provider_dylibs(ort_dir: &Path) {
    for name in [
        "libonnxruntime_providers_shared.dylib",
        "libonnxruntime_providers_coreml.dylib",
    ] {
        let path = ort_dir.join(name);
        if path.exists() {
            let _ = ort::util::preload_dylib(&path);
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn log_missing_macos_provider_dylibs(app: &AppHandle, ort_dir: &Path, dylib_path: &Path) {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, ort_dir, dylib_path);
    }

    #[cfg(target_os = "macos")]
    {
        let shared = ort_dir.join("libonnxruntime_providers_shared.dylib");
        if !shared.exists() {
            crate::utils::log_warn(
            app,
            "embedding_debug",
            format!(
                "ONNX Runtime found at {} but provider shared dylib is missing; embeddings may rely on runtime-fetched providers.",
                dylib_path.display()
            ),
        );
        }

        let coreml = ort_dir.join("libonnxruntime_providers_coreml.dylib");
        if !coreml.exists() {
            crate::utils::log_warn(
            app,
            "embedding_debug",
            format!(
                "ONNX Runtime found at {} but CoreML provider dylib is missing; embeddings will fall back to CPU.",
                dylib_path.display()
            ),
        );
        }
    }
}

#[cfg(target_os = "macos")]
fn is_valid_macos_ort_dylib(path: &Path) -> bool {
    if !is_nonempty_file(path) {
        return false;
    }

    let arch_ok = current_macos_arch()
        .map(|expected_arch| macos_dylib_has_arch(path, expected_arch))
        .unwrap_or(true);

    arch_ok && macos_dylib_is_loadable(path)
}

#[cfg(all(
    not(target_os = "macos"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn is_valid_macos_ort_dylib(path: &Path) -> bool {
    is_nonempty_file(path)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn resolve_bundled_onnxruntime(app: &AppHandle) -> Option<PathBuf> {
    let candidates = if cfg!(target_os = "windows") {
        vec![
            "onnxruntime/onnxruntime.dll".to_string(),
            "onnxruntime.dll".to_string(),
        ]
    } else if cfg!(target_os = "macos") {
        let versioned = macos_primary_dylib_name();
        vec![
            format!("onnxruntime/{}", versioned),
            versioned,
            "onnxruntime/libonnxruntime.dylib".to_string(),
            "libonnxruntime.dylib".to_string(),
        ]
    } else {
        vec![
            "onnxruntime/libonnxruntime.so".to_string(),
            "libonnxruntime.so".to_string(),
        ]
    };

    for candidate in candidates {
        let Ok(path) = app.path().resolve(&candidate, BaseDirectory::Resource) else {
            continue;
        };
        if cfg!(target_os = "macos") {
            if is_valid_macos_ort_dylib(&path) {
                return Some(path);
            }
        } else if is_nonempty_file(&path) {
            return Some(path);
        }
    }

    None
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn is_nonempty_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

trait IntoInitResult {
    fn into_init_result(self) -> Result<bool, String>;
}

impl IntoInitResult for bool {
    fn into_init_result(self) -> Result<bool, String> {
        Ok(self)
    }
}

impl<E: std::fmt::Display> IntoInitResult for Result<bool, E> {
    fn into_init_result(self) -> Result<bool, String> {
        self.map_err(|err| err.to_string())
    }
}
