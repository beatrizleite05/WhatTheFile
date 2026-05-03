# WhatTheFile — Implementation Plan

Living roadmap: phases, current status, next steps.

For decisions and rationale, see [technical-decisions.md](technical-decisions.md). For module/IPC/schema, see [architecture.md](architecture.md) and [db-schema.md](db-schema.md). For requirements, see [requirements.md](requirements.md).

---

## Goal

WhatTheFile is a local-first desktop app for finding files by meaning — natural-language semantic search over user-configured folders, on macOS and Windows, with all AI inference running locally via Ollama.

---

## Locked Requirements (summary)

Detailed in [requirements.md](requirements.md). Key constraints:

- **File types (v1):** `pdf`, `docx`, `xlsx`, `xls`, `xlsm`, `csv`, `txt`, `md`, `png`, `jpg/jpeg`, `webp`
- **Privacy:** strictly local — Ollama only, no cloud fallback in v1
- **Packaging:** macOS (DMG) + Windows (MSI)
- **Folder scope:** user-defined; no implicit indexing
- **Indexing:** incremental on app open; background service is post-v1
- **Duplicates:** listed separately
- **Query UX:** must stay responsive under very broad searches

---

## Implementation Phases

### Phase A — Project Skeleton ✅

Scaffold `desktop/` (Tauri + React + TS, Vite, vitest), `desktop/src-tauri/` (Rust module stubs), strict `tsconfig`, clippy + rustfmt. Wire `lib.rs` command stubs.

**Exit criteria:** `npm run tauri dev` launches a blank window on macOS and Windows. Clean test/build on both targets.

---

### Phase B — Indexer, Manifests, Incremental Detection ✅

Two-phase scan (mtime fast pass + blake3 fingerprint), move detection, base SQLite schema, `indexPlanner.ts`, `policy.ts`, `eventCoalescer.ts`, index job manifests, `add_root` / `start_indexing` Tauri commands, `indexing://progress` / `indexing://completed` events.

**Exit criteria:** index a folder of `.txt` files, persist to SQLite, skip unchanged files on re-run, detect renames without re-processing.

---

### Phase C — Extraction and OCR ✅

Per-type extractors (PDF text-layer + Tesseract OCR fallback, DOCX, XLSX/XLS/XLSM/CSV, TXT/MD), `vision.rs` (Ollama qwen2.5vl), `chunker.rs` mirroring `chunker.ts`. Skip re-extraction on unchanged fingerprint + model version.

**Exit criteria:** all v1 file types extract non-empty text. Image files produce a description. Unchanged files skipped on re-index.

---

### Phase D — Database, Embeddings, Retrieval — *in progress, retrieval-quality rework*

Original Phase D landed FTS5 + sqlite-vec + RRF + cursor pagination. **Symptom:** semantic results are irrelevant. Root cause: missing query/document prefixes for `nomic-embed-text-v2-moe`, plus chunk size and snippet UX issues. See decisions 4, 7-10, 15-17, 22-30 in [technical-decisions.md](technical-decisions.md).

Rework lands in three migration-aligned groups (decision 30):

**Group A — No reindex.**
- Snippet source selection per pass (decision 16) and `<mark>` highlighting (decision 17)
- File-type expansion: `.webp`, `.xls`, `.xlsm` (decision 18)
- Eval-diff against current baseline

**Group B — Re-embedding only.**
- Model-agnostic embedding config (decision 7)
- `search_query:` / `search_document:` prefixes (decision 9)
- Unconditional L2 normalisation + explicit `vec_distance_cosine` (decision 8)
- Chunk size 256→512 / overlap 32→64 (decision 4)
- Bump `model_version` → existing chunks re-embed; extracted text preserved
- Eval-diff against group-A baseline

**Group C — Image re-extraction.**
- Vision determinism: `temperature: 0` (decision 15)
- Unified structured vision prompt with `SUBJECTS / SCENE / VISUAL_DETAILS / TEXT` (decision 15)
- Re-run vision pipeline on existing image rows
- Eval-diff against group-B baseline

**Eval harness (parallel, blocks all groups).** Native Rust binary at `src-tauri/src/bin/eval.rs`, TS metrics consumer, curated corpus + queries (decisions 22-26). Persist results under `test/fixtures/eval-results/`. Pre-push regression gate at >5% absolute drop on Recall@10 or MRR.

**Exit criteria:**
- Eval harness runs end-to-end; baseline.json committed.
- Each group ships with eval-diff showing no regression on validated decisions.
- 80% coverage on `src/core/` and `src-tauri/src/` excluding glue (decision 27).
- Ollama models unloaded after index completion.
- Provisional decisions (4, 8, 9, 15, 16) confirmed by eval; documented in HISTORY.md.

---

### Phase E — Desktop UI

Onboarding (mandatory, folder picker + privacy notice + skip-with-warning), search bar with query pills, virtualised result grid, preview pane (60/40), indexing progress + pause/stop/reindex, settings (scope editor, delete index, audit log), Ollama offline banner, empty/zero-result states. Hooks: `useSearch`, `useIndexing`, `useSettings`, `useOllamaStatus`. 100% keyboard-navigable; spring-physics animations; OS vibrancy.

**Exit criteria:**
- Full end-to-end flow works (onboard → index → search → open).
- All acceptance criteria from [requirements.md](requirements.md) pass.
- E2E suite: ≤10 happy-path scenarios via `tauri-driver` + WebDriverIO (decision 28).
- Coverage target met for new UI code.

---

### Phase F — Packaging and Distribution

`tauri.conf.json` per target, code signing + notarization (macOS), NSIS/WiX + signing (Windows), Ollama install check at first launch, `docs/BUILD.md` runbook, GitHub Actions release workflow.

**Exit criteria:** clean-machine DMG + MSI install completes. App launches, onboards, indexes, searches. Build runbook verified.

---

## Current Status

**Date:** 2026-05-03

- Phases A, B, C complete. Phase D landed initial implementation; retrieval-quality rework in progress.
- All decisions for the Phase D rework locked (technical-decisions.md §4, §7-10, §15-30).
- **Commits 1 + 2 done** — eval harness scaffold landed; pre-rework baseline committed: Recall@10=0.714, MRR=0.662, NDCG@10=0.676 (n=56). Group A=0.000 (filetype expansion not yet landed), Group C=0.091 (vision prompt rework not yet landed).
- Next: **commit 3** (Group A: snippet rework + filetype expansion). See [IMPLEMENTATION.md](IMPLEMENTATION.md).

Uncommitted local changes (pre-existing, not part of commit 1): minor cleanups to `desktop/src/api/runtime.ts` and `desktop/src/components/SkipWarningBanner.tsx`.

---

## Acceptance Criteria

Locked per [requirements.md](requirements.md). Repeated here for phase exit reference:

- Onboarding completes before first use — folder selection, privacy notice, local behaviour explained.
- Indexes one or more user-configured folders; resumes without re-indexing unchanged files.
- Detects rename/move without re-extracting or re-embedding when fingerprint is unchanged.
- Natural-language query returns relevant files with hybrid (FTS + semantic) ranking.
- Semantic search understands intent and synonyms — exact keyword match not required.
- Results are file-level; chunk hits collapsed before display.
- Duplicates appear as separate result entries.
- Result tiles include file path and context snippet; clicking opens the file.
- All v1 file types are indexed and retrievable.
- Image search works via the vision-description pipeline.
- Multilingual queries work for pt-BR + en via accent folding and cross-lingual embeddings.
- Indexing progress visible; pause, stop, reindex controls work.
- Limited-mode warning when Ollama is unavailable.
- Query UI stays responsive under very broad searches.
- Ollama model memory is released after index completion or idle timeout.
- Privacy controls (scope editor, audit log, delete index) always visible and functional.
- All writes happen only under app-owned data directories — source folders never touched.
- macOS (DMG) and Windows (MSI) builds reproducible and documented.
