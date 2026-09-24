use std::cell::Cell;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "lettuce-ort-{label}-{}-{nanos}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }

    fn join(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn never() -> bool {
    false
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("parent dir");
    std::fs::write(path, bytes).expect("write");
}

fn locations(root: &TempDir, resources: bool) -> OnnxRuntimeLocations {
    OnnxRuntimeLocations {
        install_dir: root.join("onnxruntime"),
        resource_dir: resources.then(|| root.join("resources")),
    }
}

#[test]
fn runtime_version_follows_the_ort_api_version() {
    assert!(ONNX_RUNTIME_VERSION.starts_with(&format!("1.{}.", ort::MINOR_VERSION)));
}

#[test]
fn archives_are_the_official_release_assets_per_platform() {
    let windows = onnx_runtime_archives(OnnxRuntimeHostOs::Windows, "x86_64").expect("windows");
    assert_eq!(windows.len(), 1);
    assert_eq!(
        windows[0].url,
        "https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/onnxruntime-win-x64-1.22.0.zip"
    );
    assert_eq!(
        windows[0].library_entry,
        "onnxruntime-win-x64-1.22.0/lib/onnxruntime.dll"
    );
    assert_eq!(windows[0].sibling_extension, Some("dll"));

    let linux = onnx_runtime_archives(OnnxRuntimeHostOs::Linux, "x86_64").expect("linux");
    assert_eq!(
        linux[0].url,
        "https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/onnxruntime-linux-x64-1.22.0.tgz"
    );
    assert_eq!(
        linux[0].library_entry,
        "onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so.1.22.0"
    );
    assert_eq!(linux[0].library_name, "libonnxruntime.so");
    assert_eq!(linux[0].sibling_extension, None);

    let mac = onnx_runtime_archives(OnnxRuntimeHostOs::MacOs, "aarch64").expect("mac");
    assert_eq!(
        mac.iter()
            .map(|archive| archive.file_name.as_str())
            .collect::<Vec<_>>(),
        [
            "onnxruntime-osx-arm64-1.22.0.tgz",
            "onnxruntime-osx-universal2-1.22.0.tgz"
        ]
    );
    assert_eq!(mac[1].library_name, "libonnxruntime.1.22.0.dylib");
    assert_eq!(
        mac[1].library_entry,
        "onnxruntime-osx-universal2-1.22.0/lib/libonnxruntime.1.22.0.dylib"
    );
    let intel = onnx_runtime_archives(OnnxRuntimeHostOs::MacOs, "x86_64").expect("intel");
    assert_eq!(intel[0].file_name, "onnxruntime-osx-x86_64-1.22.0.tgz");

    assert!(matches!(
        onnx_runtime_archives(OnnxRuntimeHostOs::Linux, "aarch64"),
        Err(OnnxRuntimeProvisionError::UnsupportedPlatform { .. })
    ));
}

#[test]
fn override_wins_over_bundled_and_downloaded_runtimes() {
    let root = TempDir::new("order");
    let custom = root.join("custom/libonnxruntime.so");
    write(&custom, b"override");
    write(
        &root.join("resources/onnxruntime/libonnxruntime.so"),
        b"bundled",
    );
    write(&root.join("onnxruntime/libonnxruntime.so"), b"downloaded");
    let locations = locations(&root, true);

    let resolved = resolve_installed_onnx_runtime(
        OnnxRuntimeHostOs::Linux,
        Some(custom.as_os_str()),
        &locations,
    )
    .expect("override");
    assert_eq!(resolved.source, OnnxRuntimeSource::Override);
    assert_eq!(resolved.library, custom);

    let resolved = resolve_installed_onnx_runtime(OnnxRuntimeHostOs::Linux, None, &locations)
        .expect("bundled");
    assert_eq!(resolved.source, OnnxRuntimeSource::Bundled);
    assert_eq!(
        resolved.library,
        root.join("resources/onnxruntime/libonnxruntime.so")
    );

    std::fs::remove_file(root.join("resources/onnxruntime/libonnxruntime.so")).expect("remove");
    let resolved = resolve_installed_onnx_runtime(OnnxRuntimeHostOs::Linux, None, &locations)
        .expect("downloaded");
    assert_eq!(resolved.source, OnnxRuntimeSource::Downloaded);
    assert_eq!(resolved.library, root.join("onnxruntime/libonnxruntime.so"));
}

#[test]
fn blank_empty_or_missing_overrides_fall_through() {
    let root = TempDir::new("override");
    write(&root.join("onnxruntime/libonnxruntime.so"), b"downloaded");
    let empty = root.join("empty.so");
    write(&empty, b"");
    let locations = locations(&root, false);
    for value in [
        OsStr::new("   "),
        empty.as_os_str(),
        OsStr::new("/definitely/missing/libonnxruntime.so"),
    ] {
        let resolved =
            resolve_installed_onnx_runtime(OnnxRuntimeHostOs::Linux, Some(value), &locations)
                .expect("downloaded");
        assert_eq!(resolved.source, OnnxRuntimeSource::Downloaded);
    }
    assert!(empty.exists(), "an override file is never deleted");

    let padded = root.join("onnxruntime/libonnxruntime.so");
    let padded = format!("  {}  ", padded.display());
    let resolved = resolve_installed_onnx_runtime(
        OnnxRuntimeHostOs::Linux,
        Some(OsStr::new(&padded)),
        &locations,
    )
    .expect("trimmed override");
    assert_eq!(resolved.source, OnnxRuntimeSource::Override);
}

#[test]
fn windows_runtimes_need_the_shared_provider_library() {
    let root = TempDir::new("windows");
    let custom = root.join("custom/onnxruntime.dll");
    write(&custom, b"dll");
    let locations = locations(&root, false);
    assert!(
        resolve_installed_onnx_runtime(
            OnnxRuntimeHostOs::Windows,
            Some(custom.as_os_str()),
            &locations
        )
        .is_none()
    );
    write(&root.join("custom/onnxruntime_providers_shared.dll"), b"");
    assert!(
        resolve_installed_onnx_runtime(
            OnnxRuntimeHostOs::Windows,
            Some(custom.as_os_str()),
            &locations
        )
        .is_none(),
        "a zero-byte provider library does not count"
    );
    write(&root.join("custom/onnxruntime_providers_shared.dll"), b"p");
    assert_eq!(
        resolve_installed_onnx_runtime(
            OnnxRuntimeHostOs::Windows,
            Some(custom.as_os_str()),
            &locations
        )
        .expect("override")
        .source,
        OnnxRuntimeSource::Override
    );

    let downloaded = root.join("onnxruntime/onnxruntime.dll");
    write(&downloaded, b"dll");
    assert!(resolve_installed_onnx_runtime(OnnxRuntimeHostOs::Windows, None, &locations).is_none());
    assert!(
        is_nonempty_file(&downloaded),
        "a library missing its providers is replaced by the download, not deleted"
    );
}

#[test]
fn bundled_candidates_follow_the_legacy_order() {
    let root = TempDir::new("bundled");
    write(&root.join("resources/libonnxruntime.so"), b"flat");
    write(&root.join("resources/onnxruntime/libonnxruntime.so"), b"");
    let resolved =
        resolve_installed_onnx_runtime(OnnxRuntimeHostOs::Linux, None, &locations(&root, true))
            .expect("bundled");
    assert_eq!(resolved.library, root.join("resources/libonnxruntime.so"));
    assert!(
        root.join("resources/onnxruntime/libonnxruntime.so")
            .exists(),
        "bundled files are never deleted"
    );
}

#[test]
fn a_zero_byte_downloaded_library_is_deleted_for_redownload() {
    let root = TempDir::new("heal");
    let library = root.join("onnxruntime/libonnxruntime.so");
    write(&library, b"");
    assert!(
        resolve_installed_onnx_runtime(OnnxRuntimeHostOs::Linux, None, &locations(&root, false))
            .is_none()
    );
    assert!(
        std::fs::symlink_metadata(&library).is_err(),
        "the zero-byte library is gone"
    );
}

fn tar_archive(
    path: &Path,
    files: &[(&str, &[u8])],
    symlinks: &[(&str, &str)],
    hard_links: &[(&str, &str)],
) {
    let file = std::fs::File::create(path).expect("archive");
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);
    for (name, bytes) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        builder
            .append_data(&mut header, name, *bytes)
            .expect("file entry");
    }
    for (kind, links) in [
        (tar::EntryType::Symlink, symlinks),
        (tar::EntryType::Link, hard_links),
    ] {
        for (name, target) in links {
            let mut header = tar::Header::new_gnu();
            header.set_size(0);
            header.set_mode(0o777);
            header.set_entry_type(kind);
            builder
                .append_link(&mut header, name, target)
                .expect("link entry");
        }
    }
    builder
        .into_inner()
        .expect("tar")
        .finish()
        .expect("gzip")
        .flush()
        .expect("flush");
}

fn no_partial_files(directory: &Path) -> bool {
    std::fs::read_dir(directory)
        .expect("directory")
        .flatten()
        .all(|entry| !entry.file_name().to_string_lossy().ends_with(".partial"))
}

#[test]
fn macos_archives_write_symlinks_and_hard_links_as_copies() {
    let root = TempDir::new("mac-tar");
    let archive = onnx_runtime_archives(OnnxRuntimeHostOs::MacOs, "aarch64")
        .expect("mac")
        .remove(0);
    let archive_file = root.join(&archive.file_name);
    tar_archive(
        &archive_file,
        &[
            (
                "onnxruntime-osx-arm64-1.22.0/lib/libonnxruntime.1.22.0.dylib",
                b"runtime",
            ),
            (
                "onnxruntime-osx-arm64-1.22.0/lib/libonnxruntime_providers_shared.dylib",
                b"shared",
            ),
            (
                "onnxruntime-osx-arm64-1.22.0/lib/cmake/skip.dylib",
                b"nested",
            ),
            ("onnxruntime-osx-arm64-1.22.0/include/header.h", b"header"),
        ],
        &[(
            "onnxruntime-osx-arm64-1.22.0/lib/libonnxruntime.dylib",
            "libonnxruntime.1.22.0.dylib",
        )],
        &[(
            "onnxruntime-osx-arm64-1.22.0/lib/libonnxruntime_providers_coreml.dylib",
            "onnxruntime-osx-arm64-1.22.0/lib/libonnxruntime_providers_shared.dylib",
        )],
    );
    let install = root.join("onnxruntime");
    let library =
        extract_onnx_runtime_archive(&archive, &archive_file, &install, &never).expect("extract");
    assert_eq!(library, install.join("libonnxruntime.1.22.0.dylib"));
    for (name, bytes) in [
        ("libonnxruntime.1.22.0.dylib", b"runtime".as_slice()),
        ("libonnxruntime.dylib", b"runtime"),
        ("libonnxruntime_providers_shared.dylib", b"shared"),
        ("libonnxruntime_providers_coreml.dylib", b"shared"),
    ] {
        let path = install.join(name);
        let metadata = std::fs::symlink_metadata(&path).expect("metadata");
        assert!(metadata.file_type().is_file(), "{name} is a real file");
        assert!(metadata.len() > 0, "{name} is not empty");
        assert_eq!(std::fs::read(&path).expect("read"), bytes);
    }
    assert!(!install.join("skip.dylib").exists());
    assert!(no_partial_files(&install));
}

#[test]
fn linux_archives_follow_a_link_chain_to_the_library() {
    let root = TempDir::new("linux-tar");
    let archive = onnx_runtime_archives(OnnxRuntimeHostOs::Linux, "x86_64")
        .expect("linux")
        .remove(0);
    let archive_file = root.join(&archive.file_name);
    tar_archive(
        &archive_file,
        &[(
            "./onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so.1.22.0.real",
            b"elf",
        )],
        &[(
            "onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so.1",
            "./libonnxruntime.so.1.22.0.real",
        )],
        &[(
            "onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so.1.22.0",
            "onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so.1",
        )],
    );
    let install = root.join("onnxruntime");
    let library =
        extract_onnx_runtime_archive(&archive, &archive_file, &install, &never).expect("extract");
    assert_eq!(library, install.join("libonnxruntime.so"));
    assert_eq!(std::fs::read(&library).expect("read"), b"elf");
    assert!(
        std::fs::symlink_metadata(&library)
            .expect("metadata")
            .file_type()
            .is_file()
    );
    assert_eq!(
        std::fs::read_dir(&install).expect("install").count(),
        1,
        "only the library is unpacked on Linux"
    );
}

#[test]
fn empty_entries_and_broken_links_never_become_libraries() {
    let root = TempDir::new("broken");
    let archive = onnx_runtime_archives(OnnxRuntimeHostOs::Linux, "x86_64")
        .expect("linux")
        .remove(0);
    let empty = root.join("empty.tgz");
    tar_archive(
        &empty,
        &[(
            "onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so.1.22.0",
            b"",
        )],
        &[],
        &[],
    );
    let install = root.join("onnxruntime");
    assert!(matches!(
        extract_onnx_runtime_archive(&archive, &empty, &install, &never),
        Err(OnnxRuntimeProvisionError::EmptyEntry(_))
    ));
    assert!(!is_nonempty_file(&install.join("libonnxruntime.so")));
    assert!(std::fs::symlink_metadata(install.join("libonnxruntime.so")).is_err());

    let broken = root.join("broken.tgz");
    tar_archive(
        &broken,
        &[],
        &[(
            "onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so.1.22.0",
            "libonnxruntime.so.missing",
        )],
        &[],
    );
    assert!(matches!(
        extract_onnx_runtime_archive(&archive, &broken, &install, &never),
        Err(OnnxRuntimeProvisionError::BrokenLink { .. })
    ));

    let missing = root.join("missing.tgz");
    tar_archive(
        &missing,
        &[("onnxruntime-linux-x64-1.22.0/lib/other.so", b"x")],
        &[],
        &[],
    );
    assert!(matches!(
        extract_onnx_runtime_archive(&archive, &missing, &install, &never),
        Err(OnnxRuntimeProvisionError::MissingEntry(_))
    ));
    assert!(no_partial_files(&install));
}

#[test]
fn unneeded_unresolvable_links_are_skipped() {
    let root = TempDir::new("skip-links");
    let archive = onnx_runtime_archives(OnnxRuntimeHostOs::MacOs, "aarch64")
        .expect("mac")
        .remove(0);
    let archive_file = root.join(&archive.file_name);
    tar_archive(
        &archive_file,
        &[
            (
                "onnxruntime-osx-arm64-1.22.0/lib/libonnxruntime.1.22.0.dylib",
                b"runtime",
            ),
            ("onnxruntime-osx-arm64-1.22.0/lib/libempty.dylib", b""),
        ],
        &[
            (
                "onnxruntime-osx-arm64-1.22.0/lib/libdangling.dylib",
                "libmissing.dylib",
            ),
            (
                "onnxruntime-osx-arm64-1.22.0/lib/liboutside.dylib",
                "../../elsewhere/liboutside.dylib",
            ),
        ],
        &[],
    );
    let install = root.join("onnxruntime");
    let library =
        extract_onnx_runtime_archive(&archive, &archive_file, &install, &never).expect("extract");
    assert_eq!(std::fs::read(library).expect("library"), b"runtime");
    for skipped in ["libdangling.dylib", "liboutside.dylib", "libempty.dylib"] {
        assert!(
            std::fs::symlink_metadata(install.join(skipped)).is_err(),
            "{skipped}"
        );
    }
    assert!(no_partial_files(&install));
}

#[test]
fn cancelling_stops_unpacking_before_any_library_is_written() {
    let root = TempDir::new("cancel");
    let archive = onnx_runtime_archives(OnnxRuntimeHostOs::Linux, "x86_64")
        .expect("linux")
        .remove(0);
    let archive_file = root.join(&archive.file_name);
    tar_archive(
        &archive_file,
        &[(
            "onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so.1.22.0",
            b"elf",
        )],
        &[],
        &[],
    );
    let install = root.join("onnxruntime");
    assert_eq!(
        install_onnx_runtime_archive(
            OnnxRuntimeHostOs::Linux,
            &archive,
            &archive_file,
            &install,
            &|| true,
        ),
        Err(OnnxRuntimeProvisionError::Cancelled)
    );
    assert!(std::fs::symlink_metadata(install.join("libonnxruntime.so")).is_err());
}

#[test]
fn a_cancelled_macos_unpack_never_leaves_the_main_library_without_siblings() {
    let root = TempDir::new("mac-cancel");
    let archive = onnx_runtime_archives(OnnxRuntimeHostOs::MacOs, "aarch64")
        .expect("mac")
        .remove(0);
    let archive_file = root.join(&archive.file_name);
    tar_archive(
        &archive_file,
        &[
            (
                "onnxruntime-osx-arm64-1.22.0/lib/libonnxruntime.1.22.0.dylib",
                b"runtime",
            ),
            (
                "onnxruntime-osx-arm64-1.22.0/lib/libonnxruntime_providers_coreml.dylib",
                b"coreml",
            ),
            (
                "onnxruntime-osx-arm64-1.22.0/lib/libonnxruntime_providers_shared.dylib",
                b"shared",
            ),
        ],
        &[],
        &[],
    );
    let install = root.join("onnxruntime");
    let library = install.join("libonnxruntime.1.22.0.dylib");
    let cancel_after_first_write = || {
        std::fs::read_dir(&install)
            .map(|entries| entries.flatten().count() > 0)
            .unwrap_or(false)
    };
    assert_eq!(
        extract_onnx_runtime_archive(&archive, &archive_file, &install, &cancel_after_first_write),
        Err(OnnxRuntimeProvisionError::Cancelled)
    );
    assert!(std::fs::symlink_metadata(&library).is_err());

    let written = std::cell::RefCell::new(Vec::new());
    let record_order = || {
        let mut names = std::fs::read_dir(&install)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        names.sort();
        written.borrow_mut().push(names);
        false
    };
    std::fs::remove_dir_all(&install).expect("reset");
    extract_onnx_runtime_archive(&archive, &archive_file, &install, &record_order)
        .expect("extract");
    assert!(
        written
            .borrow()
            .iter()
            .all(|names| !names.contains(&"libonnxruntime.1.22.0.dylib".to_owned())),
        "the main library is written after every sibling"
    );
    assert!(is_nonempty_file(&library));
}

#[test]
fn windows_installs_require_the_shared_provider_library() {
    let root = TempDir::new("zip-no-shared");
    let archive = onnx_runtime_archives(OnnxRuntimeHostOs::Windows, "x86_64")
        .expect("windows")
        .remove(0);
    let archive_file = root.join(&archive.file_name);
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive_file).expect("zip"));
    writer
        .start_file(
            "onnxruntime-win-x64-1.22.0/lib/onnxruntime.dll",
            zip::write::SimpleFileOptions::default(),
        )
        .expect("entry");
    writer.write_all(b"ort").expect("bytes");
    writer.finish().expect("finish");
    assert_eq!(
        install_onnx_runtime_archive(
            OnnxRuntimeHostOs::Windows,
            &archive,
            &archive_file,
            &root.join("onnxruntime"),
            &never,
        ),
        Err(OnnxRuntimeProvisionError::MissingEntry(
            "onnxruntime_providers_shared.dll".to_owned()
        ))
    );
}

#[test]
fn windows_archives_unpack_every_dll_of_the_lib_folder() {
    let root = TempDir::new("zip");
    let archive = onnx_runtime_archives(OnnxRuntimeHostOs::Windows, "x86_64")
        .expect("windows")
        .remove(0);
    let archive_file = root.join(&archive.file_name);
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive_file).expect("zip"));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in [
        (
            "onnxruntime-win-x64-1.22.0/lib/onnxruntime.dll",
            b"ort".as_slice(),
        ),
        (
            "onnxruntime-win-x64-1.22.0/lib/onnxruntime_providers_shared.dll",
            b"shared",
        ),
        ("onnxruntime-win-x64-1.22.0/lib/onnxruntime.lib", b"import"),
        ("onnxruntime-win-x64-1.22.0/include/onnxruntime.dll", b"no"),
    ] {
        writer.start_file(name, options).expect("entry");
        writer.write_all(bytes).expect("bytes");
    }
    writer.finish().expect("finish");

    let install = root.join("onnxruntime");
    let resolved = install_onnx_runtime_archive(
        OnnxRuntimeHostOs::Windows,
        &archive,
        &archive_file,
        &install,
        &never,
    )
    .expect("install");
    assert_eq!(resolved.source, OnnxRuntimeSource::Downloaded);
    assert_eq!(std::fs::read(&resolved.library).expect("ort"), b"ort");
    assert_eq!(
        std::fs::read(install.join("onnxruntime_providers_shared.dll")).expect("shared"),
        b"shared"
    );
    assert!(!install.join("onnxruntime.lib").exists());
    let found = resolve_installed_onnx_runtime(
        OnnxRuntimeHostOs::Windows,
        None,
        &OnnxRuntimeLocations {
            install_dir: install,
            resource_dir: None,
        },
    )
    .expect("installed");
    assert_eq!(found, resolved);
}

#[test]
fn adhoc_signing_is_needed_for_unsigned_and_team_signed_dylibs() {
    let display = |success: bool, stderr: &str| CodesignDisplay {
        success,
        stderr: stderr.to_owned(),
    };
    assert!(!macos_dylib_needs_adhoc_signature(None));
    assert!(macos_dylib_needs_adhoc_signature(Some(&display(
        false,
        "libonnxruntime.1.22.0.dylib: code object is not signed at all"
    ))));
    assert!(macos_dylib_needs_adhoc_signature(Some(&display(
        true,
        "Executable=/x/libonnxruntime.1.22.0.dylib\nAuthority=Developer ID Application: Microsoft Corporation (UBF8T346G9)\nTeamIdentifier=UBF8T346G9\n"
    ))));
    assert!(!macos_dylib_needs_adhoc_signature(Some(&display(
        true,
        "Executable=/x/libonnxruntime.1.22.0.dylib\nSignature=adhoc\nTeamIdentifier=not set\n"
    ))));
    assert!(!macos_dylib_needs_adhoc_signature(Some(&display(
        true,
        "Executable=/x/lib.dylib\nTeamIdentifier=\n"
    ))));
    assert!(!macos_dylib_needs_adhoc_signature(Some(&display(
        true,
        "Executable=/x/lib.dylib\n"
    ))));
}

#[test]
fn runtime_commits_once_and_a_failure_stays_retryable() {
    let once = RuntimeOnce::new();
    let first = Path::new("/runtime/a.so");
    let second = Path::new("/runtime/b.so");
    let calls = Cell::new(0);

    for error in [
        OnnxRuntimeInitError::Load("bad".to_owned()),
        OnnxRuntimeInitError::MissingLibrary(first.to_path_buf()),
        OnnxRuntimeInitError::InvalidPath,
    ] {
        let failed = once.initialize(OnnxRuntimeBinding::Library(first), || {
            calls.set(calls.get() + 1);
            Err(error.clone())
        });
        assert_eq!(failed, Err(error));
        assert_eq!(once.committed(), None);
    }

    let committed = once.initialize(OnnxRuntimeBinding::Library(first), || {
        calls.set(calls.get() + 1);
        Ok(())
    });
    assert_eq!(committed, Ok(OnnxRuntimeInitialization::Initialized));
    assert_eq!(
        once.committed(),
        Some(CommittedOnnxRuntime::Library(first.to_path_buf()))
    );

    for binding in [
        OnnxRuntimeBinding::Library(first),
        OnnxRuntimeBinding::Library(second),
        OnnxRuntimeBinding::Linked,
    ] {
        let reused = once.initialize(binding, || {
            calls.set(calls.get() + 1);
            Err(OnnxRuntimeInitError::Load("must not run".to_owned()))
        });
        assert_eq!(reused, Ok(OnnxRuntimeInitialization::AlreadyInitialized));
    }
    assert_eq!(calls.get(), 4);
}

/// `ort` pins the library path in `init_from` and its `OnceLock` marks a
/// failed environment setup as done, so a retry could report success over
/// an uninitialized environment. Such failures must stick.
#[test]
fn failures_inside_ort_are_permanent_for_the_process() {
    for permanent in [
        OnnxRuntimeInitError::Environment("setup failed".to_owned()),
        OnnxRuntimeInitError::Panicked("could not load".to_owned()),
    ] {
        assert!(permanent.is_permanent());
        let once = RuntimeOnce::new();
        let calls = Cell::new(0);
        let library = Path::new("/runtime/a.so");
        let first = once.initialize(OnnxRuntimeBinding::Library(library), || {
            calls.set(calls.get() + 1);
            Err(permanent.clone())
        });
        assert_eq!(first, Err(permanent.clone()));
        for binding in [
            OnnxRuntimeBinding::Library(library),
            OnnxRuntimeBinding::Library(Path::new("/runtime/b.so")),
            OnnxRuntimeBinding::Linked,
        ] {
            let again = once.initialize(binding, || {
                calls.set(calls.get() + 1);
                Ok(())
            });
            assert_eq!(again, Err(permanent.clone()));
        }
        assert_eq!(calls.get(), 1);
        assert_eq!(once.committed(), None);
    }
    for retryable in [
        OnnxRuntimeInitError::InvalidPath,
        OnnxRuntimeInitError::MissingLibrary(PathBuf::from("/x")),
        OnnxRuntimeInitError::Load("x".to_owned()),
    ] {
        assert!(!retryable.is_permanent());
    }
}

#[test]
fn missing_or_unloadable_libraries_fail_without_initializing() {
    let root = TempDir::new("init");
    let missing = root.join("libonnxruntime.so");
    assert_eq!(
        initialize_process_onnx_runtime(OnnxRuntimeBinding::Library(&missing), "test"),
        Err(OnnxRuntimeInitError::MissingLibrary(missing.clone()))
    );
    write(&missing, b"not a shared library");
    assert!(matches!(
        initialize_process_onnx_runtime(OnnxRuntimeBinding::Library(&missing), "test"),
        Err(OnnxRuntimeInitError::Load(_))
    ));
    assert!(committed_onnx_runtime().is_none());
    assert!(matches!(
        *PROCESS_RUNTIME
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        ProcessState::Uninitialized
    ));
}

#[test]
fn panic_payloads_become_messages() {
    let payload = std::panic::catch_unwind(|| panic!("loader failed")).expect_err("panicked");
    assert_eq!(panic_message(payload.as_ref()), "loader failed");
    let payload = std::panic::catch_unwind(|| std::panic::panic_any(7_u8)).expect_err("panicked");
    assert_eq!(panic_message(payload.as_ref()), "unknown panic");
}
