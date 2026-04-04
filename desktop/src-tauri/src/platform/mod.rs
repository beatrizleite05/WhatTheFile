use crate::errors::AppError;
use std::path::PathBuf;

pub fn app_data_dir() -> Result<PathBuf, AppError> {
    todo!("platform-specific: ~/Library/Application Support/WhatTheFile (mac) or %LOCALAPPDATA%\\WhatTheFile (win)")
}

pub fn copy_to_clipboard(_text: &str) -> Result<(), AppError> {
    todo!("platform-specific clipboard write")
}

pub fn pdfium_dir() -> String {
    std::env::var("PDFIUM_DIR").unwrap_or_else(|_| "./".to_string())
}

/// Return the directory that contains Tesseract language data files.
///
/// Checks `TESSDATA_PREFIX` first (always wins, regardless of OS).
/// Falls back to the OS-conventional install location.
pub fn tessdata_dir() -> String {
    if let Ok(val) = std::env::var("TESSDATA_PREFIX") {
        return val;
    }

    #[cfg(target_os = "macos")]
    {
        "/opt/homebrew/share/tessdata".to_string()
    }
    #[cfg(target_os = "windows")]
    {
        r"C:\Program Files\Tesseract-OCR\tessdata".to_string()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "/usr/share/tessdata".to_string()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
