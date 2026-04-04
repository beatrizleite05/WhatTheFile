use crate::errors::AppError;

pub const OLLAMA_BASE_URL: &str = "http://localhost:11434";

pub async fn ensure_model_loaded(_model: &str) -> Result<(), AppError> {
    todo!("Phase D: check Ollama, pull model if needed, enforce single model lease")
}

pub async fn unload_model(_model: &str) -> Result<(), AppError> {
    todo!("Phase D: explicitly release model from Ollama memory")
}

pub async fn release_stale_models() -> Result<(), AppError> {
    todo!("Phase D: on startup, detect and release models from previous crash")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
