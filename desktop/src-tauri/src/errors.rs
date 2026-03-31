use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(String),
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
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
