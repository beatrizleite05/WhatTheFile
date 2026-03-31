use crate::errors::AppError;

pub async fn init_db(_db_path: &str) -> Result<(), AppError> {
    todo!("Phase D: initialize SQLite + sqlite-vec schema")
}

pub async fn upsert_file(_path: &str, _fingerprint: &str) -> Result<i64, AppError> {
    todo!("Phase D: upsert file record")
}

pub async fn delete_file(_path: &str) -> Result<(), AppError> {
    todo!("Phase D: delete file and its chunks")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
