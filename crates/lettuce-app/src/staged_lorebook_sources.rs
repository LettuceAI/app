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
    let mut documents = Vec::with_capacity(sources.len());
    let mut total = 0u64;
    for (id, label) in sources {
        if label.trim().is_empty() {
            return Err(StagedLorebookSourceError::InvalidLabel.into());
        }
        let opened = store.open_ready(*id)?;
        if opened.asset.kind != AssetKind::SourceDocument {
            return Err(StagedLorebookDocumentError::InvalidDocument);
        }
        if opened.blob.byte_size > MAX_STAGED_LOREBOOK_SOURCE_BYTES as u64 {
            return Err(StagedLorebookSourceError::SourceTooLarge.into());
        }
        total = total.saturating_add(opened.blob.byte_size);
        if total > MAX_STAGED_LOREBOOK_TOTAL_SOURCE_BYTES as u64 {
            return Err(StagedLorebookSourceError::TotalTooLarge.into());
        }
        let mut bytes = Vec::new();
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
        documents.push((label, opened.blob.mime_type, bytes));
    }
    let inputs = documents
        .iter()
        .map(|(label, mime, bytes)| match mime.as_str() {
            "application/pdf" => Ok(StagedLorebookSourceInput::PdfFile { name: label, bytes }),
            "text/plain" => Ok(StagedLorebookSourceInput::Utf8File { name: label, bytes }),
            _ => Err(StagedLorebookDocumentError::InvalidDocument),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut excerpts = lettuce_creation::prepare_staged_lorebook_sources(&inputs)?;
    for (excerpt, (id, _)) in excerpts.iter_mut().zip(sources) {
        excerpt.asset_id = Some(*id);
    }
    Ok(excerpts)
}

impl crate::StagedLorebookConfiguredRequest {
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
