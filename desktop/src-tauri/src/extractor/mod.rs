use std::path::Path;
use crate::errors::AppError;
use crate::llm::vision;

mod pdf;

/// Result of text extraction for any supported file type.
pub struct ExtractResult {
    /// Searchable text content (stripped of markup for MD; flattened for CSV/XLSX).
    pub text: String,
    /// 1.0 for non-OCR sources; average Tesseract word confidence for OCR paths.
    pub confidence: f32,
    /// ISO 639-1 language code detected at extraction time, or empty string.
    pub lang_hint: String,
}

/// Extract searchable text from `path`.
///
/// Routes by file extension.  Images are described via the Ollama vision
/// pipeline.  OCR is attempted for PDFs whose text layer is absent or empty.
pub fn extract(path: &Path, ollama_url: &str) -> Result<ExtractResult, AppError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "txt" => extract_txt(path),
        "md" => extract_md(path),
        "csv" => extract_csv(path),
        "xlsx" | "xls" | "xlsm" => extract_xlsx(path),
        "docx" => extract_docx(path),
        "pdf" => pdf::extract_pdf(path, ollama_url),
        "png" | "jpg" | "jpeg" | "webp" => extract_image(path, ollama_url),
        _ => Err(AppError::Extractor(format!(
            "unsupported file type: {}",
            path.display()
        ))),
    }
}

/// Detect the language of `text` and return an ISO 639-1 code, or `"unknown"`.
pub(super) fn detect_lang(text: &str) -> String {
    use whatlang::detect;
    detect(text)
        .map(|info| info.lang().code().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

// ── TXT ───────────────────────────────────────────────────────────────────────

fn extract_txt(path: &Path) -> Result<ExtractResult, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Extractor(format!("txt read error {}: {e}", path.display())))?;
    let text = text.trim().to_string();
    let lang_hint = detect_lang(&text);
    Ok(ExtractResult { text, confidence: 1.0, lang_hint })
}

// ── MD ────────────────────────────────────────────────────────────────────────

fn extract_md(path: &Path) -> Result<ExtractResult, AppError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| AppError::Extractor(format!("md read error {}: {e}", path.display())))?;
    let text = strip_markdown(&raw).trim().to_string();
    let lang_hint = detect_lang(&text);
    Ok(ExtractResult { text, confidence: 1.0, lang_hint })
}

/// Strip common Markdown syntax, preserving human-readable text.
fn strip_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        let line = line.trim();
        // Remove heading markers (# ## ### etc.)
        let line = if let Some(rest) = line.strip_prefix('#') {
            rest.trim_start_matches('#').trim()
        } else {
            line
        };
        // Remove blockquote markers
        let line = line.trim_start_matches('>').trim();
        let line_owned = replace_md_inline(line);
        out.push_str(&line_owned);
        out.push('\n');
    }
    out
}

fn replace_md_inline(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            // Images: ![alt](url) — keep alt text
            '!' if i + 1 < chars.len() && chars[i + 1] == '[' => {
                if let Some((alt, skip)) = parse_md_link(&chars, i + 1) {
                    result.push_str(&alt);
                    i += 1 + skip;
                } else {
                    result.push(chars[i]);
                    i += 1;
                }
            }
            // Links: [text](url) — keep link text
            '[' => {
                if let Some((text, skip)) = parse_md_link(&chars, i) {
                    result.push_str(&text);
                    i += skip;
                } else {
                    result.push(chars[i]);
                    i += 1;
                }
            }
            // Bold/italic: ** or * — keep inner text
            '*' => {
                let count = if i + 1 < chars.len() && chars[i + 1] == '*' { 2 } else { 1 };
                let rest_start = i + count;
                if let Some(close) = find_md_close(&chars, rest_start, &chars[i..i + count]) {
                    let inner: String = chars[rest_start..rest_start + close].iter().collect();
                    result.push_str(inner.trim());
                    i = rest_start + close + count;
                } else {
                    i += count;
                }
            }
            // Inline code: `code` — keep inner text
            '`' => {
                let rest_start = i + 1;
                if let Some(close) = chars[rest_start..].iter().position(|&c| c == '`') {
                    let inner: String = chars[rest_start..rest_start + close].iter().collect();
                    result.push_str(inner.trim());
                    i = rest_start + close + 1;
                } else {
                    i += 1;
                }
            }
            c => {
                result.push(c);
                i += 1;
            }
        }
    }
    result
}

fn parse_md_link(chars: &[char], pos: usize) -> Option<(String, usize)> {
    if chars.get(pos) != Some(&'[') {
        return None;
    }
    let close_bracket = chars[pos..].iter().position(|&c| c == ']')?;
    let text_end = pos + close_bracket;
    if chars.get(text_end + 1) != Some(&'(') {
        return None;
    }
    let paren_start = text_end + 2;
    let close_paren = chars[paren_start..].iter().position(|&c| c == ')')?;
    let text: String = chars[pos + 1..text_end].iter().collect();
    let consumed = close_bracket + 2 + close_paren + 1;
    Some((text, consumed))
}

fn find_md_close(chars: &[char], start: usize, marker: &[char]) -> Option<usize> {
    let mlen = marker.len();
    let window = &chars[start..];
    for i in 0..window.len() {
        if i + mlen <= window.len() && &window[i..i + mlen] == marker {
            return Some(i);
        }
    }
    None
}

// ── CSV ───────────────────────────────────────────────────────────────────────

fn extract_csv(path: &Path) -> Result<ExtractResult, AppError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| AppError::Extractor(format!("csv read error {}: {e}", path.display())))?;

    if raw.contains('"') {
        log::warn!(
            "CSV file {} contains quoted fields; quoted commas will be treated as delimiters \
             and may produce misaligned key-value pairs (v1 limitation)",
            path.display()
        );
    }

    let mut lines = raw.lines();
    let header_line = match lines.next() {
        Some(l) => l,
        None => {
            return Ok(ExtractResult { text: String::new(), confidence: 1.0, lang_hint: "unknown".to_string() })
        }
    };
    let headers: Vec<&str> = split_csv_row(header_line);

    let mut output = String::new();
    for row_line in lines {
        let values = split_csv_row(row_line);
        let pairs: Vec<String> = headers
            .iter()
            .enumerate()
            .filter_map(|(i, h)| {
                let v = values.get(i).copied().unwrap_or("").trim();
                if v.is_empty() { None } else { Some(format!("{}: {}", h.trim(), v)) }
            })
            .collect();
        if !pairs.is_empty() {
            output.push_str(&pairs.join(", "));
            output.push('\n');
        }
    }

    let text = output.trim().to_string();
    let lang_hint = detect_lang(&text);
    Ok(ExtractResult { text, confidence: 1.0, lang_hint })
}

fn split_csv_row(line: &str) -> Vec<&str> {
    // Simple split — does not handle quoted commas (v1 flat normalization).
    line.split(',').collect()
}

// ── XLSX / XLS ────────────────────────────────────────────────────────────────

fn extract_xlsx(path: &Path) -> Result<ExtractResult, AppError> {
    use calamine::{open_workbook_auto, Reader, Data};

    let mut workbook = open_workbook_auto(path)
        .map_err(|e| AppError::Extractor(format!("xlsx open error {}: {e}", path.display())))?;

    let mut output = String::new();
    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    for name in &sheet_names {
        if let Ok(range) = workbook.worksheet_range(name) {
            for row in range.rows() {
                for cell in row {
                    let val = match cell {
                        Data::String(s) if !s.is_empty() => s.clone(),
                        Data::Float(f) => f.to_string(),
                        Data::Bool(b) => b.to_string(),
                        _ => continue,
                    };
                    output.push_str(&val);
                    output.push(' ');
                }
                output.push('\n');
            }
        }
    }

    let text = output.trim().to_string();
    let lang_hint = detect_lang(&text);
    Ok(ExtractResult { text, confidence: 1.0, lang_hint })
}

// ── DOCX ──────────────────────────────────────────────────────────────────────

fn extract_docx(path: &Path) -> Result<ExtractResult, AppError> {
    use docx_rs::read_docx;

    let bytes = std::fs::read(path)
        .map_err(|e| AppError::Extractor(format!("docx read error {}: {e}", path.display())))?;

    let doc = read_docx(&bytes)
        .map_err(|e| AppError::Extractor(format!("docx parse error {}: {e:?}", path.display())))?;

    let mut text = String::new();
    for child in &doc.document.children {
        if let docx_rs::DocumentChild::Paragraph(para) = child {
            for run_child in &para.children {
                if let docx_rs::ParagraphChild::Run(run) = run_child {
                    for run_content in &run.children {
                        if let docx_rs::RunChild::Text(t) = run_content {
                            text.push_str(&t.text);
                        }
                    }
                }
            }
            text.push('\n');
        }
    }

    let text = text.trim().to_string();
    let lang_hint = detect_lang(&text);
    Ok(ExtractResult { text, confidence: 1.0, lang_hint })
}

// ── Images ────────────────────────────────────────────────────────────────────

fn extract_image(path: &Path, ollama_url: &str) -> Result<ExtractResult, AppError> {
    let path_str = path.to_str().ok_or_else(|| {
        AppError::Extractor(format!("invalid image path: {}", path.display()))
    })?;
    let desc = vision::describe_image(path_str, ollama_url)?;
    let lang_hint = detect_lang(&desc);
    Ok(ExtractResult { text: desc, confidence: pdf::VISION_CONFIDENCE, lang_hint })
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
