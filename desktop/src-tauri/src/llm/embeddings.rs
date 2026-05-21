use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use crate::errors::AppError;

const MODEL: &str = "nomic-embed-text-v2-moe";
pub const EXPECTED_DIM: usize = 768;
const EMBED_TIMEOUT_SECS: u64 = 180;

pub fn embed_text(text: &str, ollama_url: &str, cancel: &Arc<AtomicBool>) -> Result<Vec<f32>, AppError> {
    let mut results = embed_texts(&[text], ollama_url, cancel)?;
    results.pop().ok_or_else(|| AppError::Llm("empty embeddings array in response".into()))
}

pub fn embed_texts(
    texts: &[&str],
    ollama_url: &str,
    cancel: &Arc<AtomicBool>,
) -> Result<Vec<Vec<f32>>, AppError> {
    if texts.is_empty() {
        return Ok(vec![]);
    }

    let owned: Vec<String> = texts.iter().map(|s| (*s).to_string()).collect();
    let url = format!("{ollama_url}/api/embed");

    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(call_ollama(&url, &owned));
    });

    let mut elapsed = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Indexer("cancelled".into()));
        }
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                elapsed += 1;
                if elapsed >= EMBED_TIMEOUT_SECS {
                    return Err(AppError::Llm(format!(
                        "ollama embed timed out after {EMBED_TIMEOUT_SECS}s ({MODEL})"
                    )));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(AppError::Llm(format!("ollama embed thread disconnected ({MODEL})")));
            }
        }
    }
}

fn call_ollama(url: &str, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError> {
    let body = serde_json::json!({
        "model": MODEL,
        "input": texts,
        "keep_alive": "5m",
    });

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(EMBED_TIMEOUT_SECS))
        .build();

    let resp = agent
        .post(url)
        .send_json(body)
        .map_err(|e| AppError::Llm(format!("batch embed request failed: {e}")))?;

    let json: serde_json::Value = resp
        .into_json()
        .map_err(|e| AppError::Llm(format!("batch embed response parse failed: {e}")))?;

    let arr = json["embeddings"]
        .as_array()
        .ok_or_else(|| AppError::Llm("missing 'embeddings' field in batch response".into()))?;

    arr.iter()
        .map(|item| {
            let floats = item
                .as_array()
                .ok_or_else(|| AppError::Llm("embedding entry is not an array".into()))?;
            let vec: Vec<f32> = floats
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();
            if vec.len() != EXPECTED_DIM {
                return Err(AppError::Llm(format!(
                    "unexpected batch embedding dimension: got {}, expected {EXPECTED_DIM}",
                    vec.len()
                )));
            }
            Ok(vec)
        })
        .collect()
}

/// Serialise a `Vec<f32>` embedding to little-endian bytes for BLOB storage.
pub fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cancel_flag() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    #[test]
    fn test_embed_text_ollama_unreachable() {
        let result = embed_text("hello world", "http://127.0.0.1:19999", &cancel_flag());
        assert!(
            matches!(result, Err(AppError::Llm(_))),
            "expected Llm error when Ollama unreachable, got: {result:?}"
        );
    }

    #[test]
    fn test_embed_texts_empty_input_returns_empty() {
        let result = embed_texts(&[], "http://127.0.0.1:19999", &cancel_flag());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_embed_texts_ollama_unreachable() {
        let result = embed_texts(&["hello", "world"], "http://127.0.0.1:19999", &cancel_flag());
        assert!(
            matches!(result, Err(AppError::Llm(_))),
            "expected Llm error when Ollama unreachable, got: {result:?}"
        );
    }

    #[test]
    fn test_embed_texts_returns_cancelled_when_flag_set() {
        let cancel = Arc::new(AtomicBool::new(true));
        let result = embed_texts(&["hello"], "http://127.0.0.1:19999", &cancel);
        assert!(
            matches!(&result, Err(AppError::Indexer(msg)) if msg == "cancelled"),
            "expected Indexer(\"cancelled\"), got: {result:?}"
        );
    }

    #[test]
    fn test_embedding_to_bytes_roundtrip() {
        let original: Vec<f32> = (0..EXPECTED_DIM).map(|i| i as f32 * 0.1).collect();
        let bytes = embedding_to_bytes(&original);
        assert_eq!(bytes.len(), EXPECTED_DIM * 4);
        let recovered: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        for (a, b) in original.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }
}
