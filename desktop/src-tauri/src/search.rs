use crate::errors::AppError;
use serde_json::Value;

pub async fn search_files(_query: &str, _limit: u32, _offset: u32) -> Result<Value, AppError> {
    todo!("Phase D: FTS5/BM25 + sqlite-vec cosine + RRF blend")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
