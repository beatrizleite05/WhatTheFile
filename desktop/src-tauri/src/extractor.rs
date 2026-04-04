use std::path::Path;
use crate::errors::AppError;
use crate::llm::vision;

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
        "xlsx" | "xls" => extract_xlsx(path),
        "docx" => extract_docx(path),
        "pdf" => extract_pdf(path, ollama_url),
        "png" | "jpg" | "jpeg" => extract_image(path, ollama_url),
        _ => Err(AppError::Extractor(format!(
            "unsupported file type: {}",
            path.display()
        ))),
    }
}

/// Detect the language of `text` and return an ISO 639-1 code, or `"unknown"`.
fn detect_lang(text: &str) -> String {
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

// ── PDF ───────────────────────────────────────────────────────────────────────

fn pdfium_instance() -> Result<pdfium_render::prelude::Pdfium, AppError> {
    use pdfium_render::prelude::*;
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
        &crate::platform::pdfium_dir(),
    ))
    .or_else(|_| Pdfium::bind_to_system_library())
        .map_err(|e| AppError::Extractor(format!("pdfium load error: {e}")))?;
    Ok(Pdfium::new(bindings))
}

fn extract_pdf(path: &Path, ollama_url: &str) -> Result<ExtractResult, AppError> {
    use pdfium_render::prelude::*;

    let path_str = path.to_str().ok_or_else(|| {
        AppError::Extractor(format!("invalid PDF path: {}", path.display()))
    })?;

    let pdfium = pdfium_instance()?;
    let doc = pdfium
        .load_pdf_from_file(path_str, None)
        .map_err(|e| AppError::Extractor(format!("pdfium open error {}: {e}", path.display())))?;

    // Extract text layer from each page.
    let mut text = String::new();
    for page in doc.pages().iter() {
        let page_text = page.text()
            .map_err(|e| AppError::Extractor(format!("pdfium text error: {e}")))?
            .all();
        if !page_text.trim().is_empty() {
            text.push_str(&page_text);
            text.push('\n');
        }
    }

    let trimmed = text.trim().to_string();
    if !trimmed.is_empty() {
        let lang_hint = detect_lang(&trimmed);
        return Ok(ExtractResult { text: trimmed, confidence: 1.0, lang_hint });
    }

    // No text layer — scanned PDF. Try OCR first; fall back to vision if OCR
    // confidence is too low or Tesseract is unavailable.
    extract_pdf_via_ocr(path, ollama_url)
}

/// OCR confidence threshold — chunks below this are discarded.
const OCR_CONFIDENCE_THRESHOLD: f32 = 0.75;

/// Confidence assigned to vision-model output.
///
/// Vision descriptions are best-effort; we use the OCR threshold as a
/// conservative proxy rather than claiming perfect (1.0) confidence.
const VISION_CONFIDENCE: f32 = OCR_CONFIDENCE_THRESHOLD;

/// Tesseract language string — primary languages for this app.
const TESS_LANG: &str = "eng+por";

/// Rasterize a scanned PDF with pdfium and run Tesseract on each page.
///
/// Pages whose mean word confidence is below `OCR_CONFIDENCE_THRESHOLD` are
/// skipped and collected for a vision-model second pass.
fn extract_pdf_via_ocr(path: &Path, ollama_url: &str) -> Result<ExtractResult, AppError> {
    use pdfium_render::prelude::*;

    let path_str = path.to_str().ok_or_else(|| {
        AppError::Extractor(format!("invalid PDF path: {}", path.display()))
    })?;

    let pdfium = pdfium_instance()?;
    let doc = pdfium
        .load_pdf_from_file(path_str, None)
        .map_err(|e| AppError::Extractor(format!("pdfium open error {}: {e}", path.display())))?;

    // Higher resolution gives Tesseract more pixels to work with.
    let render_config = PdfRenderConfig::new()
        .set_target_width(1024)
        .set_maximum_height(1440);

    let mut ocr_texts: Vec<String> = Vec::new();
    let mut low_conf_page_indices: Vec<usize> = Vec::new();
    let mut total_conf_sum = 0f32;
    let mut total_conf_count = 0u32;

    let tessdata = crate::platform::tessdata_dir();

    for (page_idx, page) in doc.pages().iter().enumerate() {
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| AppError::Extractor(format!("pdfium render error page {page_idx}: {e}")))?;

        let rgba = bitmap.as_image().into_rgba8();

        match run_tesseract_on_rgba(&tessdata, &rgba) {
            Ok((text, conf)) if conf >= OCR_CONFIDENCE_THRESHOLD && !text.trim().is_empty() => {
                ocr_texts.push(text);
                total_conf_sum += conf;
                total_conf_count += 1;
            }
            Ok((_, conf)) => {
                // OCR ran but confidence too low — queue for vision fallback.
                log::warn!("OCR confidence {conf:.2} below threshold for page {page_idx} of {}", path.display());
                low_conf_page_indices.push(page_idx);
            }
            Err(e) => {
                log::warn!("OCR failed for page {page_idx} of {}: {e}", path.display());
                low_conf_page_indices.push(page_idx);
            }
        }
    }

    // If OCR produced nothing at all, fall through to full vision pass.
    if ocr_texts.is_empty() {
        log::info!("OCR yielded no usable text for {}; falling back to vision", path.display());
        return extract_pdf_via_vision(path, ollama_url);
    }

    // For pages OCR couldn't handle, attempt vision on those pages only.
    if !low_conf_page_indices.is_empty() {
        let vision_texts = extract_pdf_pages_via_vision(path, &low_conf_page_indices, ollama_url);
        ocr_texts.extend(vision_texts);
    }

    let mean_confidence = if total_conf_count > 0 {
        total_conf_sum / total_conf_count as f32
    } else {
        OCR_CONFIDENCE_THRESHOLD
    };

    let text = ocr_texts.join("\n\n").trim().to_string();
    let lang_hint = detect_lang(&text);
    Ok(ExtractResult { text, confidence: mean_confidence, lang_hint })
}

/// Run Tesseract on a raw RGBA pixel buffer.
/// Returns `(extracted_text, mean_word_confidence_0_to_1)`.
fn run_tesseract_on_rgba(
    tessdata: &str,
    img: &image::RgbaImage,
) -> Result<(String, f32), AppError> {
    use leptess::LepTess;

    let mut lt = LepTess::new(Some(tessdata), TESS_LANG)
        .map_err(|e| AppError::Extractor(format!("Tesseract init error: {e}")))?;

    // Encode RGBA image to PNG bytes; leptess::set_image_from_mem expects an
    // encoded image format, not a raw pixel buffer.
    let mut png_buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png_buf, image::ImageFormat::Png)
        .map_err(|e| AppError::Extractor(format!("PNG encode for OCR error: {e}")))?;

    lt.set_image_from_mem(png_buf.get_ref())
        .map_err(|e| AppError::Extractor(format!("Tesseract set_image error: {e}")))?;

    let text = lt
        .get_utf8_text()
        .map_err(|e| AppError::Extractor(format!("Tesseract get_text error: {e}")))?;

    // mean_text_conf returns 0–100; normalise to 0.0–1.0.
    let conf = lt.mean_text_conf() as f32 / 100.0;

    Ok((text, conf))
}

/// Run vision model on a specific subset of pages from a PDF.
/// Returns descriptions for pages that produce non-empty output; silently skips failures.
fn extract_pdf_pages_via_vision(path: &Path, page_indices: &[usize], ollama_url: &str) -> Vec<String> {
    use pdfium_render::prelude::*;

    let pdfium = match pdfium_instance() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let path_str = match path.to_str() {
        Some(s) => s,
        None => return Vec::new(),
    };
    let doc = match pdfium.load_pdf_from_file(path_str, None) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let render_config = PdfRenderConfig::new()
        .set_target_width(512)
        .set_maximum_height(720);

    let mut descriptions = Vec::new();
    for &page_idx in page_indices.iter() {
        let page = match doc.pages().get(page_idx as u16) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let bitmap = match page.render_with_config(&render_config) {
            Ok(b) => b,
            Err(_) => continue,
        };

        let rgba = bitmap.as_image().into_rgba8();
        let img = image::RgbaImage::from_raw(
            bitmap.width() as u32,
            bitmap.height() as u32,
            rgba.into_raw(),
        );
        let img = match img {
            Some(i) => i,
            None => continue,
        };

        // Write to a temp file for the vision call.
        let tmp = match tempfile::Builder::new().suffix(".png").tempfile() {
            Ok(t) => t,
            Err(_) => continue,
        };
        {
            use std::io::Write;
            let mut buf = std::io::Cursor::new(Vec::new());
            if img.write_to(&mut buf, image::ImageFormat::Png).is_err() {
                continue;
            }
            match tmp.reopen() {
                Ok(mut f) => {
                    if let Err(e) = f.write_all(buf.get_ref()) {
                        log::warn!("vision fallback: failed to write tmp PNG for page {page_idx} of {}: {e}", path.display());
                        continue;
                    }
                }
                Err(e) => {
                    log::warn!("vision fallback: failed to reopen tmp file for page {page_idx} of {}: {e}", path.display());
                    continue;
                }
            }
        }

        let tmp_path = match tmp.path().to_str() {
            Some(s) => s,
            None => continue,
        };

        match vision::describe_image(tmp_path, ollama_url) {
            Ok(desc) if !desc.trim().is_empty() => {
                log::info!("vision fallback succeeded for page {page_idx} of {}", path.display());
                descriptions.push(desc);
            }
            Ok(_) => {}
            Err(e) => {
                log::warn!("vision fallback failed for page {page_idx} of {}: {e}", path.display());
            }
        }

    }

    descriptions
}

fn extract_pdf_via_vision(path: &Path, ollama_url: &str) -> Result<ExtractResult, AppError> {
    use pdfium_render::prelude::*;

    let path_str = path.to_str().ok_or_else(|| {
        AppError::Extractor(format!("invalid PDF path: {}", path.display()))
    })?;

    let pdfium = pdfium_instance()?;
    let doc = pdfium
        .load_pdf_from_file(path_str, None)
        .map_err(|e| AppError::Extractor(format!("pdfium open error {}: {e}", path.display())))?;

    let render_config = PdfRenderConfig::new()
        .set_target_width(512)
        .set_maximum_height(720);

    let mut descriptions = Vec::new();

    for page in doc.pages().iter() {
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| AppError::Extractor(format!("pdfium render error: {e}")))?;

        let png_bytes = bitmap
            .as_image()
            .into_rgba8()
            .to_vec();

        // Write page PNG to a temp file so vision::describe_image can read it.
        let mut tmp = tempfile::Builder::new()
            .suffix(".png")
            .tempfile()
            .map_err(|e| AppError::Extractor(format!("tmp file error: {e}")))?;
        {
            use std::io::Write;
            let img = image::RgbaImage::from_raw(
                bitmap.width() as u32,
                bitmap.height() as u32,
                png_bytes,
            )
            .ok_or_else(|| AppError::Extractor("pdfium: image buffer size mismatch".into()))?;
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| AppError::Extractor(format!("png encode error: {e}")))?;
            tmp.write_all(buf.get_ref())
                .map_err(|e| AppError::Extractor(format!("tmp write error: {e}")))?;
        }

        let tmp_path = tmp.path().to_str().ok_or_else(|| {
            AppError::Extractor("tmp path is not valid UTF-8".into())
        })?;

        match vision::describe_image(tmp_path, ollama_url) {
            Ok(desc) if !desc.trim().is_empty() => descriptions.push(desc),
            Ok(_) => {}
            Err(_) => {} // skip pages where vision fails
        }
    }

    let text = descriptions.join("\n\n").trim().to_string();
    let lang_hint = detect_lang(&text);
    Ok(ExtractResult { text, confidence: VISION_CONFIDENCE, lang_hint })
}

// ── Images ────────────────────────────────────────────────────────────────────

fn extract_image(path: &Path, ollama_url: &str) -> Result<ExtractResult, AppError> {
    let path_str = path.to_str().ok_or_else(|| {
        AppError::Extractor(format!("invalid image path: {}", path.display()))
    })?;
    let desc = vision::describe_image(path_str, ollama_url)?;
    let lang_hint = detect_lang(&desc);
    Ok(ExtractResult { text: desc, confidence: VISION_CONFIDENCE, lang_hint })
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_tmp(ext: &str, content: &[u8]) -> NamedTempFile {
        let mut f = tempfile::Builder::new()
            .suffix(&format!(".{ext}"))
            .tempfile()
            .unwrap();
        f.write_all(content).unwrap();
        f
    }

    // ── TXT ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_extract_txt_returns_content() {
        let f = write_tmp("txt", b"Hello, world!");
        let result = extract(f.path(), "http://localhost:11434").unwrap();
        assert_eq!(result.text, "Hello, world!");
        assert!((result.confidence - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_extract_txt_trims_whitespace() {
        let f = write_tmp("txt", b"  \n  hello  \n  ");
        let result = extract(f.path(), "http://localhost:11434").unwrap();
        assert_eq!(result.text, "hello");
    }

    // ── MD ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_extract_md_strips_heading() {
        let f = write_tmp("md", b"# Title\n\nSome text.");
        let result = extract(f.path(), "http://localhost:11434").unwrap();
        assert!(result.text.contains("Title"), "heading text should be preserved");
        assert!(!result.text.contains('#'), "# markers should be removed");
    }

    #[test]
    fn test_extract_md_strips_bold_and_italic() {
        let f = write_tmp("md", b"**bold** and *italic* text");
        let result = extract(f.path(), "http://localhost:11434").unwrap();
        assert!(result.text.contains("bold"), "bold text preserved");
        assert!(result.text.contains("italic"), "italic text preserved");
        assert!(!result.text.contains('*'), "asterisks should be removed");
    }

    #[test]
    fn test_extract_md_strips_inline_code() {
        let f = write_tmp("md", b"Use `code` here");
        let result = extract(f.path(), "http://localhost:11434").unwrap();
        assert!(result.text.contains("code"));
        assert!(!result.text.contains('`'));
    }

    #[test]
    fn test_extract_md_strips_link() {
        let f = write_tmp("md", b"See [example](https://example.com) for details");
        let result = extract(f.path(), "http://localhost:11434").unwrap();
        assert!(result.text.contains("example"), "link text preserved");
        assert!(!result.text.contains("https://"), "URL removed");
    }

    #[test]
    fn test_extract_md_strips_blockquote() {
        let f = write_tmp("md", b"> quoted text");
        let result = extract(f.path(), "http://localhost:11434").unwrap();
        assert!(result.text.contains("quoted text"));
        assert!(!result.text.contains('>'));
    }

    // ── CSV ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_extract_csv_formats_as_key_value() {
        let f = write_tmp("csv", b"name,age,city\nAlice,30,Berlin\nBob,25,Paris");
        let result = extract(f.path(), "http://localhost:11434").unwrap();
        assert!(result.text.contains("name: Alice"), "got: {}", result.text);
        assert!(result.text.contains("age: 30"));
        assert!(result.text.contains("city: Berlin"));
        assert!(result.text.contains("name: Bob"));
    }

    #[test]
    fn test_extract_csv_empty_file_returns_empty() {
        let f = write_tmp("csv", b"");
        let result = extract(f.path(), "http://localhost:11434").unwrap();
        assert!(result.text.is_empty());
    }

    #[test]
    fn test_extract_csv_header_only_returns_empty() {
        let f = write_tmp("csv", b"name,age,city\n");
        let result = extract(f.path(), "http://localhost:11434").unwrap();
        assert!(result.text.is_empty());
    }

    // ── Unsupported ───────────────────────────────────────────────────────────

    #[test]
    fn test_extract_unsupported_extension_errors() {
        let f = write_tmp("bin", b"\x00\x01\x02");
        let result = extract(f.path(), "http://localhost:11434");
        assert!(
            matches!(result, Err(AppError::Extractor(_))),
            "expected Extractor error for .bin file"
        );
    }

    // ── Fixture helpers ───────────────────────────────────────────────────────

    /// Returns the path to the shared sample-files directory checked into the repo.
    fn fixtures() -> std::path::PathBuf {
        // CARGO_MANIFEST_DIR = desktop/src-tauri/
        // sample-files are at  desktop/test/sample-files/
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../test/sample-files")
    }

    // ── PDF (text layer) — no external services required ─────────────────────

    #[test]
    fn test_extract_pdf_text_layer_returns_content() {
        // LinearProgramming-FEUP.pdf is a digital PDF with a rich text layer.
        let path = fixtures().join("LinearProgramming-FEUP.pdf");
        let result = extract(&path, "http://localhost:11434").unwrap();
        assert!(!result.text.is_empty(), "expected text from digital PDF");
        assert!((result.confidence - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_extract_pdf_text_layer_multiple_files() {
        // Verify several digital PDFs all produce non-empty text.
        let files = ["GreedyAlgorithms.pdf", "IntegerLinearProgramming.pdf", "drylab.pdf"];
        for name in files {
            let path = fixtures().join(name);
            let result = extract(&path, "http://localhost:11434").unwrap();
            assert!(!result.text.is_empty(), "{name} should have extractable text");
        }
    }

    // ── XLSX / XLS — no external services required ────────────────────────────

    #[test]
    fn test_extract_xls_returns_text() {
        let path = fixtures().join("file_example_XLS_50.xls");
        let result = extract(&path, "http://localhost:11434").unwrap();
        assert!(!result.text.is_empty(), "expected text from XLS file");
        assert!((result.confidence - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_extract_xls_larger_file() {
        let path = fixtures().join("file_example_XLS_1000.xls");
        let result = extract(&path, "http://localhost:11434").unwrap();
        assert!(!result.text.is_empty());
    }

    // ── OCR unit tests — no external services required ────────────────────────

    #[test]
    fn test_ocr_confidence_threshold_constant() {
        // Threshold must be exactly 0.75 per spec.
        assert!((OCR_CONFIDENCE_THRESHOLD - 0.75).abs() < 1e-6);
    }

    #[test]
    fn test_ocr_on_clean_png_image() {
        // Use a real photo from the fixture set — Tesseract should initialise
        // and return a result (text may be empty for a photo, but must not panic
        // or error out).
        let tessdata = crate::platform::tessdata_dir();
        let img_path = fixtures().join("16626587.png");
        let img = image::open(&img_path)
            .expect("fixture image should be readable")
            .into_rgba8();
        // run_tesseract_on_rgba must not panic or error even on a photo.
        let result = run_tesseract_on_rgba(&tessdata, &img);
        assert!(result.is_ok(), "Tesseract should not error on a valid image: {result:?}");
        let (_, conf) = result.unwrap();
        assert!(conf >= 0.0 && conf <= 1.0, "confidence must be in [0, 1], got {conf}");
    }

    #[test]
    fn test_ocr_lang_constant_contains_eng_and_por() {
        assert!(TESS_LANG.contains("eng"), "must include English");
        assert!(TESS_LANG.contains("por"), "must include Portuguese");
    }

    // ── Scanned PDF — requires pdfium (no Ollama needed for OCR path) ─────────

    #[test]
    #[ignore = "requires pdfium library; Naac_appLetter.pdf is a scanned PDF — exercises the OCR path"]
    fn test_extract_pdf_scanned_via_ocr() {
        let path = fixtures().join("Naac_appLetter.pdf");
        let result = extract(&path, "http://localhost:11434").unwrap();
        assert!(!result.text.is_empty(), "scanned PDF should produce OCR text");
        // Confidence must be in the valid range (may be below threshold for a
        // noisy scan, in which case vision fallback ran and confidence is 0.75).
        assert!(result.confidence > 0.0 && result.confidence <= 1.0);
    }

    // ── Image + vision fallback — require Ollama ──────────────────────────────

    #[test]
    #[ignore = "requires Ollama running with qwen2.5vl:7b or llava:7b"]
    fn test_extract_image_png_returns_description() {
        let path = fixtures().join("16626587.png");
        let result = extract(&path, "http://localhost:11434").unwrap();
        assert!(!result.text.is_empty(), "vision model should describe the image");
    }

    #[test]
    #[ignore = "requires Ollama running with qwen2.5vl:7b or llava:7b"]
    fn test_extract_image_jpg_returns_description() {
        let path = fixtures().join("images.jpg");
        let result = extract(&path, "http://localhost:11434").unwrap();
        assert!(!result.text.is_empty(), "vision model should describe the image");
    }

    #[test]
    #[ignore = "requires pdfium + Ollama; exercises OCR→vision fallback path for a scanned PDF"]
    fn test_extract_pdf_scanned_ocr_then_vision_fallback() {
        // Same file as the OCR test — if OCR confidence is too low, vision runs.
        let path = fixtures().join("Naac_appLetter.pdf");
        let result = extract(&path, "http://localhost:11434").unwrap();
        assert!(!result.text.is_empty(), "should produce output via OCR or vision");
    }

    // ── DOCX — requires a fixture not yet in sample-files ─────────────────────

    #[test]
    fn test_extract_docx_returns_text() {
        let path = fixtures().join("Guia de Montagem Indústria 4.0.docx");
        let result = extract(&path, "http://localhost:11434").unwrap();
        assert!(!result.text.is_empty(), "expected text from DOCX file");
        assert!((result.confidence - 1.0).abs() < 1e-6);
    }
}
