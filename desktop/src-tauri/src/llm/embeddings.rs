use crate::errors::AppError;

/// nomic-embed-text-v2-moe returns 768-dimensional embeddings.
/// The db schema (chunks_vec) and this constant must stay in sync.
const MODEL: &str = "nomic-embed-text-v2-moe";
pub const EXPECTED_DIM: usize = 768;

pub fn embed_text(text: &str, ollama_url: &str) -> Result<Vec<f32>, AppError> {
    let mut results = embed_texts(&[text], ollama_url)?;
    results.pop().ok_or_else(|| AppError::Llm("empty embeddings array in response".into()))
}

/// Embed a batch of texts in a single Ollama request using `/api/embed`.
///
/// Ollama 0.1.31+ accepts `input` as an array, returning one embedding per
/// element in the same order.  This reduces N per-chunk HTTP round-trips to
/// one call per file during the indexing extraction pass.
pub fn embed_texts(texts: &[&str], ollama_url: &str) -> Result<Vec<Vec<f32>>, AppError> {
    if texts.is_empty() {
        return Ok(vec![]);
    }

    let url = format!("{ollama_url}/api/embed");
    let body = serde_json::json!({
        "model": MODEL,
        "input": texts,
        "keep_alive": "5m",
    });

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(120))
        .build();

    let resp = agent
        .post(&url)
        .send_json(body)
        .map_err(|e| AppError::Llm(format!("batch embed request failed: {e}")))?;

    let json: serde_json::Value = resp
        .into_json()
        .map_err(|e| AppError::Llm(format!("batch embed response parse failed: {e}")))?;

    // `/api/embed` returns `{ "embeddings": [[...], [...]] }`
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

    #[test]
    fn test_embed_text_ollama_unreachable() {
        let result = embed_text("hello world", "http://127.0.0.1:19999");
        assert!(
            matches!(result, Err(AppError::Llm(_))),
            "expected Llm error when Ollama unreachable, got: {result:?}"
        );
    }

    #[test]
    fn test_embed_texts_empty_input_returns_empty() {
        // Empty slice must short-circuit without hitting the network.
        let result = embed_texts(&[], "http://127.0.0.1:19999");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_embed_texts_ollama_unreachable() {
        let result = embed_texts(&["hello", "world"], "http://127.0.0.1:19999");
        assert!(
            matches!(result, Err(AppError::Llm(_))),
            "expected Llm error when Ollama unreachable, got: {result:?}"
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
