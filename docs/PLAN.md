# WhatTheFile — Implementation Plan

## Goal

WhatTheFile is a local-first desktop app for finding files by meaning — using natural language and semantic search that understands the context of your files, not just their names. It indexes content from folders the user explicitly configures (at onboarding or via settings) across all supported file types (PDF, DOCX, XLSX, CSV, TXT, MD, PNG, JPG/JPEG) using local AI via Ollama. All processing happens on-device — nothing leaves the machine. Available on macOS and Windows.

---

## Locked Requirements

- **File types for v1:** PDF, DOCX, XLSX, CSV, TXT, MD, PNG, JPG/JPEG
- **Extensibility:** architecture must support adding audio/video later without schema breakage
- **OCR priority:** `pt-BR` and `en` first; keep pipeline multilingual-ready
- **Privacy:** strictly local inference/indexing — Ollama only, no cloud fallback in v1
- **Packaging:** macOS (DMG) + Windows (MSI)
- **Folder scope:** user-defined at onboarding or via settings — no implicit indexing
- **Background indexing:** incremental index runs on app open (mtime + fingerprint detection); background service is post-v1 roadmap
- **Duplicate results:** duplicates are listed separately in search results — no grouped presentation
- **Operational robustness:** avoid stale Ollama models lingering in memory
- **Query UX:** must stay responsive even for very broad searches (thousands of matches)

---

## Stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri (macOS + Windows) |
| Frontend | React + TypeScript |
| Backend | Rust (Tauri backend) — extraction, indexing, embedding, retrieval |
| Database | SQLite + `sqlite-vec` — metadata, FTS5, and vector tables in a single file |
| Text embeddings | Ollama `nomic-embed-text-v2-moe` (768-dim) |
| Image understanding | Ollama `qwen2.5vl:7b` → text description → `nomic-embed-text` |

No Python dependency. All AI inference goes through Ollama.

---

## Directory Layout

```
desktop/
  src/                        React + TypeScript frontend
    core/                     Pure-logic TS modules (query parsing, ranking, chunking, etc.)
    hooks/                    Feature-specific state hooks
    components/               UI components
    utils.ts                  Shared frontend utilities
  src-tauri/src/              Rust backend
    indexer.rs                Two-phase incremental indexing, move detection
    extractor.rs              Text extraction (PDF, DOCX, XLSX, CSV, TXT, MD)
    chunker.rs                Text splitting — mirrors chunker.ts logic
    embeddings.rs             Ollama nomic-embed-text adapter
    vision.rs                 Image → vision description → embed pipeline
    search.rs                 FTS5/BM25 + sqlite-vec cosine retrieval, RRF score blending
    db.rs                     SQLite schema, migrations, upsert/move/delete
    config.rs                 User config and indexed roots
    runtime.rs                Ollama lifecycle management
    lib.rs                    Tauri command layer only — no business logic
    platform/                 All OS-specific code (paths, clipboard, GPU)

shared/                       Reserved — shared schemas and contracts
```

**Runtime user data (outside repo):**
- `~/Library/Application Support/WhatTheFile/db/index.sqlite` (macOS)
- `%LOCALAPPDATA%\WhatTheFile\db\index.sqlite` (Windows)
- `cache/extracted/<index-id>/` — normalized extraction artifacts
- `cache/index-jobs/` — index job manifests, fingerprints, checkpoints
- `logs/activity.log` — audit/activity trail

---

## Core Design

### 1. Read-Only Scanning

- Never write to target roots.
- Track each source root by stable ID.
- Incremental detection:
  - Fast pass: directory mtimes, file size, file mtime
  - Fingerprint fallback: blake3 hashing for content-change detection
- Rename/move detection: if fingerprint unchanged but path changed, update index mapping without re-extracting or re-embedding.
- Interrupted index jobs restart from scratch (per-file checkpointing is post-MVP).

### 2. Extraction Pipeline

- Per-file extractor handles each supported type independently: PDF, DOCX, XLSX, CSV, TXT, MD → raw text; PNG, JPG/JPEG → vision model description → text.
- Images: `qwen2.5vl:7b` (fallback: `llava:7b`) generates a text description → fed into the same text embedding pipeline as documents.
- OCR confidence threshold: `0.75` — removes noisy text and reduces index size.
- Spreadsheets: flat normalization — file-level retrieval (sheet-aware is post-MVP).
- Chunking: 256-token windows with 32-token overlap — deterministic pure function.
- Strict single-responsibility: extractor never chunks; chunker never calls extractor or embeddings.
- Skip unchanged files using fingerprint + model-version cache keys.

### 3. Indexing and Retrieval

- Persist per file: path, media type, timestamps, size, blake3 fingerprint, confidence, extracted text, structured metadata.
- Two query paths:
  - **Keyword/FTS:** FTS5/BM25
  - **Semantic:** vector similarity via `nomic-embed-text-v2-moe` (768-dim embeddings)
- Ranking: RRF blend (`1/(60+rank_fts) + 1/(60+rank_vec)`) in Rust `search.rs`; `ranking.ts` only re-sorts for display.
- Single global index with `root_id` partitioning — cross-root search by default, per-root filtering available.
- Results paged/cursor-based — no unbounded queries.
- Progressive hydration: metadata and snippets first, thumbnails lazy-loaded.
- Multilingual: accent folding + token translation dictionary (pt-BR and en priority).

### 3.1 Natural Language Query Parsing

Parse user input into structured intent (`SearchRequest`):

| Field | Description |
|---|---|
| `queryText` | Free text for FTS/vector |
| `mediaTypes[]` | File type filter |
| `dateFrom` / `dateTo` | Date range |
| `minConfidence` | Confidence threshold filter |
| `rootScope[]` | Limit to specific indexed folders |

- Deterministic parser for obvious patterns (`from 2024`, `pdf`, `in Documents`) — lives in `queryParser.ts`.
- LLM fallback only for queries >5 words with >3 unrecognized tokens, never in `keyword` mode.
- 3-tier JSON fallback for LLM responses: `JSON.parse` → brace-balanced extraction → regex field salvage.

### 4. Desktop UX

- Single global search box — natural language and exact filename/path queries in the same input.
- Mandatory onboarding before first use: folder selection (quick presets + custom picker), privacy notice, local behavior explained.
- "Skip for now" available with a clear warning that search quality depends on indexing setup.
- Result grid: thumbnail tiles with file path and context snippet, virtualized/paginated — no unbounded rendering.
- Results are file-level — chunk hits are collapsed before display.
- Clicking a result opens the file; space key opens full preview modal.
- Indexing progress visible when a job is running; pause, stop, and reindex controls work.
- Privacy/trust controls always visible: scope editor, delete all indexed data, activity/audit log.
- Limited-mode warning when Ollama is unavailable.

### 4.1 UI/UX Principles

- **Keyboard-first:** 100% of workflows executable without a mouse.
- **Semantic transparency:** UI visually confirms AI parsing — recognized parameters (dates, file types, scopes) appear as colored text pills inside the search bar.
- **Latency masking:** fluid motion and instant micro-interactions mask processing time.
- **Layout:** 720–800px window width; two-pane split (60% file list / 40% preview pane); strict 8-point grid.
- **Visual language:** OS-level background blur (vibrancy); native system fonts (SF Pro / Segoe UI); hierarchy via weight and contrast only.
- **Results:** rich 1–2 line context snippets; top result pre-selected; monoline file-type icons.
- **Motion:** spring physics (high stiffness, low damping) for window and list animations.
- **Empty states:** 3–4 clickable prompt suggestions based on recent local file activity.
- **Zero-result fallback:** actionable pivot suggestion, not a generic error.

### 4.2 Query Responsiveness Safeguards

- Virtualized/paginated result grid — no unbounded rendering.
- API returns capped pages (`limit/offset` or cursor) plus total-count estimate.
- Queries run asynchronously with cancellation and debounce on new keystrokes.
- Staged query fallback: fast FTS/metadata first → semantic rerank on top-K candidates only.

### 4.3 Ollama Lifecycle

- All inference calls use bounded keep-alive defaults.
- On index completion or idle timeout, explicitly unload model from Ollama.
- Single model lease — only one heavy model stays loaded at a time.
- On startup, detect and release stale Ollama models from a previous crash.

---

## IPC Contract (Tauri)

- Frontend calls Rust via `invoke()` for commands; push updates arrive as events (no polling).
- All timestamps are unix seconds. Wire format is `camelCase` — Rust uses `#[serde(rename_all = "camelCase")]`.
- Errors surface as human-readable strings.

**Key commands:** `search`, `start_indexing`, `add_root`, `open_file`, `get_runtime_status`

**Key events:** `indexing://progress`, `indexing://completed`, `index://changed` (triggers re-query on frontend)

---

## Implementation Phases

### Phase A — Project Skeleton

**Goal:** Establish the full repository structure with working build toolchains on both platforms before any feature work.

- Scaffold `desktop/` with Tauri + React + TypeScript, Vite, and `vitest`.
- Scaffold `desktop/src-tauri/` with all planned Rust module stubs (`indexer.rs`, `extractor.rs`, etc.) — each compiles but contains only `todo!()` bodies.
- Wire `lib.rs` command stubs: `search`, `start_indexing`, `add_root`, `open_file`, `get_runtime_status`.
- Configure `tsconfig.json` in strict mode; configure `clippy` and `rustfmt`.
- Verify `npm test` (vitest), `cargo test`, and `npx tsc --noEmit` all pass with empty suites.
- Set up `shared/` directory for future cross-layer schemas.

**Exit criteria:** `npm run tauri dev` launches a blank window on macOS and Windows. CI passes clean builds on both targets.

---

### Phase B — Indexer, Manifests, and Incremental Detection

**Goal:** Build the scanning and change-detection engine — the foundation everything else writes to.

- Implement `config.rs`: persist user-configured root folders by stable ID; load/save on startup.
- Implement `indexer.rs` two-phase scan:
  - **Fast pass:** walk directory tree, compare file mtime + size against index state.
  - **Fingerprint pass:** blake3 hash files that passed the fast-pass filter.
  - **Move detection:** if fingerprint matches an existing record but path differs, update the path without re-extracting.
- Implement `db.rs` base schema: `files` table (path, root\_id, mtime, size, fingerprint, media\_type, confidence, extracted\_text) + schema version table.
- Implement `indexPlanner.ts` (`src/core/`): pure TS logic that decides which files need indexing given current index state vs. a filesystem scan result.
- Implement `policy.ts` (`src/core/`): allow/deny decisions (extensions, size cap, hidden files, glob exclusions).
- Implement `eventCoalescer.ts` (`src/core/`): deduplicate and batch filesystem watch events.
- Index job manifests: write a JSON checkpoint file per job to `cache/index-jobs/`; used to resume after crash (full restart for MVP).
- Expose `add_root` and `start_indexing` Tauri commands; emit `indexing://progress` and `indexing://completed` events.

**Exit criteria:** App can index a folder of inert `.txt` files, persist metadata to SQLite, skip unchanged files on re-run, and detect a renamed file without re-processing.

---

### Phase C — Extraction and OCR

**Goal:** Extract searchable text from every supported file type.

- Implement `extractor.rs` per-type extractors:
  - **PDF:** `pdf-extract` or `lopdf` — text layer first; fall through to OCR if text layer is absent or below confidence threshold.
  - **DOCX:** `docx-rs` — extract paragraph runs.
  - **XLSX / CSV:** `calamine` — flatten all cells to `key: value` lines (file-level, sheet-aware is post-MVP).
  - **TXT / MD:** read as-is; strip markdown syntax for embedding, preserve for display.
- Implement OCR pipeline: `leptess` (Tesseract bindings) with `pt-BR` + `en` language packs; skip chunks below confidence `0.75`.
- Implement `vision.rs`: call Ollama `qwen2.5vl:7b` (fallback: `llava:7b`) with the image; receive text description; feed into text embedding pipeline.
- Implement `chunker.rs` (mirrors `chunker.ts`): 256-token windows with 32-token overlap; pure function with no I/O.
- Implement `chunker.ts` (`src/core/`): identical windowing logic in TypeScript; used for frontend display and tests.
- Skip re-extraction when fingerprint + model version are unchanged (cache key stored in `db`).

**Exit criteria:** All v1 file types extract non-empty text. Image files produce a text description. Unchanged files are skipped on re-index. Confidence-filtered chunks are not stored.

---

### Phase D — Database Schema, Embeddings, and Retrieval

**Goal:** Build the full search stack — embeddings, FTS5, vector similarity, and the query API.

- Finalize `db.rs` schema:
  - `chunks` table: `file_id`, `chunk_index`, `text`, `embedding` (blob), `token_count`.
  - FTS5 virtual table over `chunks.text`.
  - `sqlite-vec` virtual table over `chunks.embedding`.
  - Migration system (version table + sequential migration files).
- Implement `embeddings.rs`: call Ollama `nomic-embed-text`; return `Vec<f32>` (64-dim); batched where possible.
- Implement `search.rs`:
  - BM25 pass via FTS5 `MATCH` — retrieve top-K candidates.
  - Vector pass via `sqlite-vec` cosine similarity — retrieve top-K candidates.
  - RRF score blend: `1/(60+rank_fts) + 1/(60+rank_vec)`.
  - Collapse chunk hits to file-level results before returning.
  - Cursor-based pagination; enforce result cap.
- Implement `queryParser.ts` (`src/core/`): parse natural language into `SearchRequest`; deterministic core; LLM fallback for complex queries only.
- Implement `ranking.ts` (`src/core/`): display-layer re-sort only (does not blend scores — that's Rust's job).
- Implement `pagination.ts` (`src/core/`): normalize limit/offset, enforce safety caps.
- Expose `search` Tauri command; emit `index://changed` on any write.
- Implement `runtime.rs`: Ollama lifecycle — startup check, model keep-alive bounds, explicit unload after idle or index completion.

**Exit criteria:** Full-text and semantic queries return ranked, file-level, paginated results. RRF blending is in Rust. `queryParser.ts` parses dates, file types, and scope filters correctly. Ollama models are unloaded after index completion.

---

### Phase E — Desktop UI

**Goal:** Build the full user-facing interface: onboarding, search, result grid, and preview pane.

- **Onboarding flow:** mandatory on first launch — folder picker with quick presets, privacy notice, local-vs-cloud explanation. "Skip for now" shows a persistent warning.
- **Search bar:** single input for natural language and exact queries; dynamic query pills for recognized parameters (dates, file types, scope) using `queryParser.ts`.
- **Result grid:**
  - Virtualized tile list (e.g., `react-virtual`) — no unbounded rendering.
  - Each tile: monoline file-type icon, file name, path, 1–2 line context snippet.
  - Top result pre-selected on load; `Enter` opens it.
  - Thumbnails lazy-loaded as tiles enter viewport.
- **Preview pane:** 60/40 split; updates instantly as user arrows through results; space key opens full modal preview.
- **Indexing progress:** progress bar/status while a job runs; pause, stop, and reindex-folder controls.
- **Settings:** scope editor (add/remove indexed folders), delete all indexed data, activity/audit log viewer.
- **Ollama offline banner:** limited-mode warning when Ollama is unreachable.
- **Empty states:** 3–4 clickable recent-activity prompt suggestions when search bar is empty; actionable pivot when query returns zero results.
- Implement `hooks/` for: `useSearch`, `useIndexing`, `useSettings`, `useOllamaStatus`.
- Strict 8-point grid; OS vibrancy background; spring-physics animations.
- 100% keyboard-navigable.

**Exit criteria:** Full end-to-end flow works — onboard, index a folder, search, open a file. All acceptance criteria from the list below pass. Keyboard-only navigation works throughout.

---

### Phase F — Packaging and Distribution

**Goal:** Produce reproducible, signed, installable builds for macOS (DMG) and Windows (MSI).

- Configure Tauri `tauri.conf.json` for both targets: app icon, bundle ID, version, entitlements.
- **macOS:** configure code signing (Apple Developer certificate) and notarization via `tauri-action`; produce DMG.
- **Windows:** configure NSIS or WiX installer; code signing via Windows certificate; produce MSI.
- Bundle Ollama installation check/prompt at first launch: if Ollama is not installed, surface a one-click install link.
- Write reproducible build runbook (`docs/BUILD.md`): environment setup, signing prerequisites, build commands, artifact locations.
- Verify builds on clean macOS and Windows VMs — no dev-environment dependencies leak into the bundle.
- Add GitHub Actions workflow: build both targets on tagged releases.

**Exit criteria:** A clean-machine install of the DMG and MSI completes successfully. App launches, passes onboarding, indexes a test folder, and returns search results. Build runbook is complete and verified.

---

## Acceptance Criteria

- Onboarding completes before first use — folder selection, privacy notice, and local behavior explained.
- Can index one or more user-configured folders and resume without re-indexing unchanged files.
- Detects rename/move without re-extracting or re-embedding when fingerprint is unchanged.
- Natural language query returns relevant files with context-aware, confidence-weighted ranking.
- Semantic search understands intent and synonyms — exact keyword match is not required.
- Results are file-level — chunk hits are collapsed before display.
- Duplicates appear as separate entries in results.
- Result tiles include file path and context snippet; clicking opens the file.
- All supported file types (PDF, DOCX, XLSX, CSV, TXT, MD, PNG, JPG/JPEG) are indexed and retrievable.
- Image search works via vision description pipeline.
- Multilingual queries work for pt-BR and en via accent folding and translation dictionary.
- Indexing progress is visible when a job is running; pause, stop, and reindex controls work.
- Limited-mode warning appears when Ollama is unavailable.
- Query UI stays responsive under very broad searches (thousands of matches).
- Ollama model memory is released automatically after index completion or idle timeout.
- Privacy controls (scope editor, audit log, delete index) are always visible and functional.
- All writes happen only under app-owned data directories — source folders are never touched.
- macOS (DMG) and Windows (MSI) builds are reproducible and documented.

---

## Decision Log

| # | Decision | Choice |
|---|---|---|
| 1 | Desktop shell | Tauri |
| 2 | Indexing strategy | Hybrid — fast metadata pass + fingerprint fallback |
| 3 | Background runtime | Index on app open (incremental); background service is post-v1 |
| 4 | Chunking | 256-token window, 32-token overlap |
| 5 | Ranking | RRF in Rust (`search.rs`); display-layer sort only in `ranking.ts` |
| 6 | Embedding dimensions | 768-dim (`nomic-embed-text-v2-moe`) — v2-moe outputs 768-dim by architecture; not configurable. Storage ~3KB/chunk; acceptable at desktop scale. |
| 7 | Embedding provider | Ollama — `nomic-embed-text-v2-moe` for text; `qwen2.5vl:7b` / `llava:7b` for images |
| 8 | Spreadsheet parsing | Flat normalization (file-level); sheet-aware is post-MVP |
| 9 | OCR confidence threshold | `0.75` |
| 10 | Multilingual | Accent folding + pt-BR/en translation dictionary |
| 11 | Privacy UX | Explicit controls — scope editor, audit log, delete index |
| 12 | Duplicate presentation | Listed separately |
