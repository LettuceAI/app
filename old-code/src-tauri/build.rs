use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::process::Command;

const ORT_VERSION: &str = "1.22.0";
const DEFAULT_KOKORO_ESPEAK_ANDROID_BUNDLE_URL: &str =
    "https://github.com/LettuceAI/app/releases/download/espeak-android-bundle-v2/kokoro-espeak-android-bundle.tar.gz";

fn macos_primary_dylib_name() -> String {
    format!("libonnxruntime.{}.dylib", ORT_VERSION)
}

fn macos_archive_name(target_arch: &str) -> Option<&'static str> {
    match target_arch {
        "arm64" => Some("arm64"),
        "x86_64" => Some("x86_64"),
        _ => None,
    }
}

fn macos_archive_candidates(target_arch: &str) -> Vec<(String, String)> {
    let mut candidates = Vec::new();

    if let Some(archive_name) = macos_archive_name(target_arch) {
        candidates.push((
            format!(
                "https://github.com/microsoft/onnxruntime/releases/download/v{0}/onnxruntime-osx-{1}-{0}.tgz",
                ORT_VERSION, archive_name
            ),
            format!("onnxruntime-osx-{}-{}/lib/", archive_name, ORT_VERSION),
        ));
    }

    candidates.push((
        format!(
            "https://github.com/microsoft/onnxruntime/releases/download/v{0}/onnxruntime-osx-universal2-{0}.tgz",
            ORT_VERSION
        ),
        format!("onnxruntime-osx-universal2-{}/lib/", ORT_VERSION),
    ));

    candidates
}

fn main() {
    println!("cargo:rerun-if-env-changed=ORT_LIB_LOCATION");
    println!("cargo:rerun-if-env-changed=KOKORO_ESPEAK_ANDROID_BUNDLE_URL");
    println!("cargo:rerun-if-env-changed=KOKORO_ESPEAK_ANDROID_BUNDLE_PATH");
    println!("cargo:rerun-if-env-changed=KOKORO_ANDROID_BRIDGE_CLASS");
    println!("cargo:rerun-if-changed=tauri.conf.json");

    export_android_bridge_class();

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target_os.as_str() {
        "android" => {
            println!("cargo:warning=Detected Android build, checking ONNX Runtime libraries...");
            setup_android_libs().expect("Failed to setup Android libraries");
            setup_android_espeak_bundle().expect("Failed to setup Android eSpeak NG bundle");
            validate_android_espeak_bundle().expect("Failed to validate Android eSpeak NG bundle");
            link_android_espeak_library().expect("Failed to link Android eSpeak NG library");
        }
        "ios" => {
            println!("cargo:warning=Detected iOS build.");
            match env::var("ORT_LIB_LOCATION") {
                Ok(path) if !path.trim().is_empty() => {
                    println!("cargo:warning=Using iOS ONNX Runtime from {}", path);
                }
                _ => println!(
                    "cargo:warning=ORT_LIB_LOCATION is not set. Xcode builds should invoke scripts/run-tauri-ios-xcode-onnxruntime.sh before cargo. Direct cargo iOS builds should set ORT_LIB_LOCATION to an extracted ONNX Runtime slice directory."
                ),
            }
        }
        "macos" => {
            println!("cargo:warning=Detected macOS build, preparing ONNX Runtime library...");
            setup_macos_libs().expect("Failed to setup macOS ONNX Runtime library");
        }
        "linux" => {
            println!(
                "cargo:warning=Detected Linux desktop build, preparing ONNX Runtime library..."
            );
            setup_linux_libs().expect("Failed to setup Linux ONNX Runtime library");
        }
        "windows" => {
            println!(
                "cargo:warning=Detected Windows desktop build, preparing ONNX Runtime library..."
            );
            setup_windows_libs().expect("Failed to setup Windows ONNX Runtime library");
        }
        _ => {
            println!(
                "cargo:warning=Detected desktop build for target OS '{}'; ensuring ONNX Runtime resource directory exists.",
                target_os
            );
            ensure_resource_dir().expect("Failed to create ONNX Runtime resource directory");
        }
    }

    tauri_build::build();
}

fn export_android_bridge_class() {
    const BRIDGE_CLASS_NAME: &str = "KokoroPhonemizerBridge";
    let value = env::var("KOKORO_ANDROID_BRIDGE_CLASS")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            let identifier = read_tauri_identifier().unwrap_or_else(|err| {
                println!(
                    "cargo:warning=Could not read tauri.conf.json identifier ({err}); falling back to com.lettuceai.app for the Kokoro JNI bridge."
                );
                "com.lettuceai.app".to_string()
            });
            // Dotted form — used with ClassLoader.loadClass() at runtime, since
            // env.find_class() can't see app classes from a Tauri jni_handle context.
            format!("{}.{}", identifier, BRIDGE_CLASS_NAME)
        });
    println!("cargo:rustc-env=KOKORO_ANDROID_BRIDGE_CLASS={value}");
}

fn read_tauri_identifier() -> anyhow::Result<String> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let path = manifest_dir.join("tauri.conf.json");
    let raw = fs::read_to_string(&path)?;
    let value: serde_json::Value = serde_json::from_str(&raw)?;
    let identifier = value
        .get("identifier")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| anyhow::anyhow!("tauri.conf.json is missing a top-level `identifier`"))?;
    Ok(identifier.to_string())
}

fn ensure_resource_dir() -> anyhow::Result<PathBuf> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let resource_dir = manifest_dir.join("onnxruntime");
    fs::create_dir_all(&resource_dir)?;
    Ok(resource_dir)
}

fn setup_android_libs() -> anyhow::Result<()> {
    let resource_dir = ensure_resource_dir()?;
    println!(
        "cargo:warning=Ensured ONNX Runtime resource dir at {:?} for Android build",
        resource_dir
    );

    let jni_libs_path = PathBuf::from("gen/android/app/src/main/jniLibs");

    let targets = vec![
        ("arm64-v8a", "jni/arm64-v8a/libonnxruntime.so"),
        ("armeabi-v7a", "jni/armeabi-v7a/libonnxruntime.so"),
        ("x86_64", "jni/x86_64/libonnxruntime.so"),
    ];

    let mut missing = false;
    for (arch, _) in &targets {
        let lib_path = jni_libs_path.join(arch).join("libonnxruntime.so");
        if !lib_path.exists() {
            missing = true;
            break;
        }
    }

    if !missing {
        println!("cargo:warning=ONNX Runtime libs already present.");
        return Ok(());
    }

    println!(
        "cargo:warning=Downloading ONNX Runtime Android v{}...",
        ORT_VERSION
    );
    let url = format!(
        "https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android/{0}/onnxruntime-android-{0}.aar",
        ORT_VERSION
    );

    let response = reqwest::blocking::get(&url)?.bytes()?;
    let reader = Cursor::new(response);
    let mut zip = zip::ZipArchive::new(reader)?;

    for (arch, internal_path) in targets {
        let dest_dir = jni_libs_path.join(arch);
        fs::create_dir_all(&dest_dir)?;

        let dest_file = dest_dir.join("libonnxruntime.so");

        match zip.by_name(internal_path) {
            Ok(mut file) => {
                let mut outfile = fs::File::create(&dest_file)?;
                io::copy(&mut file, &mut outfile)?;
                println!("cargo:warning=Extracted: {:?}", dest_file);
            }
            Err(_) => {
                println!(
                    "cargo:warning=Could not find {} in AAR, skipping...",
                    internal_path
                );
            }
        }
    }

    Ok(())
}

fn validate_android_espeak_bundle() -> anyhow::Result<()> {
    let native_targets = ["arm64-v8a", "armeabi-v7a", "x86", "x86_64"];
    let jni_libs_path = PathBuf::from("gen/android/app/src/main/jniLibs");
    let mut missing = Vec::new();

    for abi in native_targets {
        let path = jni_libs_path.join(abi).join("libttsespeak.so");
        if !path.is_file() {
            missing.push(path.display().to_string());
        }
    }

    let data_dir = PathBuf::from("gen/android/app/src/main/assets/kokoro/espeak-ng-data");
    let phontab = data_dir.join("phontab");
    if !data_dir.is_dir() || !phontab.is_file() {
        missing.push(data_dir.display().to_string());
    }

    if !missing.is_empty() {
        anyhow::bail!(
            "Android Kokoro requires bundled eSpeak NG. Add libttsespeak.so under each supported jniLibs ABI and add eSpeak data under assets/kokoro/espeak-ng-data. Missing:\n{}",
            missing.join("\n")
        );
    }

    println!("cargo:warning=Android eSpeak NG bundle is present.");
    Ok(())
}

fn link_android_espeak_library() -> anyhow::Result<()> {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let abi = match target_arch.as_str() {
        "aarch64" => "arm64-v8a",
        "arm" => "armeabi-v7a",
        "x86_64" => "x86_64",
        "x86" => "x86",
        _ => anyhow::bail!(
            "Unsupported Android architecture '{}' for eSpeak NG native library",
            target_arch
        ),
    };
    let lib_dir = PathBuf::from("gen/android/app/src/main/jniLibs").join(abi);
    let lib_path = lib_dir.join("libttsespeak.so");
    if !lib_path.is_file() {
        anyhow::bail!("Missing Android eSpeak NG library: {}", lib_path.display());
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=ttsespeak");
    Ok(())
}

fn setup_android_espeak_bundle() -> anyhow::Result<()> {
    if android_espeak_bundle_complete() {
        println!("cargo:warning=Android eSpeak NG bundle already present.");
        return Ok(());
    }

    if let Ok(path) = env::var("KOKORO_ESPEAK_ANDROID_BUNDLE_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            println!(
                "cargo:warning=Extracting Android eSpeak NG bundle from {}",
                trimmed
            );
            extract_android_espeak_bundle(&fs::read(trimmed)?, trimmed)?;
            return Ok(());
        }
    }

    if let Ok(url) = env::var("KOKORO_ESPEAK_ANDROID_BUNDLE_URL") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            println!(
                "cargo:warning=Downloading Android eSpeak NG bundle from {}",
                trimmed
            );
            let response = reqwest::blocking::get(trimmed)?.error_for_status()?;
            let bytes = response.bytes()?;
            extract_android_espeak_bundle(&bytes, trimmed)?;
            return Ok(());
        }
    }

    println!(
        "cargo:warning=Downloading Android eSpeak NG bundle from default URL {}",
        DEFAULT_KOKORO_ESPEAK_ANDROID_BUNDLE_URL
    );
    let response =
        reqwest::blocking::get(DEFAULT_KOKORO_ESPEAK_ANDROID_BUNDLE_URL)?.error_for_status()?;
    let bytes = response.bytes()?;
    extract_android_espeak_bundle(&bytes, DEFAULT_KOKORO_ESPEAK_ANDROID_BUNDLE_URL)?;
    Ok(())
}

fn android_espeak_bundle_complete() -> bool {
    let native_targets = ["arm64-v8a", "armeabi-v7a", "x86", "x86_64"];
    let jni_libs_path = PathBuf::from("gen/android/app/src/main/jniLibs");
    let data_dir = PathBuf::from("gen/android/app/src/main/assets/kokoro/espeak-ng-data");

    native_targets
        .iter()
        .all(|abi| jni_libs_path.join(abi).join("libttsespeak.so").is_file())
        && data_dir.join("phontab").is_file()
}

fn extract_android_espeak_bundle(bytes: &[u8], source_name: &str) -> anyhow::Result<()> {
    if source_name.ends_with(".zip") {
        return extract_android_espeak_zip(bytes);
    }
    if source_name.ends_with(".tar.gz") || source_name.ends_with(".tgz") {
        return extract_android_espeak_tgz(bytes);
    }

    extract_android_espeak_zip(bytes).or_else(|zip_err| {
        extract_android_espeak_tgz(bytes).map_err(|tgz_err| {
            anyhow::anyhow!(
                "Failed to extract Android eSpeak NG bundle as zip ({zip_err}) or tar.gz ({tgz_err})"
            )
        })
    })
}

fn extract_android_espeak_zip(bytes: &[u8]) -> anyhow::Result<()> {
    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().replace('\\', "/");
        let Some(dest_path) = android_espeak_archive_dest(&name) else {
            continue;
        };
        if entry.is_dir() {
            fs::create_dir_all(dest_path)?;
            continue;
        }
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut outfile = fs::File::create(dest_path)?;
        io::copy(&mut entry, &mut outfile)?;
    }

    Ok(())
}

fn extract_android_espeak_tgz(bytes: &[u8]) -> anyhow::Result<()> {
    let reader = Cursor::new(bytes);
    let tar = flate2::read::GzDecoder::new(reader);
    let mut archive = tar::Archive::new(tar);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().replace('\\', "/");
        let Some(dest_path) = android_espeak_archive_dest(&name) else {
            continue;
        };
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(dest_path)?;
            continue;
        }
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut outfile = fs::File::create(dest_path)?;
        io::copy(&mut entry, &mut outfile)?;
    }

    Ok(())
}

fn android_espeak_archive_dest(name: &str) -> Option<PathBuf> {
    let normalized = name.trim_start_matches("./");
    let relative = normalized.trim_start_matches('/');
    let relative = relative
        .find("jniLibs/")
        .map(|index| &relative[index..])
        .or_else(|| {
            relative
                .find("assets/kokoro/espeak-ng-data/")
                .map(|index| &relative[index..])
        })
        .or_else(|| {
            relative
                .find("espeak-ng-data/")
                .map(|index| &relative[index..])
        })
        .unwrap_or(relative);

    if let Some(path) = relative.strip_prefix("jniLibs/") {
        return Some(PathBuf::from("gen/android/app/src/main/jniLibs").join(path));
    }
    if let Some(path) = relative.strip_prefix("assets/kokoro/espeak-ng-data/") {
        return Some(
            PathBuf::from("gen/android/app/src/main/assets/kokoro/espeak-ng-data").join(path),
        );
    }
    if let Some(path) = relative.strip_prefix("espeak-ng-data/") {
        return Some(
            PathBuf::from("gen/android/app/src/main/assets/kokoro/espeak-ng-data").join(path),
        );
    }

    None
}

fn setup_linux_libs() -> anyhow::Result<()> {
    let resource_dir = ensure_resource_dir()?;

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_arch != "x86_64" {
        println!(
            "cargo:warning=Unsupported Linux architecture '{}' for bundled ONNX Runtime; runtime fetch fallback will be used.",
            target_arch
        );
        return Ok(());
    }

    let target_path = resource_dir.join("libonnxruntime.so");

    if let Ok(path) = env::var("ORT_LIB_LOCATION") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            let src_dir = Path::new(trimmed);
            copy_linux_so_from_dir(src_dir, &target_path)?;
            println!(
                "cargo:warning=ORT_LIB_LOCATION is set for Linux build ({}); copied ONNX Runtime library into {:?}.",
                trimmed, target_path
            );
            return Ok(());
        }
    }

    if is_nonempty_file(&target_path) {
        println!(
            "cargo:warning=Linux ONNX Runtime already present at {:?}",
            target_path
        );
        return Ok(());
    }
    if target_path.exists() {
        println!(
            "cargo:warning=Existing Linux ONNX Runtime at {:?} is empty or corrupt; re-downloading.",
            target_path
        );
        fs::remove_file(&target_path)?;
    }

    let archive_url = format!(
        "https://github.com/microsoft/onnxruntime/releases/download/v{0}/onnxruntime-linux-x64-{0}.tgz",
        ORT_VERSION
    );
    let lib_path_in_archive = format!(
        "onnxruntime-linux-x64-{}/lib/libonnxruntime.so",
        ORT_VERSION
    );

    println!(
        "cargo:warning=Downloading ONNX Runtime Linux v{}...",
        ORT_VERSION
    );
    let response = reqwest::blocking::get(&archive_url)?
        .error_for_status()?
        .bytes()?;
    extract_tgz_single_file(&response, &lib_path_in_archive, &target_path)?;

    if !is_nonempty_file(&target_path) {
        anyhow::bail!(
            "ONNX Runtime library missing or empty after Linux download: {}",
            target_path.display()
        );
    }

    println!("cargo:warning=Extracted: {:?}", target_path);
    Ok(())
}

fn copy_linux_so_from_dir(src_dir: &Path, dest_path: &Path) -> anyhow::Result<()> {
    if !src_dir.exists() {
        anyhow::bail!("ORT_LIB_LOCATION does not exist: {}", src_dir.display());
    }
    if !src_dir.is_dir() {
        anyhow::bail!("ORT_LIB_LOCATION is not a directory: {}", src_dir.display());
    }

    let exact = src_dir.join("libonnxruntime.so");
    if exact.exists() {
        fs::copy(&exact, dest_path)?;
        return Ok(());
    }

    let mut fallback: Option<PathBuf> = None;
    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with("libonnxruntime.so") {
            fallback = Some(path);
            break;
        }
    }

    if let Some(source) = fallback {
        fs::copy(source, dest_path)?;
        return Ok(());
    }

    anyhow::bail!(
        "ORT_LIB_LOCATION is missing libonnxruntime.so (or versioned variant): {}",
        src_dir.display()
    )
}

fn setup_windows_libs() -> anyhow::Result<()> {
    let resource_dir = ensure_resource_dir()?;

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let archive_arch = match target_arch.as_str() {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => {
            println!(
                "cargo:warning=Unsupported Windows architecture '{}' for bundled ONNX Runtime; runtime fetch fallback will be used.",
                target_arch
            );
            return Ok(());
        }
    };

    let target_path = resource_dir.join("onnxruntime.dll");

    if let Ok(path) = env::var("ORT_LIB_LOCATION") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            copy_windows_dll_from_dir(Path::new(trimmed), &target_path)?;
            println!(
                "cargo:warning=ORT_LIB_LOCATION is set for Windows build ({}); copied ONNX Runtime library into {:?}.",
                trimmed, target_path
            );
            return Ok(());
        }
    }

    if is_nonempty_file(&target_path) {
        println!(
            "cargo:warning=Windows ONNX Runtime already present at {:?}",
            target_path
        );
        return Ok(());
    }
    if target_path.exists() {
        println!(
            "cargo:warning=Existing Windows ONNX Runtime at {:?} is empty or corrupt; re-downloading.",
            target_path
        );
        fs::remove_file(&target_path)?;
    }

    let archive_url = format!(
        "https://github.com/microsoft/onnxruntime/releases/download/v{0}/onnxruntime-win-{1}-{0}.zip",
        ORT_VERSION, archive_arch
    );
    let dll_path_in_archive = format!(
        "onnxruntime-win-{}-{}/lib/onnxruntime.dll",
        archive_arch, ORT_VERSION
    );

    println!(
        "cargo:warning=Downloading ONNX Runtime Windows v{}...",
        ORT_VERSION
    );
    let response = reqwest::blocking::get(&archive_url)?
        .error_for_status()?
        .bytes()?;
    extract_zip_single_file(&response, &dll_path_in_archive, &target_path)?;

    if !is_nonempty_file(&target_path) {
        anyhow::bail!(
            "ONNX Runtime library missing or empty after Windows download: {}",
            target_path.display()
        );
    }

    println!("cargo:warning=Extracted: {:?}", target_path);
    Ok(())
}

fn copy_windows_dll_from_dir(src_dir: &Path, dest_path: &Path) -> anyhow::Result<()> {
    if !src_dir.exists() {
        anyhow::bail!("ORT_LIB_LOCATION does not exist: {}", src_dir.display());
    }
    if !src_dir.is_dir() {
        anyhow::bail!("ORT_LIB_LOCATION is not a directory: {}", src_dir.display());
    }

    let exact = src_dir.join("onnxruntime.dll");
    if exact.exists() {
        fs::copy(&exact, dest_path)?;
        return Ok(());
    }

    anyhow::bail!(
        "ORT_LIB_LOCATION is missing onnxruntime.dll: {}",
        src_dir.display()
    )
}

fn setup_macos_libs() -> anyhow::Result<()> {
    let resource_dir = ensure_resource_dir()?;

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let expected_arch = match target_arch.as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        _ => {
            println!(
                "cargo:warning=Unsupported macOS architecture '{}' for bundled ONNX Runtime; runtime fetch fallback will be used.",
                target_arch
            );
            return Ok(());
        }
    };

    if let Ok(path) = env::var("ORT_LIB_LOCATION") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            copy_macos_dylibs_from_dir(Path::new(trimmed), &resource_dir)?;
            validate_macos_ort_dylibs(&resource_dir, expected_arch, false)?;
            println!(
                "cargo:warning=ORT_LIB_LOCATION is set for macOS build ({}); copied ONNX Runtime dylibs into {:?}.",
                trimmed, resource_dir
            );
            return Ok(());
        }
    }

    let dylib_path = resource_dir.join(macos_primary_dylib_name());
    let shared_path = resource_dir.join("libonnxruntime_providers_shared.dylib");
    let coreml_path = resource_dir.join("libonnxruntime_providers_coreml.dylib");
    if dylib_path.exists() || resource_dir.join("libonnxruntime.dylib").exists() {
        match validate_macos_ort_dylibs(&resource_dir, expected_arch, false) {
            Ok(()) => {
                if !shared_path.exists() {
                    println!(
                        "cargo:warning=macOS ONNX Runtime provider shared dylib is missing; embeddings may rely on runtime-fetched providers."
                    );
                }
                if !coreml_path.exists() {
                    println!(
                        "cargo:warning=macOS ONNX Runtime CoreML provider dylib is missing; runtime will use CPU fallback for embeddings."
                    );
                }
                println!(
                    "cargo:warning=macOS ONNX Runtime already present at {:?}",
                    dylib_path
                );
                return Ok(());
            }
            Err(err) => {
                println!(
                    "cargo:warning=Existing macOS ONNX Runtime dylibs are invalid for target arch '{}': {}. Refreshing download.",
                    expected_arch, err
                );
            }
        }
    }

    let mut last_error: Option<anyhow::Error> = None;
    for (archive_url, lib_dir_in_archive) in macos_archive_candidates(expected_arch) {
        println!(
            "cargo:warning=Downloading ONNX Runtime macOS v{} (target {}, archive {})...",
            ORT_VERSION, expected_arch, archive_url
        );

        let response = match reqwest::blocking::get(&archive_url) {
            Ok(response) => response,
            Err(err) => {
                println!(
                    "cargo:warning=Failed to fetch macOS ONNX Runtime archive '{}': {}",
                    archive_url, err
                );
                last_error = Some(err.into());
                continue;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            println!(
                "cargo:warning=macOS ONNX Runtime archive '{}' returned HTTP {}",
                archive_url, status
            );
            last_error = Some(anyhow::anyhow!(
                "macOS ONNX Runtime archive '{}' returned HTTP {}",
                archive_url,
                status
            ));
            continue;
        }

        let response = response.bytes()?;
        if let Err(err) = extract_tgz_dylibs_from_dir(&response, &lib_dir_in_archive, &resource_dir)
        {
            println!(
                "cargo:warning=Failed to extract macOS ONNX Runtime archive '{}': {}",
                archive_url, err
            );
            last_error = Some(err);
            continue;
        }

        match validate_macos_ort_dylibs(&resource_dir, expected_arch, false) {
            Ok(()) => {
                last_error = None;
                break;
            }
            Err(err) => {
                println!(
                    "cargo:warning=Downloaded macOS ONNX Runtime archive '{}' is invalid for target arch '{}': {}",
                    archive_url, expected_arch, err
                );
                last_error = Some(err);
            }
        }
    }

    if let Some(err) = last_error {
        return Err(err);
    }
    if !shared_path.exists() {
        println!(
            "cargo:warning=Downloaded macOS ONNX Runtime does not include libonnxruntime_providers_shared.dylib; embeddings may rely on runtime-fetched providers."
        );
    }
    if !coreml_path.exists() {
        println!(
            "cargo:warning=Downloaded macOS ONNX Runtime does not include libonnxruntime_providers_coreml.dylib; runtime will fall back to CPU for embeddings."
        );
    }
    if dylib_path.exists() {
        println!("cargo:warning=Extracted: {:?}", dylib_path);
    }

    Ok(())
}

fn copy_macos_dylibs_from_dir(src_dir: &Path, dest_dir: &Path) -> anyhow::Result<()> {
    if !src_dir.exists() {
        anyhow::bail!("ORT_LIB_LOCATION does not exist: {}", src_dir.display());
    }
    if !src_dir.is_dir() {
        anyhow::bail!("ORT_LIB_LOCATION is not a directory: {}", src_dir.display());
    }

    let mut copied_count = 0usize;
    let mut has_main_dylib = false;
    let mut has_shared_provider = false;
    let mut has_coreml_provider = false;
    let primary_name = macos_primary_dylib_name();

    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("dylib") {
            continue;
        }

        let Some(file_name) = path.file_name() else {
            continue;
        };
        let dest_path = dest_dir.join(file_name);
        fs::copy(&path, &dest_path)?;
        copied_count += 1;

        match file_name.to_string_lossy().as_ref() {
            "libonnxruntime.dylib" => has_main_dylib = true,
            "libonnxruntime_providers_shared.dylib" => has_shared_provider = true,
            "libonnxruntime_providers_coreml.dylib" => has_coreml_provider = true,
            name if name == primary_name => has_main_dylib = true,
            _ => {}
        }
    }

    if copied_count == 0 {
        anyhow::bail!(
            "No .dylib files found in ORT_LIB_LOCATION: {}",
            src_dir.display()
        );
    }

    if !has_main_dylib {
        anyhow::bail!(
            "ORT_LIB_LOCATION is missing {} (or libonnxruntime.dylib): {}",
            primary_name,
            src_dir.display()
        );
    }

    if !has_shared_provider {
        println!(
            "cargo:warning=ORT_LIB_LOCATION does not include libonnxruntime_providers_shared.dylib; embeddings may rely on runtime-fetched providers."
        );
    }

    if !has_coreml_provider {
        println!(
            "cargo:warning=ORT_LIB_LOCATION does not include libonnxruntime_providers_coreml.dylib; runtime will use CPU fallback for embeddings."
        );
    }

    Ok(())
}

fn dylib_supports_arch(path: &Path, expected_arch: &str) -> anyhow::Result<bool> {
    let output = match Command::new("lipo").arg("-archs").arg(path).output() {
        Ok(out) => out,
        Err(err) => {
            println!(
                "cargo:warning=Unable to execute lipo for '{}': {}. Skipping arch validation.",
                path.display(),
                err
            );
            return Ok(true);
        }
    };

    if !output.status.success() {
        println!(
            "cargo:warning=lipo -archs failed for '{}'; skipping arch validation.",
            path.display()
        );
        return Ok(true);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.split_whitespace().any(|arch| arch == expected_arch))
}

fn validate_macos_ort_dylibs(
    dylib_dir: &Path,
    expected_arch: &str,
    require_coreml: bool,
) -> anyhow::Result<()> {
    let main = dylib_dir.join(macos_primary_dylib_name());
    let fallback_main = dylib_dir.join("libonnxruntime.dylib");
    let shared = dylib_dir.join("libonnxruntime_providers_shared.dylib");
    let coreml = dylib_dir.join("libonnxruntime_providers_coreml.dylib");
    let main = if main.exists() { main } else { fallback_main };

    if !main.exists() {
        anyhow::bail!(
            "Missing {} (or libonnxruntime.dylib) in {}",
            macos_primary_dylib_name(),
            dylib_dir.display()
        );
    }
    if fs::metadata(&main)?.len() == 0 {
        anyhow::bail!("Dylib '{}' is empty", main.display());
    }
    if require_coreml && !coreml.exists() {
        anyhow::bail!(
            "Missing libonnxruntime_providers_coreml.dylib in {}",
            dylib_dir.display()
        );
    }

    if !dylib_supports_arch(&main, expected_arch)? {
        anyhow::bail!(
            "Dylib '{}' does not include target arch '{}'",
            main.display(),
            expected_arch
        );
    }

    if shared.exists() && !dylib_supports_arch(&shared, expected_arch)? {
        anyhow::bail!(
            "Dylib '{}' does not include target arch '{}'",
            shared.display(),
            expected_arch
        );
    }
    if shared.exists() && fs::metadata(&shared)?.len() == 0 {
        anyhow::bail!("Dylib '{}' is empty", shared.display());
    }

    if coreml.exists() && !dylib_supports_arch(&coreml, expected_arch)? {
        anyhow::bail!(
            "Dylib '{}' does not include target arch '{}'",
            coreml.display(),
            expected_arch
        );
    }
    if coreml.exists() && fs::metadata(&coreml)?.len() == 0 {
        anyhow::bail!("Dylib '{}' is empty", coreml.display());
    }

    Ok(())
}

fn is_nonempty_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

fn write_extracted_file(reader: &mut impl io::Read, dest_path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = dest_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("ort-download");
    let tmp_path = dest_path.with_file_name(format!("{}.tmp", file_name));
    let mut outfile = fs::File::create(&tmp_path)?;
    let written = io::copy(reader, &mut outfile)?;
    drop(outfile);
    if written == 0 {
        let _ = fs::remove_file(&tmp_path);
        anyhow::bail!(
            "Extracted ONNX Runtime file is empty: {}",
            dest_path.display()
        );
    }
    fs::rename(&tmp_path, dest_path)?;
    Ok(())
}

fn extract_tgz_single_file(bytes: &[u8], entry_path: &str, dest_path: &Path) -> anyhow::Result<()> {
    let mut target = entry_path.to_string();

    for _ in 0..8 {
        let reader = Cursor::new(bytes);
        let tar = flate2::read::GzDecoder::new(reader);
        let mut archive = tar::Archive::new(tar);
        let mut next_target: Option<String> = None;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_string_lossy().replace('\\', "/");
            if path != target {
                continue;
            }

            let entry_type = entry.header().entry_type();
            if entry_type.is_symlink() || entry_type.is_hard_link() {
                let Some(link_name) = entry
                    .link_name()?
                    .map(|link| link.to_string_lossy().replace('\\', "/"))
                else {
                    anyhow::bail!("Unable to resolve link target for '{}' in archive", target);
                };
                next_target = Some(if link_name.contains('/') {
                    link_name
                } else {
                    match target.rfind('/') {
                        Some(index) => format!("{}/{}", &target[..index], link_name),
                        None => link_name,
                    }
                });
                break;
            }

            return write_extracted_file(&mut entry, dest_path);
        }

        match next_target {
            Some(resolved) => {
                println!(
                    "cargo:warning=Archive entry '{}' is a link; following to '{}'",
                    target, resolved
                );
                target = resolved;
            }
            None => anyhow::bail!("Could not find '{}' in ONNX Runtime archive", target),
        }
    }

    anyhow::bail!(
        "Too many link hops while resolving '{}' in ONNX Runtime archive",
        entry_path
    )
}

fn extract_zip_single_file(bytes: &[u8], entry_path: &str, dest_path: &Path) -> anyhow::Result<()> {
    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().replace('\\', "/");
        if name != entry_path {
            continue;
        }
        return write_extracted_file(&mut entry, dest_path);
    }

    anyhow::bail!("Could not find '{}' in ONNX Runtime archive", entry_path)
}

fn extract_tgz_dylibs_from_dir(
    bytes: &[u8],
    entry_dir: &str,
    dest_dir: &Path,
) -> anyhow::Result<()> {
    let reader = Cursor::new(bytes);
    let tar = flate2::read::GzDecoder::new(reader);
    let mut archive = tar::Archive::new(tar);
    let mut extracted_files: HashMap<String, Vec<u8>> = HashMap::new();
    let mut linked_files: Vec<(String, String)> = Vec::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().replace('\\', "/");
        let Some(relative_path) = path.strip_prefix(entry_dir) else {
            continue;
        };
        if relative_path.contains('/') || !relative_path.ends_with(".dylib") {
            continue;
        }
        let Some(filename) = Path::new(&path).file_name() else {
            continue;
        };
        let filename = filename.to_string_lossy().to_string();
        let entry_type = entry.header().entry_type();

        if entry_type.is_symlink() || entry_type.is_hard_link() {
            let Some(target_name) = entry.link_name()?.and_then(|target| {
                target
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
            }) else {
                anyhow::bail!("Unable to resolve linked dylib target for '{}'", path);
            };
            linked_files.push((filename, target_name));
            continue;
        }

        let mut contents = Vec::new();
        io::copy(&mut entry, &mut contents)?;
        extracted_files.insert(filename, contents);
    }

    for (filename, contents) in &extracted_files {
        fs::create_dir_all(dest_dir)?;
        fs::write(dest_dir.join(filename), contents)?;
    }

    for (filename, target_name) in linked_files {
        let Some(target_contents) = extracted_files.get(&target_name) else {
            anyhow::bail!(
                "Linked dylib '{}' points to missing target '{}'",
                filename,
                target_name
            );
        };
        fs::create_dir_all(dest_dir)?;
        fs::write(dest_dir.join(filename), target_contents)?;
    }

    if extracted_files.is_empty() {
        anyhow::bail!(
            "No .dylib entries found under '{}' in ONNX Runtime archive",
            entry_dir
        );
    }

    Ok(())
}
