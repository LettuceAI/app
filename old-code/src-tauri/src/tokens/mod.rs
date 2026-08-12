use std::sync::OnceLock;

use tiktoken_rs::{o200k_base, CoreBPE};

static TOKENIZER: OnceLock<Option<CoreBPE>> = OnceLock::new();

fn tokenizer() -> Result<&'static CoreBPE, String> {
    let cached = TOKENIZER.get_or_init(|| o200k_base().ok());
    cached
        .as_ref()
        .ok_or_else(|| "Failed to initialize o200k tokenizer".to_string())
}

#[tauri::command]
pub fn tokens_count_batch(texts: Vec<String>) -> Result<Vec<u32>, String> {
    let bpe = tokenizer()?;
    Ok(texts
        .iter()
        .map(|t| bpe.encode_ordinary(t).len() as u32)
        .collect())
}
