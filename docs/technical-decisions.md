# Technical Decisions

All architecture decisions locked for v1. Each section states the decision, the benchmark evidence behind it, and any known follow-ups.

## Locked Stack

| Area | Decision |
|---|---|
| Desktop shell | Tauri |
| Indexing | Hybrid-balanced (event-driven + reconciliation index) |
| Background runtime | Background-helper/service |
| Chunking | Moderate overlap — 256 token window, 32 token overlap |
| Ranking | RRF hybrid (default) — selectable: `hybrid` \| `keyword` \| `semantic` |
| Embedding dimensions | `embedding-64` |
| Embedding provider | Ollama local — `nomic-embed-text` for text; `qwen2.5vl:7b` (fallback: `llava:7b`) describes images → embed with `nomic-embed-text` |
| Spreadsheet parsing | Flat normalization |
| OCR | Confidence threshold `0.75` |
| Multilingual | Accent folding (FTS5 tokenizer) + language detection at index time (`whatlang`); cross-language retrieval via multilingual embeddings |
| Privacy UX | Explicit privacy controls (scope editor + audit log + delete index) |
| Duplicate presentation | Listed separately |

---

## 1. Desktop Shell — Tauri

Tauri is selected over Electron for v1.

- Lower RAM/CPU overhead for always-on/background indexing workflows.
- Smaller distribution footprint.
- Strong fit for lightweight cross-platform desktop delivery (macOS + Windows).
- Trade-off: more Rust-side integration work; tighter process supervision needed for the background helper.

Validation gate before locking in implementation: macOS and Windows build/package flow works end-to-end; Python worker supervision stable; file picker + tray/background behavior acceptable. If a critical blocker emerges, re-evaluate Electron.

---

## 2. Indexing — Hybrid-balanced

Event-driven file watching + periodic reconciliation index. Locked after benchmark against polling-only.

| Strategy | Mean freshness | p95 freshness | Missed changes | CPU units |
|---|---|---|---|---|
| Polling-only | `92ms` | `110ms` | `0` | `650` |
| Hybrid-balanced | `27.7ms` | `38ms` | `0` | `436` |

Detailed overhead:

| Strategy | Index passes | Watcher events | Files inspected | Bytes read |
|---|---|---|---|---|
| Polling-only | `9` | `0` | `135` | `15903` |
| Hybrid-balanced | `6` | `3` | `90` | `10598` |

Hybrid-balanced is 3× fresher and 33% cheaper in CPU overhead.

---

## 3. Background Runtime — Incremental indexing on startup (v1)

**v1 decision:** index runs inside the Tauri process, triggered on app open. No separate background service.

On startup, the app runs an incremental index (mtime + blake3 fingerprint detection) — only new or changed files are processed, so subsequent launches are fast. Search latency is unaffected; it always hits SQLite directly.

| Model | Freshness after close | Closed-window coverage | Complexity |
|---|---|---|---|
| Window-only | `270125ms` mean | `0%` | `2` |
| Tray-resident | `80ms` mean | `100%` | `5` |
| Background-helper | `60ms` mean | `100%` | `8` |

Window-only (index on open) is chosen for v1. The freshness gap is acceptable for personal use — users rarely add a file and immediately search for it before opening the app. Background-helper delivers the best freshness but adds permanent RAM overhead (20–50MB always resident), platform service registration, silent failure risk, and user-visible background processes — all friction that conflicts with the product's local-first, privacy-first values.

**Roadmap:** promote to background-helper/service if users report stale results as a real pain point. Implementation work required at that point:
- Service lifecycle and quit behavior
- IPC contract between window and service (recommend SQLite as shared state bus + polling)
- Reconnect behavior from the UI app
- Sleep/wake and startup revalidation
- Install/update strategy per platform

---

## 4. Chunking — Moderate overlap

| Strategy | Recall@1 | Token expansion |
|---|---|---|
| Compact | `0.50` | `1.24` |
| Moderate overlap | `1.00` | `1.16` |

Moderate overlap wins on recall and produces slightly lower token expansion. Both metrics improve together — no tradeoff.

**Locked parameters:**
- Window size: `256 tokens`
- Overlap: `32 tokens` (~12%)

Small enough to stay semantically focused, large enough to preserve context across boundaries. Overlap is conservative — just enough to prevent meaning loss at chunk edges. These are constants in `chunker.rs`, not user-configurable.

---

## 5. Ranking — Hybrid semantic + lexical (RRF)

| Strategy | MRR | Recall@1 |
|---|---|---|
| Semantic-only | `0.50` | `0.00` |
| Hybrid | `1.00` | `1.00` |

Exact and path-like queries fail completely with semantic-only. Hybrid is the clearest win in the benchmark.

**Default: Reciprocal Rank Fusion (RRF)**

```
score = 1/(k + rank_fts) + 1/(k + rank_vec)
```

`k = 60` (standard constant — dampens impact of top ranks, robust to outliers).

RRF is preferred over weighted-average blending because it requires no score normalization and degrades gracefully when one signal is absent (e.g. a file with no extractable text has no BM25 rank — RRF handles this without breaking).

**User-selectable search modes** (via `SearchRequest.mode`):

| Mode | Behaviour |
|---|---|
| `hybrid` | RRF blend of BM25 + cosine — **default** |
| `keyword` | BM25/FTS5 only — no vector query |
| `semantic` | Cosine similarity only — no FTS query |

Mode is passed in the search command and respected in `search.rs`. The query parser defaults to `hybrid` unless the user explicitly filters (e.g. UI toggle).

---

## 6. Embedding Dimensions — `embedding-64`

Benchmarked across a synthetic corpus (6 docs, 3 queries) and a real PDF corpus (16 docs, 319 queries).

**Synthetic corpus:**

| Strategy | NDCG@3 | Avg latency | Index size |
|---|---|---|---|
| `embedding-8` | `0.877` | `10.72ms` | `192` bytes |
| `embedding-64` | `1.000` | `16.08ms` | `1536` bytes |
| `embedding-256` | `1.000` | `22.20ms` | `6144` bytes |

**Real PDF corpus:**

| Strategy | NDCG@3 | Recall@3 | Avg latency | Index size |
|---|---|---|---|---|
| `embedding-8` | `0.242` | `0.339` | `40.2ms` | `512` bytes |
| `embedding-64` | `0.297` | `0.386` | `60.2ms` | `4096` bytes |
| `embedding-256` | `0.323` | `0.386` | `80.0ms` | `16384` bytes |

64-dim and 256-dim have identical recall@3 on the real corpus. 256-dim adds only 0.026 ndcg@3 at 4× the index size. `embedding-64` is the right default for v1.

---

## 7. Embedding Provider — Ollama local

Benchmarked three providers across 17 documents (13 text/spreadsheet + 4 image), 51 queries, including cross-modal retrieval.

| Strategy | Model | ndcg@3 | recall@3 | mrr | External transfer | Cost |
|---|---|---|---|---|---|---|
| `nomic-local` | nomic-embed-text-v1.5 + nomic-embed-vision-v1.5 | `0.596` | `0.647` | `0.617` | `0` bytes | `$0` |
| `gemini-cloud` | gemini-embedding-2-preview | `0.736` | `0.804` | `0.745` | all content | `$0.20/M tokens` |
| `infinity-local` | bge-visualized-m3 | — | — | — | `0` bytes | `$0` |

Notes:
- Infinity/BGE-Visualized-M3 was skipped — blocked by `optimum.bettertransformer` removal in Python 3.13 / Transformers 4.46+.
- Gemini does not support WebP natively; requires JPEG conversion.
- Gemini leads by ~14 points ndcg@3. Gap accepted for v1 local-first product.

**Decision: Ollama local stack.**
- `nomic-embed-text` via Ollama for text/document embeddings.
- Vision generation model describes images → description embedded with `nomic-embed-text`. One Ollama process handles both; no Python microservice, no ONNX runtime.
- **Default vision model: `qwen2.5vl:7b`** — outperforms llava on text-heavy images (receipts, forms, scanned documents), which are the dominant personal file use case. Fallback: `llava:7b` for machines that cannot load qwen (~5GB vs ~4GB).
- Trade-off accepted: native visual embeddings capture layout/spatial features that a text description cannot fully express. For document images OCR+description recovers most retrieval quality. Acceptable for v1.
- Revisit path if image retrieval quality becomes a complaint: ONNX-direct (nomic-embed-vision-v1.5 weights, no API key, no Python) before considering cloud.

---

## 8. Spreadsheet Parsing — Flat normalization

| Strategy | Precision@1 | NDCG@3 | Index size |
|---|---|---|---|
| Flat | `0.667` | `0.833` | `72` bytes |
| Sheet-aware | `1.000` | `1.000` | `208` bytes |

Sheet-aware wins on the benchmark, but the app only needs document-level context for v1. Sheet hints add index size without benefit at this retrieval granularity.

**Follow-up:** revisit in next phase with real user queries. If sheet-level queries become common, sheet-aware is the clear upgrade path.

---

## 9. OCR — Confidence threshold `0.75`

| Strategy | Precision@1 | NDCG@3 | Index size |
|---|---|---|---|
| Threshold `0.2` | `0.50` | `0.815` | `152` bytes |
| Threshold `0.75` | `1.00` | `1.00` | `96` bytes |

Stricter threshold removes noisy OCR text and improves both precision and index footprint. Could hide faint true positives on harder corpora — acceptable for v1.

---

## 10. Multilingual — Accent folding + language detection

**Accent/diacritic variation** (`relatório` → `relatorio`) is handled natively by the FTS5 tokenizer (`unicode61 remove_diacritics 2`). No extra code needed.

**Cross-language term matching** (Portuguese query finding English files) is handled by the semantic path — `nomic-embed-text` is a multilingual model trained on multilingual data. A Portuguese query and a semantically equivalent English document land close together in vector space. No translation dictionary needed.

The original benchmark showing a large gap for locale-aware normalization was FTS-only. In a hybrid search with a multilingual embedding model, the semantic path recovers most of that gap without a dictionary.

**Language detection at index time (`whatlang` crate):**
- Detect language of `extracted_text` when a file is indexed
- Store as `lang_hint TEXT` on the `files` table (e.g. `'pt'`, `'en'`, `'unknown'`)
- In v1: informational only — not used to alter RRF weights or query routing
- Hybrid blend weights stay fixed regardless of language match

**Roadmap:** use `lang_hint` to weight FTS vs semantic signals when cross-language queries become a measured pain point. The field is in the schema; the logic is not yet implemented.

**Translation dictionary: dropped.** Superseded by the multilingual embedding model on the semantic path and the FTS tokenizer on the keyword path.

---

## 11. Privacy UX — Explicit privacy controls

Scope editor + audit log + delete index. Benchmarked against a baseline with no controls.

| Setup | Coverage | Trust lift | Support incidents |
|---|---|---|---|
| Baseline | `0%` | `+0.125` | `50%` |
| Explicit controls | `100%` | `+0.500` | `0%` |

Controls materially improve the experience — not compliance theater.

**UX constraint:** each control must show its current status at a glance (always visible), but details and configuration are collapsed by default and only expanded on user interaction. Informed confidence without overwhelming.

---

## 12. Duplicate Presentation — Listed separately

Functional requirement locked. Duplicates are shown as separate results in v1. No grouped-presentation variant for MVP.

---

## 13. Query Parser — LLM Fallback Threshold

`queryParser.ts` runs a deterministic pass first, extracting structured intent from obvious patterns (`"pdf"`, `"from 2024"`, `"in Documents"`). An optional LLM pass via Ollama handles ambiguous free-form queries.

**Fallback rule (locked):** invoke the LLM only when both conditions are true:
- Query is **longer than 5 words**
- More than **3 tokens remain unrecognized** after the deterministic pass

Short queries and queries where most tokens were consumed deterministically never hit the LLM. Only long, ambiguous, free-form sentences do.

**Why explicit thresholds over a fuzzy confidence score:**
- Deterministic, testable — no probabilistic behaviour to debug
- Easy to tune if the threshold proves too aggressive or too conservative
- Adds ~200–800ms + requires Ollama available — the cost justifies a high bar

LLM is never called in `mode: 'keyword'` — user explicitly wants lexical matching, no inference.

---

## 14. macOS 26 Startup Stability — Tao/Wry Patch Set

On macOS 26 (Tahoe), app startup crashed during `applicationDidFinishLaunching` with:

- `panic in a function that cannot unwind`
- `fatal runtime error: Rust cannot catch foreign exceptions, aborting`

Crash site was consistently in Tao app delegate launch callback.

**Locked workaround for current pinned versions (`tao 0.34.8`, `wry 0.54.4`):**

- Enable `app.macOSPrivateApi = true` in Tauri config when using transparent windows.
- Enable Tauri Rust feature `macos-private-api`.
- In patched Tao, use `extern "C-unwind"` for `applicationDidFinishLaunching` callback registration/signature.
- Enable objc2 `relax-sign-encoding` for patched Tao and Wry Apple targets.

These changes are constrained to local patched crates under `desktop/src-tauri/patches/` and Tauri app config.

**Why this is acceptable for v1:**

- Resolves launch abort on supported dev hardware/OS.
- Keeps transparent + vibrancy UI behavior intact.
- Isolated and reversible once upstream releases include equivalent fixes.

**Removal condition:**

Drop local patch overrides after upgrading to upstream Tao/Wry versions that boot cleanly on macOS 26 without these modifications.
