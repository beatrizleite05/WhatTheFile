use crate::errors::AppError;

pub const OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";

const KNOWN_MODELS: &[&str] = &["nomic-embed-text-v2-moe", "qwen2.5vl:7b", "llava:7b"];

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(120))
        .build()
}

/// Lightweight agent for health checks — tolerant enough for a warming Ollama.
fn health_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(5))
        .timeout_read(std::time::Duration::from_secs(5))
        .build()
}

#[allow(dead_code)]
fn get_tags(ollama_url: &str) -> Result<serde_json::Value, AppError> {
    let url = format!("{ollama_url}/api/tags");
    agent()
        .get(&url)
        .call()
        .map_err(|e| AppError::Llm(format!("Ollama unreachable: {e}")))?
        .into_json()
        .map_err(|e| AppError::Llm(format!("tags parse error: {e}")))
}

pub fn check_ollama(ollama_url: &str) -> Result<bool, AppError> {
    let url = format!("{ollama_url}/api/tags");
    Ok(health_agent().get(&url).call().is_ok())
}

/// Returns models currently loaded in Ollama VRAM via `/api/ps`.
pub fn list_loaded_models(ollama_url: &str) -> Result<Vec<String>, AppError> {
    let url = format!("{ollama_url}/api/ps");
    let json: serde_json::Value = agent()
        .get(&url)
        .call()
        .map_err(|e| AppError::Llm(format!("Ollama /api/ps unreachable: {e}")))?
        .into_json()
        .map_err(|e| AppError::Llm(format!("/api/ps parse error: {e}")))?;
    let names = json["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    Ok(names)
}

#[allow(dead_code)]
pub fn ensure_model_loaded(model: &str, ollama_url: &str) -> Result<(), AppError> {
    let json = get_tags(ollama_url)?;
    let loaded = json["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .any(|m| m["name"].as_str().map_or(false, |n| n.starts_with(model)))
        })
        .unwrap_or(false);

    if !loaded {
        let url = format!("{ollama_url}/api/pull");
        let body = serde_json::json!({ "name": model, "stream": false });
        ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(10))
            .timeout_read(std::time::Duration::from_secs(300))
            .build()
            .post(&url)
            .send_json(body)
            .map_err(|e| AppError::Llm(format!("model pull failed for '{model}': {e}")))?;
    }
    Ok(())
}

pub fn unload_model(model: &str, ollama_url: &str) -> Result<(), AppError> {
    let url = format!("{ollama_url}/api/generate");
    let body = serde_json::json!({ "model": model, "keep_alive": "0m", "prompt": "" });
    agent()
        .post(&url)
        .send_json(body)
        .map_err(|e| AppError::Llm(format!("unload failed for '{model}': {e}")))?;
    Ok(())
}

pub fn release_stale_models(ollama_url: &str) -> Result<(), AppError> {
    // Use /api/ps (running models only) so we don't trigger a load+unload cycle
    // on every downloaded model that isn't currently in VRAM.
    let url = format!("{ollama_url}/api/ps");
    let json = match agent().get(&url).call() {
        Ok(r) => match r.into_json::<serde_json::Value>() {
            Ok(j) => j,
            Err(_) => return Ok(()),
        },
        Err(_) => return Ok(()), // Ollama not running — nothing to release
    };
    let loaded_names: Vec<String> = json["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    for name in &loaded_names {
        if KNOWN_MODELS.iter().any(|k| name.starts_with(k)) {
            let _ = unload_model(name, ollama_url);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_ollama_unreachable() {
        assert!(!check_ollama("http://127.0.0.1:19999").unwrap_or(false));
    }

    #[test]
    fn test_list_loaded_models_unreachable_returns_err() {
        let result = list_loaded_models("http://127.0.0.1:19999");
        assert!(matches!(result, Err(AppError::Llm(_))));
    }

    #[test]
    fn test_ensure_model_loaded_unreachable() {
        let result = ensure_model_loaded("nomic-embed-text-v2-moe", "http://127.0.0.1:19999");
        assert!(matches!(result, Err(AppError::Llm(_))));
    }

    #[test]
    fn test_model_list_parsing() {
        let json: serde_json::Value = serde_json::json!({
            "models": [
                { "name": "nomic-embed-text-v2-moe:latest" },
                { "name": "llava:7b" }
            ]
        });
        let names: Vec<&str> = json["models"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m["name"].as_str())
            .collect();
        assert!(names.iter().any(|n| n.starts_with("nomic-embed-text-v2-moe")));
        assert!(!names.iter().any(|n| n.starts_with("qwen2.5vl")));
    }

    #[test]
    fn test_release_stale_models_noop_when_ollama_unreachable() {
        // Must not panic or return an error when Ollama is not running
        let result = release_stale_models("http://127.0.0.1:19999");
        assert!(result.is_ok());
    }

    #[test]
    fn test_unload_model_returns_err_when_ollama_unreachable() {
        // unload_model returns Err when Ollama is not running — callers are
        // expected to ignore this error (index completion must not fail).
        let result = unload_model("nomic-embed-text-v2-moe", "http://127.0.0.1:19999");
        assert!(matches!(result, Err(AppError::Llm(_))));
    }

    #[test]
    fn test_post_index_unload_loop_does_not_panic_when_unreachable() {
        // Simulate the exact unload loop from indexer::run_scan.
        // Errors are ignored, so this must never panic.
        for model in &["nomic-embed-text-v2-moe", "qwen2.5vl:7b", "llava:7b"] {
            let _ = unload_model(model, "http://127.0.0.1:19999");
        }
    }
}
