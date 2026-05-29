use super::*;
use std::io::Write;
use std::sync::{Arc, atomic::AtomicBool};
use tempfile::NamedTempFile;

fn no_cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

fn write_tmp(ext: &str, content: &[u8]) -> NamedTempFile {
    let mut f = tempfile::Builder::new()
        .suffix(&format!(".{ext}"))
        .tempfile()
        .unwrap();
    f.write_all(content).unwrap();
    f
}

// ── TXT ───────────────────────────────────────────────────────────────────────

#[test]
fn test_extract_txt_returns_content() {
    let f = write_tmp("txt", b"Hello, world!");
    let result = extract(f.path(), "http://localhost:11434", &no_cancel()).unwrap();
    assert_eq!(result.text, "Hello, world!");
    assert!((result.confidence - 1.0).abs() < 1e-6);
}

#[test]
fn test_extract_txt_trims_whitespace() {
    let f = write_tmp("txt", b"  \n  hello  \n  ");
    let result = extract(f.path(), "http://localhost:11434", &no_cancel()).unwrap();
    assert_eq!(result.text, "hello");
}

// ── MD ────────────────────────────────────────────────────────────────────────

#[test]
fn test_extract_md_strips_heading() {
    let f = write_tmp("md", b"# Title\n\nSome text.");
    let result = extract(f.path(), "http://localhost:11434", &no_cancel()).unwrap();
    assert!(result.text.contains("Title"), "heading text should be preserved");
    assert!(!result.text.contains('#'), "# markers should be removed");
}

#[test]
fn test_extract_md_strips_bold_and_italic() {
    let f = write_tmp("md", b"**bold** and *italic* text");
    let result = extract(f.path(), "http://localhost:11434", &no_cancel()).unwrap();
    assert!(result.text.contains("bold"), "bold text preserved");
    assert!(result.text.contains("italic"), "italic text preserved");
    assert!(!result.text.contains('*'), "asterisks should be removed");
}

#[test]
fn test_extract_md_strips_inline_code() {
    let f = write_tmp("md", b"Use `code` here");
    let result = extract(f.path(), "http://localhost:11434", &no_cancel()).unwrap();
    assert!(result.text.contains("code"));
    assert!(!result.text.contains('`'));
}

#[test]
fn test_extract_md_strips_link() {
    let f = write_tmp("md", b"See [example](https://example.com) for details");
    let result = extract(f.path(), "http://localhost:11434", &no_cancel()).unwrap();
    assert!(result.text.contains("example"), "link text preserved");
    assert!(!result.text.contains("https://"), "URL removed");
}

#[test]
fn test_extract_md_strips_blockquote() {
    let f = write_tmp("md", b"> quoted text");
    let result = extract(f.path(), "http://localhost:11434", &no_cancel()).unwrap();
    assert!(result.text.contains("quoted text"));
    assert!(!result.text.contains('>'));
}

// ── CSV ───────────────────────────────────────────────────────────────────────

#[test]
fn test_extract_csv_formats_as_key_value() {
    let f = write_tmp("csv", b"name,age,city\nAlice,30,Berlin\nBob,25,Paris");
    let result = extract(f.path(), "http://localhost:11434", &no_cancel()).unwrap();
    assert!(result.text.contains("name: Alice"), "got: {}", result.text);
    assert!(result.text.contains("age: 30"));
    assert!(result.text.contains("city: Berlin"));
    assert!(result.text.contains("name: Bob"));
}

#[test]
fn test_extract_csv_empty_file_returns_empty() {
    let f = write_tmp("csv", b"");
    let result = extract(f.path(), "http://localhost:11434", &no_cancel()).unwrap();
    assert!(result.text.is_empty());
}

#[test]
fn test_extract_csv_header_only_returns_empty() {
    let f = write_tmp("csv", b"name,age,city\n");
    let result = extract(f.path(), "http://localhost:11434", &no_cancel()).unwrap();
    assert!(result.text.is_empty());
}

// ── Unsupported ───────────────────────────────────────────────────────────────

#[test]
fn test_extract_unsupported_extension_errors() {
    let f = write_tmp("bin", b"\x00\x01\x02");
    let result = extract(f.path(), "http://localhost:11434", &no_cancel());
    assert!(
        matches!(result, Err(crate::errors::AppError::Extractor(_))),
        "expected Extractor error for .bin file"
    );
}

// ── Fixture helpers ───────────────────────────────────────────────────────────

/// Returns the path to the shared sample-files directory checked into the repo.
fn fixtures() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = desktop/src-tauri/
    // sample-files are at  desktop/test/sample-files/
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test/sample-files")
}

// ── PDF (text layer) — no external services required ─────────────────────────

#[test]
fn test_extract_pdf_text_layer_returns_content() {
    let path = fixtures().join("LinearProgramming-FEUP.pdf");
    let result = extract(&path, "http://localhost:11434", &no_cancel()).unwrap();
    assert!(!result.text.is_empty(), "expected text from digital PDF");
    assert!((result.confidence - 1.0).abs() < 1e-6);
}

#[test]
fn test_extract_pdf_text_layer_multiple_files() {
    let files = ["GreedyAlgorithms.pdf", "IntegerLinearProgramming.pdf", "drylab.pdf"];
    for name in files {
        let path = fixtures().join(name);
        let result = extract(&path, "http://localhost:11434", &no_cancel()).unwrap();
        assert!(!result.text.is_empty(), "{name} should have extractable text");
    }
}

// ── XLSX / XLS — no external services required ────────────────────────────────

#[test]
fn test_extract_xls_returns_text() {
    let path = fixtures().join("file_example_XLS_50.xls");
    let result = extract(&path, "http://localhost:11434", &no_cancel()).unwrap();
    assert!(!result.text.is_empty(), "expected text from XLS file");
    assert!((result.confidence - 1.0).abs() < 1e-6);
}

#[test]
fn test_extract_xls_larger_file() {
    let path = fixtures().join("file_example_XLS_1000.xls");
    let result = extract(&path, "http://localhost:11434", &no_cancel()).unwrap();
    assert!(!result.text.is_empty());
}

// ── XLSM — no external services required ─────────────────────────────────────

#[test]
fn test_extract_xlsm_returns_text() {
    let path = fixtures().join("Exemplos_Folha_de_Calculo_101_A.xlsm");
    let result = extract(&path, "http://localhost:11434", &no_cancel()).unwrap();
    assert!(!result.text.is_empty(), "expected text from XLSM file");
    assert!((result.confidence - 1.0).abs() < 1e-6);
}

// ── WebP — routing test (no Ollama required) ──────────────────────────────────

#[test]
fn test_extract_webp_routes_to_image_extractor() {
    let path = fixtures().join("An-example-of-scanned-receipt-from-SROIE-3-dataset.webp");
    // Port 19999 has nothing listening — triggers a connection error, not "unsupported file type".
    match extract(&path, "http://127.0.0.1:19999", &no_cancel()) {
        Err(crate::errors::AppError::Extractor(msg)) => {
            assert!(!msg.contains("unsupported file type"),
                "webp must be routed to image extractor, not rejected: {msg}");
        }
        _ => {} // success or LlmCall error — both confirm routing succeeded
    }
}

// ── OCR unit tests — no external services required ────────────────────────────

#[test]
fn test_ocr_confidence_threshold_constant() {
    assert!((pdf::OCR_CONFIDENCE_THRESHOLD - 0.75).abs() < 1e-6);
}

#[test]
fn test_run_tesseract_bounded_returns_quickly_on_cancel() {
    use std::time::{Duration, Instant};
    let tessdata = crate::platform::tessdata_dir();
    let img = image::RgbaImage::new(2048, 2048);
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let started = Instant::now();
    let result = pdf::run_tesseract_bounded(tessdata, img, &cancel, 90);
    let elapsed = started.elapsed();
    assert!(
        matches!(result, Err(crate::errors::AppError::Indexer(ref msg)) if msg == "cancelled"),
        "expected cancelled error, got: {result:?}"
    );
    assert!(elapsed < Duration::from_secs(2), "cancel not honoured promptly; took {elapsed:?}");
}

#[test]
fn test_ocr_on_clean_png_image() {
    let tessdata = crate::platform::tessdata_dir();
    let img_path = fixtures().join("16626587.png");
    let img = image::open(&img_path)
        .expect("fixture image should be readable")
        .into_rgba8();
    let result = pdf::run_tesseract_on_rgba(&tessdata, &img);
    assert!(result.is_ok(), "Tesseract should not error on a valid image: {result:?}");
    let (_, conf) = result.unwrap();
    assert!(conf >= 0.0 && conf <= 1.0, "confidence must be in [0, 1], got {conf}");
}

#[test]
fn test_ocr_lang_constant_contains_eng_and_por() {
    assert!(pdf::TESS_LANG.contains("eng"), "must include English");
    assert!(pdf::TESS_LANG.contains("por"), "must include Portuguese");
}

// ── Scanned PDF — requires pdfium (no Ollama needed for OCR path) ─────────────

#[test]
#[ignore = "requires pdfium library; Naac_appLetter.pdf is a scanned PDF — exercises the OCR path"]
fn test_extract_pdf_scanned_via_ocr() {
    let path = fixtures().join("Naac_appLetter.pdf");
    let result = extract(&path, "http://localhost:11434", &no_cancel()).unwrap();
    assert!(!result.text.is_empty(), "scanned PDF should produce OCR text");
    assert!(result.confidence > 0.0 && result.confidence <= 1.0);
}

#[test]
fn test_extract_pdf_completes_in_bounded_time() {
    use std::sync::mpsc;
    use std::time::Duration;

    let path = fixtures().join("Naac_appLetter.pdf");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = extract(&path, "http://127.0.0.1:19999", &no_cancel());
        let _ = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_secs(180)) {
        Ok(Ok(result)) => assert!(!result.text.is_empty()),
        Ok(Err(e)) => panic!("extract returned error: {e:?}"),
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("extract did not return within 180s (issue #12 regression)"),
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("worker died"),
    }
}

#[test]
#[ignore = "deliberately deadlocks; documents the pdfium overlap constraint that bbbb22c works around"]
fn test_pdfium_instance_overlapping_lifetimes_deadlock_on_same_thread() {
    use std::sync::mpsc;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let first = pdf::pdfium_instance();
        let second = pdf::pdfium_instance();
        let _ = tx.send((first.is_ok(), second.is_ok()));
    });

    match rx.recv_timeout(Duration::from_secs(30)) {
        Ok((true, true)) => panic!(
            "two overlapping pdfium_instance() calls on the same thread returned — \
             the unsafety this test guards against has been fixed properly. \
             Remove the drop() workarounds added in bbbb22c, then delete this test."
        ),
        Ok((a, b)) => panic!("pdfium_instance unexpected failure: first={a} second={b}"),
        Err(mpsc::RecvTimeoutError::Timeout) => {}
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("worker died"),
    }
}


// ── Image + vision fallback — require Ollama ──────────────────────────────────

#[test]
#[ignore = "requires Ollama running with qwen2.5vl:7b or llava:7b"]
fn test_extract_image_png_returns_description() {
    let path = fixtures().join("16626587.png");
    let result = extract(&path, "http://localhost:11434", &no_cancel()).unwrap();
    assert!(!result.text.is_empty(), "vision model should describe the image");
}

#[test]
#[ignore = "requires Ollama running with qwen2.5vl:7b or llava:7b"]
fn test_extract_image_jpg_returns_description() {
    let path = fixtures().join("images.jpg");
    let result = extract(&path, "http://localhost:11434", &no_cancel()).unwrap();
    assert!(!result.text.is_empty(), "vision model should describe the image");
}

#[test]
#[ignore = "requires pdfium + Ollama; exercises OCR→vision fallback path for a scanned PDF"]
fn test_extract_pdf_scanned_ocr_then_vision_fallback() {
    let path = fixtures().join("Naac_appLetter.pdf");
    let result = extract(&path, "http://localhost:11434", &no_cancel()).unwrap();
    assert!(!result.text.is_empty(), "should produce output via OCR or vision");
}

// ── DOCX ──────────────────────────────────────────────────────────────────────

#[test]
fn test_extract_docx_returns_text() {
    let path = fixtures().join("Guia de Montagem Indústria 4.0.docx");
    let result = extract(&path, "http://localhost:11434", &no_cancel()).unwrap();
    assert!(!result.text.is_empty(), "expected text from DOCX file");
    assert!((result.confidence - 1.0).abs() < 1e-6);
}
