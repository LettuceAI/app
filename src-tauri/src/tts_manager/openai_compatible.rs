use futures_util::{stream, StreamExt};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;

use super::types::AudioModel;

const DEFAULT_REQUEST_PATH: &str = "/v1/audio/speech";
const MAX_TTS_CHUNK_CHARS: usize = 300;
const MAX_CONCURRENT_CHUNK_REQUESTS: usize = 3;

pub fn default_models() -> Vec<AudioModel> {
    vec![
        AudioModel {
            id: "gpt-4o-mini-tts".to_string(),
            name: "gpt-4o-mini-tts".to_string(),
            provider_type: "openai_tts".to_string(),
        },
        AudioModel {
            id: "tts-1".to_string(),
            name: "tts-1".to_string(),
            provider_type: "openai_tts".to_string(),
        },
        AudioModel {
            id: "tts-1-hd".to_string(),
            name: "tts-1-hd".to_string(),
            provider_type: "openai_tts".to_string(),
        },
    ]
}

#[derive(Serialize)]
struct OpenAiTtsRequest<'a> {
    model: &'a str,
    input: &'a str,
    voice: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<&'a str>,
    response_format: &'a str,
}

fn normalize_base_url(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}

fn normalize_request_path(request_path: Option<&str>) -> String {
    let trimmed = request_path.unwrap_or(DEFAULT_REQUEST_PATH).trim();
    if trimmed.is_empty() {
        return DEFAULT_REQUEST_PATH.to_string();
    }
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    }
}

pub async fn generate_speech(
    base_url: &str,
    request_path: Option<&str>,
    api_key: &str,
    text: &str,
    voice_id: &str,
    model: &str,
    prompt: Option<&str>,
) -> Result<(Vec<u8>, String), String> {
    let url = format!(
        "{}{}",
        normalize_base_url(base_url),
        normalize_request_path(request_path)
    );

    let instructions = prompt.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    let client = reqwest::Client::new();
    let chunks = split_text_chunks(text);

    if chunks.len() <= 1 {
        let input = chunks.first().map(String::as_str).unwrap_or(text);
        return request_speech_chunk(&client, &url, api_key, input, voice_id, model, instructions)
            .await;
    }

    match request_chunked_speech(
        &client,
        &url,
        api_key,
        chunks,
        voice_id,
        model,
        instructions,
    )
    .await
    {
        Ok(result) => Ok(result),
        Err(chunked_error) => {
            request_speech_chunk(&client, &url, api_key, text, voice_id, model, instructions)
                .await
                .map_err(|fallback_error| {
                    let message = format!(
                        "Chunked OpenAI-compatible TTS failed ({}); unchunked fallback failed ({})",
                        chunked_error, fallback_error
                    );
                    crate::utils::err_msg(module_path!(), line!(), message)
                })
        }
    }
}

async fn request_chunked_speech(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    chunks: Vec<String>,
    voice_id: &str,
    model: &str,
    instructions: Option<&str>,
) -> Result<(Vec<u8>, String), String> {
    let total_chunks = chunks.len();
    let requests = stream::iter(chunks.into_iter().enumerate())
        .map(|(index, chunk)| {
            let client = client.clone();
            let url = url.to_string();
            let api_key = api_key.to_string();
            let voice_id = voice_id.to_string();
            let model = model.to_string();
            let instructions = instructions.map(str::to_string);

            async move {
                request_speech_chunk(
                    &client,
                    &url,
                    &api_key,
                    &chunk,
                    &voice_id,
                    &model,
                    instructions.as_deref(),
                )
                .await
                .map_err(|err| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!(
                            "OpenAI-compatible TTS chunk {}/{} failed: {}",
                            index + 1,
                            total_chunks,
                            err
                        ),
                    )
                })
            }
        })
        .buffered(MAX_CONCURRENT_CHUNK_REQUESTS);

    futures_util::pin_mut!(requests);

    let mut audio_chunks = Vec::new();
    let mut format = None::<String>;

    while let Some(result) = requests.next().await {
        let (chunk_audio, chunk_format) = result?;
        if chunk_audio.is_empty() {
            continue;
        }

        match format.as_deref() {
            Some(existing) if media_type(existing) != media_type(&chunk_format) => {
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!(
                        "OpenAI-compatible TTS chunks returned mixed audio formats: {} and {}",
                        existing, chunk_format
                    ),
                ));
            }
            None => format = Some(chunk_format),
            _ => {}
        }

        audio_chunks.push(chunk_audio);
    }

    let format = format.unwrap_or_else(|| "audio/wav".to_string());
    let audio = concatenate_audio_chunks(&audio_chunks, &format).map_err(|err| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Failed to concatenate OpenAI-compatible TTS chunks: {}",
                err
            ),
        )
    })?;

    Ok((audio, format))
}

async fn request_speech_chunk(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    text: &str,
    voice_id: &str,
    model: &str,
    instructions: Option<&str>,
) -> Result<(Vec<u8>, String), String> {
    let request = OpenAiTtsRequest {
        model,
        input: text,
        voice: voice_id,
        instructions,
        response_format: "wav",
    };

    let response = client
        .post(url)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .header(CONTENT_TYPE, "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Request failed: {}", e))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("OpenAI-compatible TTS error ({}): {}", status, body),
        ));
    }

    let format = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or("audio/wav")
                .trim()
                .to_string()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "audio/wav".to_string());

    response
        .bytes()
        .await
        .map(|bytes| (bytes.to_vec(), format))
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read audio: {}", e),
            )
        })
}

fn media_type(format: &str) -> &str {
    format.split(';').next().unwrap_or(format).trim()
}

fn concatenate_audio_chunks(chunks: &[Vec<u8>], format: &str) -> Result<Vec<u8>, String> {
    if chunks.is_empty() {
        return Ok(Vec::new());
    }
    if chunks.len() == 1 {
        return Ok(chunks[0].clone());
    }

    match media_type(format) {
        "audio/mpeg" | "audio/mp3" => Ok(concatenate_mp3_chunks(chunks)),
        "audio/wav" | "audio/wave" | "audio/x-wav" => concatenate_wav_chunks(chunks),
        other => Err(format!(
            "chunked response format '{}' cannot be safely concatenated",
            other
        )),
    }
}

fn concatenate_mp3_chunks(chunks: &[Vec<u8>]) -> Vec<u8> {
    let total_len = chunks.iter().map(Vec::len).sum();
    let mut combined = Vec::with_capacity(total_len);
    for chunk in chunks {
        combined.extend_from_slice(strip_mp3_metadata(chunk));
    }
    combined
}

fn strip_mp3_metadata(bytes: &[u8]) -> &[u8] {
    let mut start = 0usize;

    while bytes.len().saturating_sub(start) >= 10 && &bytes[start..start + 3] == b"ID3" {
        let Some(tag_size) = synchsafe_u32(&bytes[start + 6..start + 10]) else {
            break;
        };
        let footer_size = if bytes[start + 5] & 0x10 != 0 { 10 } else { 0 };
        let next = start.saturating_add(10 + tag_size + footer_size);
        if next > bytes.len() {
            break;
        }
        start = next;
    }

    let mut end = bytes.len();
    if end.saturating_sub(start) >= 128 && &bytes[end - 128..end - 125] == b"TAG" {
        end -= 128;
    }

    &bytes[start..end]
}

fn synchsafe_u32(bytes: &[u8]) -> Option<usize> {
    if bytes.len() != 4 || bytes.iter().any(|byte| byte & 0x80 != 0) {
        return None;
    }

    Some(
        ((bytes[0] as usize) << 21)
            | ((bytes[1] as usize) << 14)
            | ((bytes[2] as usize) << 7)
            | bytes[3] as usize,
    )
}

struct WavParts {
    fmt: Vec<u8>,
    data: Vec<u8>,
}

fn parse_wav(bytes: &[u8]) -> Result<WavParts, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("invalid WAV header".to_string());
    }

    let mut offset = 12usize;
    let mut fmt = None;
    let mut data = Vec::new();

    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let data_start = offset + 8;
        let data_end = data_start
            .checked_add(chunk_size)
            .ok_or_else(|| "WAV chunk size overflow".to_string())?;

        if data_end > bytes.len() {
            return Err("truncated WAV chunk".to_string());
        }

        match chunk_id {
            b"fmt " => fmt = Some(bytes[data_start..data_end].to_vec()),
            b"data" => data.extend_from_slice(&bytes[data_start..data_end]),
            _ => {}
        }

        offset = data_end + (chunk_size % 2);
    }

    let fmt = fmt.ok_or_else(|| "WAV is missing fmt chunk".to_string())?;
    Ok(WavParts { fmt, data })
}

fn concatenate_wav_chunks(chunks: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let mut fmt = None::<Vec<u8>>;
    let mut data = Vec::new();

    for (index, chunk) in chunks.iter().enumerate() {
        let parts = parse_wav(chunk)
            .map_err(|err| format!("chunk {}/{}: {}", index + 1, chunks.len(), err))?;
        match fmt.as_deref() {
            Some(existing) if existing != parts.fmt.as_slice() => {
                return Err(format!(
                    "chunk {}/{} has a different WAV format",
                    index + 1,
                    chunks.len()
                ));
            }
            None => fmt = Some(parts.fmt),
            _ => {}
        }
        data.extend_from_slice(&parts.data);
    }

    let fmt = fmt.unwrap_or_default();
    let fmt_padding = fmt.len() % 2;
    let data_padding = data.len() % 2;
    let riff_size = 4usize
        .checked_add(8 + fmt.len() + fmt_padding)
        .and_then(|size| size.checked_add(8 + data.len() + data_padding))
        .ok_or_else(|| "combined WAV size overflow".to_string())?;

    if riff_size > u32::MAX as usize
        || data.len() > u32::MAX as usize
        || fmt.len() > u32::MAX as usize
    {
        return Err("combined WAV is too large".to_string());
    }

    let mut output = Vec::with_capacity(8 + riff_size);
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&(riff_size as u32).to_le_bytes());
    output.extend_from_slice(b"WAVE");
    output.extend_from_slice(b"fmt ");
    output.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    output.extend_from_slice(&fmt);
    if fmt_padding != 0 {
        output.push(0);
    }
    output.extend_from_slice(b"data");
    output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    output.extend_from_slice(&data);
    if data_padding != 0 {
        output.push(0);
    }

    Ok(output)
}

fn split_text_chunks(text: &str) -> Vec<String> {
    split_text_chunks_with_limit(text, MAX_TTS_CHUNK_CHARS)
}

fn split_text_chunks_with_limit(text: &str, max_chars: usize) -> Vec<String> {
    let max_chars = max_chars.max(1);
    let min_split_chars = (max_chars / 2).max(1);
    let mut remaining = text.trim();
    let mut chunks = Vec::new();

    while !remaining.is_empty() {
        if remaining.chars().count() <= max_chars {
            chunks.push(remaining.to_string());
            break;
        }

        let max_byte = byte_index_after_chars(remaining, max_chars);
        let split_at = find_split_index(remaining, max_byte, min_split_chars);
        let split_at = if split_at == 0 { max_byte } else { split_at };
        let (chunk, rest) = remaining.split_at(split_at);
        let chunk = chunk.trim();
        if !chunk.is_empty() {
            chunks.push(chunk.to_string());
        }
        remaining = rest.trim_start();
    }

    chunks
}

fn byte_index_after_chars(text: &str, count: usize) -> usize {
    if count == 0 {
        return 0;
    }

    text.char_indices()
        .nth(count)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

fn find_split_index(text: &str, max_byte: usize, min_split_chars: usize) -> usize {
    let min_byte = byte_index_after_chars(text, min_split_chars);

    find_last_boundary(text, max_byte, min_byte, is_sentence_boundary)
        .or_else(|| find_last_boundary(text, max_byte, min_byte, is_clause_boundary))
        .or_else(|| find_last_boundary(text, max_byte, min_byte, char::is_whitespace))
        .unwrap_or(max_byte)
}

fn find_last_boundary<F>(
    text: &str,
    max_byte: usize,
    min_byte: usize,
    predicate: F,
) -> Option<usize>
where
    F: Fn(char) -> bool,
{
    let mut best = None;
    for (index, ch) in text[..max_byte].char_indices() {
        let end = index + ch.len_utf8();
        if end >= min_byte && predicate(ch) {
            best = Some(include_trailing_closers(text, end, max_byte));
        }
    }
    best
}

fn include_trailing_closers(text: &str, mut index: usize, max_byte: usize) -> usize {
    while index < max_byte {
        let Some(ch) = text[index..max_byte].chars().next() else {
            break;
        };
        if is_trailing_closer(ch) {
            index += ch.len_utf8();
        } else {
            break;
        }
    }
    index
}

fn is_sentence_boundary(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | '…' | '。' | '！' | '？' | '\n' | '\r')
}

fn is_clause_boundary(ch: char) -> bool {
    matches!(ch, ',' | ';' | ':' | '，' | '；' | '：' | '、' | '—' | '–')
}

fn is_trailing_closer(ch: char) -> bool {
    matches!(
        ch,
        '"' | '\'' | '”' | '’' | ')' | ']' | '}' | '»' | '›' | '）' | '】' | '』' | '」'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_stays_in_one_chunk() {
        let chunks = split_text_chunks_with_limit("Hello there. This is short.", 100);
        assert_eq!(chunks, vec!["Hello there. This is short."]);
    }

    #[test]
    fn prefers_sentence_boundary_before_limit() {
        let text = "First sentence is short. Second sentence keeps going. Third sentence.";
        let chunks = split_text_chunks_with_limit(text, 32);

        assert_eq!(chunks[0], "First sentence is short.");
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 32));
        assert_eq!(chunks.join(" "), text);
    }

    #[test]
    fn falls_back_to_word_boundary() {
        let text = "alpha beta gamma delta epsilon zeta eta theta";
        let chunks = split_text_chunks_with_limit(text, 20);

        assert_eq!(
            chunks,
            vec!["alpha beta gamma", "delta epsilon zeta", "eta theta"]
        );
    }

    #[test]
    fn splits_long_word_on_utf8_boundary() {
        let text = "ååååååååååååååå";
        let chunks = split_text_chunks_with_limit(text, 4);

        assert_eq!(chunks, vec!["åååå", "åååå", "åååå", "ååå"]);
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 4));
    }

    #[test]
    fn includes_trailing_quotes_with_sentence_boundary() {
        let text = "She said, “This sentence fits.” Then she continued with more words.";
        let chunks = split_text_chunks_with_limit(text, 36);

        assert_eq!(chunks[0], "She said, “This sentence fits.”");
    }

    #[test]
    fn strips_mp3_metadata_before_concatenating() {
        let mut tag = Vec::from(b"ID3\x04\0\0\0\0\0\0" as &[u8]);
        tag.extend_from_slice(b"frame-a");
        let mut trailing_tag = vec![0u8; 128];
        trailing_tag[0..3].copy_from_slice(b"TAG");
        tag.extend_from_slice(&trailing_tag);

        let chunk_b = Vec::from(b"ID3\x04\0\0\0\0\0\0frame-b" as &[u8]);
        let combined = concatenate_mp3_chunks(&[tag, chunk_b]);

        assert_eq!(combined, b"frame-aframe-b");
    }

    #[test]
    fn concatenates_wav_chunks() {
        let fmt = vec![1, 0, 1, 0, 0x80, 0xbb, 0, 0, 0, 0x77, 1, 0, 2, 0, 16, 0];
        let first = test_wav(&fmt, &[1, 2, 3, 4]);
        let second = test_wav(&fmt, &[5, 6, 7, 8]);

        let combined = concatenate_wav_chunks(&[first, second]).unwrap();
        let parts = parse_wav(&combined).unwrap();

        assert_eq!(parts.fmt, fmt);
        assert_eq!(parts.data, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    fn test_wav(fmt: &[u8], data: &[u8]) -> Vec<u8> {
        let riff_size = 4 + 8 + fmt.len() + 8 + data.len();
        let mut output = Vec::new();
        output.extend_from_slice(b"RIFF");
        output.extend_from_slice(&(riff_size as u32).to_le_bytes());
        output.extend_from_slice(b"WAVE");
        output.extend_from_slice(b"fmt ");
        output.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        output.extend_from_slice(fmt);
        output.extend_from_slice(b"data");
        output.extend_from_slice(&(data.len() as u32).to_le_bytes());
        output.extend_from_slice(data);
        output
    }
}
