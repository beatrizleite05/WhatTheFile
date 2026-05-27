# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Repo Is

WhatTheFile is a local-first desktop search app (macOS + Windows) that lets users find files by semantic meaning using natural language. Everything runs on-device via Ollama. The full app is a Tauri desktop app (`desktop/`) — this repo currently contains the TypeScript `src/core/` layer and its tests, which are the pure-logic frontend modules developed independently before the Tauri shell is assembled.

## Commands

```bash
npm install          # install dependencies
npm test             # run all tests (vitest, single run)
npm run test:watch   # run tests in watch mode
```

Run a single test file:
```bash
npx vitest run test/queryParser.test.ts
```

TypeScript type-check (no emit):
```bash
npx tsc --noEmit
```

## Architecture

### Current Scope (`src/core/`)

Pure TypeScript logic — no I/O, no Tauri, no Ollama calls. These modules are the frontend brain and will be imported by the future `desktop/src/` React app.

| Module | Role |
|---|---|
| `queryParser.ts` | Parses natural language queries into structured `SearchRequest` intent (deterministic first; LLM fallback only for queries >5 words with >3 unrecognized tokens, never in `keyword` mode) |
| `ranking.ts` | Display-layer re-sort of results already scored by Rust `search.rs`; does **not** blend scores |
| `chunker.ts` | Splits raw text into overlapping windows (256-token window, 32-token overlap) — pure function |
| `fingerprint.ts` | blake3-based content-change detection helpers |
| `indexPlanner.ts` | Decides which files need indexing given current index state vs. filesystem scan |
| `policy.ts` | Allow/deny decisions for file indexing (extensions, size, hidden, globs) |
| `pagination.ts` | Normalizes limit/offset, enforces safety caps |
| `metrics.ts` | Offline evaluation utilities (Recall@K, MRR, NDCG) — benchmarks only, not in query path |
| `eventCoalescer.ts` | Deduplicates/batches filesystem watch events before handing to the indexer |
| `types.ts` | Shared TypeScript types (`FileResult`, `Chunk`, `ScanPolicy`, etc.) |

### Full App Architecture (planned `desktop/`)

```
desktop/src/          React + TypeScript frontend
  core/               (this repo's src/core/ — pure logic)
  hooks/              Feature-specific state hooks
  components/         UI components
  utils.ts            Shared utilities (basename, errorMessage, etc.)
desktop/src-tauri/    Rust backend
  indexer.rs          Two-phase incremental indexing, move detection
  extractor.rs        Text extraction (PDF, DOCX, XLSX, CSV, TXT, MD)
  chunker.rs          Text splitting — mirrors chunker.ts
  embeddings.rs       Ollama nomic-embed-text adapter
  vision.rs           Image → Ollama vision description → embed pipeline
  search.rs           FTS5/BM25 + sqlite-vec cosine retrieval and score blending
  db.rs               SQLite schema, migrations, upsert/move/delete
  lib.rs              Tauri command layer only — no business logic
  platform/           All OS-specific code lives here exclusively
```

The Rust backend owns score blending (RRF: `1/(60+rank_fts) + 1/(60+rank_vec)`). `ranking.ts` only does display-layer sorting.

### IPC Contract (Tauri)

Frontend calls Rust via `invoke()` for commands. All timestamps are unix seconds. Wire format is `camelCase` — Rust uses `#[serde(rename_all = "camelCase")]`. Errors surface as human-readable strings.

Key commands: `search`, `start_indexing`, `get_indexing_progress`, `cancel_indexing`, `add_root`, `open_file`, `get_runtime_status`, `get_activity_log`.

**Indexing progress is polled, not pushed.** `IndexingProvider` polls `get_indexing_progress` every 200ms while a job is active; the snapshot's `isComplete` field tells it to stop. The patched `tao`/`wry` crates required to boot on macOS 26 (see `desktop/src-tauri/patches/` and `desktop/src-tauri/Cargo.toml`) leave the tao event loop unable to dispatch `WebviewMessage::EvaluateScript` user events, which silently breaks every Rust→JS push path (`tauri::ipc::Channel`, `app.emit`, `webview.eval`). JS→Rust invokes still work, so we poll. Revert to push transport once upstream fixes the macOS 26 issue (tao-apps/tao#1171).

Key events still emitted (best-effort, may not reach JS on macOS 26): `indexing://completed`, `indexing://cancelled`, `index://changed`.

## Conventions

### TDD — Test First

Write the failing test before any production code. Test files live in `test/` mirroring `src/core/`. Tests cover behavior only — renaming an internal variable must never break a test.

### TypeScript

- Strict mode required. No `any` in tests or production code.
- Type field names are `camelCase` to match Rust serde wire format.
- LLM JSON responses use a 3-tier fallback: `JSON.parse` → brace-balanced extraction → regex field salvage.

### Rust (for `desktop/src-tauri/`)

- All errors via `thiserror` through a central `AppError` enum. No `.unwrap()` in production code.
- Every module must have `#[cfg(test)] mod tests { ... }`.
- Platform-conditional code lives exclusively in `platform/` — no `#[cfg(target_os)]` elsewhere.

### App.tsx (future)

Layout orchestrator only. Feature state and handlers go in `src/hooks/`. New tool views (e.g. `DuplicatesView`) are self-contained components; `App.tsx` only mounts them.

## Definition of Done

1. All tests pass (`npm test` + `cargo test` for Rust)
2. No `.unwrap()` in new Rust code
3. No TypeScript errors in strict mode (`npx tsc --noEmit`)
4. Every new acceptance criterion is covered by at least one test

## Agent skills

### Issue tracker

Issues live in the `beatrizleite05/WhatTheFile` repo on GitHub (uses the `gh` CLI). See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical names: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context — `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
