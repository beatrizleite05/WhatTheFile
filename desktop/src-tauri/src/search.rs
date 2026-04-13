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
        "SELECT f.id, f.root_id, r.path, f.rel_path, f.filename, f.media_type,
                f.size_bytes, f.indexed_at, f.confidence, f.mtime_ns, f.extracted_text
         FROM files f
         JOIN roots r ON r.id = f.root_id
         WHERE f.id IN ({placeholders})
           AND f.deleted_at IS NULL
           AND r.active = 1"
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
                    r.get::<_, String>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, f32>(8)?,
                    r.get::<_, i64>(9)?,
                    r.get::<_, String>(10)?,
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
            let (file_id, root_id, root_path, rel_path, filename, media_type,
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

            let abs_path = std::path::Path::new(&root_path)
                .join(&rel_path)
                .to_string_lossy()
                .into_owned();

            out.push(FileSearchResult {
                file_id,
                root_id,
                path: abs_path,
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
#[path = "search_test.rs"]
mod tests;
