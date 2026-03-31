use crate::errors::AppError;

pub async fn describe_image(_image_path: &str) -> Result<String, AppError> {
    todo!("Phase C: call Ollama qwen2.5vl:7b (fallback llava:7b), return text description")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
