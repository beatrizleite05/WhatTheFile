use std::path::Path;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use crate::errors::AppError;
use crate::llm::vision;
use super::{detect_lang, ExtractResult};

/// OCR confidence threshold — chunks below this are discarded.
pub(super) const OCR_CONFIDENCE_THRESHOLD: f32 = 0.75;

/// Hard per-page wall-clock budget for Tesseract. Leptess cannot be interrupted
/// mid-page, so this lets the indexer skip a pathological page rather than wedge.
pub(super) const OCR_PAGE_TIMEOUT_SECS: u64 = 90;

/// Confidence assigned to vision-model output.
///
/// Vision descriptions are best-effort; we use the OCR threshold as a
/// conservative proxy rather than claiming perfect (1.0) confidence.
pub(super) const VISION_CONFIDENCE: f32 = OCR_CONFIDENCE_THRESHOLD;

/// Tesseract language string — primary languages for this app.
pub(super) const TESS_LANG: &str = "eng+por";

pub(super) fn pdfium_instance() -> Result<pdfium_render::prelude::Pdfium, AppError> {
    use pdfium_render::prelude::*;
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
        &crate::platform::pdfium_dir(),
    ))
    .or_else(|_| Pdfium::bind_to_system_library())
        .map_err(|e| AppError::Extractor(format!("pdfium load error: {e}")))?;
    Ok(Pdfium::new(bindings))
}

pub(super) fn extract_pdf(path: &Path, ollama_url: &str, cancel: &Arc<AtomicBool>) -> Result<ExtractResult, AppError> {
    let path_str = path.to_str().ok_or_else(|| {
        AppError::Extractor(format!("invalid PDF path: {}", path.display()))
    })?;

    log::info!("pdf: opening {}", path.display());
    let pdfium = pdfium_instance()?;
    let doc = pdfium
        .load_pdf_from_file(path_str, None)
        .map_err(|e| AppError::Extractor(format!("pdfium open error {}: {e}", path.display())))?;
    let page_count = doc.pages().len();
    log::info!("pdf: opened {} ({} pages); extracting text layer", path.display(), page_count);

    let mut text = String::new();
    for (page_idx, page) in doc.pages().iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Indexer("cancelled".into()));
        }
        let page_text = page.text()
            .map_err(|e| AppError::Extractor(format!("pdfium text error: {e}")))?
            .all();
        log::debug!("pdf text-layer page {}/{} of {}: chars={}", page_idx + 1, page_count, path.display(), page_text.len());
        if !page_text.trim().is_empty() {
            text.push_str(&page_text);
            text.push('\n');
        }
    }

    let trimmed = text.trim().to_string();
    if !trimmed.is_empty() {
        log::info!("pdf: text layer of {} yielded {} chars; done", path.display(), trimmed.len());
        let lang_hint = detect_lang(&trimmed);
        return Ok(ExtractResult { text: trimmed, confidence: 1.0, lang_hint });
    }

    log::info!("pdf: no text layer for {}; falling back to OCR", path.display());
    drop(doc);
    drop(pdfium);
    extract_pdf_via_ocr(path, ollama_url, cancel)
}

/// Rasterize a scanned PDF with pdfium and run Tesseract on each page.
///
/// Pages whose mean word confidence is below `OCR_CONFIDENCE_THRESHOLD` are
/// skipped and collected for a vision-model second pass.
fn extract_pdf_via_ocr(path: &Path, ollama_url: &str, cancel: &Arc<AtomicBool>) -> Result<ExtractResult, AppError> {
    use pdfium_render::prelude::*;

    let path_str = path.to_str().ok_or_else(|| {
        AppError::Extractor(format!("invalid PDF path: {}", path.display()))
    })?;

    log::info!("ocr: pdfium re-bind for {}", path.display());
    let pdfium = pdfium_instance()?;
    log::info!("ocr: pdfium re-open for {}", path.display());
    let doc = pdfium
        .load_pdf_from_file(path_str, None)
        .map_err(|e| AppError::Extractor(format!("pdfium open error {}: {e}", path.display())))?;
    log::info!("ocr: pdfium re-opened for {}", path.display());

    let render_config = PdfRenderConfig::new()
        .set_target_width(1024)
        .set_maximum_height(1440);

    let mut ocr_texts: Vec<String> = Vec::new();
    let mut low_conf_page_indices: Vec<usize> = Vec::new();
    let mut total_conf_sum = 0f32;
    let mut total_conf_count = 0u32;

    let tessdata = crate::platform::tessdata_dir();
    log::info!("ocr: tessdata={tessdata}");
    let total_pages = doc.pages().len();
    log::info!("ocr: starting page loop ({total_pages} pages) for {}", path.display());

    for (page_idx, page) in doc.pages().iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Indexer("cancelled".into()));
        }
        log::info!("OCR page {}/{} of {}: rendering", page_idx + 1, total_pages, path.display());
        let render_started = std::time::Instant::now();
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| AppError::Extractor(format!("pdfium render error page {page_idx}: {e}")))?;
        let render_ms = render_started.elapsed().as_millis();

        let rgba_started = std::time::Instant::now();
        let rgba = bitmap.as_image().into_rgba8();
        let rgba_ms = rgba_started.elapsed().as_millis();
        log::info!("OCR page {}/{} of {}: rendered render_ms={render_ms} rgba_ms={rgba_ms}; running tesseract", page_idx + 1, total_pages, path.display());

        let started = std::time::Instant::now();
        let outcome = run_tesseract_bounded(tessdata.clone(), rgba, cancel, OCR_PAGE_TIMEOUT_SECS);
        let elapsed_ms = started.elapsed().as_millis();

        match outcome {
            Ok((text, conf)) if conf >= OCR_CONFIDENCE_THRESHOLD && !text.trim().is_empty() => {
                log::info!("OCR page {}/{} of {}: ok conf={conf:.2} ms={elapsed_ms}", page_idx + 1, total_pages, path.display());
                ocr_texts.push(text);
                total_conf_sum += conf;
                total_conf_count += 1;
            }
            Ok((_, conf)) => {
                log::warn!("OCR page {}/{} of {}: low conf={conf:.2} ms={elapsed_ms}", page_idx + 1, total_pages, path.display());
                low_conf_page_indices.push(page_idx);
            }
            Err(AppError::Indexer(msg)) if msg == "cancelled" => {
                return Err(AppError::Indexer("cancelled".into()));
            }
            Err(e) => {
                log::warn!("OCR page {}/{} of {}: error ms={elapsed_ms} {e}", page_idx + 1, total_pages, path.display());
                low_conf_page_indices.push(page_idx);
            }
        }
    }

    if ocr_texts.is_empty() {
        log::info!("OCR yielded no usable text for {}; falling back to vision", path.display());
        drop(doc);
        drop(pdfium);
        return extract_pdf_via_vision(path, ollama_url, cancel);
    }

    drop(doc);
    drop(pdfium);
    // For pages OCR couldn't handle, attempt vision on those pages only.
    if !low_conf_page_indices.is_empty() {
        let vision_texts = extract_pdf_pages_via_vision(path, &low_conf_page_indices, ollama_url, cancel);
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
pub(super) fn run_tesseract_on_rgba(
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

/// Run Tesseract on a worker thread with cancel polling and a wall-clock budget.
///
/// Tesseract cannot be interrupted mid-page (leptess holds a native handle and
/// `get_utf8_text` blocks), so the worker keeps running after timeout — but the
/// caller is unblocked and can skip the page. This prevents one pathological
/// page from wedging the entire indexer.
pub(super) fn run_tesseract_bounded(
    tessdata: String,
    img: image::RgbaImage,
    cancel: &Arc<AtomicBool>,
    timeout_secs: u64,
) -> Result<(String, f32), AppError> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run_tesseract_on_rgba(&tessdata, &img));
    });

    let mut elapsed = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Indexer("cancelled".into()));
        }
        match rx.recv_timeout(std::time::Duration::from_secs(1)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                elapsed += 1;
                if elapsed >= timeout_secs {
                    return Err(AppError::Extractor(format!(
                        "Tesseract timed out after {timeout_secs}s"
                    )));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(AppError::Extractor("Tesseract worker disconnected".into()));
            }
        }
    }
}

/// Run vision model on a specific subset of pages from a PDF.
/// Returns descriptions for pages that produce non-empty output; silently skips failures.
fn extract_pdf_pages_via_vision(path: &Path, page_indices: &[usize], ollama_url: &str, cancel: &Arc<AtomicBool>) -> Vec<String> {
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
        if cancel.load(Ordering::Relaxed) {
            break;
        }
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

        let started = std::time::Instant::now();
        let outcome = vision::describe_image(tmp_path, ollama_url, cancel);
        let elapsed_ms = started.elapsed().as_millis();
        match outcome {
            Ok(desc) if !desc.trim().is_empty() => {
                log::info!("vision page {page_idx} of {}: ok ms={elapsed_ms} desc_len={}", path.display(), desc.len());
                descriptions.push(desc);
            }
            Ok(_) => {
                log::warn!("vision page {page_idx} of {}: empty ms={elapsed_ms}", path.display());
            }
            Err(e) => {
                log::warn!("vision page {page_idx} of {}: error ms={elapsed_ms} {e}", path.display());
            }
        }

    }

    descriptions
}

fn extract_pdf_via_vision(path: &Path, ollama_url: &str, cancel: &Arc<AtomicBool>) -> Result<ExtractResult, AppError> {
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
    let total_pages = doc.pages().len();

    for (page_idx, page) in doc.pages().iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Indexer("cancelled".into()));
        }
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
        let started = std::time::Instant::now();
        let outcome = vision::describe_image(tmp_path, ollama_url, cancel);
        let elapsed_ms = started.elapsed().as_millis();
        match outcome {
            Ok(desc) if !desc.trim().is_empty() => {
                log::info!("vision page {}/{} of {}: ok ms={elapsed_ms} desc_len={}", page_idx + 1, total_pages, path.display(), desc.len());
                descriptions.push(desc);
            }
            Ok(_) => {
                log::warn!("vision page {}/{} of {}: empty ms={elapsed_ms}", page_idx + 1, total_pages, path.display());
            }
            Err(e) => {
                log::warn!("vision page {}/{} of {}: error ms={elapsed_ms} {e}", page_idx + 1, total_pages, path.display());
            }
        }
    }

    let text = descriptions.join("\n\n").trim().to_string();
    let lang_hint = detect_lang(&text);
    Ok(ExtractResult { text, confidence: VISION_CONFIDENCE, lang_hint })
}
