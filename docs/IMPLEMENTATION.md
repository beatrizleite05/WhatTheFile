# Phase D Rework — Implementation Plan

Ordered commit sequence to implement decisions 4, 7-11, 14-18, 22-32, 34-35 from [technical-decisions.md](technical-decisions.md). Each commit is independently reviewable; commits 5 and 6 each gate on `npm run eval:diff`.

**Approach:** one commit at a time, reviewed before the next starts (per session-end alignment 2026-04-30).

Status legend: ☐ not started · ◐ in progress · ☑ done

---

## Commit 1 — Eval harness scaffold + draft queries  ☑

**Goal:** infrastructure to measure relevance, plus a curated query set ready for user review.

**Scope:**

- `desktop/src-tauri/src/bin/eval.rs` — native binary; opens the same SQLite DB the app uses, indexes the corpus at `desktop/test/sample-files/`, runs each query through real `search_files()`, emits per-query JSON to stdout.
- `desktop/test/fixtures/eval-queries.json` — `{ query: string, expectedFiles: string[] }[]` ; ~40-60 entries grouped by intent type (filename keyword / conceptual / cross-language pt-BR↔en / image-content / negative case). **Drafted by Claude after reading each sample file; reviewed and corrected by user before commit closes.**
- `desktop/scripts/eval.ts` — TS runner: invokes the Rust binary, reads JSON, calls existing `metrics.ts` for Recall@K / MRR / NDCG, writes `test/fixtures/eval-results/<YYYY-MM-DD>-<sha>.json`.
- `desktop/scripts/eval-diff.ts` — compares latest run to `test/fixtures/eval-results/baseline.json`; supports `--group A|B|C` flag for group-specific gates (decision 30).
- `desktop/scripts/eval-promote.ts` — copies latest run to `baseline.json`; appends one-line entry to `test/fixtures/eval-results/HISTORY.md`.
- `package.json` scripts: `eval`, `eval:diff`, `eval:promote`.
- Initial empty `baseline.json` and `HISTORY.md`.

**Checkpoint:** Claude pauses after drafting `eval-queries.json`. User reviews queries (especially pt-BR semantics and image-content expectations) and signs off before commit lands.

**Exit criteria:** `npm run eval` runs end-to-end and emits a JSON results file. `eval:diff` and `eval:promote` exist but are no-ops (no baseline yet).

---

## Commit 2 — Capture baseline  ☑

**Goal:** lock the pre-rework metrics as the comparison point for Group A.

**Scope:**

- Run `npm run eval` against current `main`.
- `npm run eval:promote` — promotes the run to `baseline.json`.
- `HISTORY.md` entry: `<date>: pre-rework baseline. Recall@10=X.XX, MRR=X.XX, NDCG=X.XX. Captured against <sha>.`

**Exit criteria:** `baseline.json` committed with concrete numbers. The next commit's eval-diff has something to compare against.

---

## Commit 3 — Group A: snippet rework + file-type expansion  ☑

**Goal:** zero-reindex relevance UX improvements + scope additions.

**Scope:**

- **Snippet source selection (decision 16):** `search.rs::build_snippet` picks per-file based on which pass ranked higher.
  - FTS-leading: use FTS5 `snippet()` with non-printable delimiters (`\x01...\x02`).
  - Vector-leading: first sentence(s) of the best-scoring chunk.
- **Snippet highlighting (decision 17):** server-side HTML-escape; convert delimiters → `<mark>...</mark>`. Frontend `ResultTile` parses snippet, splits on `<mark>` boundaries, emits React elements (no raw-HTML injection).
- **File-type expansion (decision 18):** add `.webp` to image branch in `extractor::extract`; verify `.xls` / `.xlsm` already pass through `calamine::open_workbook_auto` (currently routed); update `detect_media_type` in `indexer.rs` to recognise the new extensions.

**Tests:**

- `search_test.rs` — snippet source selection unit tests (one FTS-leading, one vector-leading, one tie).
- `extractor` tests — `.webp` smoke test using a fixture image; `.xls` / `.xlsm` smoke tests using existing fixtures.
- `ResultTile` test — confirms `<mark>` parsing renders highlighted segments and never injects raw HTML.

**Validation:**

- `npm run eval:diff --group A` — gate: within ±2% on Recall@10 AND MRR vs. baseline.
- If gate fails: investigate (Group A is not supposed to change ranking; any drift is a bug).

**Exit criteria:** gate passes; merge; `eval:promote` to refresh baseline as the group-A reference for Group B.

---

## Commit 4 — Schema migration: split extraction / embedding versions  ☐

**Goal:** decouple the two pipeline-version fields so Group B can re-embed without re-extracting (decision 31).

**Scope:**

- New `db::run_migrations` step:
  ```sql
  ALTER TABLE files ADD COLUMN extraction_version TEXT NOT NULL DEFAULT '';
  ALTER TABLE files ADD COLUMN embedding_version  TEXT NOT NULL DEFAULT '';
  UPDATE files SET extraction_version = model_version WHERE model_version != '';
  ALTER TABLE files DROP COLUMN model_version;
  ```
- Update `db_files::find_files_needing_extraction` (and add `find_files_needing_embedding`) to use the split fields per decision 31's table.
- Update `indexer.rs::run_extraction` to write the new fields:
  - On extraction: set `extraction_version = "ext-v1"`; clear `embedding_version` so embed phase runs.
  - On embedding: set `embedding_version` to the current per-model identifier.
- Add a `phase: "embedding"` step distinct from `"extracting"` in `run_scan`, emitting `indexing://progress` events.

**Tests:**

- `db_test.rs` — migration applies cleanly to a v3 schema; backfill copies `model_version` correctly.
- `indexer_test.rs` — file with stale `embedding_version` only re-embeds (no extractor calls); file with stale `extraction_version` re-extracts AND re-embeds.

**Validation:** no behavior change for end users; eval should be unchanged. Run `npm run eval:diff --group A` (still uses A's gate) — expect flat metrics.

**Exit criteria:** schema migration tested; `extraction_version` / `embedding_version` separated; ready for Group B to bump `embedding_version` independently.

---

## Commit 5 — Group B: model-agnostic config + prefixes + L2 + chunk size  ☐

**Goal:** the headline relevance fix.

**Scope:**

- **Model-agnostic config (decision 7):**
  ```rust
  pub struct EmbeddingModel {
      pub name: &'static str,
      pub dim: usize,
      pub query_prefix: &'static str,
      pub doc_prefix: &'static str,
  }
  pub const NOMIC_V2_MOE: EmbeddingModel = EmbeddingModel {
      name: "nomic-embed-text-v2-moe",
      dim: 768,
      query_prefix: "search_query: ",
      doc_prefix: "search_document: ",
  };
  ```
- **Prefixes (decision 9):** `embed_texts` accepts an `EmbeddingModel` reference and prepends `doc_prefix`. New `embed_query` helper prepends `query_prefix`. `search.rs` uses `embed_query`; `indexer.rs` uses `embed_texts` with `doc_prefix`.
- **L2 normalisation (decision 8):** new `normalize_l2` function; called unconditionally on every embedding before `embedding_to_bytes`. SQL switches to explicit `vec_distance_cosine`.
- **Chunk size 256 → 512 / overlap 32 → 64 (decision 4):** update both `chunker.rs` and `chunker.ts` constants.
- **Bump `embedding_version`** to `"nomic-embed-text-v2-moe:768:prefixed-l2"`. The migration runs automatically on app open via the indexer logic from commit 4.

**Tests:**

- `embeddings_test.rs` — prefix concatenation; `normalize_l2` produces unit vectors (within f32 epsilon); empty input handled.
- `chunker` tests — update existing tests for 512/64 (the existing tests assert specific token positions; they'll need values updated).
- Live-Ollama integration test (`#[cfg_attr(not(feature = "live-ollama"), ignore)]`): embed same text with and without `doc_prefix`; cosine similarity between the two should drop notably.

**Validation:**

- `npm run eval:diff --group B` — gate: Recall@10 ≥ +5% absolute AND MRR ≥ +5% absolute vs. group-A baseline.
- **If gate fails: do not merge.** Run the Group B diagnosis tree (decision 30 §"Group B diagnosis tree"). Document path and outcome in `HISTORY.md`.

**Exit criteria:** gate passes; merge; `eval:promote` to refresh baseline as the group-B reference for Group C.

---

## Commit 6 — Group C: vision determinism + unified prompt  ☐

**Goal:** improve image-query relevance.

**Scope:**

- **Determinism (decision 15):** add `"options": { "temperature": 0 }` to `vision::call_ollama` body.
- **Unified prompt:** replace the current `PROMPT` constant with the structured 4-field prompt:
  ```
  Describe this image. Output these fields, one per line:
  SUBJECTS: <main subjects/objects>
  SCENE: <setting and context>
  VISUAL_DETAILS: <colors, style, notable elements>
  TEXT: <transcribe ALL visible text verbatim, preserving order. Write 'NONE' if no text is visible.>
  ```
- **Bump `extraction_version`** to `"ext-v2-vision-structured"` for image media types only (PDFs and other docs stay on `ext-v1`). This forces re-extraction of images on next index, which feeds back through the embedding pipeline.

**Tests:**

- `vision_test.rs` — request body includes `temperature: 0` and the structured prompt.
- Live-Ollama image test: feed a fixture screenshot; assert response contains the four field labels.

**Validation:**

- `npm run eval:diff --group C` — gate: image-subset Recall@10 ≥ +5% absolute vs. group-B baseline. Slice eval results by `media_type` in image set.
- Overall corpus metrics may be flat (image queries are a minority of the eval set) — that's expected, not a failure.

**Exit criteria:** image-subset gate passes; merge; `eval:promote` to refresh baseline.

---

## Commit 7 — Returning-user UX: `lastSeenVersion` + re-embed toast  ☐

**Goal:** explain visible changes to existing users without forcing re-onboarding (decisions 32-33).

**Scope:**

- `wtf:lastSeenVersion` localStorage key. On `App.tsx` mount: if `lastSeenVersion < currentAppVersion`, render a non-blocking "What's New" sheet listing user-visible changes for the release. On dismiss → set `lastSeenVersion = currentAppVersion`.
- Re-embed toast: when `useIndexing` observes a `phase: "embedding"` event on an already-indexed root (i.e. not first-time indexing), show a non-blocking auto-dismissing toast: *"Updated search quality — re-indexing existing files. This takes a few minutes and runs locally."*
- `IndexingProgress.tsx` copy: show *"Improving search quality"* when `phase === "embedding"` on a previously-indexed root.

**Tests:**

- React Testing Library: sheet renders when `lastSeenVersion < currentAppVersion`; doesn't render when equal.
- Toast renders on simulated `phase: "embedding"` event with prior root state.

**Exit criteria:** no eval impact (UI-only); manual smoke test of the upgrade flow on a local install with a stale `lastSeenVersion`.

---

## Commit 8 — Activity log: `pipeline_migrated` event  ☐

**Goal:** honor the privacy-trust principle without polluting the log (decision 35).

**Scope:**

- New activity-log event_type `pipeline_migrated`. Written once per group migration (B or C) at completion, with the JSON payload from decision 35.
- `ActivityLog.tsx` renders `pipeline_migrated` with summary line + click-to-expand JSON detail.

**Tests:**

- Indexer integration test: simulate Group B migration; assert one `pipeline_migrated` activity row.
- `ActivityLog.tsx` snapshot of rendered summary + expanded detail.

**Exit criteria:** migrations leave a single audit trail entry; user can inspect what changed and when.

---

## Commit 9 — Phase E quality gates: E2E + 80% coverage  ☐

**Goal:** Phase E exit criteria (decisions 27, 28).

**Scope:**

- `tauri-driver` + WebDriverIO setup. ≤10 happy-path scenarios:
  1. Onboarding (preset + custom + skip-with-warning)
  2. Add root via Settings
  3. Index a fixture folder; observe progress event
  4. Run a search; arrow-key through results
  5. Open file (mocked filesystem opener)
  6. Space-key preview modal
  7. Settings → delete index → re-onboard prompt
  8. Ollama-offline banner (mocked unreachable runtime)
  9. Re-embed toast (force `embedding_version` mismatch)
  10. Activity log displays a `pipeline_migrated` entry
- Coverage tooling: `vitest --coverage` for TS; `cargo tarpaulin` for Rust. Configure exclusions: `lib.rs` Tauri command glue, `platform/`, `bin/`.
- Pre-push hook (`scripts/git-hooks/pre-push`) wired to `npm run setup` symlink. Hook runs the full Layer-2 gate (decision 23).

**Exit criteria:** all 10 E2E scenarios pass on local Mac; coverage ≥ 80% on `src/core/` and `src-tauri/src/`; pre-push hook installed and confirmed running on a real push.

---

## Resuming this plan in a fresh session

A fresh agent should:

1. Read this file plus [PLAN.md](PLAN.md) (Current Status section) and [technical-decisions.md](technical-decisions.md).
2. Find the topmost ☐ commit in the list above.
3. Verify nothing has changed in the codebase that affects its scope (`git log --oneline` since the plan's last update).
4. Implement that commit's scope; pause at the documented checkpoints (notably commit 1's query-review step, and commits 5/6's eval-diff gate).
5. After merge, mark ☑ here and update [PLAN.md](PLAN.md) Current Status.
