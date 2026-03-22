use std::sync::Mutex;
use tauri::AppHandle;
use tokenizers::Tokenizer;

static TOKENIZER: Mutex<Option<Tokenizer>> = Mutex::new(None);

/// Get or initialize the global tokenizer instance
fn get_tokenizer(app: &AppHandle) -> Result<Tokenizer, String> {
    let mut tokenizer_lock = TOKENIZER.lock().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to lock tokenizer: {}", e),
        )
    })?;

    if tokenizer_lock.is_none() {
        let model_dir = crate::embedding::embedding_model_dir(app)?;
        let tokenizer_path = model_dir.join("tokenizer.json");

        if !tokenizer_path.exists() {
            return Err(
                "Tokenizer not found. Please download the embedding model first.".to_string(),
            );
        }

        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to load tokenizer from {:?}: {}", tokenizer_path, e),
            )
        })?;

        *tokenizer_lock = Some(tokenizer);
    }

    Ok(tokenizer_lock.as_ref().unwrap().clone())
}

/// Count tokens in a text string using the embedding model's tokenizer
pub fn count_tokens(app: &AppHandle, text: &str) -> Result<u32, String> {
    let tokenizer = get_tokenizer(app)?;

    let encoding = tokenizer.encode(text, false).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Tokenization failed: {}", e),
        )
    })?;

    Ok(encoding.get_ids().len() as u32)
}

/// Count tokens for multiple texts
#[allow(dead_code)]
pub fn count_tokens_batch(app: &AppHandle, texts: &[String]) -> Result<Vec<u32>, String> {
    texts.iter().map(|text| count_tokens(app, text)).collect()
}
