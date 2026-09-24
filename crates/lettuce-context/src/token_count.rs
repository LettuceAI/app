//! Token counts shown next to prompt text, measured with the o200k encoding.

use std::sync::OnceLock;

use tiktoken_rs::CoreBPE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Failed to initialize o200k tokenizer")]
pub struct TokenizerUnavailable;

fn tokenizer() -> Result<&'static CoreBPE, TokenizerUnavailable> {
    static TOKENIZER: OnceLock<Option<CoreBPE>> = OnceLock::new();
    TOKENIZER
        .get_or_init(|| tiktoken_rs::o200k_base().ok())
        .as_ref()
        .ok_or(TokenizerUnavailable)
}

/// The o200k token count of each text, special tokens read as plain text.
pub fn count_tokens_batch(texts: &[String]) -> Result<Vec<u32>, TokenizerUnavailable> {
    let tokenizer = tokenizer()?;
    Ok(texts
        .iter()
        .map(|text| u32::try_from(tokenizer.encode_ordinary(text).len()).unwrap_or(u32::MAX))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_each_text() {
        let counts = count_tokens_batch(&[
            String::new(),
            "hello world".to_owned(),
            "<|endoftext|>".to_owned(),
        ])
        .expect("tokenizer");
        assert_eq!(counts[0], 0);
        assert_eq!(counts[1], 2);
        assert!(counts[2] > 1);
    }
}
