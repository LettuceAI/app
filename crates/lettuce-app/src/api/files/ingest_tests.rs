use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_media::{AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, RetentionClass};
use lettuce_types::OperationId;

use crate::api::tests::{RecordingEvents, StdFiles};
use crate::api::{ApiContext, assets_ingest, lorebook_project_create, lorebook_project_get};

fn context() -> (std::path::PathBuf, ApiContext) {
    let root = std::env::temp_dir().join(format!("lettuce-document-ingest-{}", OperationId::new()));
    std::fs::create_dir_all(&root).expect("root");
    let context = ApiContext::open_desktop(
        &root,
        None,
        Arc::new(lettuce_settings::InMemorySecretStore::new()),
        Arc::new(RecordingEvents::default()),
        Arc::new(StdFiles),
        "lettuce-asset://localhost".into(),
        None,
    )
    .expect("context");
    (root, context)
}

fn pdf() -> Vec<u8> {
    let stream = "BT /F1 12 Tf 20 80 Td (Harbour source text) Tj ET";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
    ];
    let mut document = "%PDF-1.4\n".to_owned();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(document.len());
        document.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
    }
    let xref = document.len();
    document.push_str("xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        document.push_str(&format!("{offset:010} 00000 n \n"));
    }
    document.push_str(&format!(
        "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    ));
    document.into_bytes()
}

#[tokio::test]
async fn source_document_ingest_keeps_names_and_projects_keep_source_identities() {
    let (root, context) = context();
    let path = root.join("World notes.md");
    std::fs::write(&path, "# Coast\nA harbour and its districts.").expect("notes");
    let uploaded = assets_ingest(
        &context,
        dto::AssetsIngestRequest {
            source: dto::FileSource {
                uri: path.display().to_string(),
            },
            role: dto::AssetIngestRole::SourceDocument,
        },
    )
    .await
    .expect("document ingest");
    let asset = context
        .media()
        .expect("media")
        .open_ready(uploaded.asset_id.parse().expect("asset id"))
        .expect("opened source")
        .asset;
    assert_eq!(asset.kind, AssetKind::SourceDocument);
    assert_eq!(
        asset.provenance.source_label.as_deref(),
        Some("World notes.md")
    );
    assert_eq!(asset.provenance.source_uri_redacted, None);
    std::fs::remove_file(&path).expect("original file is no longer needed");
    let legacy = context
        .media()
        .expect("media")
        .ingest(
            b"An older source without an original name".as_slice(),
            IngestRequest::new(
                AssetKind::SourceDocument,
                AssetOrigin::Legacy,
                RetentionClass::Persistent,
                AssetProvenanceV1::default(),
            ),
        )
        .expect("old source")
        .asset
        .id;
    let request = dto::LorebookProjectCreateRequest {
        client_operation_id: OperationId::new().to_string(),
        brief: "Describe the coast".into(),
        lorebook_name: Some("Coast".into()),
        target_count: Some(5),
        sources: vec![
            dto::LorebookProjectSourceInput::Text {
                label: "A note".into(),
                text: "Keep the harbour central.".into(),
            },
            dto::LorebookProjectSourceInput::Document {
                asset_id: uploaded.asset_id.clone(),
            },
            dto::LorebookProjectSourceInput::Document {
                asset_id: legacy.to_string(),
            },
        ],
    };
    let project = lorebook_project_create(&context, request.clone())
        .await
        .expect("project");
    assert_eq!(project.sources.len(), 3);
    assert_eq!(project.sources[0].label, "A note");
    assert_eq!(
        project.sources[0].text.as_deref(),
        Some("Keep the harbour central.")
    );
    assert_eq!(project.sources[1].source_id, "src_02");
    assert_eq!(project.sources[1].label, "World notes.md");
    assert_eq!(project.sources[1].text, None);
    assert_eq!(
        project.sources[1]
            .document
            .as_ref()
            .expect("document")
            .asset_id,
        uploaded.asset_id
    );
    assert_eq!(project.sources[2].label, legacy.to_string());
    assert_eq!(
        lorebook_project_create(&context, request)
            .await
            .expect("replay"),
        project
    );
    assert_eq!(
        lorebook_project_get(
            &context,
            dto::LorebookProjectGetRequest {
                project_id: project.project_id.clone(),
            }
        )
        .await
        .expect("reload"),
        project
    );
    drop(context);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn source_document_ingest_accepts_pdf_and_rejects_binary_and_wrong_roles() {
    let (root, context) = context();
    let path = root.join("Map.pdf");
    std::fs::write(&path, pdf()).expect("pdf");
    let uploaded = assets_ingest(
        &context,
        dto::AssetsIngestRequest {
            source: dto::FileSource {
                uri: path.display().to_string(),
            },
            role: dto::AssetIngestRole::SourceDocument,
        },
    )
    .await
    .expect("pdf ingest");
    let opened = context
        .media()
        .expect("media")
        .open_ready(uploaded.asset_id.parse().expect("asset id"))
        .expect("pdf source");
    assert_eq!(opened.blob.mime_type, "application/pdf");
    assert_eq!(
        opened.asset.provenance.source_label.as_deref(),
        Some("Map.pdf")
    );
    drop(opened);
    let project = lorebook_project_create(
        &context,
        dto::LorebookProjectCreateRequest {
            client_operation_id: OperationId::new().to_string(),
            brief: "Describe the harbour".into(),
            lorebook_name: Some("Map".into()),
            target_count: Some(5),
            sources: vec![dto::LorebookProjectSourceInput::Document {
                asset_id: uploaded.asset_id,
            }],
        },
    )
    .await
    .expect("PDF text extraction through the API");
    assert_eq!(project.sources[0].label, "Map.pdf");
    assert_eq!(project.sources[0].text, None);
    let wrong_role = assets_ingest(
        &context,
        dto::AssetsIngestRequest {
            source: dto::FileSource {
                uri: path.display().to_string(),
            },
            role: dto::AssetIngestRole::Attachment,
        },
    )
    .await
    .expect_err("document is not a chat image");
    assert_eq!(wrong_role.code, ApiErrorCode::InvalidInput);
    let invalid = root.join("Invalid.txt");
    std::fs::write(&invalid, [0xff, 0xfe]).expect("invalid text");
    let error = assets_ingest(
        &context,
        dto::AssetsIngestRequest {
            source: dto::FileSource {
                uri: invalid.display().to_string(),
            },
            role: dto::AssetIngestRole::SourceDocument,
        },
    )
    .await
    .expect_err("invalid text is refused");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    drop(context);
    std::fs::remove_dir_all(root).expect("cleanup");
}
