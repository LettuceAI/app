use super::tests::{Reply, harness};
use lettuce_contracts::{self as dto, ApiErrorCode, ApiEvent};

async fn developer(context: &super::ApiContext, enabled: bool) {
    let view = super::settings_get(context).await.expect("settings");
    super::settings_update(
        context,
        dto::SettingsUpdateRequest {
            expected_revision: view.revision,
            patch: dto::SettingsPatch::General {
                developer_mode_enabled: Some(enabled),
                pure_mode: None,
                analytics_enabled: None,
                update_checks_enabled: None,
                auto_download_character_card_avatars: None,
                manual_mode_context_window: None,
                lorebook_scan_depth: None,
            },
        },
    )
    .await
    .expect("developer mode");
}

#[tokio::test]
async fn log_commands_write_page_search_export_and_delete_with_a_gated_mirror() {
    let h = harness(Reply::Text("Hello."));
    let path = std::env::temp_dir().join(format!("api-logs-{}", lettuce_types::OperationId::new()));
    std::fs::create_dir(&path).expect("directory");
    let output =
        lettuce_observability::local_output(lettuce_observability::LocalOutputConfig::new(&path))
            .expect("output");
    h.context.attach_logs(path.clone(), output.sink.clone());
    let request = dto::LogAppendRequest {
        timestamp: "2026-10-09T12:00:00Z".into(),
        level: dto::LogLevel::Info,
        component: "frontend".into(),
        function: None,
        message: "first line".into(),
    };
    super::log_append(&h.context, request.clone())
        .await
        .expect("append in debug build");
    assert!(
        !h.events
            .events()
            .iter()
            .any(|event| matches!(event, ApiEvent::DeveloperLogLine { .. }))
    );
    developer(&h.context, true).await;
    super::log_append(&h.context, request.clone())
        .await
        .expect("append");
    assert_eq!(
        h.events
            .events()
            .iter()
            .filter(|event| matches!(event, ApiEvent::DeveloperLogLine { .. }))
            .count(),
        1
    );
    developer(&h.context, false).await;
    super::log_append(&h.context, request)
        .await
        .expect("append without mirror");
    assert_eq!(
        h.events
            .events()
            .iter()
            .filter(|event| matches!(event, ApiEvent::DeveloperLogLine { .. }))
            .count(),
        1
    );
    let list = super::logs_list(&h.context).await.expect("list");
    assert_eq!(list.files.len(), 1);
    let name = list.files[0].clone();
    let page = super::log_read_page(
        &h.context,
        dto::LogReadPageRequest {
            name: name.clone(),
            offset: 1,
            limit: 1,
        },
    )
    .await
    .expect("page");
    assert_eq!(page.total, 3);
    assert_eq!(page.lines.len(), 1);
    let found = super::log_search(
        &h.context,
        dto::LogSearchRequest {
            name: name.clone(),
            query: "FIRST".into(),
            case_sensitive: false,
            whole_word: true,
            regex: false,
        },
    )
    .await
    .expect("search");
    assert_eq!(found.matches, vec![0, 1, 2]);
    let bad = super::log_search(
        &h.context,
        dto::LogSearchRequest {
            name: name.clone(),
            query: "[".into(),
            case_sensitive: false,
            whole_word: false,
            regex: true,
        },
    )
    .await
    .expect_err("invalid regex");
    assert_eq!(bad.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        super::log_read_page(
            &h.context,
            dto::LogReadPageRequest {
                name: "../private".into(),
                offset: 0,
                limit: 1
            }
        )
        .await
        .expect_err("traversal")
        .code,
        ApiErrorCode::NotFound
    );
    let original = path.join(&name);
    let same_target = super::log_export(
        &h.context,
        dto::LogExportRequest {
            name: name.clone(),
            target: dto::FileTarget {
                uri: original.to_string_lossy().into_owned(),
            },
        },
    )
    .await;
    assert_eq!(
        same_target.expect_err("cannot overwrite source log").code,
        ApiErrorCode::Conflict
    );
    assert_eq!(
        std::fs::read_to_string(&original)
            .expect("original")
            .lines()
            .count(),
        3
    );
    let export = path.join("export.txt");
    super::log_export(
        &h.context,
        dto::LogExportRequest {
            name: name.clone(),
            target: dto::FileTarget {
                uri: export.to_string_lossy().into_owned(),
            },
        },
    )
    .await
    .expect("export");
    assert_eq!(
        std::fs::read_to_string(export)
            .expect("export")
            .lines()
            .count(),
        3
    );
    super::log_delete(&h.context, dto::LogNameRequest { name })
        .await
        .expect("delete");
    assert!(
        super::logs_list(&h.context)
            .await
            .expect("list")
            .files
            .is_empty()
    );
    super::logs_clear(&h.context).await.expect("clear");
    drop(output);
    std::fs::remove_dir_all(path).expect("cleanup");
}

#[tokio::test]
async fn missing_log_host_is_typed() {
    let h = harness(Reply::Text("Hello."));
    assert_eq!(
        super::logs_list(&h.context)
            .await
            .expect_err("host absent")
            .code,
        ApiErrorCode::Unavailable
    );
}
