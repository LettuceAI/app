use super::tests::{Reply, harness};
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_models::{ModelCatalog, ModelProfileRepository, ProviderAccountRepository};
use lettuce_types::ProviderAccountId;
use std::future::Future;

#[tokio::test]
async fn usage_wrong_kind_and_missing_key_are_typed() {
    let h = harness(Reply::Text("Hello."));
    let mut account = h
        .context
        .backend()
        .database()
        .provider_accounts()
        .expect("accounts")
        .remove(0);
    let request = dto::ProviderNanoGptUsageRequest {
        account_id: account.id.to_string(),
    };
    let wrong = super::provider_nanogpt_usage(&h.context, request.clone())
        .await
        .expect_err("wrong kind");
    assert_eq!(wrong.code, ApiErrorCode::InvalidInput);
    assert!(matches!(
        wrong.details,
        Some(dto::ApiErrorDetails::ProviderQuota {
            reason: dto::ProviderQuotaFailure::WrongProvider,
            ..
        })
    ));
    let revision = account.revision;
    account.provider_kind = "nanogpt".into();
    account.protocol = lettuce_models::ProviderProtocol::OpenAiCompatible;
    account.api_key_ref = None;
    ProviderAccountRepository::upsert(h.context.backend().database(), account, Some(revision))
        .expect("nano account");
    let missing = super::provider_nanogpt_usage(&h.context, request)
        .await
        .expect_err("missing key");
    assert_eq!(missing.code, ApiErrorCode::InvalidInput);
    assert!(matches!(
        missing.details,
        Some(dto::ApiErrorDetails::ProviderQuota {
            reason: dto::ProviderQuotaFailure::MissingApiKey,
            ..
        })
    ));
}

#[tokio::test(start_paused = true)]
async fn coalesces_per_account_for_300_seconds_and_keeps_one_in_flight() {
    let state = super::nanogpt::QuotaState::default();
    let a = ProviderAccountId::new();
    let b = ProviderAccountId::new();
    let first = state.claim(a, false).expect("first");
    assert!(state.claim(a, false).is_none());
    assert!(state.claim(b, false).is_some());
    tokio::time::advance(std::time::Duration::from_secs(600)).await;
    assert!(state.claim(a, false).is_none());
    drop(first);
    assert!(state.claim(a, false).is_none());
    tokio::time::advance(std::time::Duration::from_secs(299)).await;
    assert!(state.claim(a, false).is_none());
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    assert!(state.claim(a, false).is_some());
}

async fn nano_account(h: &super::tests::Harness, endpoint: String) -> String {
    let view = super::provider_account_save(
        &h.context,
        dto::ProviderAccountSaveRequest {
            account: dto::ProviderAccountInput {
                id: None,
                provider_kind: "nanogpt".into(),
                label: "NanoGPT".into(),
                base_url: Some(endpoint),
                enabled: true,
                streaming_enabled: false,
                allow_invalid_tls: false,
                config: serde_json::json!({"kind":"standard"}),
            },
            api_key: Some("nano-secret-canary".into()),
            clear_api_key: false,
            expected_revision: None,
            client_operation_id: "nano-account".into(),
        },
    )
    .await
    .expect("account");
    view.id
}

#[tokio::test]
async fn usage_non_success_preserves_provider_text_and_redacts_credentials() {
    use tokio::io::AsyncWriteExt;
    let h = harness(Reply::Text("Hello."));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let id = nano_account(
        &h,
        format!("http://{}/paid/v1", listener.local_addr().expect("address")),
    )
    .await;
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let request = read_request(&mut socket).await;
        assert!(request.starts_with("GET /subscription/v1/usage "));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer nano-secret-canary")
        );
        let body = r#"{"error":{"message":"quota unavailable nano-secret-canary"}}"#;
        socket.write_all(format!("HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.expect("response");
    });
    let error = super::provider_nanogpt_usage(
        &h.context,
        dto::ProviderNanoGptUsageRequest { account_id: id },
    )
    .await
    .expect_err("provider failure");
    assert_eq!(error.code, ApiErrorCode::Unavailable);
    assert!(
        matches!(error.details, Some(dto::ApiErrorDetails::ProviderQuota { status: Some(403), provider_message: Some(ref message), .. }) if message == "quota unavailable [REDACTED]")
    );
    server.await.expect("server");
}

#[tokio::test]
async fn completed_request_returns_success_while_quota_check_is_waiting_or_fails() {
    use std::sync::Arc;
    use tokio::io::AsyncWriteExt;
    let h = harness(Reply::Text("Hello."));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let id = nano_account(
        &h,
        format!("http://{}", listener.local_addr().expect("address")),
    )
    .await;
    let mut model = h
        .context
        .backend()
        .database()
        .model_profiles()
        .expect("profiles")
        .remove(0);
    let revision = model.revision;
    model.provider_account_id = id.parse().expect("account");
    ModelProfileRepository::upsert(h.context.backend().database(), model, Some(revision))
        .expect("model");
    let (started, received) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        read_request(&mut socket).await;
        started.send(()).expect("started");
        released.await.expect("release");
        socket.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.expect("failure");
    });
    let conversation = super::tests::launch(&h, "nano-request").await;
    let sink = Arc::new(super::tests::RecordingStream::default());
    super::tests::send(&h, &conversation, "nano-send", "Hi", sink.clone())
        .await
        .expect("send");
    let worker = super::ConversationGenerationWorker::new(h.context.clone());
    tokio::time::timeout(std::time::Duration::from_secs(5), worker.run_once())
        .await
        .expect("request does not wait for usage")
        .expect("request");
    tokio::time::timeout(std::time::Duration::from_secs(5), received)
        .await
        .expect("quota check was signalled")
        .expect("quota check started");
    assert!(
        sink.events()
            .iter()
            .any(|event| matches!(event, dto::GenerationEvent::Completed { .. }))
    );
    let manual = super::provider_nanogpt_usage(
        &h.context,
        dto::ProviderNanoGptUsageRequest { account_id: id },
    );
    tokio::pin!(manual);
    std::future::poll_fn(|cx| {
        assert!(manual.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    release.send(()).expect("release");
    let error = manual.await.expect_err("joined background failure");
    assert!(matches!(
        error.details,
        Some(dto::ApiErrorDetails::ProviderQuota {
            reason: dto::ProviderQuotaFailure::ProviderRejected,
            status: Some(500),
            ..
        })
    ));
    server.await.expect("server");
}

#[tokio::test]
async fn every_quota_level_emits_once_and_context_restart_does_not_repeat_it() {
    use tokio::io::AsyncWriteExt;
    let h = harness(Reply::Text("Hello."));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let id = nano_account(
        &h,
        format!("http://{}", listener.local_addr().expect("address")),
    )
    .await;
    let server = tokio::spawn(async move {
        for used in [75, 75, 90, 90, 100, 100, 100] {
            let (mut socket, _) = listener.accept().await.expect("accept");
            read_request(&mut socket).await;
            let body = serde_json::json!({"weekly":{"used":used,"limit":100,"resetAt":"window"}})
                .to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.expect("response");
        }
    });
    for _ in 0..6 {
        super::provider_nanogpt_usage(
            &h.context,
            dto::ProviderNanoGptUsageRequest {
                account_id: id.clone(),
            },
        )
        .await
        .expect("usage");
    }
    let restart = h
        .context
        .with_test_secrets(h.context.secret_store().clone());
    super::provider_nanogpt_usage(
        &restart,
        dto::ProviderNanoGptUsageRequest {
            account_id: id.clone(),
        },
    )
    .await
    .expect("usage after restart");
    let levels: Vec<_> = h
        .events
        .events()
        .into_iter()
        .filter_map(|event| match event {
            dto::ApiEvent::ProviderQuota { account_id, level } if account_id == id => Some(level),
            _ => None,
        })
        .collect();
    assert_eq!(
        levels,
        vec![
            dto::ProviderQuotaLevel::NearLimit,
            dto::ProviderQuotaLevel::AlmostExhausted,
            dto::ProviderQuotaLevel::Exhausted
        ]
    );
    server.await.expect("server");
}

#[tokio::test]
async fn shutdown_during_usage_check_cancels_without_emitting_a_warning() {
    let h = harness(Reply::Text("Hello."));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let id = nano_account(
        &h,
        format!("http://{}", listener.local_addr().expect("address")),
    )
    .await;
    let (started, received) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        read_request(&mut socket).await;
        started.send(()).expect("started");
        std::future::pending::<()>().await;
    });
    let context = h.context.clone();
    let command = tokio::spawn(async move {
        super::provider_nanogpt_usage(
            &context,
            dto::ProviderNanoGptUsageRequest { account_id: id },
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), received)
        .await
        .expect("started in time")
        .expect("started");
    h.context.begin_shutdown();
    assert_eq!(
        command.await.expect("command").expect_err("shutdown").code,
        ApiErrorCode::Cancelled
    );
    assert!(
        !h.events
            .events()
            .iter()
            .any(|event| matches!(event, dto::ApiEvent::ProviderQuota { .. }))
    );
    server.abort();
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    use tokio::io::AsyncReadExt;
    let mut request = Vec::new();
    loop {
        let mut buffer = [0; 1024];
        let count = socket.read(&mut buffer).await.expect("request read");
        assert!(count > 0, "request ended before its headers");
        request.extend_from_slice(&buffer[..count]);
        if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            return String::from_utf8(request).expect("request text");
        }
    }
}
