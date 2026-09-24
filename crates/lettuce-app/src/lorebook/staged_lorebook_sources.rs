use std::io::Read;

use lettuce_creation::{
    MAX_STAGED_LOREBOOK_SOURCE_BYTES, MAX_STAGED_LOREBOOK_TOTAL_SOURCE_BYTES,
    StagedLorebookSourceError, StagedLorebookSourceExcerpt, StagedLorebookSourceInput,
};
use lettuce_media::{AssetKind, LocalMediaBlobStore, MediaAssetRepository, MediaBlobRepository};
use lettuce_types::AssetId;

#[derive(Debug, thiserror::Error)]
pub enum StagedLorebookDocumentError {
    #[error("source document could not be opened: {0}")]
    Media(#[from] lettuce_media::MediaStoreError),
    #[error("source document bytes do not match their catalog record")]
    InvalidDocument,
    #[error("source document read failed")]
    Read,
    #[error("source preparation failed: {0}")]
    Source(#[from] StagedLorebookSourceError),
}

pub fn prepare_staged_lorebook_documents<B: MediaBlobRepository, A: MediaAssetRepository>(
    store: &LocalMediaBlobStore<B, A>,
    sources: &[(AssetId, String)],
) -> Result<Vec<StagedLorebookSourceExcerpt>, StagedLorebookDocumentError> {
    let inputs = sources
        .iter()
        .map(|(asset_id, label)| StagedLorebookIntakeSource::Document {
            asset_id: *asset_id,
            label,
        })
        .collect::<Vec<_>>();
    prepare_staged_lorebook_intake(store, &inputs)
}

pub enum StagedLorebookIntakeSource<'a> {
    Text { label: &'a str, body: &'a str },
    Document { asset_id: AssetId, label: &'a str },
}

impl std::fmt::Debug for StagedLorebookIntakeSource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text { body, .. } => f
                .debug_struct("Text")
                .field("byte_len", &body.len())
                .finish_non_exhaustive(),
            Self::Document { asset_id, .. } => f
                .debug_struct("Document")
                .field("asset_id", asset_id)
                .finish_non_exhaustive(),
        }
    }
}

pub fn prepare_staged_lorebook_intake<B: MediaBlobRepository, A: MediaAssetRepository>(
    store: &LocalMediaBlobStore<B, A>,
    sources: &[StagedLorebookIntakeSource<'_>],
) -> Result<Vec<StagedLorebookSourceExcerpt>, StagedLorebookDocumentError> {
    let mut excerpts = Vec::with_capacity(sources.len());
    let mut total = 0u64;
    for (index, source) in sources.iter().enumerate() {
        let (label, asset_id) = match source {
            StagedLorebookIntakeSource::Text { label, .. } => (*label, None),
            StagedLorebookIntakeSource::Document { asset_id, label } => (*label, Some(*asset_id)),
        };
        if label.trim().is_empty() {
            return Err(StagedLorebookSourceError::InvalidLabel.into());
        }
        let mut bytes = Vec::new();
        let input = match source {
            StagedLorebookIntakeSource::Text { body, .. } => {
                count_source_bytes(&mut total, body.len() as u64)?;
                StagedLorebookSourceInput::Text { label, body }
            }
            StagedLorebookIntakeSource::Document { asset_id, .. } => {
                let opened = store.open_ready(*asset_id)?;
                if opened.asset.kind != AssetKind::SourceDocument {
                    return Err(StagedLorebookDocumentError::InvalidDocument);
                }
                count_source_bytes(&mut total, opened.blob.byte_size)?;
                opened
                    .reader
                    .take(opened.blob.byte_size + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| StagedLorebookDocumentError::Read)?;
                if bytes.len() as u64 != opened.blob.byte_size
                    || blake3::hash(&bytes).to_hex().as_str() != opened.blob.content_hash.as_str()
                {
                    return Err(StagedLorebookDocumentError::InvalidDocument);
                }
                match opened.blob.mime_type.as_str() {
                    "application/pdf" => StagedLorebookSourceInput::PdfFile {
                        name: label,
                        bytes: &bytes,
                    },
                    "text/plain" => StagedLorebookSourceInput::Utf8File {
                        name: label,
                        bytes: &bytes,
                    },
                    _ => return Err(StagedLorebookDocumentError::InvalidDocument),
                }
            }
        };
        for mut excerpt in lettuce_creation::prepare_staged_lorebook_sources(&[input])? {
            excerpt.source_id = format!("src_{:02}", index + 1);
            excerpt.asset_id = asset_id;
            excerpts.push(excerpt);
        }
    }
    Ok(excerpts)
}

fn count_source_bytes(total: &mut u64, bytes: u64) -> Result<(), StagedLorebookSourceError> {
    if bytes > MAX_STAGED_LOREBOOK_SOURCE_BYTES as u64 {
        return Err(StagedLorebookSourceError::SourceTooLarge);
    }
    *total = total.saturating_add(bytes);
    if *total > MAX_STAGED_LOREBOOK_TOTAL_SOURCE_BYTES as u64 {
        return Err(StagedLorebookSourceError::TotalTooLarge);
    }
    Ok(())
}

impl crate::StagedLorebookConfiguredRequest {
    pub fn with_intake<B: MediaBlobRepository, A: MediaAssetRepository>(
        mut self,
        store: &LocalMediaBlobStore<B, A>,
        sources: &[StagedLorebookIntakeSource<'_>],
    ) -> Result<Self, StagedLorebookDocumentError> {
        self.excerpts = prepare_staged_lorebook_intake(store, sources)?;
        Ok(self)
    }

    pub fn with_documents<B: MediaBlobRepository, A: MediaAssetRepository>(
        mut self,
        store: &LocalMediaBlobStore<B, A>,
        sources: &[(AssetId, String)],
    ) -> Result<Self, StagedLorebookDocumentError> {
        self.excerpts = prepare_staged_lorebook_documents(store, sources)?;
        Ok(self)
    }
}

impl crate::StagedLorebookAdmissionRequest<'_> {
    pub fn with_documents<B: MediaBlobRepository, A: MediaAssetRepository>(
        mut self,
        store: &LocalMediaBlobStore<B, A>,
        sources: &[(AssetId, String)],
    ) -> Result<Self, StagedLorebookDocumentError> {
        self.excerpts = prepare_staged_lorebook_documents(store, sources)?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_media::{AssetOrigin, AssetProvenanceV1, IngestRequest, RetentionClass};
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};

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

    #[test]
    fn protected_documents_extract_pdf_and_text_without_raw_workflow_bytes() {
        let root = std::env::temp_dir().join(format!("lettuce-source-{}", AssetId::new()));
        let snapshot = DirectorySnapshot::new(&root).expect("directories");
        let authority = FilesystemAuthority::new(&snapshot).expect("authority");
        let catalog = root.join("catalog.sqlite");
        let store = LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read authority"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write authority"),
            lettuce_database::Database::open(&catalog).expect("blob catalog"),
            lettuce_database::Database::open(&catalog).expect("asset catalog"),
        );
        let ingest = |bytes: &[u8]| {
            store
                .ingest(
                    bytes,
                    IngestRequest::new(
                        AssetKind::SourceDocument,
                        AssetOrigin::Upload,
                        RetentionClass::Library,
                        AssetProvenanceV1::default(),
                    ),
                )
                .expect("source asset")
                .asset
                .id
        };
        let pdf_id = ingest(&pdf());
        let text_id = ingest("# World 🌍\nHarbour notes".as_bytes());
        let excerpts = prepare_staged_lorebook_documents(
            &store,
            &[(pdf_id, " Map.pdf ".into()), (text_id, "Notes.md".into())],
        )
        .expect("extract protected sources");
        assert_eq!(excerpts[0].source_id, "src_01");
        assert_eq!(excerpts[0].asset_id, Some(pdf_id));
        assert_eq!(excerpts[0].label, "Map.pdf");
        assert!(excerpts[0].content.contains("Harbour source text"));
        assert_eq!(excerpts[1].source_id, "src_02");
        assert_eq!(excerpts[1].asset_id, Some(text_id));
        assert_eq!(excerpts[1].content, "# World 🌍\nHarbour notes");
        let body = "🌍".repeat(20_001);
        let mixed = prepare_staged_lorebook_intake(
            &store,
            &[
                StagedLorebookIntakeSource::Text {
                    label: " Preface ",
                    body: &body,
                },
                StagedLorebookIntakeSource::Document {
                    asset_id: pdf_id,
                    label: "Map.pdf",
                },
                StagedLorebookIntakeSource::Text {
                    label: "Afterword",
                    body: "End notes",
                },
                StagedLorebookIntakeSource::Document {
                    asset_id: text_id,
                    label: "Notes.md",
                },
            ],
        )
        .expect("mixed intake");
        assert_eq!(
            mixed
                .iter()
                .map(|source| source.source_id.as_str())
                .collect::<Vec<_>>(),
            ["src_01", "src_02", "src_03", "src_04"]
        );
        assert_eq!(
            mixed
                .iter()
                .map(|source| source.asset_id)
                .collect::<Vec<_>>(),
            [None, Some(pdf_id), None, Some(text_id)]
        );
        assert_eq!(mixed[0].label, "Preface");
        assert_eq!(
            mixed[0].content,
            format!("{}\n[…truncated]", "🌍".repeat(20_000))
        );
        assert_eq!(mixed[1].content, excerpts[0].content);
        assert_eq!(mixed[2].content, "End notes");
        assert_eq!(mixed[3].content, excerpts[1].content);
        let large = "x".repeat(MAX_STAGED_LOREBOOK_SOURCE_BYTES);
        let mut at_limit = (0..4)
            .map(|_| StagedLorebookIntakeSource::Text {
                label: "Large notes",
                body: &large,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            prepare_staged_lorebook_intake(&store, &at_limit)
                .expect("exact total")
                .len(),
            4
        );
        at_limit.push(StagedLorebookIntakeSource::Document {
            asset_id: text_id,
            label: "Notes.md",
        });
        assert!(matches!(
            prepare_staged_lorebook_intake(&store, &at_limit),
            Err(StagedLorebookDocumentError::Source(
                StagedLorebookSourceError::TotalTooLarge
            ))
        ));
        at_limit.reverse();
        assert!(matches!(
            prepare_staged_lorebook_intake(&store, &at_limit),
            Err(StagedLorebookDocumentError::Source(
                StagedLorebookSourceError::TotalTooLarge
            ))
        ));
        let malformed = ingest(b"%PDF-1.4\nnot a PDF");
        assert!(matches!(
            prepare_staged_lorebook_documents(&store, &[(malformed, "bad.pdf".into())]),
            Err(StagedLorebookDocumentError::Source(
                StagedLorebookSourceError::InvalidPdf
            ))
        ));
        drop(store);
        drop(authority);
        std::fs::remove_dir_all(root).expect("cleanup fixture");
    }
}
