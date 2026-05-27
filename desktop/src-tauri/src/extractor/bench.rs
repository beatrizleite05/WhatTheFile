//! Per-page timing harness for the PDF → vision extraction path.
//!
//! cargo test --release -- --ignored --nocapture pdf_vision_bench
//!
//! Env: BENCH_VISION_MODEL, BENCH_PDF

use super::pdf::pdfium_instance;
use pdfium_render::prelude::*;
use std::path::PathBuf;
use std::time::Instant;

const DEFAULT_MODEL: &str = "moondream:1.8b";
const DEFAULT_FIXTURE_REL: &str = "../test/sample-files/Naac_appLetter.pdf";
const OLLAMA_URL: &str = "http://localhost:11434";
const PROMPT: &str = "Describe this image in detail, focusing on any text, objects, and context visible.";

fn vm_stat_swap() -> Option<(u64, u64)> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let out = std::process::Command::new("vm_stat").output().ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    let mut swapins: Option<u64> = None;
    let mut swapouts: Option<u64> = None;
    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("swapins:") {
            swapins = line.split_whitespace().last()?.trim_end_matches('.').parse().ok();
        } else if lower.starts_with("swapouts:") {
            swapouts = line.split_whitespace().last()?.trim_end_matches('.').parse().ok();
        }
    }
    Some((swapins?, swapouts?))
}

fn render_page_to_png(page: &PdfPage) -> Result<Vec<u8>, String> {
    let cfg = PdfRenderConfig::new()
        .set_target_width(512)
        .set_maximum_height(720);
    let bitmap = page.render_with_config(&cfg).map_err(|e| format!("render: {e}"))?;
    let rgba = bitmap.as_image().into_rgba8();
    let img = image::RgbaImage::from_raw(bitmap.width() as u32, bitmap.height() as u32, rgba.into_raw())
        .ok_or("rgba size mismatch")?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).map_err(|e| format!("png encode: {e}"))?;
    Ok(buf.into_inner())
}

fn call_ollama_generate(model: &str, image_b64: &str) -> Result<(String, u128), String> {
    let url = format!("{OLLAMA_URL}/api/generate");
    let body = serde_json::json!({
        "model": model,
        "prompt": PROMPT,
        "images": [image_b64],
        "stream": false,
        "keep_alive": "10m",
    });

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(600))
        .build();

    let t0 = Instant::now();
    let response = agent
        .post(&url)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("request failed: {e}"))?;
    let json: serde_json::Value = response.into_json().map_err(|e| format!("parse: {e}"))?;
    let elapsed_ms = t0.elapsed().as_millis();

    let text = json["response"]
        .as_str()
        .ok_or("missing 'response' field")?
        .to_string();
    Ok((text, elapsed_ms))
}

#[test]
#[ignore = "diagnostic harness; requires Ollama running with the chosen vision model"]
fn pdf_vision_bench() {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    let model = std::env::var("BENCH_VISION_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let fixture = std::env::var("BENCH_PDF").unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(DEFAULT_FIXTURE_REL)
            .to_string_lossy()
            .into_owned()
    });

    println!("[BENCH] model    = {model}");
    println!("[BENCH] fixture  = {fixture}");
    println!("[BENCH] ollama   = {OLLAMA_URL}");

    let pdfium = pdfium_instance().expect("pdfium load");
    let doc = pdfium.load_pdf_from_file(&fixture, None).expect("pdf open");
    let page_count = doc.pages().len();
    println!("[BENCH] pages    = {page_count}");

    let swap_before = vm_stat_swap();
    if let Some((si, so)) = swap_before {
        println!("[BENCH] vm_stat before: swapins={si} swapouts={so}");
    }

    let first_page = doc.pages().get(0).expect("first page");
    let png = render_page_to_png(&first_page).expect("render page 0");
    let b64 = B64.encode(&png);
    let t0 = Instant::now();
    let warmup = call_ollama_generate(&model, &b64);
    let warmup_ms = t0.elapsed().as_millis();
    match &warmup {
        Ok((text, _)) => println!("[BENCH] warmup(p0) ms={warmup_ms} desc_len={}", text.len()),
        Err(e) => {
            println!("[BENCH] warmup FAILED ms={warmup_ms} err={e}");
            panic!("warmup failed — aborting bench");
        }
    }

    let total_t0 = Instant::now();
    let mut per_page_ms: Vec<u128> = Vec::with_capacity(page_count as usize);
    let mut errors = 0;

    for page_idx in 0..page_count {
        let page = match doc.pages().get(page_idx) {
            Ok(p) => p,
            Err(e) => {
                println!("[BENCH] page {page_idx} get error: {e}");
                errors += 1;
                continue;
            }
        };
        let render_t0 = Instant::now();
        let png = match render_page_to_png(&page) {
            Ok(p) => p,
            Err(e) => {
                println!("[BENCH] page {page_idx} render error: {e}");
                errors += 1;
                continue;
            }
        };
        let render_ms = render_t0.elapsed().as_millis();
        let b64 = B64.encode(&png);

        match call_ollama_generate(&model, &b64) {
            Ok((text, vision_ms)) => {
                per_page_ms.push(vision_ms);
                println!(
                    "[BENCH] page {page_idx}: render_ms={render_ms} vision_ms={vision_ms} desc_len={}",
                    text.len()
                );
            }
            Err(e) => {
                println!("[BENCH] page {page_idx}: render_ms={render_ms} vision FAILED err={e}");
                errors += 1;
            }
        }
    }

    let total_ms = total_t0.elapsed().as_millis();
    let swap_after = vm_stat_swap();

    let n = per_page_ms.len() as u128;
    let mean = if n > 0 { per_page_ms.iter().sum::<u128>() / n } else { 0 };
    let min = per_page_ms.iter().min().copied().unwrap_or(0);
    let max = per_page_ms.iter().max().copied().unwrap_or(0);

    println!("[BENCH] ── summary ─────────────────────────────────────────");
    println!("[BENCH] model={model} pages={page_count} ok={n} errors={errors}");
    println!("[BENCH] warmup_ms={warmup_ms} total_ms={total_ms}");
    println!("[BENCH] per_page_ms min={min} mean={mean} max={max}");
    if let (Some((si0, so0)), Some((si1, so1))) = (swap_before, swap_after) {
        println!(
            "[BENCH] swap delta: swapins={} swapouts={}",
            si1.saturating_sub(si0),
            so1.saturating_sub(so0)
        );
    }
    println!("[BENCH] ────────────────────────────────────────────────────");
}
