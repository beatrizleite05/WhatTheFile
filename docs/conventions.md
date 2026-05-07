# Coding Conventions & Core Principles

## Core Principles

**Read-only scanning**
- Never write to indexed source roots.
- Detect rename/move as index mapping updates only — never mutate source files.
- Incremental detection: metadata fast-path, fingerprint fallback.

**Processing and indexing**
- Extract and normalize content for all supported media types in MVP.
- Chunking constants (in `chunker.rs` and `chunker.ts`): window `512 tokens`, overlap `64 tokens` — not user-configurable.
- Embeddings: every chunk text is prefixed with `search_document: ` before embedding; every query is prefixed with `search_query: `. L2-normalise output unconditionally before storing or comparing. Use `vec_distance_cosine` explicitly.
- Dual retrieval: keyword/FTS + semantic vector.
- Default ranking: RRF — `score = 1/(60 + rank_fts) + 1/(60 + rank_vec)`.
- Search mode is caller-specified via `SearchRequest.mode`: `hybrid` (default) | `keyword` | `semantic`.
- `root_id` always filterable.
- Return paged/cursor results for large query sets.
- Progressive hydration: metadata first, previews lazy.

**Query responsiveness**
- No unbounded render — virtualized/paginated result grid.
- Async query execution with cancel/debounce.
- Time-budgeted retrieval with staged fallback/rerank.

**Ollama lifecycle**
- Bounded keep-alive defaults.
- Explicit unload on idle/index completion.
- Single model lease policy to avoid excess memory.
- Startup cleanup for stale model state after crash.

**Resilient**
- Index jobs write a completion checkpoint at the end. Interrupted index jobs restart from scratch.
- Per-file checkpointing is a post-MVP upgrade if large collections become a common case.

**Local-only**
- All processing runs on-device by default. No data leaves the device without explicit user opt-in.
- No cloud APIs in v1.

**Platform isolation**
- OS-specific code lives exclusively in `platform/`. No `#[cfg(target_os)]`, `std::env::consts::OS`, or platform-conditional logic outside this module. No exceptions.

**Extensibility**
- Modality-aware segment storage — schema stays forward-compatible for audio/video.
- Collapse segment hits to file-level UI results.

---

## Coding Conventions

### Rust

- Use `thiserror` for all error types via a central `AppError` enum.
- Propagate errors with `?`. Never use `.unwrap()` in production code.
- Every new Rust module must include `#[cfg(test)] mod tests { ... }` with unit tests.

### Frontend (TypeScript / React)

- TypeScript strict mode is required.
- `queryParser.ts` LLM fallback triggers only when query length > 5 words AND > 3 tokens are unrecognized after the deterministic pass. Never in `mode: 'keyword'`.
- TS type names use `camelCase` fields to match Rust's `#[serde(rename_all = "camelCase")]` on the wire.
- JSON parsing from LLM responses uses a 3-tier fallback strategy:
  1. Direct `JSON.parse`
  2. Brace-balanced extraction
  3. Regex field salvage
- Shared utilities (`basename`, `errorMessage`, etc.) live in `src/utils.ts`.
- Shared modal styles live in `shared-modal.css` — new modals must extend those base styles.

### App.tsx Responsibilities

`App.tsx` is a **layout orchestrator only**. It is allowed to hold:
- Layout state (sidebar visibility, open modals, preview panel)
- Search and filter state
- Thin mode-switching coordination

It must **not** hold feature-specific state or handlers. Those belong in dedicated hooks under `src/hooks/`.

New tool modes (e.g. `DuplicatesView`, `PdfPasswordsView`) must be **self-contained components** that manage their own data loading. `App.tsx` only mounts them — it does not orchestrate their internals.

---

## TDD / XP

**Test-first.** No production code without a failing test. Write the test, watch it fail, implement the minimum to make it pass, refactor.

**One test file per module.** Colocated under `test/` mirroring `src/core/`. Rust tests live in `#[cfg(test)] mod tests` inside each `.rs` file.

**What each layer tests:**

| Layer | Scope | Where it runs |
|---|---|---|
| Pure logic (TS `src/core/`) | Unit — behaviour only, no I/O, no mocks | CI + local |
| Rust modules | Unit — per-function in `#[cfg(test)]`; integration against in-memory SQLite | CI + local |
| Tauri commands (`lib.rs`) | Integration — real DB, **live Ollama** (gated by `live-ollama` feature) | Local pre-push only |
| Frontend integration | Real Tauri shell + DB; live Ollama | Local pre-push only |
| Frontend components | Behaviour via React Testing Library — no snapshot tests | CI + local |
| E2E | `tauri-driver` + WebDriverIO; ≤10 happy-path scenarios | Local pre-push only |
| Eval harness | Native Rust binary feeds curated corpus into real `search_files()`; emits per-query JSON; TS metrics consumer computes Recall@K, MRR, NDCG | Local pre-push only |

**CI gate (every PR):** `npm test` + `cargo test` (no `live-ollama`) + `npx tsc --noEmit` + `cargo clippy` + `cargo fmt --check`. Fast (<2 min), no Ollama dependency.

**Local pre-push gate (developer's Mac):** everything above plus `cargo test --features live-ollama`, `npm run test:integration`, `npm run test:e2e`, `npm run eval`, `npm run eval:diff`. Live Ollama is the only honest target environment for this product.

**Coverage target:** 80% line coverage on `src/core/` and `src-tauri/src/`. Excludes `lib.rs` Tauri command glue, `platform/`, `bin/` (eval binary). Verified by `vitest --coverage` and `cargo tarpaulin`.

**Pre-push hook:** versioned shell script at `scripts/git-hooks/pre-push`, installed via `npm run setup` (one-line symlink). No husky.

**Eval workflow:**
- `npm run eval` — runs corpus, writes `test/fixtures/eval-results/<YYYY-MM-DD>-<sha>.json`.
- `npm run eval:diff` — compares latest run to `baseline.json`; **fails the push** if Recall@10 OR MRR drops by >5% absolute.
- `npm run eval:promote` — manually copies the latest run to `baseline.json` after the developer reviews deltas. Append a note to `HISTORY.md`.

**Test quality rules:**
- Test behavior, not implementation. If renaming an internal variable breaks a test, the test is wrong.
- No `any` casts in test code — same TypeScript strict mode as production.
- Each test has one clear failure reason. Split tests that assert multiple unrelated things.
- Use real values, not magic strings — test inputs should resemble actual file paths, queries, and metadata.

**Definition of done for a task:**
1. All new and existing tests pass (`npm run test` + `cargo test`)
2. No `.unwrap()` in new Rust code
3. No TypeScript errors in strict mode
4. The acceptance criterion it addresses is explicitly covered by at least one test
