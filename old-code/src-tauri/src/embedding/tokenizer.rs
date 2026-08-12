use std::path::PathBuf;
use std::sync::Mutex;
use tauri::AppHandle;
use tokenizers::Tokenizer;

static TOKENIZER: Mutex<Option<(PathBuf, Tokenizer)>> = Mutex::new(None);

fn get_tokenizer(app: &AppHandle) -> Result<Tokenizer, String> {
    let tokenizer_path = super::resolve_runtime_model(app)?.tokenizer_path;

    let mut tokenizer_lock = TOKENIZER.lock().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to lock tokenizer: {}", e),
        )
    })?;

    let needs_reload = match tokenizer_lock.as_ref() {
        Some((cached_path, _)) => cached_path != &tokenizer_path,
        None => true,
    };

    if needs_reload {
        if !tokenizer_path.exists() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Tokenizer not found at {:?}. Please download the embedding model first.",
                    tokenizer_path
                ),
            ));
        }

        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to load tokenizer from {:?}: {}", tokenizer_path, e),
            )
        })?;

        *tokenizer_lock = Some((tokenizer_path, tokenizer));
    }

    Ok(tokenizer_lock.as_ref().unwrap().1.clone())
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
