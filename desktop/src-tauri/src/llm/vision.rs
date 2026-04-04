use crate::errors::AppError;
use base64::{engine::general_purpose::STANDARD as B64, Engine};

const PRIMARY_MODEL: &str = "qwen2.5vl:7b";
const FALLBACK_MODEL: &str = "llava:7b";
const PROMPT: &str = "Describe this image in detail, focusing on any text, objects, and context visible.";
// 180s covers cold-start model loading (~60-90s) plus inference time.
const VISION_TIMEOUT_SECS: u64 = 180;

/// Call the Ollama vision model to produce a text description of an image.
///
/// Tries `qwen2.5vl:7b` first; falls back to `llava:7b` if the primary model
/// is not available (Ollama returns a 404-style "model not found" error).
///
/// `ollama_base_url` — e.g. `"http://localhost:11434"`.
pub fn describe_image(image_path: &str, ollama_base_url: &str) -> Result<String, AppError> {
    let bytes = std::fs::read(image_path)
        .map_err(|e| AppError::Extractor(format!("cannot read image {image_path}: {e}")))?;
    let encoded = B64.encode(&bytes);

    // Try primary, fall back on model-not-found errors.
    match call_ollama_bounded(ollama_base_url, PRIMARY_MODEL, &encoded) {
        Ok(desc) => Ok(desc),
        Err(AppError::Llm(ref msg)) if msg.contains("model") && msg.contains("not found") => {
            call_ollama_bounded(ollama_base_url, FALLBACK_MODEL, &encoded)
        }
        Err(e) => Err(e),
    }
}

/// Calls Ollama on a background thread and waits at most `VISION_TIMEOUT_SECS`.
/// This enforces a hard wall-clock timeout regardless of HTTP chunked keepalives.
fn call_ollama_bounded(base_url: &str, model: &str, image_b64: &str) -> Result<String, AppError> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let base_url = base_url.to_string();
    let model = model.to_string();
    let image_b64 = image_b64.to_string();

    let model_name = model.clone();
    std::thread::spawn(move || {
        let _ = tx.send(call_ollama(&base_url, &model, &image_b64));
    });

    rx.recv_timeout(std::time::Duration::from_secs(VISION_TIMEOUT_SECS))
        .map_err(|_| AppError::Llm(format!("ollama timed out after {VISION_TIMEOUT_SECS}s ({model_name})")))?
}

fn call_ollama(base_url: &str, model: &str, image_b64: &str) -> Result<String, AppError> {
    let url = format!("{base_url}/api/generate");
    let body = serde_json::json!({
        "model": model,
        "prompt": PROMPT,
        "images": [image_b64],
        "stream": false
    });

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .build();
    let response = agent
        .post(&url)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| AppError::Llm(format!("ollama request failed ({model}): {e}")))?;

    let json: serde_json::Value = response
        .into_json()
        .map_err(|e| AppError::Llm(format!("failed to parse ollama response ({model}): {e}")))?;

    let text = json["response"]
        .as_str()
        .ok_or_else(|| AppError::Llm(format!("ollama response missing 'response' field ({model})")))?
        .to_string();

    if text.is_empty() {
        return Err(AppError::Llm(format!("ollama returned empty description ({model})")));
    }

    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_describe_image_ollama_unreachable() {
        // Point at a port where nothing is listening.
        let result = describe_image(
            "src/llm/vision.rs", // any readable file works for the read step
            "http://127.0.0.1:19999",
        );
        assert!(
            matches!(result, Err(AppError::Llm(_))),
            "expected Llm error when Ollama is unreachable, got: {result:?}"
        );
    }

    #[test]
    #[ignore = "requires Ollama running with qwen2.5vl:7b or llava:7b"]
    fn test_describe_image_live() {
        let img_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../test/sample-files/16626587.png");
        let desc = describe_image(img_path.to_str().unwrap(), "http://localhost:11434")
            .expect("live Ollama call should succeed");
        assert!(!desc.is_empty(), "description must not be empty");
    }
}
