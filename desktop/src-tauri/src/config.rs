use crate::errors::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub indexed_roots: Vec<String>,
}

pub fn load_config() -> Result<AppConfig, AppError> {
    todo!("Phase B: load config from app data dir")
}

pub fn save_config(_config: &AppConfig) -> Result<(), AppError> {
    todo!("Phase B: persist config to app data dir")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
