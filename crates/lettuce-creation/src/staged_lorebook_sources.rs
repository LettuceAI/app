use crate::{MAX_STAGED_LOREBOOK_EXCERPT_CHARS, StagedLorebookSourceExcerpt};

pub const MAX_STAGED_LOREBOOK_SOURCE_BYTES: usize = 50 * 1024 * 1024;
pub const MAX_STAGED_LOREBOOK_TOTAL_SOURCE_BYTES: usize = 200 * 1024 * 1024;
pub const STAGED_LOREBOOK_TRUNCATION_MARKER: &str = "\n[…truncated]";

pub enum StagedLorebookSourceInput<'a> {
    Text { label: &'a str, body: &'a str },
    Utf8File { name: &'a str, bytes: &'a [u8] },
    PdfFile { name: &'a str, bytes: &'a [u8] },
}

impl std::fmt::Debug for StagedLorebookSourceInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, byte_len) = match self {
            Self::Text { body, .. } => ("Text", body.len()),
            Self::Utf8File { bytes, .. } => ("Utf8File", bytes.len()),
            Self::PdfFile { bytes, .. } => ("PdfFile", bytes.len()),
        };
        f.debug_struct(kind)
            .field("byte_len", &byte_len)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StagedLorebookSourceError {
    #[error("source label is blank")]
    InvalidLabel,
    #[error("source exceeds the legacy 50 MiB limit")]
    SourceTooLarge,
    #[error("sources exceed the legacy 200 MiB total limit")]
    TotalTooLarge,
    #[error("text file is not valid UTF-8")]
    InvalidUtf8,
    #[error("PDF text extraction failed")]
    InvalidPdf,
}

pub fn prepare_staged_lorebook_sources(
    sources: &[StagedLorebookSourceInput<'_>],
) -> Result<Vec<StagedLorebookSourceExcerpt>, StagedLorebookSourceError> {
    let mut excerpts = Vec::with_capacity(sources.len());
    let mut total_bytes = 0usize;
    for (index, source) in sources.iter().enumerate() {
        let (label, bytes) = match source {
            StagedLorebookSourceInput::Text { label, body } => (*label, body.as_bytes()),
            StagedLorebookSourceInput::Utf8File { name, bytes }
            | StagedLorebookSourceInput::PdfFile { name, bytes } => (*name, *bytes),
        };
        if bytes.len() > MAX_STAGED_LOREBOOK_SOURCE_BYTES {
            return Err(StagedLorebookSourceError::SourceTooLarge);
        }
        total_bytes = total_bytes.saturating_add(bytes.len());
        if total_bytes > MAX_STAGED_LOREBOOK_TOTAL_SOURCE_BYTES {
            return Err(StagedLorebookSourceError::TotalTooLarge);
        }
        let label = label.trim();
        if label.is_empty() {
            return Err(StagedLorebookSourceError::InvalidLabel);
        }
        let extracted;
        let text = if matches!(source, StagedLorebookSourceInput::PdfFile { .. }) {
            extracted = pdf_extract::extract_text_from_mem(bytes)
                .map_err(|_| StagedLorebookSourceError::InvalidPdf)?;
            extracted.as_str()
        } else {
            std::str::from_utf8(bytes).map_err(|_| StagedLorebookSourceError::InvalidUtf8)?
        };
        let mut chars = text.chars();
        let mut content: String = chars
            .by_ref()
            .take(MAX_STAGED_LOREBOOK_EXCERPT_CHARS)
            .collect();
        if chars.next().is_some() {
            content.push_str(STAGED_LOREBOOK_TRUNCATION_MARKER);
        }
        excerpts.push(StagedLorebookSourceExcerpt {
            source_id: format!("src_{:02}", index + 1),
            label: label.to_owned(),
            content,
        });
    }
    Ok(excerpts)
}

pub(crate) fn valid_staged_excerpt(content: &str) -> bool {
    content.chars().count() <= MAX_STAGED_LOREBOOK_EXCERPT_CHARS
        || content
            .strip_suffix(STAGED_LOREBOOK_TRUNCATION_MARKER)
            .is_some_and(|body| body.chars().count() == MAX_STAGED_LOREBOOK_EXCERPT_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_sources_preserve_order_and_the_legacy_truncation_marker() {
        let body = "🦀".repeat(MAX_STAGED_LOREBOOK_EXCERPT_CHARS + 1);
        let excerpts = prepare_staged_lorebook_sources(&[
            StagedLorebookSourceInput::Text {
                label: " Notes ",
                body: &body,
            },
            StagedLorebookSourceInput::Utf8File {
                name: "world.md",
                bytes: "# World 🌍\n".as_bytes(),
            },
        ])
        .expect("prepare sources");
        assert_eq!(excerpts[0].source_id, "src_01");
        assert_eq!(excerpts[1].source_id, "src_02");
        assert_eq!(excerpts[0].label, "Notes");
        assert_eq!(excerpts[1].content, "# World 🌍\n");
        assert_eq!(
            excerpts[0].content,
            format!(
                "{}{}",
                "🦀".repeat(MAX_STAGED_LOREBOOK_EXCERPT_CHARS),
                STAGED_LOREBOOK_TRUNCATION_MARKER
            )
        );
        crate::StagedLorebookProject::create(
            lettuce_types::CreationWorkflowId::new(),
            "World".into(),
            None,
            5,
            excerpts,
            lettuce_types::TimestampMillis::new(1),
        )
        .expect("marked excerpt is valid project input");
        assert!(!valid_staged_excerpt(&body));
        assert!(!valid_staged_excerpt(
            &(body + STAGED_LOREBOOK_TRUNCATION_MARKER)
        ));
        let exact = "x".repeat(MAX_STAGED_LOREBOOK_EXCERPT_CHARS);
        assert_eq!(
            prepare_staged_lorebook_sources(&[StagedLorebookSourceInput::Text {
                label: "exact",
                body: &exact
            }])
            .expect("exact boundary")[0]
                .content,
            exact
        );
    }

    #[test]
    fn legacy_byte_limits_and_invalid_utf8_are_rejected_before_admission() {
        assert_eq!(
            prepare_staged_lorebook_sources(&[StagedLorebookSourceInput::Utf8File {
                name: "bad.txt",
                bytes: &[0xff]
            }]),
            Err(StagedLorebookSourceError::InvalidUtf8)
        );
        let bytes = vec![b'x'; MAX_STAGED_LOREBOOK_SOURCE_BYTES + 1];
        assert_eq!(
            prepare_staged_lorebook_sources(&[StagedLorebookSourceInput::Utf8File {
                name: "large.txt",
                bytes: &bytes
            }]),
            Err(StagedLorebookSourceError::SourceTooLarge)
        );
        let sources: Vec<_> = (0..4)
            .map(|_| StagedLorebookSourceInput::Utf8File {
                name: "notes.txt",
                bytes: &bytes[..MAX_STAGED_LOREBOOK_SOURCE_BYTES],
            })
            .collect();
        assert_eq!(
            prepare_staged_lorebook_sources(&sources)
                .expect("exact total boundary")
                .len(),
            4
        );
        let mut sources = sources;
        sources.push(StagedLorebookSourceInput::Text {
            label: "extra",
            body: "x",
        });
        assert_eq!(
            prepare_staged_lorebook_sources(&sources),
            Err(StagedLorebookSourceError::TotalTooLarge)
        );
    }
}
