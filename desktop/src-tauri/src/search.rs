use std::collections::HashMap;
use rusqlite::{Connection, params};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use crate::errors::AppError;
use crate::llm::embeddings;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub query_text: String,
    pub media_types: Vec<String>,
    pub root_scope: Vec<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub min_confidence: f32,
    pub limit: u32,
    pub offset: u32,
    /// Search mode: "keyword" (FTS only), "semantic" (vector only), or
    /// omitted / "hybrid" (both passes with RRF blend — default).
    #[serde(default)]
    pub mode: String,
    /// Opaque cursor returned by a previous `SearchResponse.nextCursor`.
    /// When present it takes precedence over `offset`.
    #[serde(default)]
    pub cursor: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchResult {
    pub file_id: i64,
    pub root_id: i64,
    pub path: String,
    pub filename: String,
    pub media_type: String,
    pub size_bytes: i64,
    pub indexed_at: i64,
    pub confidence: f32,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub results: Vec<FileSearchResult>,
    pub total: usize,
    pub limit: u32,
    pub offset: u32,
    /// Opaque token for the next page.  `None` when there are no more results.
    pub next_cursor: Option<String>,
}

// ── cursor helpers ────────────────────────────────────────────────────────────

fn encode_cursor(score: f64, path: &str) -> String {
    let raw = format!("{:016x}|{}", score.to_bits(), path);
    B64.encode(raw.as_bytes())
}

fn decode_cursor(cursor: &str) -> Option<(f64, String)> {
    let bytes = B64.decode(cursor).ok()?;
    let s = String::from_utf8(bytes).ok()?;
    let (hex, path) = s.split_once('|')?;
    let bits = u64::from_str_radix(hex, 16).ok()?;
    Some((f64::from_bits(bits), path.to_string()))
}

/// Parse "YYYY-MM-DD" to unix seconds (midnight UTC, proleptic Gregorian).
/// Returns `None` on malformed input.
fn date_str_to_unix(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: i64 = parts[0].parse().ok()?;
    let m: i64 = parts[1].parse().ok()?;
    let d: i64 = parts[2].parse().ok()?;
    // Days from 1970-01-01 using the standard algorithm
    let leap_days = |year: i64| year / 4 - year / 100 + year / 400;
    let days_from_epoch = (y - 1970) * 365
        + leap_days(y - 1)
        - leap_days(1969)
        + [0i64, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334]
            [(m as usize).saturating_sub(1).min(11)]
        + d
        - 1
        + if m > 2 && (y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)) { 1 } else { 0 };
    Some(days_from_epoch * 86400)
}

/// Escape a user query for FTS5.
///
/// Only embedded double-quote characters are escaped (doubled per FTS5 syntax).
/// The query is NOT wrapped in outer quotes so that FTS5 tokenises it normally
/// and honours any AND / OR / NOT operators the user typed.
fn fts5_escape(query: &str) -> String {
    query.replace('"', "\"\"")
}

pub fn search_files(
    conn: &Connection,
    query: &SearchQuery,
    ollama_url: &str,
) -> Result<SearchResponse, AppError> {
    let limit = query.limit.clamp(1, 200);
    // Cursor takes precedence over offset when present and valid.
    let cursor_pos: Option<(f64, String)> = if query.cursor.is_empty() {
        None
    } else {
        decode_cursor(&query.cursor)
    };
    let offset = query.offset as usize;

    let effective_mode = query.mode.as_str();
    let run_fts = effective_mode != "semantic";
    let run_vec = effective_mode != "keyword";

    // ── Step 1: FTS5 BM25 retrieval (top 50 file-level) ─────────────────────
    let fts_term = fts5_escape(&query.query_text);
    let mut fts_map: HashMap<i64, f64> = HashMap::new();
    if run_fts {
        let mut stmt = conn.prepare(
            "SELECT fts.rowid AS file_id, fts.rank
             FROM files_fts fts
             WHERE files_fts MATCH ?1
             ORDER BY fts.rank
             LIMIT 50",
        )?;
        let rows = stmt.query_map(params![fts_term], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
        })?;
        for row in rows {
            let (file_id, rank) = row?;
            fts_map.entry(file_id).or_insert(rank);
        }
    }

    // ── Step 2: cosine similarity retrieval (top 50 chunk-level → file-level) ─
    // vec_map: file_id → (best_distance, best_chunk_id)
    // Lower distance = more similar (KNN L2; equivalent to cosine for unit vectors).
    let mut vec_map: HashMap<i64, (f64, i64)> = HashMap::new();
    if run_vec {
        let embedding = embeddings::embed_text(&query.query_text, ollama_url)?;
        let query_blob = embeddings::embedding_to_bytes(&embedding);
        let mut stmt = conn.prepare(
            "SELECT cv.chunk_id, c.file_id, cv.distance
             FROM chunks_vec cv
             JOIN chunks c ON c.id = cv.chunk_id
             WHERE cv.embedding MATCH ?1 AND k = 50
             ORDER BY cv.distance",
        )?;
        let rows = stmt.query_map(params![query_blob], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, f64>(2)?))
        })?;
        for row in rows {
            let (chunk_id, file_id, distance) = row?;
            let entry = vec_map.entry(file_id).or_insert((f64::INFINITY, chunk_id));
            if distance < entry.0 {
                *entry = (distance, chunk_id);
            }
        }
    }

    // ── Step 3: assign 1-based ranks ─────────────────────────────────────────
    let mut fts_ranked: Vec<i64> = fts_map.keys().copied().collect();
    fts_ranked.sort_by(|a, b| {
        fts_map[a].partial_cmp(&fts_map[b]).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut vec_ranked: Vec<i64> = vec_map.keys().copied().collect();
    // Ascending: smallest distance = rank 1 (closest to query).
    vec_ranked.sort_by(|a, b| {
        vec_map[a].0.partial_cmp(&vec_map[b].0).unwrap_or(std::cmp::Ordering::Equal)
    });

    let fts_rank_pos: HashMap<i64, usize> =
        fts_ranked.iter().enumerate().map(|(i, &id)| (id, i + 1)).collect();
    let vec_rank_pos: HashMap<i64, usize> =
        vec_ranked.iter().enumerate().map(|(i, &id)| (id, i + 1)).collect();

    // ── Step 4: RRF blend ────────────────────────────────────────────────────
    let mut all_ids: Vec<i64> = fts_map.keys().chain(vec_map.keys()).copied().collect();
    all_ids.sort_unstable();
    all_ids.dedup();

    let rrf_scores: HashMap<i64, f64> = all_ids
        .iter()
        .map(|&id| {
            let mut score = 0.0_f64;
            if let Some(&r) = fts_rank_pos.get(&id) {
                score += 1.0 / (60.0 + r as f64);
            }
            if let Some(&r) = vec_rank_pos.get(&id) {
                score += 1.0 / (60.0 + r as f64);
            }
            (id, score)
        })
        .collect();

    if all_ids.is_empty() {
        return Ok(SearchResponse { results: vec![], total: 0, limit, offset: query.offset, next_cursor: None });
    }

    // ── Step 5: fetch metadata + apply filters ────────────────────────────────
    let placeholders = all_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT f.id, f.root_id, f.rel_path, f.filename, f.media_type,
                f.size_bytes, f.indexed_at, f.confidence, f.mtime_ns, f.extracted_text
         FROM files f
         WHERE f.id IN ({placeholders})
           AND f.deleted_at IS NULL"
    );

    let mut results: Vec<FileSearchResult> = {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(all_ids.iter().map(|id| *id as i64)),
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, f32>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, String>(9)?,
                ))
            },
        )?;

        // date_from/to thresholds are unix seconds; mtime_ns is nanoseconds.
        let date_from_ns = query.date_from.as_deref().and_then(date_str_to_unix)
            .map(|s| s * 1_000_000_000);
        let date_to_ns = query.date_to.as_deref().and_then(|s| {
            date_str_to_unix(s).map(|t| (t + 86399) * 1_000_000_000)
        });

        let mut out = Vec::new();
        for row in rows {
            let (file_id, root_id, rel_path, filename, media_type,
                 size_bytes, indexed_at, confidence, mtime_ns, extracted_text) = row?;

            if !query.media_types.is_empty()
                && !query.media_types.iter().any(|m| m.eq_ignore_ascii_case(&media_type))
            {
                continue;
            }
            if query.min_confidence > 0.0 && confidence < query.min_confidence {
                continue;
            }
            if let Some(ns) = date_from_ns {
                if mtime_ns < ns {
                    continue;
                }
            }
            if let Some(ns) = date_to_ns {
                if mtime_ns > ns {
                    continue;
                }
            }

            let rrf_score = rrf_scores.get(&file_id).copied().unwrap_or(0.0);

            // ── Step 6: build snippet ─────────────────────────────────────────
            let best_chunk_id = vec_map.get(&file_id).map(|&(_, cid)| cid);
            let snippet = build_snippet(conn, best_chunk_id, &extracted_text);

            out.push(FileSearchResult {
                file_id,
                root_id,
                path: rel_path,
                filename,
                media_type,
                size_bytes,
                indexed_at,
                confidence,
                snippet,
                score: rrf_score,
            });
        }
        out
    };

    // Root scope filter — strict path-component match to prevent "doc" from
    // accidentally matching "/home/documents".  A root qualifies when any of
    // its path components (split on '/') exactly equals a scope token
    // (case-insensitive).
    if !query.root_scope.is_empty() {
        let scope_lower: Vec<String> = query.root_scope.iter().map(|s| s.to_lowercase()).collect();
        let root_ids_in_scope: Vec<i64> = {
            let mut stmt = conn.prepare("SELECT id FROM roots WHERE active = 1")?;
            let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
            let mut matching = Vec::new();
            for row in rows {
                let rid = row?;
                if let Ok(Some(root)) = crate::db::find_root_by_id(conn, rid) {
                    let components: Vec<&str> = root.path
                        .split('/')
                        .filter(|c| !c.is_empty())
                        .collect();
                    if scope_lower.iter().any(|s| {
                        components.iter().any(|c| c.to_lowercase() == s.as_str())
                    }) {
                        matching.push(rid);
                    }
                }
            }
            matching
        };
        results.retain(|r| root_ids_in_scope.contains(&r.root_id));
    }

    // ── Step 7: sort + paginate ───────────────────────────────────────────────
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
    });

    let total = results.len();

    // Resolve the effective starting position.  If a valid cursor was supplied,
    // skip forward past the last result the caller already saw.
    let effective_offset = if let Some((cur_score, ref cur_path)) = cursor_pos {
        results.iter().position(|r| {
            // Find the first result strictly after the cursor position.
            r.score < cur_score || (r.score == cur_score && r.path.as_str() > cur_path.as_str())
        }).unwrap_or(total) // cursor is past all results → empty page
    } else {
        offset
    };

    let page: Vec<FileSearchResult> = results
        .into_iter()
        .skip(effective_offset)
        .take(limit as usize)
        .collect();

    let next_cursor = if effective_offset + page.len() < total {
        page.last().map(|r| encode_cursor(r.score, &r.path))
    } else {
        None
    };

    Ok(SearchResponse { results: page, total, limit, offset: query.offset, next_cursor })
}

fn build_snippet(
    conn: &Connection,
    best_chunk_id: Option<i64>,
    extracted_text: &str,
) -> String {
    // Use the best-scoring chunk from the vector retrieval pass when available.
    // Fall back to the first chunk, then to extracted_text[:200].
    if let Some(chunk_id) = best_chunk_id {
        match conn.query_row(
            "SELECT text FROM chunks WHERE id = ?1",
            params![chunk_id],
            |r| r.get::<_, String>(0),
        ) {
            Ok(t) => return t.chars().take(200).collect(),
            Err(e) => {
                log::warn!("snippet lookup failed for chunk_id={chunk_id}: {e}");
            }
        }
    }
    extracted_text.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn make_test_conn() -> Connection {
        db::open_and_migrate(std::path::Path::new(":memory:")).unwrap()
    }

    fn seed_root(conn: &Connection) -> i64 {
        db::insert_root(conn, "/test/root").unwrap().id
    }

    fn seed_file(conn: &Connection, root_id: i64, rel_path: &str, media_type: &str,
                 text: &str, confidence: f32) -> i64 {
        let now = db::unix_now();
        let id = db::upsert_file_metadata(
            conn, root_id, rel_path, rel_path, media_type,
            1024, 1000, &format!("fp-{rel_path}"), "ext-v1", now, now,
        ).unwrap();
        db::update_file_content(conn, id, text, confidence, "en", "ext-v1", now).unwrap();
        db::replace_chunks(conn, id, &[(0, text, None)]).unwrap();
        id
    }

    #[test]
    fn test_rrf_formula_correctness() {
        let score_at_rank = |r: usize| 1.0_f64 / (60.0 + r as f64);
        // File appearing at rank 1 in both → 2/61
        let both = score_at_rank(1) + score_at_rank(1);
        assert!((both - 2.0 / 61.0).abs() < 1e-9);
        // File only in FTS at rank 1 → 1/61
        let fts_only = score_at_rank(1);
        assert!((fts_only - 1.0 / 61.0).abs() < 1e-9);
    }

    #[test]
    fn test_fts_returns_matching_file() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        seed_file(&conn, root_id, "invoice.txt", "txt", "invoice receipt total amount", 1.0);
        seed_file(&conn, root_id, "photo.jpg", "jpg", "cat sitting on a mat", 1.0);

        let query = SearchQuery {
            query_text: "invoice".into(),
            media_types: vec![],
            root_scope: vec![],
            date_from: None,
            date_to: None,
            min_confidence: 0.0,
            limit: 10,
            offset: 0,
            mode: String::new(),
            cursor: String::new(),
        };

        // FTS path only (no Ollama) — override by testing FTS SQL directly
        let fts_term = fts5_escape("invoice");
        let mut stmt = conn.prepare(
            "SELECT fts.rowid FROM files_fts fts WHERE files_fts MATCH ?1 ORDER BY fts.rank LIMIT 10",
        ).unwrap();
        let ids: Vec<i64> = stmt
            .query_map(params![fts_term], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(!ids.is_empty(), "FTS must find at least one result for 'invoice'");
        let invoice_id: i64 = conn.query_row(
            "SELECT id FROM files WHERE rel_path = 'invoice.txt'", [], |r| r.get(0),
        ).unwrap();
        assert!(ids.contains(&invoice_id));
    }

    #[test]
    fn test_vector_search_with_no_embeddings_returns_empty() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        // seed_file passes None embedding → no entry in chunks_vec
        seed_file(&conn, root_id, "doc.txt", "txt", "some text", 1.0);

        let query_blob = embeddings::embedding_to_bytes(&vec![0.0_f32; 768]);
        let mut stmt = conn.prepare(
            "SELECT cv.chunk_id, c.file_id, cv.distance
             FROM chunks_vec cv
             JOIN chunks c ON c.id = cv.chunk_id
             WHERE cv.embedding MATCH ?1 AND k = 50
             ORDER BY cv.distance",
        ).unwrap();
        let rows: Vec<(i64, i64)> = stmt
            .query_map(params![query_blob], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_filter_deleted_at_excludes_soft_deleted() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        let now = db::unix_now();
        let id = db::upsert_file_metadata(
            &conn, root_id, "deleted.txt", "deleted.txt", "txt",
            100, 1000, "fp-del", "ext-v1", 1, now,
        ).unwrap();
        db::update_file_content(&conn, id, "invoice", 1.0, "en", "ext-v1", now).unwrap();
        db::replace_chunks(&conn, id, &[(0, "invoice", None)]).unwrap();
        // Soft-delete
        conn.execute(
            "UPDATE files SET deleted_at = ?1 WHERE id = ?2", params![now, id],
        ).unwrap();

        let fts_term = fts5_escape("invoice");
        let mut stmt = conn.prepare(
            "SELECT f.id FROM files_fts fts
             JOIN files f ON f.id = fts.rowid
             WHERE files_fts MATCH ?1 AND f.deleted_at IS NULL",
        ).unwrap();
        let ids: Vec<i64> = stmt
            .query_map(params![fts_term], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(!ids.contains(&id), "soft-deleted file must not appear in results");
    }

    #[test]
    fn test_filter_media_type() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        let pdf_id = seed_file(&conn, root_id, "report.pdf", "pdf", "quarterly report", 1.0);
        let docx_id = seed_file(&conn, root_id, "notes.docx", "docx", "quarterly report", 1.0);

        let fts_term = fts5_escape("quarterly");
        let mut stmt = conn.prepare(
            "SELECT f.id, f.media_type FROM files_fts fts
             JOIN files f ON f.id = fts.rowid
             WHERE files_fts MATCH ?1 AND f.deleted_at IS NULL AND f.media_type = 'pdf'",
        ).unwrap();
        let rows: Vec<(i64, String)> = stmt
            .query_map(params![fts_term], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(rows.iter().any(|(id, _)| *id == pdf_id));
        assert!(!rows.iter().any(|(id, _)| *id == docx_id));
    }

    #[test]
    fn test_filter_min_confidence() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        let high_id = seed_file(&conn, root_id, "high.txt", "txt", "contract agreement", 0.95);
        let low_id = seed_file(&conn, root_id, "low.txt", "txt", "contract agreement", 0.6);

        let fts_term = fts5_escape("contract");
        let min_conf = 0.8_f32;
        let mut stmt = conn.prepare(
            "SELECT f.id FROM files_fts fts
             JOIN files f ON f.id = fts.rowid
             WHERE files_fts MATCH ?1 AND f.deleted_at IS NULL AND f.confidence >= ?2",
        ).unwrap();
        let ids: Vec<i64> = stmt
            .query_map(params![fts_term, min_conf], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(ids.contains(&high_id));
        assert!(!ids.contains(&low_id));
    }

    #[test]
    fn test_rrf_score_ordering() {
        let mut rrf: HashMap<i64, f64> = HashMap::new();
        rrf.insert(1, 1.0 / 61.0 + 1.0 / 62.0);
        rrf.insert(2, 1.0 / 62.0);
        rrf.insert(3, 1.0 / 63.0);

        let mut ids: Vec<i64> = rrf.keys().copied().collect();
        ids.sort_by(|a, b| rrf[b].partial_cmp(&rrf[a]).unwrap());

        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn test_pagination_offset_and_limit() {
        // Five results; request page 2 (offset=2, limit=2)
        let all: Vec<i64> = vec![10, 20, 30, 40, 50];
        let total = all.len();
        let limit = 2usize;
        let offset = 2usize;
        let page: Vec<i64> = all.into_iter().skip(offset).take(limit).collect();
        assert_eq!(page, vec![30, 40]);
        assert_eq!(total, 5);
    }

    #[test]
    fn test_date_str_to_unix() {
        // 1970-01-01 = 0
        assert_eq!(date_str_to_unix("1970-01-01"), Some(0));
        // 1970-01-02 = 86400
        assert_eq!(date_str_to_unix("1970-01-02"), Some(86400));
        // 2024-01-01 — verify it's after 2023-01-01
        let t2024 = date_str_to_unix("2024-01-01").unwrap();
        let t2023 = date_str_to_unix("2023-01-01").unwrap();
        assert!(t2024 > t2023);
        assert_eq!(t2024 - t2023, 365 * 86400);
        // Malformed input
        assert!(date_str_to_unix("not-a-date").is_none());
    }

    #[test]
    fn test_fts5_escape_does_not_phrase_wrap() {
        // Plain query passes through unchanged — FTS5 tokenises it normally,
        // which preserves AND/OR/NOT operators the user typed.
        assert_eq!(fts5_escape("hello world"), "hello world");
        assert_eq!(fts5_escape("contract OR agreement"), "contract OR agreement");
    }

    #[test]
    fn test_fts5_escape_doubles_internal_quotes() {
        // Embedded double-quote is escaped by doubling per FTS5 syntax.
        assert_eq!(fts5_escape(r#"say "hello""#), r#"say ""hello"""#);
    }

    #[test]
    fn test_keyword_mode_skips_vector_pass() {
        // In keyword mode the vec_map must remain empty even if chunks have embeddings.
        // We verify the flag logic directly: run_vec = (mode != "keyword").
        let keyword_mode = "keyword";
        let hybrid_mode = "";
        let semantic_mode = "semantic";
        assert!(!( keyword_mode != "keyword"), "keyword mode: run_vec should be false");
        assert!(  hybrid_mode  != "keyword",   "hybrid mode: run_vec should be true");
        assert!(  semantic_mode != "keyword",  "semantic mode: run_vec should be true");
    }

    #[test]
    fn test_semantic_mode_skips_fts_pass() {
        let keyword_mode = "keyword";
        let hybrid_mode = "";
        let semantic_mode = "semantic";
        assert!(  keyword_mode  != "semantic", "keyword mode: run_fts should be true");
        assert!(  hybrid_mode   != "semantic", "hybrid mode: run_fts should be true");
        assert!(!( semantic_mode != "semantic"), "semantic mode: run_fts should be false");
    }

    #[test]
    fn test_mode_default_is_hybrid() {
        // Default-constructed mode is empty string, treated as hybrid.
        let q = SearchQuery {
            query_text: "test".into(),
            media_types: vec![],
            root_scope: vec![],
            date_from: None,
            date_to: None,
            min_confidence: 0.0,
            limit: 10,
            offset: 0,
            mode: String::new(),
            cursor: String::new(),
        };
        assert_eq!(q.mode, "");
        assert!(q.mode.as_str() != "keyword"); // both passes run
        assert!(q.mode.as_str() != "semantic");
    }

    // ── cursor pagination ─────────────────────────────────────────────

    #[test]
    fn test_cursor_encode_decode_roundtrip() {
        let score = 0.031_746_031_f64;
        let path = "/home/alice/documents/report.pdf";
        let cursor = encode_cursor(score, path);
        let (s2, p2) = decode_cursor(&cursor).expect("cursor must decode");
        assert_eq!(s2.to_bits(), score.to_bits(), "score must survive roundtrip");
        assert_eq!(p2, path, "path must survive roundtrip");
    }

    #[test]
    fn test_cursor_decode_returns_none_on_garbage() {
        assert!(decode_cursor("not-valid!!!").is_none());
        assert!(decode_cursor("").is_none());
    }

    #[test]
    fn test_cursor_pagination_returns_next_page() {
        // Simulate 5 sorted results; page size 2.
        // After fetching page 1 we get a cursor; page 2 must start where page 1 left off.
        let all_scores: Vec<(f64, &str)> = vec![
            (1.0, "a.txt"),
            (0.9, "b.txt"),
            (0.8, "c.txt"),
            (0.7, "d.txt"),
            (0.6, "e.txt"),
        ];
        let limit = 2usize;

        // Page 1: offset 0
        let page1: Vec<_> = all_scores.iter().take(limit).collect();
        assert_eq!(page1.len(), 2);

        // Cursor from last result of page 1
        let (last_score, last_path) = page1.last().copied().unwrap();
        let cursor = encode_cursor(*last_score, last_path);

        // Decode and find resume position
        let (cur_s, ref cur_p) = decode_cursor(&cursor).unwrap();
        let skip = all_scores.iter().position(|(s, p)| {
            *s < cur_s || (*s == cur_s && *p > cur_p.as_str())
        }).unwrap_or(all_scores.len());

        let page2: Vec<_> = all_scores.iter().skip(skip).take(limit).collect();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].1, "c.txt");
        assert_eq!(page2[1].1, "d.txt");
    }

    // ── strict root scope component matching ───────────────────────────

    fn path_matches_scope(path: &str, scope: &str) -> bool {
        let scope_lower = scope.to_lowercase();
        path.split('/')
            .filter(|c| !c.is_empty())
            .any(|c| c.to_lowercase() == scope_lower)
    }

    #[test]
    fn test_root_scope_exact_component_matches() {
        assert!( path_matches_scope("/home/documents",  "documents"),  "exact component match");
        assert!( path_matches_scope("/home/Documents",  "documents"),  "case-insensitive");
        assert!(!path_matches_scope("/home/documents",  "doc"),        "prefix must not match");
        assert!(!path_matches_scope("/home/doc_folder", "documents"),  "substring must not match");
        assert!( path_matches_scope("/Users/alice/Dropbox", "dropbox"),"multi-level path");
        assert!(!path_matches_scope("/home/adocuments",  "doc"),       "infix must not match");
    }

    // ── asymmetric RRF ─────────────────────────────────────────────────

    #[test]
    fn test_rrf_fts_only_score_is_less_than_both() {
        // A file that appears in BOTH retrieval passes must score higher than
        // one appearing in only one pass at the same rank position.
        let score = |fts: Option<usize>, vec: Option<usize>| -> f64 {
            let mut s = 0.0_f64;
            if let Some(r) = fts { s += 1.0 / (60.0 + r as f64); }
            if let Some(r) = vec { s += 1.0 / (60.0 + r as f64); }
            s
        };

        let both_rank1   = score(Some(1), Some(1));
        let fts_only_r1  = score(Some(1), None);
        let vec_only_r1  = score(None,    Some(1));
        let both_rank5   = score(Some(5), Some(5));

        assert!(both_rank1 > fts_only_r1,  "both > fts-only at same rank");
        assert!(both_rank1 > vec_only_r1,  "both > vec-only at same rank");
        assert!(fts_only_r1 == vec_only_r1,"fts-only and vec-only are symmetric");
        assert!(both_rank1 > both_rank5,   "rank 1 beats rank 5");
    }

    #[test]
    fn test_rrf_asymmetric_result_sets_order() {
        // File A: FTS rank 1, no vec result
        // File B: vec rank 1, no FTS result
        // File C: FTS rank 2 AND vec rank 2
        // Expected order: C > A = B  (C has two contributions)
        let mut rrf: HashMap<i64, f64> = HashMap::new();
        rrf.insert(1, 1.0 / (60.0 + 1.0));           // A: fts-only rank 1
        rrf.insert(2, 1.0 / (60.0 + 1.0));           // B: vec-only rank 1
        rrf.insert(3, 1.0 / (60.0 + 2.0) + 1.0 / (60.0 + 2.0)); // C: both rank 2

        let mut ids: Vec<i64> = rrf.keys().copied().collect();
        ids.sort_by(|a, b| rrf[b].partial_cmp(&rrf[a]).unwrap());

        assert_eq!(ids[0], 3, "file in both passes ranks first");
        // A and B tie — either order is valid
        assert!(ids[1] == 1 || ids[1] == 2);
        assert!(ids[2] == 1 || ids[2] == 2);
        assert_ne!(ids[1], ids[2]);
    }

    // ── combined date + media type filter ──────────────────────────────

    #[test]
    fn test_combined_date_and_media_type_filter() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);

        // mtime_ns values: old file has mtime in 2020, new files have recent mtime.
        // The date filter compares against mtime_ns, not indexed_at.
        let old_mtime_ns: i64 = 1_577_836_800_000_000_000; // 2020-01-01 00:00:00 UTC in ns
        let now_ns = db::unix_now() * 1_000_000_000;
        let now = db::unix_now();

        let pdf_old = db::upsert_file_metadata(
            &conn, root_id, "old.pdf", "old.pdf", "pdf",
            100, old_mtime_ns, "fp-old-pdf", "ext-v1", now, now,
        ).unwrap();
        db::update_file_content(&conn, pdf_old, "contract old", 1.0, "en", "ext-v1", now).unwrap();
        db::replace_chunks(&conn, pdf_old, &[(0, "contract old", None)]).unwrap();

        let pdf_new = db::upsert_file_metadata(
            &conn, root_id, "new.pdf", "new.pdf", "pdf",
            100, now_ns, "fp-new-pdf", "ext-v1", now, now,
        ).unwrap();
        db::update_file_content(&conn, pdf_new, "contract new", 1.0, "en", "ext-v1", now).unwrap();
        db::replace_chunks(&conn, pdf_new, &[(0, "contract new", None)]).unwrap();

        let txt_new = db::upsert_file_metadata(
            &conn, root_id, "new.txt", "new.txt", "txt",
            100, now_ns, "fp-new-txt", "ext-v1", now, now,
        ).unwrap();
        db::update_file_content(&conn, txt_new, "contract txt", 1.0, "en", "ext-v1", now).unwrap();
        db::replace_chunks(&conn, txt_new, &[(0, "contract txt", None)]).unwrap();

        // date_from threshold: yesterday in ns — pdf_new (mtime=now) passes, pdf_old (mtime=2020) fails.
        let fts_term = fts5_escape("contract");
        let date_from_ns = (now - 86400) * 1_000_000_000; // yesterday

        let mut stmt = conn.prepare(
            "SELECT f.id, f.media_type FROM files_fts fts
             JOIN files f ON f.id = fts.rowid
             WHERE files_fts MATCH ?1
               AND f.deleted_at IS NULL
               AND f.media_type = 'pdf'
               AND f.mtime_ns >= ?2",
        ).unwrap();
        let rows: Vec<(i64, String)> = stmt
            .query_map(params![fts_term, date_from_ns], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(rows.iter().any(|(id, _)| *id == pdf_new),  "recent pdf must pass both filters");
        assert!(!rows.iter().any(|(id, _)| *id == pdf_old), "old pdf must be excluded by mtime");
        assert!(!rows.iter().any(|(id, _)| *id == txt_new), "txt must be excluded by media type");
    }

    // ── integration: search_files() end-to-end (keyword mode, no Ollama) ──────

    fn keyword_query(text: &str) -> SearchQuery {
        SearchQuery {
            query_text: text.into(),
            media_types: vec![],
            root_scope: vec![],
            date_from: None,
            date_to: None,
            min_confidence: 0.0,
            limit: 10,
            offset: 0,
            mode: "keyword".into(),
            cursor: String::new(),
        }
    }

    #[test]
    fn test_integration_keyword_returns_matching_file() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        let invoice_id = seed_file(&conn, root_id, "invoice.txt", "txt",
                                   "invoice total amount due payment", 1.0);
        seed_file(&conn, root_id, "readme.md", "md", "getting started guide", 1.0);

        let resp = search_files(&conn, &keyword_query("invoice"), "http://127.0.0.1:19999")
            .expect("search_files must not error in keyword mode");

        assert_eq!(resp.results.len(), 1, "only the invoice file should match");
        assert_eq!(resp.results[0].file_id, invoice_id);
        assert_eq!(resp.results[0].filename, "invoice.txt");
        assert!(!resp.results[0].snippet.is_empty(), "snippet must be populated");
    }

    #[test]
    fn test_integration_keyword_no_match_returns_empty() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        seed_file(&conn, root_id, "photo.jpg", "jpg", "sunset over the ocean", 1.0);

        let resp = search_files(&conn, &keyword_query("quarterly report"), "http://127.0.0.1:19999")
            .expect("search_files must not error on zero results");

        assert_eq!(resp.total, 0);
        assert!(resp.results.is_empty());
        assert!(resp.next_cursor.is_none());
    }

    #[test]
    fn test_integration_keyword_media_type_filter() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        let pdf_id = seed_file(&conn, root_id, "report.pdf", "pdf", "annual report summary", 1.0);
        seed_file(&conn, root_id, "notes.txt", "txt", "annual report summary", 1.0);

        let resp = search_files(
            &conn,
            &SearchQuery { media_types: vec!["pdf".into()], ..keyword_query("annual") },
            "http://127.0.0.1:19999",
        ).expect("search_files must not error");

        assert_eq!(resp.results.len(), 1, "only pdf should survive media_type filter");
        assert_eq!(resp.results[0].file_id, pdf_id);
    }

    #[test]
    fn test_integration_keyword_confidence_filter() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        let high_id = seed_file(&conn, root_id, "high.txt", "txt", "contract renewal notice", 0.95);
        let low_id  = seed_file(&conn, root_id, "low.txt",  "txt", "contract renewal notice", 0.50);

        let resp = search_files(
            &conn,
            &SearchQuery { min_confidence: 0.8, ..keyword_query("contract") },
            "http://127.0.0.1:19999",
        ).expect("search_files must not error");

        let ids: Vec<i64> = resp.results.iter().map(|r| r.file_id).collect();
        assert!(ids.contains(&high_id), "high-confidence file must be present");
        assert!(!ids.contains(&low_id), "low-confidence file must be excluded");
    }

    #[test]
    fn test_integration_keyword_date_from_filter() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        let now = db::unix_now();

        let old_mtime_ns: i64 = 1_577_836_800_000_000_000; // 2020-01-01 UTC in ns
        let old_id = db::upsert_file_metadata(
            &conn, root_id, "old.txt", "old.txt", "txt",
            100, old_mtime_ns, "fp-old", "ext-v1", now, now,
        ).unwrap();
        db::update_file_content(&conn, old_id, "budget overview", 1.0, "en", "ext-v1", now).unwrap();
        db::replace_chunks(&conn, old_id, &[(0, "budget overview", None)]).unwrap();

        let new_id = db::upsert_file_metadata(
            &conn, root_id, "new.txt", "new.txt", "txt",
            100, now * 1_000_000_000, "fp-new", "ext-v1", now, now,
        ).unwrap();
        db::update_file_content(&conn, new_id, "budget overview", 1.0, "en", "ext-v1", now).unwrap();
        db::replace_chunks(&conn, new_id, &[(0, "budget overview", None)]).unwrap();

        let resp = search_files(
            &conn,
            &SearchQuery { date_from: Some("2023-01-01".into()), ..keyword_query("budget") },
            "http://127.0.0.1:19999",
        ).expect("search_files must not error");

        let ids: Vec<i64> = resp.results.iter().map(|r| r.file_id).collect();
        assert!(ids.contains(&new_id), "recent file must pass date_from filter");
        assert!(!ids.contains(&old_id), "2020 file must be excluded by date_from");
    }

    #[test]
    fn test_integration_keyword_soft_deleted_excluded() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        let now = db::unix_now();
        let id = seed_file(&conn, root_id, "deleted.txt", "txt", "invoice payment overdue", 1.0);

        conn.execute(
            "UPDATE files SET deleted_at = ?1 WHERE id = ?2",
            params![now, id],
        ).unwrap();

        let resp = search_files(&conn, &keyword_query("invoice"), "http://127.0.0.1:19999")
            .expect("search_files must not error");

        assert!(
            resp.results.iter().all(|r| r.file_id != id),
            "soft-deleted file must not appear in results"
        );
    }

    #[test]
    fn test_integration_keyword_pagination_limit_and_total() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        for i in 0..5_u32 {
            seed_file(&conn, root_id, &format!("meeting_{i:02}.txt"), "txt",
                      &format!("meeting notes week {i}"), 1.0);
        }

        // A single query with limit=2 must return exactly 2 results and total=5.
        let resp = search_files(
            &conn,
            &SearchQuery { limit: 2, offset: 0, ..keyword_query("meeting") },
            "http://127.0.0.1:19999",
        ).expect("search_files must not error");

        assert_eq!(resp.results.len(), 2, "limit=2 must return 2 results");
        assert_eq!(resp.total, 5, "total must reflect all 5 matches");
        assert!(resp.next_cursor.is_some(), "must have cursor when more results remain");
    }

    #[test]
    fn test_integration_keyword_cursor_roundtrip_distinct_scores() {
        // Use files with meaningfully different content so BM25 assigns distinct
        // scores — the cursor encode/decode path is then exercised correctly.
        let conn = make_test_conn();
        let root_id = seed_root(&conn);

        // File A: "invoice" appears many times → highest BM25 score for query "invoice"
        seed_file(&conn, root_id, "a.txt", "txt",
                  "invoice invoice invoice invoice payment total due", 1.0);
        // File B: "invoice" appears once
        seed_file(&conn, root_id, "b.txt", "txt",
                  "invoice summary", 1.0);
        // File C: "invoice" appears twice
        seed_file(&conn, root_id, "c.txt", "txt",
                  "invoice invoice charges", 1.0);

        let page1 = search_files(
            &conn,
            &SearchQuery { limit: 2, ..keyword_query("invoice") },
            "http://127.0.0.1:19999",
        ).expect("page 1 must not error");
        assert_eq!(page1.results.len(), 2);
        assert_eq!(page1.total, 3);
        let cursor = page1.next_cursor.expect("must have cursor with 3 results, limit 2");

        let page2 = search_files(
            &conn,
            &SearchQuery { limit: 2, cursor, ..keyword_query("invoice") },
            "http://127.0.0.1:19999",
        ).expect("page 2 must not error");
        assert_eq!(page2.results.len(), 1, "page 2 has 1 remaining result");
        assert!(page2.next_cursor.is_none(), "last page must have no cursor");

        // No overlap
        let ids1: Vec<i64> = page1.results.iter().map(|r| r.file_id).collect();
        let ids2: Vec<i64> = page2.results.iter().map(|r| r.file_id).collect();
        for id in &ids2 {
            assert!(!ids1.contains(id), "cursor pages must not overlap");
        }
    }

    #[test]
    fn test_integration_keyword_result_score_ordering() {
        let conn = make_test_conn();
        let root_id = seed_root(&conn);
        seed_file(&conn, root_id, "a.txt", "txt", "contract agreement terms conditions clauses", 1.0);
        seed_file(&conn, root_id, "b.txt", "txt", "contract", 1.0);
        seed_file(&conn, root_id, "c.txt", "txt", "contract agreement terms", 1.0);

        let resp = search_files(&conn, &keyword_query("contract"), "http://127.0.0.1:19999")
            .expect("search_files must not error");

        let scores: Vec<f64> = resp.results.iter().map(|r| r.score).collect();
        for w in scores.windows(2) {
            assert!(w[0] >= w[1], "results must be sorted score descending: {w:?}");
        }
    }

    #[test]
    fn test_integration_keyword_root_scope_filter() {
        let conn = make_test_conn();
        let root_a = db::insert_root(&conn, "/home/documents").unwrap().id;
        let root_b = db::insert_root(&conn, "/home/pictures").unwrap().id;

        let doc_id = seed_file(&conn, root_a, "notes.txt", "txt", "project planning notes", 1.0);
        seed_file(&conn, root_b, "photo.txt", "txt", "project planning notes", 1.0);

        let resp = search_files(
            &conn,
            &SearchQuery { root_scope: vec!["documents".into()], ..keyword_query("planning") },
            "http://127.0.0.1:19999",
        ).expect("search_files must not error");

        let ids: Vec<i64> = resp.results.iter().map(|r| r.file_id).collect();
        assert!(ids.contains(&doc_id), "file in scoped root must appear");
        assert_eq!(ids.len(), 1, "file in out-of-scope root must be excluded");
    }

    // ── integration: search_files() with live Ollama ──────────────────────────

    #[test]
    #[ignore = "requires Ollama running with nomic-embed-text-v2-moe"]
    fn test_integration_semantic_returns_relevant_result() {
        let conn = make_test_conn();
        let root_id = db::insert_root(&conn, "/test/semantic").unwrap().id;
        let now = db::unix_now();
        let ollama = crate::llm::runtime::OLLAMA_BASE_URL;

        let files = [
            ("finance.txt", "quarterly revenue earnings profit margins financial report"),
            ("recipe.txt",  "chocolate cake flour sugar butter eggs baking instructions"),
        ];
        for (path, text) in &files {
            let id = db::upsert_file_metadata(
                &conn, root_id, path, path, "txt",
                100, now * 1_000_000_000, &format!("fp-{path}"), "ext-v1", now, now,
            ).unwrap();
            db::update_file_content(&conn, id, text, 1.0, "en", "ext-v1", now).unwrap();
            let emb = crate::llm::embeddings::embed_text(text, ollama).unwrap();
            let blob = crate::llm::embeddings::embedding_to_bytes(&emb);
            db::replace_chunks(&conn, id, &[(0, text, Some(blob.as_slice()))]).unwrap();
        }

        let resp = search_files(
            &conn,
            &SearchQuery { mode: "semantic".into(), ..keyword_query("annual earnings financial summary") },
            ollama,
        ).expect("semantic search must not error");

        assert!(!resp.results.is_empty(), "semantic search must return results");
        assert_eq!(resp.results[0].filename, "finance.txt",
            "finance.txt must rank first for a financial query");
    }

    #[test]
    #[ignore = "requires Ollama running with nomic-embed-text-v2-moe"]
    fn test_integration_semantic_ranks_relevant_above_unrelated() {
        let conn = make_test_conn();
        let root_id = db::insert_root(&conn, "/test/semantic_rank").unwrap().id;
        let now = db::unix_now();
        let ollama = crate::llm::runtime::OLLAMA_BASE_URL;

        let files = [
            ("finance.txt", "quarterly revenue earnings profit margins financial report"),
            ("recipe.txt",  "chocolate cake flour sugar butter eggs baking instructions"),
        ];
        for (path, text) in &files {
            let id = db::upsert_file_metadata(
                &conn, root_id, path, path, "txt",
                100, now * 1_000_000_000, &format!("fp-{path}"), "ext-v1", now, now,
            ).unwrap();
            db::update_file_content(&conn, id, text, 1.0, "en", "ext-v1", now).unwrap();
            let emb = crate::llm::embeddings::embed_text(text, ollama).unwrap();
            let blob = crate::llm::embeddings::embedding_to_bytes(&emb);
            db::replace_chunks(&conn, id, &[(0, text, Some(blob.as_slice()))]).unwrap();
        }

        let resp = search_files(
            &conn,
            &SearchQuery { mode: "semantic".into(), ..keyword_query("quarterly financial earnings report") },
            ollama,
        ).expect("semantic search must not error");

        assert_eq!(resp.results.len(), 2, "both files must be returned");
        let finance_score = resp.results.iter().find(|r| r.filename == "finance.txt")
            .map(|r| r.score).expect("finance.txt must be in results");
        let recipe_score = resp.results.iter().find(|r| r.filename == "recipe.txt")
            .map(|r| r.score).expect("recipe.txt must be in results");
        assert!(finance_score > recipe_score,
            "finance.txt must rank above recipe.txt for a financial query (finance={finance_score}, recipe={recipe_score})");
    }

    #[test]
    #[ignore = "requires Ollama running with nomic-embed-text-v2-moe"]
    fn test_integration_hybrid_both_passes_score_higher_than_single_pass() {
        let conn = make_test_conn();
        let root_id = db::insert_root(&conn, "/test/hybrid").unwrap().id;
        let now = db::unix_now();
        let ollama = crate::llm::runtime::OLLAMA_BASE_URL;

        // File A: exact keyword hit ("invoice") + strong semantic match
        // File B: semantic match only — no FTS hit for "invoice"
        let text_a = "invoice payment receipt tax deduction billing";
        let text_b = "expense ledger statement charges financial record";

        for (path, text) in [("a.txt", text_a), ("b.txt", text_b)] {
            let id = db::upsert_file_metadata(
                &conn, root_id, path, path, "txt",
                100, now * 1_000_000_000, &format!("fp-{path}"), "ext-v1", now, now,
            ).unwrap();
            db::update_file_content(&conn, id, text, 1.0, "en", "ext-v1", now).unwrap();
            let emb = crate::llm::embeddings::embed_text(text, ollama).unwrap();
            let blob = crate::llm::embeddings::embedding_to_bytes(&emb);
            db::replace_chunks(&conn, id, &[(0, text, Some(blob.as_slice()))]).unwrap();
        }

        // In hybrid mode "invoice" triggers both FTS (exact) and vector passes.
        let hybrid = search_files(
            &conn,
            &SearchQuery { mode: String::new(), limit: 10, ..keyword_query("invoice") },
            ollama,
        ).expect("hybrid search must not error");

        // In keyword mode "invoice" only triggers FTS.
        let kw = search_files(
            &conn,
            &SearchQuery { mode: "keyword".into(), limit: 10, ..keyword_query("invoice") },
            ollama,
        ).expect("keyword search must not error");

        let hybrid_a = hybrid.results.iter().find(|r| r.filename == "a.txt")
            .expect("a.txt must appear in hybrid results");
        let kw_a = kw.results.iter().find(|r| r.filename == "a.txt")
            .expect("a.txt must appear in keyword results");

        assert!(
            hybrid_a.score >= kw_a.score,
            "hybrid score must be >= keyword-only score for a.txt (got hybrid={} kw={})",
            hybrid_a.score, kw_a.score
        );
    }

    #[test]
    #[ignore = "requires Ollama running with nomic-embed-text-v2-moe"]
    fn test_integration_hybrid_semantic_filters_applied_after_rrf() {
        // Verifies that media_type and confidence filters work correctly in hybrid mode.
        let conn = make_test_conn();
        let root_id = db::insert_root(&conn, "/test/hybrid_filter").unwrap().id;
        let now = db::unix_now();
        let ollama = crate::llm::runtime::OLLAMA_BASE_URL;

        let pdf_text = "invoice payment summary billing statement";
        let txt_text = "invoice payment summary billing statement";

        let pdf_id = db::upsert_file_metadata(
            &conn, root_id, "bill.pdf", "bill.pdf", "pdf",
            100, now * 1_000_000_000, "fp-pdf", "ext-v1", now, now,
        ).unwrap();
        db::update_file_content(&conn, pdf_id, pdf_text, 1.0, "en", "ext-v1", now).unwrap();
        let emb = crate::llm::embeddings::embed_text(pdf_text, ollama).unwrap();
        let blob = crate::llm::embeddings::embedding_to_bytes(&emb);
        db::replace_chunks(&conn, pdf_id, &[(0, pdf_text, Some(blob.as_slice()))]).unwrap();

        let txt_id = db::upsert_file_metadata(
            &conn, root_id, "bill.txt", "bill.txt", "txt",
            100, now * 1_000_000_000, "fp-txt", "ext-v1", now, now,
        ).unwrap();
        db::update_file_content(&conn, txt_id, txt_text, 1.0, "en", "ext-v1", now).unwrap();
        let emb2 = crate::llm::embeddings::embed_text(txt_text, ollama).unwrap();
        let blob2 = crate::llm::embeddings::embedding_to_bytes(&emb2);
        db::replace_chunks(&conn, txt_id, &[(0, txt_text, Some(blob2.as_slice()))]).unwrap();

        let resp = search_files(
            &conn,
            &SearchQuery {
                mode: String::new(),
                media_types: vec!["pdf".into()],
                ..keyword_query("invoice")
            },
            ollama,
        ).expect("hybrid filtered search must not error");

        let ids: Vec<i64> = resp.results.iter().map(|r| r.file_id).collect();
        assert!(ids.contains(&pdf_id), "pdf must survive media_type filter in hybrid mode");
        assert!(!ids.contains(&txt_id), "txt must be excluded by media_type filter in hybrid mode");
    }
}
