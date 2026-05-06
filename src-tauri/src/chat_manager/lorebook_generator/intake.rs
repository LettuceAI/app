use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use super::state::{SourceExcerpt, SourceFileKind, SourceInput, WorldDigest};

const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 200 * 1024 * 1024;
const MAX_EXCERPT_CHARS: usize = 20_000;

pub fn build_digest(sources: &[SourceInput]) -> Result<WorldDigest, String> {
    let mut excerpts = Vec::with_capacity(sources.len());
    let mut total_bytes = 0usize;

    for (i, source) in sources.iter().enumerate() {
        let id = format!("src_{:02}", i + 1);
        let (label, content) = match source {
            SourceInput::Text { label, body } => {
                if body.len() > MAX_FILE_BYTES {
                    return Err(format!("Pasted text \"{label}\" exceeds 50 MB limit"));
                }
                total_bytes = total_bytes.saturating_add(body.len());
                (label.clone(), truncate_to_chars(body, MAX_EXCERPT_CHARS))
            }
            SourceInput::File {
                name,
                data_base64,
                kind,
            } => {
                let bytes = BASE64
                    .decode(data_base64.as_bytes())
                    .map_err(|e| format!("Failed to decode \"{name}\": {e}"))?;
                if bytes.len() > MAX_FILE_BYTES {
                    return Err(format!("File \"{name}\" exceeds 50 MB limit"));
                }
                total_bytes = total_bytes.saturating_add(bytes.len());
                let text = match kind {
                    SourceFileKind::Txt | SourceFileKind::Md => String::from_utf8(bytes)
                        .map_err(|_| format!("File \"{name}\" is not valid UTF-8"))?,
                    SourceFileKind::Pdf => extract_pdf_text(&bytes)
                        .map_err(|e| format!("Failed to extract PDF \"{name}\": {e}"))?,
                };
                (name.clone(), truncate_to_chars(&text, MAX_EXCERPT_CHARS))
            }
        };

        if total_bytes > MAX_TOTAL_BYTES {
            return Err("Total source size exceeds 200 MB limit".to_string());
        }

        excerpts.push(SourceExcerpt { id, label, content });
    }

    Ok(WorldDigest { excerpts })
}

fn truncate_to_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out = String::with_capacity(max_chars);
    for (i, c) in s.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(c);
    }
    out.push_str("\n[…truncated]");
    out
}

fn extract_pdf_text(bytes: &[u8]) -> Result<String, String> {
    pdf_extract::extract_text_from_mem(bytes).map_err(|e| e.to_string())
}
