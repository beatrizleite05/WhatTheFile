use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("indexer error: {0}")]
    Indexer(String),
    #[error("extractor error: {0}")]
    Extractor(String),
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("search error: {0}")]
    Search(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("tauri error: {0}")]
    Tauri(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
