# Phase A — Project Skeleton Design

**Date:** 2026-03-31
**Scope:** Full Phase A as defined in PLAN.md — TypeScript `src/core/` implementations + Tauri desktop scaffold

---

## Context

WhatTheFile is a local-first desktop search app (macOS + Windows). The repo already contains all 11 TypeScript test files for `src/core/` pure-logic modules, but no implementations yet. Phase A establishes the full repository structure with working build toolchains before any feature work (Phase B onward).

Approach chosen: **Scaffold first, implement inside** — create `desktop/` Tauri structure, migrate existing `test/` and config files into it, then implement `src/core/` modules in their permanent home so tests run from the final structure from day one.

---

## Directory Structure

```
WhatTheFile/
  .github/
    workflows/              ← GitHub Actions CI (tagged release builds)
  desktop/
    package.json            ← moved + Tauri/React/Vite deps added
    tsconfig.json           ← moved (strict mode, unchanged)
    vitest.config.ts        ← moved (test glob unchanged: test/**/*.test.ts)
    vite.config.ts          ← new (Vite + React plugin)
    index.html              ← new (Vite HTML entry)
    public/                 ← Vite static assets (favicon, file-type icons)
    src/
      core/                 ← 10 pure TypeScript modules (no I/O)
      hooks/                ← empty stubs (useSearch, useIndexing, useSettings, useOllamaStatus)
      components/           ← empty stubs
      utils/                ← shared frontend utilities (basename, errorMessage, etc.)
      api/                  ← Tauri invoke wrappers, split by domain
        search.ts
        indexing.ts
        settings.ts
        runtime.ts
        index.ts            ← re-exports
      App.tsx               ← minimal layout skeleton
      main.tsx              ← React entry point
    test/                   ← moved from root (import paths unchanged)
    src-tauri/
      Cargo.toml
      build.rs              ← standard Tauri build script
      tauri.conf.json       ← app id, window, bundle (macOS DMG + Windows MSI)
      capabilities/         ← Tauri v2 capability JSON files
      icons/                ← app icons
      migrations/           ← SQL migration files loaded by db.rs at runtime
      src/
        main.rs             ← fn main() → tauri_app::run()
        lib.rs              ← Tauri command layer only (no business logic)
        errors.rs           ← central AppError enum via thiserror
        indexer.rs
        extractor.rs
        chunker.rs
        search.rs
        db.rs
        config.rs
        llm/
          mod.rs
          embeddings.rs     ← nomic-embed-text Ollama adapter
          vision.rs         ← qwen2.5vl image pipeline
          runtime.rs        ← Ollama lifecycle management
        platform/
          mod.rs            ← OS-specific: data dirs, clipboard, GPU
  shared/
    .gitkeep                ← reserved for cross-layer schemas/contracts
  scripts/                  ← build-local, sign, release helpers
  docs/
  CLAUDE.md
```

---

## TypeScript Core Modules (`desktop/src/core/`)

All implementations are pure functions — no I/O, no Tauri, no Ollama. Tests already exist; implementation order is simplest → most complex.

| # | Module | Exports | Key logic |
|---|---|---|---|
| 1 | `types.ts` | `FileResult`, `Chunk`, `ScanPolicy`, `SearchRequest`, `FileState`, `FsEvent` | Type definitions only — no runtime code |
| 2 | `pagination.ts` | `normalizePagination` | Cap limit to maxLimit; clamp negative values to safe defaults |
| 3 | `fingerprint.ts` | `buildFingerprint` | Node `crypto` SHA-256 over concatenated `content+mtime+size` |
| 4 | `indexPlanner.ts` | `planIndexAction` | 4-state decision: `index` (new) / `delete` (gone) / `noop` (same hash) / `reindex` (hash changed) |
| 5 | `chunker.ts` | `chunkText` | Whitespace tokenization; sliding window with configurable size/overlap; throws when overlap ≥ size |
| 6 | `eventCoalescer.ts` | `coalesceEvents` | Map-based dedup per path; order-preserving; create→delete normalizes to delete |
| 7 | `ranking.ts` | `rankFiles` | Immutable sort: score desc, path asc tiebreak; slice to topK; empty/negative-K returns [] |
| 8 | `policy.ts` | `normalizeExt`, `shouldIndexFile` | Glob→regex matcher; checks: root prefix, hidden file, excluded extension, file size, glob patterns |
| 9 | `metrics.ts` | `recallAtK`, `mrr`, `ndcgAtK` | Standard IR evaluation: Recall@K, MRR, NDCG@K with log2 DCG |
| 10 | `queryParser.ts` | `parseNaturalLanguageQuery` | Regex patterns for year/today dates, media types, `in [Scope]`, `min confidence N`; `parserConfidence` < 0.5 flags LLM fallback (>5 words AND >3 unresolved tokens) |

**No new npm dependencies** — `fingerprint.ts` uses Node built-in `crypto` module.

### Key type shapes (inferred from tests)

```typescript
// FileResult — ranking.test.ts
interface FileResult {
  fileId: number; rootId: number; path: string; filename: string;
  mediaType: string; sizeBytes: number; indexedAt: number;
  confidence: number; snippet: string; score: number;
}

// Chunk — chunker.test.ts
interface Chunk { startToken: number; endToken: number; text: string; }

// ScanPolicy — policy.test.ts
interface ScanPolicy {
  includeRoots: string[]; excludeGlobs: string[];
  excludedExtensions: string[]; maxFileSizeBytes: number; includeHidden: boolean;
}

// SearchRequest — queryParser.test.ts
interface SearchRequest {
  queryText: string; mediaTypes: string[]; rootScope: string[];
  dateFrom: string; dateTo: string; minConfidence: number; parserConfidence: number;
}

// FileState — indexPlanner.test.ts
interface FileState { path: string; hash: string; mtimeMs: number; sizeBytes: number; }

// FsEvent — eventCoalescer.test.ts
interface FsEvent { path: string; type: 'created' | 'modified' | 'deleted'; ts: number; }
```

---

## Rust Stubs (`desktop/src-tauri/src/`)

All Rust files are valid compilable stubs. No `.unwrap()` in any non-test code. Every module has `#[cfg(test)] mod tests {}`.

| File | Purpose | Stub contents |
|---|---|---|
| `main.rs` | Entry point | `fn main()` calling `tauri_app::run()` |
| `lib.rs` | Tauri command layer | `#[tauri::command]` stubs: `search`, `start_indexing`, `add_root`, `open_file`, `get_runtime_status` |
| `errors.rs` | Central error type | `AppError` enum with variants: `Db`, `Indexer`, `Extractor`, `Llm`, `Search`, `Config`, `Io` |
| `db.rs` | SQLite operations | Stub: `init_db`, `upsert_file`, `delete_file` signatures |
| `indexer.rs` | Two-phase scan | Stub: `run_index_job` signature |
| `extractor.rs` | File text extraction | Stub: `extract_text` signature |
| `chunker.rs` | Text splitting | Stub: `chunk_text` signature |
| `search.rs` | FTS5 + vector retrieval | Stub: `search_files` signature |
| `config.rs` | User config persistence | Stub: `load_config`, `save_config` signatures |
| `llm/embeddings.rs` | nomic-embed-text adapter | Stub: `embed_text` signature |
| `llm/vision.rs` | Image → description → embed | Stub: `describe_image` signature |
| `llm/runtime.rs` | Ollama lifecycle | Stub: `ensure_model_loaded`, `unload_model` signatures |
| `platform/mod.rs` | OS-specific helpers | Stub: `app_data_dir`, `copy_to_clipboard` signatures |

**`Cargo.toml` dependencies:**
```toml
[dependencies]
tauri = { version = "2", features = ["protocol-asset"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

---

## Frontend Skeleton

Minimal React app — just enough to satisfy Tauri's build requirements:

- **`main.tsx`** — `ReactDOM.createRoot(...).render(<App />)`
- **`App.tsx`** — single `<div>WhatTheFile</div>` placeholder
- **`api/`** — typed `invoke()` wrapper stubs per domain (no logic)
- **`hooks/`** — empty hook files: `useSearch.ts`, `useIndexing.ts`, `useSettings.ts`, `useOllamaStatus.ts`

---

## CI Skeleton (`.github/workflows/`)

Single workflow file `ci.yml` for Phase A:
- Trigger: `push` to main + `pull_request`
- Jobs: `typecheck` (`npx tsc --noEmit`) + `test` (`npm test`) running inside `desktop/`
- Rust jobs stubbed but gated behind Cargo availability check (deferred to Phase F)

---

## Verification

```bash
# Inside desktop/
npm install
npm test              # all 11 test suites green, 0 failures
npx tsc --noEmit      # 0 TypeScript errors in strict mode

# Rust — deferred until Cargo + Tauri CLI installed:
cargo check           # stubs compile clean
cargo clippy          # 0 warnings (linter)
cargo test            # empty test blocks pass
npm run tauri dev     # blank window launches (Phase A exit criterion)
```

**Definition of Done (Phase A):**
1. `npm test` — all 11 suites pass
2. `npx tsc --noEmit` — 0 errors
3. Rust stubs written, valid, no `.unwrap()` in non-test code
4. `.github/workflows/ci.yml` runs typecheck + tests on push
5. `desktop/` structure matches the layout above exactly
