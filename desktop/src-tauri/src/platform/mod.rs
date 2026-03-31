use crate::errors::AppError;
use std::path::PathBuf;

pub fn app_data_dir() -> Result<PathBuf, AppError> {
    todo!("platform-specific: ~/Library/Application Support/WhatTheFile (mac) or %LOCALAPPDATA%\\WhatTheFile (win)")
}

pub fn copy_to_clipboard(_text: &str) -> Result<(), AppError> {
    todo!("platform-specific clipboard write")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
