use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_models::ModelCatalog;

use super::tests::{Reply, harness};
use super::{provider_accounts_list, provider_catalog, provider_verify};

#[tokio::test]
async fn catalog_and_accounts_are_read_only_and_never_expose_keys() {
    let harness = harness(Reply::Text("Hello."));
    let before = harness
        .context
        .backend()
        .database()
        .provider_accounts()
        .expect("accounts");
    let catalog = provider_catalog(&harness.context).await.expect("catalog");
    assert_eq!(catalog.providers.len(), 25);
    assert!(
        catalog
            .providers
            .iter()
            .any(|provider| provider.kind == "custom")
    );
    let accounts = provider_accounts_list(&harness.context)
        .await
        .expect("accounts");
    assert_eq!(accounts.len(), before.len());
    let json = serde_json::to_string(&accounts).expect("views");
    assert!(!json.contains("api_key_ref"));
    assert!(!json.contains("secret_owner_id"));
    assert_eq!(
        harness
            .context
            .backend()
            .database()
            .provider_accounts()
            .expect("accounts"),
        before
    );
}

#[tokio::test]
async fn invalid_draft_is_typed_and_saves_nothing() {
    let harness = harness(Reply::Text("Hello."));
    let before = harness
        .context
        .backend()
        .database()
        .provider_accounts()
        .expect("accounts");
    let error = provider_verify(
        &harness.context,
        dto::ProviderVerifyRequest::Draft {
            draft: dto::ProviderVerifyDraft {
                provider_kind: "custom".into(),
                base_url: Some("https://user:secret@example.invalid".into()),
                api_key: Some("secret-canary".into()),
                config: serde_json::json!({"kind":"standard"}),
            },
        },
    )
    .await
    .expect_err("invalid endpoint");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert!(!format!("{error:?}").contains("secret-canary"));
    assert_eq!(
        harness
            .context
            .backend()
            .database()
            .provider_accounts()
            .expect("accounts"),
        before
    );
}

#[test]
fn verification_debug_redacts_draft_credentials() {
    let request = dto::ProviderVerifyRequest::Draft {
        draft: dto::ProviderVerifyDraft {
            provider_kind: "openai".into(),
            base_url: None,
            api_key: Some("secret-canary".into()),
            config: serde_json::json!({"kind":"standard"}),
        },
    };
    assert!(!format!("{request:?}").contains("secret-canary"));
}

#[tokio::test]
async fn bad_draft_key_keeps_error_text_redacts_reflection_and_never_writes() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let endpoint = format!("http://{}", listener.local_addr().expect("address"));
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut bytes = vec![0; 8192];
        let count = socket.read(&mut bytes).await.expect("request");
        let request = String::from_utf8(bytes[..count].to_vec()).expect("HTTP");
        assert!(request.starts_with("POST /draft-check?version=1 HTTP/1.1"));
        assert!(
            request
                .to_lowercase()
                .contains("x-draft-key: secret-canary")
        );
        let body = r#"{"error":{"message":"Your key secret-canary was refused"}}"#;
        socket.write_all(format!("HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.expect("reply");
    });
    let harness = harness(Reply::Text("Hello."));
    let before = harness
        .context
        .backend()
        .database()
        .provider_accounts()
        .expect("accounts");
    let error = provider_verify(&harness.context, dto::ProviderVerifyRequest::Draft {
        draft: dto::ProviderVerifyDraft {
            provider_kind: "custom".into(), base_url: Some(endpoint), api_key: Some("secret-canary".into()),
            config: serde_json::json!({"kind":"custom","chat_path":"/draft-check?version=1","models_path":null,"streaming":true,"auth":{"header":{"name":"x-draft-key"}}}),
        },
    }).await.expect_err("bad key");
    tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await
        .expect("server responded")
        .expect("server");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        error.details,
        Some(dto::ApiErrorDetails::ProviderVerification {
            status: Some(401),
            provider_message: "Your key [REDACTED] was refused".into(),
        })
    );
    assert!(
        !serde_json::to_string(&error)
            .expect("error")
            .contains("secret-canary")
    );
    assert_eq!(
        harness
            .context
            .backend()
            .database()
            .provider_accounts()
            .expect("accounts"),
        before
    );
}

#[tokio::test]
async fn certificate_listing_returns_metadata_without_pem_or_a_count_cap() {
    use lettuce_settings::{DeviceSettingsStore, TrustedCertificate};
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let mut settings = database.load_device_settings().expect("settings");
    settings.trusted_certificates.push(TrustedCertificate {
        id: uuid::Uuid::new_v4(),
        name: "company.pem".into(),
        pem: "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----".into(),
        imported_at: 123,
    });
    database
        .save_device_settings(settings.clone())
        .expect("certificates");
    let rows = super::certificates_list(&harness.context)
        .await
        .expect("certificates");
    assert_eq!(rows.certificates.len(), 1);
    assert_eq!(rows.certificates[0].name, "company.pem");
    assert_eq!(rows.certificates[0].imported_at, 123);
    assert!(
        !serde_json::to_string(&rows)
            .expect("views")
            .contains("CERTIFICATE")
    );
    assert_eq!(database.load_device_settings().expect("settings"), settings);
}
