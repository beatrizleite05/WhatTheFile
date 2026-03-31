use crate::errors::AppError;

pub async fn extract_text(_path: &str) -> Result<String, AppError> {
    todo!("Phase C: implement per-type text extraction")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
