# Architecture

## System Diagram

```
                 ┌──────────────────────────────┐
                 │            UI App            │
                 │  search box / grid / preview │
                 └──────────────┬───────────────┘
                                │
                                ▼
                 ┌──────────────────────────────┐
                 │         Orchestrator         │
                 │ index start/stop/pause/retry │
                 └───────┬───────────┬──────────┘
                         │           │
             indexing path           query path
                         │           │
                         ▼           ▼
        ┌────────────────────────┐  ┌────────────────────────┐
        │       File Indexer     │  │      Query Parser      │
        │ watch + discover files │  │ NL -> structured query │
        └───────────┬────────────┘  └───────────┬────────────┘
                    │                            │
                    ▼                            ▼
        ┌────────────────────────┐   ┌────────────────────────┐
        │         Policy         │   │   Retrieval Engine     │
        │ allow/deny file index  │   │ FTS + vector candidate │
        └───────────┬────────────┘   └───────────┬────────────┘
                    │                            │
              ┌─────┴─────┐                      ▼
              │           │          ┌────────────────────────┐
              ▼           ▼          │        Ranking         │
  ┌──────────────┐  ┌──────────────┐ │ BM25 + cosine blending │
  │  Extractors  │  │   vision.rs  │ └───────────┬────────────┘
  │ text + docs  │  │ img describe │             │
  └──────┬───────┘  └──────┬───────┘             ▼
         │                 │          ┌────────────────────────┐
         ▼                 │          │      Pagination        │
  ┌──────────────┐         │          │   safe limit/offset    │
  │   Chunker    │         │          └───────────┬────────────┘
  │ split text   │         │                      │
  └──────┬───────┘         │                      ▼
         │                 │          ┌────────────────────────┐
         └────────┬────────┘          │       UI Results       │
                  │                   │     list + preview     │
                  ▼                   └────────────────────────┘
        ┌────────────────────────┐
        │   Embeddings Adapter   │
        │  Ollama nomic-embed    │
        └───────────┬────────────┘
                    │
                    ▼
        ┌──────────────────────────────────────────────────────┐
        │                    Local Index                       │
        │         SQLite metadata + FTS5 + vector table        │
        └──────────────────────────────────────────────────────┘
```

---

## Rust Modules (`desktop/src-tauri/src/`)

| Module | Responsibility | Must not |
|---|---|---|
| `indexer.rs` | Two-phase incremental indexing, blake3 fingerprinting, move detection, cooperative cancellation | Write to source directories |
| `extractor.rs` | Text extraction from PDF, DOCX, XLSX, CSV, TXT, MD — returns raw text per file | Chunk or embed; single responsibility |
| `chunker.rs` | Split raw text into overlapping windows — deterministic, no side effects | Call extractor or embeddings; pure function |
| `embeddings.rs` | Generate embeddings via Ollama for text chunks and image descriptions. Applies `search_query:` / `search_document:` prefixes per the per-model `EmbeddingModel` config. L2-normalises output unconditionally | Store to DB directly; assume nomic-specific behaviour |
| `bin/eval.rs` | Native eval binary — runs the curated corpus through `search_files()` and emits per-query JSON for the TS metrics consumer | Be linked into the Tauri shell or any production code path |
| `vision.rs` | Decode image → Ollama vision description → text for embedding pipeline | Store or embed directly |
| `search.rs` | FTS + vector retrieval, BM25 + cosine score blending, top-K result assembly | Write to DB |
| `db.rs` | Schema, migrations, upsert/move/delete, index job checkpointing, raw reads | Contain ranking or retrieval logic |
| `config.rs` | AppPaths resolution, data directory creation | — |
| `lib.rs` | Tauri commands, index worker spawning, setup/download flow, cancellation | Contain business logic — thin command layer only |
| `runtime.rs` | Ollama health/status checks | — |
| `platform/` | OS abstractions — paths, clipboard, GPU detection | Be imported outside `lib.rs` and `config.rs` |

## Frontend Modules (`desktop/src/`)

| Module | Responsibility | Must not |
|---|---|---|
| `queryParser.ts` | Parse natural language into structured intent (queryText, mediaTypes, date range, root scope, minConfidence) | Call LLM directly — deterministic core only |
| `ranking.ts` | Display-layer sorting/filtering of already-ranked results from Rust | Perform score blending — that lives in `search.rs` |
| `pagination.ts` | Normalize limit/offset, apply safety caps for UI responsiveness | Execute queries |
| `metrics.ts` | Offline evaluation utilities (Recall@K, MRR, NDCG). Consumes per-query JSON emitted by `src-tauri/src/bin/eval.rs` and produces `test/fixtures/eval-results/<sha>.json` | Touch the runtime query path |

---

## IPC Contract

Commands (`invoke()`) for actions and reads. Events (`emit/listen`) for push updates — no polling.

**Commands**

| Command | Params | Returns |
|---|---|---|
| `get_app_paths` | — | `{ db, cache, logs }` |
| `get_runtime_status` | — | `{ ollama_available, models_loaded, active_model }` |
| `get_setup_status` | — | `{ onboarding_complete, roots_configured }` |
| `list_roots` | — | `Root[]` |
| `add_root` | `path` | `Root` |
| `remove_root` | `root_id` | — |
| `start_indexing` | `root_id?` | `IndexJob` |
| `pause_indexing` | `job_id` | — |
| `cancel_indexing` | `job_id` | — |
| `search` | `SearchRequest` | `SearchResponse` |
| `open_file` | `path` | — |
| `get_file_thumbnail` | `path` | `base64 \| null` |
| `get_activity_log` | `limit, offset` | `{ entries, total }` |
| `delete_index` | `root_id?` | — |

`start_indexing(null)` indexes all roots. `delete_index(null)` wipes everything.

**SearchRequest**
```typescript
{
  queryText:      string
  mode:           'hybrid' | 'keyword' | 'semantic'  // default: 'hybrid'
  mediaTypes?:    string[]
  dateFrom?:      number   // unix seconds
  dateTo?:        number
  rootScope?:     number[]
  minConfidence?: number
  limit:          number
  offset:         number
}
```

**Push Events**

| Event | Payload |
|---|---|
| `indexing://progress` | `{ jobId, rootId, phase, filesDone, filesTotal, currentFile }` |
| `indexing://completed` | `{ jobId, rootId, filesAdded, filesUpdated, filesMoved, filesDeleted, errorCount, durationMs }` |
| `indexing://error` | `{ jobId, rootId, file?, error }` |
| `indexing://paused` | `{ jobId }` |
| `indexing://cancelled` | `{ jobId }` |
| `runtime://status` | `{ ollama_available, models_loaded, active_model }` |
| `index://changed` | `{ rootId, added, updated, removed }` |

`index://changed` fires after `indexing://completed` — frontend re-runs the current search query on receipt.

**Conventions**
- All timestamps: unix seconds.
- Wire format: `camelCase` — Rust uses `#[serde(rename_all = "camelCase")]`.
- Commands return `Result<T, String>` — errors surface as exceptions in `invoke()`, always human-readable strings at the boundary.
- `get_file_thumbnail` is lazy — called only as tiles enter the viewport.

---

## Folder Structure

```
desktop/                          Tauri app — Rust backend + React frontend
  src/                            React + TypeScript frontend
    hooks/                        Feature-specific state hooks
    components/                   UI components and tool views
    utils.ts                      Shared frontend utilities
  src-tauri/src/                  Rust backend
    indexer.rs
    extractor.rs
    chunker.rs
    embeddings.rs
    vision.rs
    search.rs
    db.rs
    config.rs
    runtime.rs
    lib.rs                        Tauri command layer
    platform/                     OS-specific abstractions (paths, clipboard, GPU)

shared/                           Reserved — shared schemas and contracts
```

---

## Stack

- **Desktop shell:** Tauri (macOS + Windows v1; Linux post-MVP)
- **Embedding provider:** Ollama local. v1 default: `nomic-embed-text-v2-moe` (768-dim) for text; `qwen2.5vl:7b` (fallback `llava:7b`) describes images → fed back through the text embedding pipeline. Model-agnostic per-model config (see [technical-decisions.md](technical-decisions.md) §7).
- **Index:** SQLite + `sqlite-vec` — single file, metadata + FTS5 + vector table, atomic writes — see [db-schema.md](db-schema.md) for full schema, DDL, and key patterns
- **Background runtime:** Incremental indexing on app open — background service is post-v1 roadmap

### Rust Crates

| Crate | Role |
|---|---|
| `rusqlite` | SQLite bindings — all DB reads/writes |
| `sqlite-vec` | Vector search extension for SQLite |
| `ureq` | Synchronous HTTP client for Ollama API calls |
| `image` | Image decoding for the vision pipeline (PNG, JPEG, etc.) |
| `base64` | Encode image bytes for Ollama vision API |
| `regex` | LLM JSON fallback (tier 3 salvage) + query parsing |
| `blake3` | Fast file fingerprinting (content-change detection) |
| `walkdir` | Recursive directory traversal in indexer |
| `whatlang` | Language detection on extracted text at index time |

---

## Runtime Data Locations

All app-managed writes stay in app-owned user data directories.

| OS | Base path |
|---|---|
| macOS | `~/Library/Application Support/WhatTheFile/` |
| Windows | `%LOCALAPPDATA%\WhatTheFile\` |
| Linux | `~/.local/share/WhatTheFile/` |

Paths under base:
- `db/index.sqlite`
- `cache/extracted/<index-id>/` — normalized extraction artifacts
- `cache/index-jobs/` — index job manifests, fingerprints, checkpoints
- `logs/activity.log` — audit/activity trail

---

## Build & Test

Frontend commands run from `desktop/`. Rust commands run from `desktop/src-tauri/`.

```bash
# From desktop/
npm install            # install frontend dependencies
npm run dev            # launch Tauri in dev mode
npm run test           # run frontend tests (Vitest)
npm run tauri:build    # produce macOS DMG / Windows MSI

# From desktop/src-tauri/
cargo build            # build Rust backend
cargo test             # run Rust unit tests
```

### Running Tests

```bash
# From desktop/
npm run test:rust
```

This runs unit tests for:
- Path/config resolution
- Natural query parser
- SQLite schema, pagination, and FTS search
- Indexer rename/move detection
- Resumable index-job persistence and recovery

> **Post-MVP / Linux note:** On NVIDIA + Wayland systems, WebKitGTK may fail to render due to DMABuf incompatibility. Workaround: `WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=wayland,x11 npm run dev`. Revisit when Linux becomes a target platform.
