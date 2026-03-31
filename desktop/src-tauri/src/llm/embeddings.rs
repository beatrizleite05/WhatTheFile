use crate::errors::AppError;

pub async fn embed_text(_text: &str) -> Result<Vec<f32>, AppError> {
    todo!("Phase D: call Ollama nomic-embed-text, return 64-dim Vec<f32>")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
