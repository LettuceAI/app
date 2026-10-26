use lettuce_contracts::{self as dto, ApiErrorCode};

use crate::api::tests::{Reply, harness};

fn filter() -> dto::LogViewFilter {
    dto::LogViewFilter {
        levels: vec![
            dto::LogLevel::Info,
            dto::LogLevel::Warn,
            dto::LogLevel::Error,
        ],
        hidden_components: Vec::new(),
        from: None,
        to: None,
        query: String::new(),
        case_sensitive: false,
        whole_word: false,
        regex: false,
        only_matches: false,
        isolation: None,
    }
}

fn directory() -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("logs-query-{}", lettuce_types::OperationId::new()));
    std::fs::create_dir(&path).expect("directory");
    path
}

fn attach(
    context: &crate::api::ApiContext,
    path: &std::path::Path,
) -> lettuce_observability::LocalOutput {
    let output =
        lettuce_observability::local_output(lettuce_observability::LocalOutputConfig::new(path))
            .expect("output");
    context.attach_logs(path.to_owned(), output.sink.clone());
    output
}

async fn query(
    context: &crate::api::ApiContext,
    filter: dto::LogViewFilter,
    offset: u64,
    limit: u32,
) -> dto::LogQueryView {
    crate::api::log_query(
        context,
        dto::LogQueryRequest {
            name: "app-2026-10-25.log".into(),
            filter,
            offset,
            limit,
        },
    )
    .await
    .expect("query")
}

#[tokio::test]
async fn whole_file_query_pages_filtered_positions_without_losing_raw_indices() {
    let h = harness(Reply::Text("Hello."));
    let path = directory();
    let output = attach(&h.context, &path);
    let text = (0..2505)
        .map(|index| {
            format!(
                "[2026-10-25T09:12:03.114+03:00] {} worker at=file.rs:1 | item {index}\n",
                if index % 2 == 0 { "DEBUG" } else { "WARN" }
            )
        })
        .collect::<String>();
    std::fs::write(path.join("app-2026-10-25.log"), text).expect("write");
    let view = query(&h.context, filter(), 1000, 2).await;
    assert_eq!((view.total, view.filtered_total), (2505, 1252));
    assert_eq!((view.counts.debug, view.counts.warn), (1253, 1252));
    assert_eq!(
        view.components,
        vec![dto::LogComponentCount {
            component: "worker".into(),
            count: 2505
        }]
    );
    assert_eq!(
        view.lines
            .iter()
            .map(|line| (line.index, line.position))
            .collect::<Vec<_>>(),
        vec![(2001, 1000), (2003, 1001)]
    );
    assert_eq!(
        view.problems.last(),
        Some(&dto::LogPosition {
            index: 2503,
            position: 1251
        })
    );
    assert!(view.matches.is_empty());
    let mut search = filter();
    search.query = "item 2503".into();
    search.only_matches = true;
    let view = query(&h.context, search, 0, 10).await;
    assert_eq!(view.filtered_total, 1);
    assert_eq!(
        view.matches,
        vec![dto::LogPosition {
            index: 2503,
            position: 0
        }]
    );
    let mut all_levels = filter();
    all_levels.levels.push(dto::LogLevel::Debug);
    assert_eq!(
        query(&h.context, all_levels, 0, u32::MAX).await.lines.len(),
        2000
    );
    drop(output);
    std::fs::remove_dir_all(path).expect("cleanup");
}

#[tokio::test]
async fn query_search_time_isolation_and_malformed_lines_keep_view_semantics() {
    let h = harness(Reply::Text("Hello."));
    let path = directory();
    let output = attach(&h.context, &path);
    let lines = [
        "[2026-10-25T09:12:01.114+03:00] INFO alpha | Token tokens",
        "[2026-10-25T09:12:59.999+03:00] ERROR beta | token",
        "[2026-10-25T09:13:00+03:00] WARN alpha | TOKEN",
        "continuation without a parsed prefix",
        "[2026-10-25T09:12:30Z] TRACE other | token",
    ];
    std::fs::write(path.join("app-2026-10-25.log"), lines.join("\n")).expect("write");
    let mut selected = filter();
    selected.hidden_components = vec!["alpha".into()];
    selected.from = Some("09:12".into());
    selected.to = Some("09:12".into());
    let view = query(&h.context, selected.clone(), 0, 20).await;
    assert_eq!(
        view.lines.iter().map(|line| line.index).collect::<Vec<_>>(),
        vec![1, 3, 4]
    );
    selected.isolation = Some(vec![0, 2, 3]);
    assert_eq!(
        query(&h.context, selected, 0, 20)
            .await
            .lines
            .iter()
            .map(|line| line.index)
            .collect::<Vec<_>>(),
        vec![0, 3]
    );
    let mut selected = filter();
    selected.levels.clear();
    assert_eq!(
        query(&h.context, selected, 0, 20)
            .await
            .lines
            .iter()
            .map(|line| line.index)
            .collect::<Vec<_>>(),
        vec![3, 4]
    );
    let mut selected = filter();
    selected.query = "token".into();
    selected.case_sensitive = true;
    selected.whole_word = true;
    assert_eq!(
        query(&h.context, selected.clone(), 0, 20)
            .await
            .matches
            .iter()
            .map(|line| line.index)
            .collect::<Vec<_>>(),
        vec![1, 4]
    );
    selected.case_sensitive = false;
    assert_eq!(
        query(&h.context, selected.clone(), 0, 20)
            .await
            .matches
            .len(),
        4
    );
    selected.whole_word = false;
    selected.regex = true;
    selected.query = "T[oO][kK][eE][nN]$".into();
    assert_eq!(query(&h.context, selected, 0, 20).await.matches.len(), 3);
    for (field, mut selected) in [("from", filter()), ("query", filter())] {
        if field == "from" {
            selected.from = Some("25:99".into());
        } else {
            selected.query = "[".into();
            selected.regex = true;
        }
        let error = crate::api::log_query(
            &h.context,
            dto::LogQueryRequest {
                name: "app-2026-10-25.log".into(),
                filter: selected,
                offset: 0,
                limit: 20,
            },
        )
        .await
        .expect_err("invalid filter");
        assert_eq!(error.code, ApiErrorCode::InvalidInput);
        assert_eq!(
            error.details,
            Some(dto::ApiErrorDetails::InvalidField {
                field: field.into()
            })
        );
    }
    assert_eq!(
        crate::api::log_query(
            &h.context,
            dto::LogQueryRequest {
                name: "../app-2026-10-25.log".into(),
                filter: filter(),
                offset: 0,
                limit: 20
            }
        )
        .await
        .expect_err("traversal")
        .code,
        ApiErrorCode::NotFound
    );
    drop(output);
    std::fs::remove_dir_all(path).expect("cleanup");
}

#[tokio::test]
async fn selected_export_preserves_bytes_order_and_protects_output_on_bad_indices() {
    let h = harness(Reply::Text("Hello."));
    let path = directory();
    let output = attach(&h.context, &path);
    let source = path.join("app-2026-10-25.log");
    let text = "[2026-10-25T09:12:00Z] INFO alpha | first\r\n[2026-10-25T09:12:01Z] ERROR beta | second\nlast without newline";
    std::fs::write(&source, text).expect("write");
    let target = path.join("selected.txt");
    let request = |selection| dto::LogExportSelectionRequest {
        name: "app-2026-10-25.log".into(),
        target: dto::FileTarget {
            uri: target.display().to_string(),
        },
        selection,
    };
    crate::api::log_export_selection(
        &h.context,
        request(dto::LogSelection::Indices {
            indices: vec![2, 0, 2],
        }),
    )
    .await
    .expect("selected export");
    assert_eq!(
        std::fs::read_to_string(&target).expect("target"),
        "[2026-10-25T09:12:00Z] INFO alpha | first\r\nlast without newline"
    );
    let before = std::fs::read(&target).expect("before");
    for indices in [vec![3], vec![0, u64::MAX]] {
        let error = crate::api::log_export_selection(
            &h.context,
            request(dto::LogSelection::Indices { indices }),
        )
        .await
        .expect_err("bad indices");
        assert_eq!(error.code, ApiErrorCode::InvalidInput);
        assert_eq!(std::fs::read(&target).expect("preserved"), before);
    }
    let mut selected = filter();
    selected.levels = vec![dto::LogLevel::Error];
    let view = query(&h.context, selected.clone(), 0, 20).await;
    crate::api::log_export_selection(
        &h.context,
        request(dto::LogSelection::Filter { filter: selected }),
    )
    .await
    .expect("filtered export");
    assert_eq!(
        std::fs::read_to_string(&target).expect("filtered"),
        "[2026-10-25T09:12:01Z] ERROR beta | second\nlast without newline"
    );
    assert_eq!(
        view.lines.iter().map(|line| line.index).collect::<Vec<_>>(),
        vec![1, 2]
    );
    let same = crate::api::log_export_selection(
        &h.context,
        dto::LogExportSelectionRequest {
            name: "app-2026-10-25.log".into(),
            target: dto::FileTarget {
                uri: source.display().to_string(),
            },
            selection: dto::LogSelection::Indices { indices: vec![0] },
        },
    )
    .await
    .expect_err("source protected");
    assert_eq!(same.code, ApiErrorCode::Conflict);
    assert_eq!(std::fs::read_to_string(&source).expect("source"), text);
    #[cfg(unix)]
    {
        let alias = path.join("alias.txt");
        std::fs::hard_link(&source, &alias).expect("alias");
        let error = crate::api::log_export_selection(
            &h.context,
            dto::LogExportSelectionRequest {
                name: "app-2026-10-25.log".into(),
                target: dto::FileTarget {
                    uri: alias.display().to_string(),
                },
                selection: dto::LogSelection::Indices { indices: vec![0] },
            },
        )
        .await
        .expect_err("alias protected");
        assert_eq!(error.code, ApiErrorCode::Conflict);
    }
    assert!(!std::fs::read_dir(&path).expect("directory").any(|entry| {
        entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .ends_with(".partial")
    }));
    drop(output);
    std::fs::remove_dir_all(path).expect("cleanup");
}

#[tokio::test]
async fn selection_export_refuses_database_and_managed_media_targets() {
    use crate::api::tests::{NoImages, harness_over_files};
    use crate::api::{ApiDatabaseFiles, NoModels};
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority};
    use lettuce_types::TimestampMillis;
    use std::sync::Arc;

    let root = directory();
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let location = crate::AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority)
        .expect("location");
    let active = location.active_path().expect("active");
    let backend =
        Arc::new(crate::AppBackend::open(&active, TimestampMillis::new(10)).expect("backend"));
    let h = harness_over_files(
        backend,
        Reply::Text("ok"),
        Arc::new(lettuce_jobs::SystemClock),
        None,
        Some(root.clone()),
        Arc::new(NoModels),
        Arc::new(NoImages),
        Some(ApiDatabaseFiles {
            location,
            active: active.clone(),
        }),
    );
    let logs = root.join("logs");
    std::fs::create_dir(&logs).expect("logs");
    let output = attach(&h.context, &logs);
    std::fs::write(
        logs.join("app-2026-10-25.log"),
        "[2026-10-25T09:12:00Z] INFO source | record\n",
    )
    .expect("log");
    let media = root.join("platform-v2/media-blobs");
    std::fs::create_dir_all(&media).expect("media");
    let media_file = media.join("image.webp");
    std::fs::write(&media_file, b"protected image").expect("image");
    for target in [active, media_file] {
        let before = std::fs::read(&target).expect("before");
        let error = crate::api::log_export_selection(
            &h.context,
            dto::LogExportSelectionRequest {
                name: "app-2026-10-25.log".into(),
                target: dto::FileTarget {
                    uri: target.display().to_string(),
                },
                selection: dto::LogSelection::Indices { indices: vec![0] },
            },
        )
        .await
        .expect_err("protected target");
        assert_eq!(error.code, ApiErrorCode::Conflict);
        assert_eq!(std::fs::read(&target).expect("preserved"), before);
    }
    drop(output);
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn ordinary_log_changes_notify_without_mirroring_private_lines() {
    let h = harness(Reply::Text("Hello."));
    let path = directory();
    let output = attach(&h.context, &path);
    output
        .sink
        .append(&lettuce_observability::LogEntry {
            timestamp: "2026-10-26T12:00:00Z".into(),
            level: "INFO".into(),
            component: "ordinary".into(),
            function: None,
            message: "private line".into(),
        })
        .expect("append");
    let changes = || {
        h.events
            .events()
            .iter()
            .filter(|event| matches!(event, dto::ApiEvent::LogsChanged))
            .count()
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while changes() == 0 && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(changes(), 1);
    assert!(
        !h.events
            .events()
            .iter()
            .any(|event| matches!(event, dto::ApiEvent::DeveloperLogLine { .. }))
    );
    let files = crate::api::logs_list(&h.context).await.expect("list").files;
    crate::api::log_delete(
        &h.context,
        dto::LogNameRequest {
            name: files[0].clone(),
        },
    )
    .await
    .expect("delete");
    assert_eq!(changes(), 2);
    crate::api::logs_clear(&h.context).await.expect("clear");
    assert_eq!(changes(), 3);
    assert!(
        !h.events
            .events()
            .iter()
            .any(|event| matches!(event, dto::ApiEvent::DeveloperLogLine { .. }))
    );
    drop(output);
    std::fs::remove_dir_all(path).expect("cleanup");
}

struct TargetWriterOnly;

impl crate::api::FileAccess for TargetWriterOnly {
    fn describe(
        &self,
        _: &str,
    ) -> Result<crate::api::FileDescription, crate::api::FileAccessError> {
        Err(crate::api::FileAccessError::Unsupported)
    }
    fn open(
        &self,
        _: &str,
    ) -> Result<Box<dyn crate::api::FileReader>, crate::api::FileAccessError> {
        Err(crate::api::FileAccessError::Unsupported)
    }
    fn create(
        &self,
        _: &str,
    ) -> Result<Box<dyn std::io::Write + Send>, crate::api::FileAccessError> {
        Err(crate::api::FileAccessError::Unsupported)
    }
    fn create_export(
        &self,
        uri: &str,
        source: Option<&std::fs::File>,
        protected: crate::api::ExportProtection<'_>,
    ) -> Result<Box<dyn std::io::Write + Send>, crate::api::FileAccessError> {
        crate::api::tests::StdFiles.create_export(uri, source, protected)
    }
}

#[tokio::test]
async fn selection_export_uses_private_staging_without_host_backup_or_read_access() {
    use std::sync::Arc;
    let root = directory();
    let h = harness(Reply::Text("Hello."));
    let context = crate::api::ApiContext::new(crate::api::ApiContextParts {
        backend: h.context.shared_backend(),
        secret_store: h.context.secret_store().clone(),
        inference: h.provider.clone(),
        image_provider: Arc::new(crate::api::tests::NoImages),
        models: Arc::new(crate::api::NoModels),
        speech: Arc::new(crate::api::NoSpeech),
        media: None,
        events: h.events.clone(),
        clock: Arc::new(lettuce_jobs::SystemClock),
        files: Arc::new(TargetWriterOnly),
        app_folder: Some(root.clone()),
        resource_dir: None,
        database_files: None,
        asset_url_base: "test-asset://host".into(),
    });
    let output = attach(&context, &root);
    std::fs::write(root.join("app-2026-10-25.log"), "first\nsecond\n").expect("log");
    let target = root.join("picked.txt");
    crate::api::log_export_selection(
        &context,
        dto::LogExportSelectionRequest {
            name: "app-2026-10-25.log".into(),
            target: dto::FileTarget {
                uri: target.display().to_string(),
            },
            selection: dto::LogSelection::Indices { indices: vec![1] },
        },
    )
    .await
    .expect("export without host temporary-file support");
    assert_eq!(
        std::fs::read_to_string(target).expect("exported"),
        "second\n"
    );
    assert!(!std::fs::read_dir(&root).expect("directory").any(|entry| {
        entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .ends_with(".partial")
    }));
    drop(output);
    drop(context);
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}
